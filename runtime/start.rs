use std::{collections::HashSet, collections::HashMap, env};

type SnekVal = u64;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ErrCode {
    InvalidArgument = 1,
    Overflow = 2,
    IndexOutOfBounds = 3,
    InvalidVecSize = 4,
    OutOfMemory = 5,
}

const TRUE: u64 = 7;
const FALSE: u64 = 3;

static mut HEAP_START: *mut u64 = std::ptr::null_mut();
static mut HEAP_END: *mut u64 = std::ptr::null_mut();

#[link(name = "our_code")]
extern "C" {
    // The \x01 here is an undocumented feature of LLVM that ensures
    // it does not add an underscore in front of the name.
    // Courtesy of Max New (https://maxsnew.com/teaching/eecs-483-fa22/hw_adder_assignment.html)
    #[link_name = "\x01our_code_starts_here"]
    fn our_code_starts_here(input: u64, heap_start: *const u64, heap_end: *const u64) -> u64;
}

#[export_name = "\x01snek_error"]
pub extern "C" fn snek_error(errcode: i64) {
    if errcode == ErrCode::InvalidArgument as i64 {
        eprintln!("invalid argument");
    } else if errcode == ErrCode::Overflow as i64 {
        eprintln!("overflow");
    } else if errcode == ErrCode::IndexOutOfBounds as i64 {
        eprintln!("index out of bounds");
    } else if errcode == ErrCode::InvalidVecSize as i64 {
        eprintln!("vector size must be non-negative");
    } else {
        eprintln!("an error ocurred {}", errcode);
    }
    std::process::exit(errcode as i32);
}

#[export_name = "\x01snek_print"]
pub unsafe extern "C" fn snek_print(val: SnekVal) -> SnekVal {
    println!("{}", snek_str(val, &mut HashSet::new()));
    val
}

/// This function is called when the program needs to allocate `count` words of memory and there's no
/// space left. The function should try to clean up space by triggering a garbage collection. If there's
/// not enough space to hold `count` words after running the garbage collector, the program should terminate
/// with an `out of memory` error.
///
/// Args:
///     * `count`: The number of words the program is trying to allocate, including an extra word for
///       the size of the vector and an extra word to store metadata for the garbage collector, e.g.,
///       to allocate a vector of size 5, `count` will be 7.
///     * `heap_ptr`: The current position of the heap pointer (i.e., the value stored in `%r15`). It
///       is guaranteed that `heap_ptr + 8 * count > HEAP_END`, i.e., this function is only called if
///       there's not enough space to allocate `count` words.
///     * `stack_base`: A pointer to the "base" of the stack.
///     * `curr_rbp`: The value of `%rbp` in the stack frame that triggered the allocation.
///     * `curr_rsp`: The value of `%rsp` in the stack frame that triggered the allocation.
///
/// Returns:
///
/// The new heap pointer where the program should allocate the vector (i.e., the new value of `%r15`)
///
#[export_name = "\x01snek_try_gc"]
pub unsafe extern "C" fn snek_try_gc(
    count: isize,
    heap_ptr: *const u64,
    stack_base: *const u64,
    curr_rbp: *const u64,
    curr_rsp: *const u64,
) -> *const u64 {
    let new_heap_ptr = snek_gc(heap_ptr, stack_base, curr_rbp, curr_rsp);
    // eprintln!("new heap ptr {:p}, space needed {:p}, max {:p}", new_heap_ptr, new_heap_ptr.offset(-count), HEAP_END);
    if new_heap_ptr.offset(count) as u64 >= HEAP_END as u64 {
        eprintln!("out of memory");
        std::process::exit(ErrCode::OutOfMemory as i32)
    }
    new_heap_ptr
}

/// This function should trigger garbage collection and return the updated heap pointer (i.e., the new
/// value of `%r15`). See [`snek_try_gc`] for a description of the meaning of the arguments.
#[export_name = "\x01snek_gc"]
pub unsafe extern "C" fn snek_gc(
    heap_ptr: *const u64,
    stack_base: *const u64,
    curr_rbp: *const u64,
    curr_rsp: *const u64,
) -> *const u64 {

    // // // // //
    // MARKING  //
    // // // // //

    // Iterate over the entire heap's vectors and mark any vectors that are pointed towards
    let total_offset: usize = (heap_ptr as usize - HEAP_START as usize) / 8;
    let mut offset: usize = 0;
    while offset < total_offset {
        let size = HEAP_START.add(1 + offset).read() as usize;
        for i in 0..size {
            let elem = HEAP_START.add(2 + i + offset).read();
            if elem != TRUE && elem != FALSE && elem != 1 && elem & 1 == 1 {
                // This is a vector. Will mark its location
                let mut vec_addr = (elem - 1) as *mut u64;
                *vec_addr = 1
            }
        }
        offset += size + 2;
    }

    // Iterate over the stack and mark any vectors that are pointed towards
    let mut rsp = curr_rsp;
    let mut rbp = curr_rbp;
    loop {
        // Iterating over local variables in this stack frame
        let mut ptr = rbp;
        ptr.sub(1);
        while ptr >= rsp {
            let val = *ptr;
            // Check if value is a vector then marking if it is
            if val != TRUE && val != FALSE && val != 1 && val & 1 == 1 {
                // This is a vector. Will mark its location
                let mut vec_addr = (val - 1) as *mut u64;
                *vec_addr = 1
            }
            ptr = ptr.sub(1);
        }

        // Checking if stack_base was reached
        if rbp == stack_base {
            break;
        }

        // resetting the rsp and rbp to the next stack frame
        let prev_rbp = *rbp;
        rsp = rbp.add(2);
        rbp = prev_rbp as *mut u64;
    }
    
    // Check registers for any vectors and marking them
    // idk if I actually need to do this

    // // // // // //
    // FORWARDING  //
    // // // // // //

    // initializing move_to
    let mut move_to: usize = 0;

    // iterating over the vectors then if it's marked, setting it to move_to. Also keeping track of reference update mapping
    let total_offset: usize = (heap_ptr as usize - HEAP_START as usize) / 8;
    let mut offset: usize = 0;
    let mut reference_map: HashMap<u64, u64> = HashMap::new();
    while offset < total_offset {
        // eprintln!("Offset Difference {}, {}", offset, total_offset);
        let size = HEAP_START.add(1 + offset).read() as usize;
        let marked = HEAP_START.add(offset).read() as u64;
        if marked == 1 {
            // Set the marked vector to the move_to address
            *HEAP_START.add(offset) = (HEAP_START as u64) + (move_to as u64) * 8;
            reference_map.insert((HEAP_START.offset(offset as isize) as u64) + 1, (HEAP_START as u64) + (move_to as u64) * 8 + 1);
            // *markPtr = move_to as u64;
            // eprintln!("Marked Vector! Moving to {} slot of heap array...", move_to);
            move_to += size + 2;
        } else {
            // eprintln!("Unmarked Vector! Will be deleted...");
        }
        
        offset += size + 2;
    }

    // Printing out the reference map
    // eprintln!("\nReference Map");
    // reference_map.iter().for_each(|(old, new)| {
        // eprintln!("Key: {}, Value: {}", old, new);
    // });
    // eprintln!();

    // // // // // // //  //
    // REFERENCE UPDATING //
    // // // // // // //  //

    // Updating any vectors referenced in the heap
    let total_offset: usize = (heap_ptr as usize - HEAP_START as usize) / 8;
    let mut offset: usize = 0;
    while offset < total_offset {
        let size = HEAP_START.add(1 + offset).read() as usize;
        for i in 0..size {
            let elem = HEAP_START.add(2 + i + offset).read();
            if elem != TRUE && elem != FALSE && elem != 1 && elem & 1 == 1 {
                // This is a vector. Will update its reference
                // eprintln!("Updating Heap Vector Reference {} to {}", elem, reference_map[&elem]);
                *HEAP_START.add(2 + i + offset) = reference_map[&elem];
                let new_elem = HEAP_START.add(2 + i + offset).read();
            }
        }
        offset += size + 2;
    }

    // Updating any vectors referenced in the stack
    let mut rsp = curr_rsp;
    let mut rbp = curr_rbp;
    loop {
        // Iterating over local variables in this stack frame
        let mut ptr = rbp as *mut u64;
        ptr.sub(1);
        while ptr >= rsp as *mut u64 {
            let val = *ptr;
            // Check if value is a vector then marking if it is
            if val != TRUE && val != FALSE && val != 1 && val & 1 == 1 {
                // This is a vector. Will update its reference
                // eprintln!("Updating Stack Vector Reference {} to {}", val, reference_map[&val]);
                *ptr = reference_map[&val];
            }
            ptr = ptr.sub(1);
        }

        // Checking if stack_base was reached
        if rbp == stack_base {
            break;
        }

        // resetting the rsp and rbp to the next stack frame
        let prev_rbp = *rbp;
        rsp = rbp.add(2);
        rbp = prev_rbp as *mut u64;
    }

    // // // // // // // // //
    // SHIFTING ALL VECTORS //
    // // // // // // // // //

    // Iterating over heap and starting to shift over values
    let total_offset: usize = (heap_ptr as usize - HEAP_START as usize) / 8;
    let mut offset: usize = 0;
    let mut removed = 0;
    while offset < total_offset {
        let mark = HEAP_START.add(offset).read() as u64;
        let size = HEAP_START.add(1 + offset).read() as usize;
        if mark != 0 {
            // eprintln!("Need to shift values from {:p} to {:p}", HEAP_START.offset(offset as isize), mark as *mut u64);
            let new_addr = mark as *mut u64;
            for i in 0..size+2 {
                *new_addr.offset(i as isize) = HEAP_START.offset(offset as isize + i as isize).read() as u64;
            }
        } else {
            // eprintln!("Don't need to shift values");
            removed += size + 2;
        }
        offset += size + 2;
    }
    // eprintln!("Removed {} words", removed);
    let new_heap_ptr = heap_ptr.offset(-(removed as isize));
    // eprintln!("Heap Pointer Switching From {:p} to {:p}", heap_ptr, new_heap_ptr);

    new_heap_ptr
}

/// A helper function that can be called with the `(snek-printstack)` snek function. It prints the stack
/// See [`snek_try_gc`] for a description of the meaning of the arguments.
#[export_name = "\x01snek_print_stack"]
pub unsafe extern "C" fn snek_print_stack(
    stack_base: *const u64,
    curr_rbp: *const u64,
    curr_rsp: *const u64,
) {
    let mut ptr = stack_base;
    println!("-----------------------------------------");
    while ptr >= curr_rsp {
        let val = *ptr;
        println!("{ptr:?}: {:#0x}", val);
        ptr = ptr.sub(1);
    }
    println!("-----------------------------------------");
}

unsafe fn snek_str(val: SnekVal, seen: &mut HashSet<SnekVal>) -> String {
    if val == TRUE {
        format!("true")
    } else if val == FALSE {
        format!("false")
    } else if val & 1 == 0 {
        format!("{}", (val as i64) >> 1)
    } else if val == 1 {
        format!("nil")
    } else if val & 1 == 1 {
        if !seen.insert(val) {
            return "[...]".to_string();
        }
        let addr = (val - 1) as *const u64;
        let size = addr.add(1).read() as usize;
        let mut res = "[".to_string();
        for i in 0..size {
            let elem = addr.add(2 + i).read();
            res = res + &snek_str(elem, seen);
            if i < size - 1 {
                res = res + ", ";
            }
        }
        seen.remove(&val);
        res + "]"
    } else {
        format!("unknown value: {val}")
    }
}

fn parse_input(input: &str) -> u64 {
    match input {
        "true" => TRUE,
        "false" => FALSE,
        _ => (input.parse::<i64>().unwrap() << 1) as u64,
    }
}

fn parse_heap_size(input: &str) -> usize {
    input.parse::<usize>().unwrap()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let input = if args.len() >= 2 { &args[1] } else { "false" };
    let heap_size = if args.len() >= 3 { &args[2] } else { "10000" };
    let input = parse_input(&input);
    let heap_size = parse_heap_size(&heap_size);

    // Initialize heap
    let mut heap: Vec<u64> = Vec::with_capacity(heap_size);
    unsafe {
        HEAP_START = heap.as_mut_ptr();
        HEAP_END = HEAP_START.add(heap_size);
    }

    let i: u64 = unsafe { our_code_starts_here(input, HEAP_START, HEAP_END) };
    unsafe { snek_print(i) };
}
