mod infra;

// Your tests go here!
success_tests! {
    {
        name: make_vec_succ,
        file: "make_vec.snek",
        input: "5",
        expected: "[0, 0, 0, 0, 0]",
    },
    {
        name: vec_succ,
        file: "vec.snek",
        expected: "[0, 1, 2, 3]",
    },
    {
        name: vec_get_succ,
        file: "vec_get.snek",
        input: "3",
        expected: "3",
    },
    {
        name: linked_list_manipulations,
        file: "linked_list_manipulations.snek",
        expected: "1\n2\n3\n4\n5\n5\n4\n3\n2\n1\nnil"
    },
    {
        name: merge_sort,
        file: "merge_sort.snek",
        input: "1000",
        expected: "89"
    },
    {
        name: forest_flame_example,
        file: "example.snek",
        expected: "[nil, [1, 2], nil]\nnil\nnil"
    },
    {
        name: personal_test1,
        file: "personal_test1.snek",
        expected: "[false, true, 17]\n[3, 4]\n[3, 4]"
    },
    {
        name: personal_test2_succ,
        file: "personal_test2.snek",
        input: "false",
        heap_size: 12,
        expected: "15"
    }
}

runtime_error_tests! {
    {
        name: make_vec_oom,
        file: "make_vec.snek",
        input: "5",
        heap_size: 5,
        expected: "out of memory",
    },
    {
        name: vec_get_oob,
        file: "vec_get.snek",
        input: "5",
        expected: "",
    },
    {
        name: personal_test2_fail,
        file: "personal_test2.snek",
        input: "false",
        heap_size: 11,
        expected: "out of memory"
    },
    {
        name: insertion_sort_oom,
        file: "insertion_sort.snek",
        input: "1000",
        heap_size: 1001,
        expected: "out of memory",
    }
}

static_error_tests! {}
