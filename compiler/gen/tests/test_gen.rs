#[macro_use]
extern crate pretty_assertions;
#[macro_use]
extern crate indoc;

extern crate bumpalo;
extern crate inkwell;
extern crate libc;
extern crate roc_gen;

#[macro_use]
mod helpers;

#[cfg(test)]
mod test_gen {
    use crate::helpers::{can_expr, infer_expr, uniq_expr, CanExprOut};
    use bumpalo::Bump;
    use inkwell::context::Context;
    use inkwell::execution_engine::JitFunction;
    use inkwell::passes::PassManager;
    use inkwell::types::BasicType;
    use inkwell::OptimizationLevel;
    use roc_collections::all::ImMap;
    use roc_gen::llvm::build::{build_proc, build_proc_header};
    use roc_gen::llvm::convert::basic_type_from_layout;
    use roc_mono::expr::{Expr, Procs};
    use roc_mono::layout::Layout;
    use roc_types::subs::Subs;
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    #[test]
    fn basic_str() {
        assert_evals_to!(
            "\"shirt and hat\"",
            CString::new("shirt and hat").unwrap().as_c_str(),
            *const c_char,
            CStr::from_ptr
        );
    }

    #[test]
    fn basic_int() {
        assert_evals_to!("123", 123, i64);
    }

    #[test]
    fn basic_float() {
        assert_evals_to!("1234.0", 1234.0, f64);
    }

    #[test]
    fn empty_list_len() {
        assert_evals_to!("List.len []", 0, usize);
    }

    #[test]
    fn basic_int_list_len() {
        assert_evals_to!("List.len [ 12, 9, 6, 3 ]", 4, usize);
    }

    // #[test]
    // fn loaded_int_list_len() {
    //     assert_evals_to!(
    //         indoc!(
    //             r#"
    //                 nums = [ 2, 4, 6 ]

    //                 List.len nums
    //             "#
    //         ),
    //         3,
    //         usize
    //     );
    // }

    // #[test]
    // fn fn_int_list_len() {
    //     assert_evals_to!(
    //         indoc!(
    //             r#"
    //                 # TODO remove this annotation once monomorphization works!
    //                 getLen = \list -> List.len list

    //                 nums = [ 2, 4, 6 ]

    //                 getLen nums
    //             "#
    //         ),
    //         3,
    //         usize
    //     );
    // }

    //     #[test]
    //     fn int_list_is_empty() {
    //         assert_evals_to!("List.isEmpty [ 12, 9, 6, 3 ]", 0, u8, |x| x);
    //     }
    //
    #[test]
    fn empty_list_literal() {
        assert_evals_to!("[]", &[], &'static [i64], |x| x);
    }

    #[test]
    fn int_list_literal() {
        assert_evals_to!("[ 12, 9, 6, 3 ]", &[12, 9, 6, 3], &'static [i64], |x| x);
    }

    #[test]
    fn head_int_list() {
        assert_evals_to!("List.getUnsafe [ 12, 9, 6, 3 ] 0", 12, i64);
    }

    #[test]
    fn get_int_list() {
        assert_evals_to!("List.getUnsafe [ 12, 9, 6 ] 1", 9, i64);
    }

    #[test]
    fn get_set_unique_int_list() {
        assert_evals_to!("List.getUnsafe (List.set [ 12, 9, 7, 3 ] 1 42) 1", 42, i64);
    }

    #[test]
    fn set_unique_int_list() {
        assert_evals_to!(
            "List.set [ 12, 9, 7, 1, 5 ] 2 33",
            &[12, 9, 33, 1, 5],
            &'static [i64],
            |x| x
        );
    }

    #[test]
    fn set_unique_list_oob() {
        assert_evals_to!(
            "List.set [ 3, 17, 4.1 ] 1337 9.25",
            &[3.0, 17.0, 4.1],
            &'static [f64],
            |x| x
        );
    }

    #[test]
    fn set_shared_int_list() {
        assert_evals_to!(
            indoc!(
                r#"
                    shared = [ 2.1, 4.3 ]

                    # This should not mutate the original
                    x = List.getUnsafe (List.set shared 1 7.7) 1

                    { x, y: List.getUnsafe shared 1 }
                "#
            ),
            (7.7, 4.3),
            (f64, f64),
            |x| x
        );
    }

    #[test]
    fn set_shared_list_oob() {
        assert_evals_to!(
            indoc!(
                r#"
                    shared = [ 2, 4 ]

                    # This List.set is out of bounds, and should have no effect
                    x = List.getUnsafe (List.set shared 422 0) 1

                    { x, y: List.getUnsafe shared 1 }
                "#
            ),
            (4, 4),
            (i64, i64),
            |x| x
        );
    }

    #[test]
    fn get_unique_int_list() {
        assert_evals_to!(
            indoc!(
                r#"
                    shared = [ 2, 4 ]

                    List.getUnsafe shared 1
                "#
            ),
            4,
            i64
        );
    }

    #[test]
    fn branch_first_float() {
        assert_evals_to!(
            indoc!(
                r#"
                    when 1.23 is
                        1.23 -> 12
                        _ -> 34
                "#
            ),
            12,
            i64
        );
    }

    #[test]
    fn branch_second_float() {
        assert_evals_to!(
            indoc!(
                r#"
                        when 2.34 is
                            1.23 -> 63
                            _ -> 48
                    "#
            ),
            48,
            i64
        );
    }

    #[test]
    fn branch_third_float() {
        assert_evals_to!(
            indoc!(
                r#"
                   when 10.0 is
                       1.0 -> 63
                       2.0 -> 48
                       _ -> 112
                   "#
            ),
            112,
            i64
        );
    }

    #[test]
    fn branch_first_int() {
        assert_evals_to!(
            indoc!(
                r#"
                        when 1 is
                            1 -> 12
                            _ -> 34
                    "#
            ),
            12,
            i64
        );
    }

    #[test]
    fn branch_second_int() {
        assert_evals_to!(
            indoc!(
                r#"
                        when 2 is
                            1 -> 63
                            _ -> 48
                    "#
            ),
            48,
            i64
        );
    }

    #[test]
    fn branch_third_int() {
        assert_evals_to!(
            indoc!(
                r#"
                when 10 is
                    1 -> 63
                    2 -> 48
                    _ -> 112
                "#
            ),
            112,
            i64
        );
    }

    #[test]
    fn branch_store_variable() {
        assert_evals_to!(
            indoc!(
                r#"
                        when 0 is
                            1 -> 12
                            a -> a
                    "#
            ),
            0,
            i64
        );
    }

    #[test]
    fn one_element_tag() {
        assert_evals_to!(
            indoc!(
                r#"
                x : [ Pair Int ]
                x = Pair 2

                0x3
                "#
            ),
            3,
            i64
        );
    }

    #[test]
    fn when_one_element_tag() {
        assert_evals_to!(
            indoc!(
                r#"
                x : [ Pair Int Int ]
                x = Pair 0x2 0x3

                when x is
                    Pair l r -> l + r
                "#
            ),
            5,
            i64
        );
    }

    #[test]
    fn twice_record_access() {
        assert_evals_to!(
            indoc!(
                r#"
                x =  {a: 0x2, b: 0x3 }

                x.a + x.b
                "#
            ),
            5,
            i64
        );
    }

    #[test]
    fn gen_when_one_branch() {
        assert_evals_to!(
            indoc!(
                r#"
                    when 3.14 is
                        _ -> 23
                "#
            ),
            23,
            i64
        );
    }

    #[test]
    fn gen_large_when_int() {
        assert_evals_to!(
            indoc!(
                r#"
                    foo = \num ->
                        when num is
                            0 -> 200
                            -3 -> 111 # TODO adding more negative numbers reproduces parsing bugs here
                            3 -> 789
                            1 -> 123
                            2 -> 456
                            _ -> 1000

                    foo -3
                "#
            ),
            111,
            i64
        );
    }

    #[test]
    fn int_negate() {
        assert_evals_to!("Num.neg 123", -123, i64);
    }

    // #[test]
    // fn gen_large_when_float() {
    //     assert_evals_to!(
    //         indoc!(
    //             r#"
    //                 foo = \num ->
    //                     when num is
    //                         0.5 -> 200.1
    //                         -3.6 -> 111.2 # TODO adding more negative numbers reproduces parsing bugs here
    //                         3.6 -> 789.5
    //                         1.7 -> 123.3
    //                         2.8 -> 456.4
    //                         _ -> 1000.6

    //                 foo -3.6
    //             "#
    //         ),
    //         111.2,
    //         f64
    //     );
    // }

    #[test]
    fn gen_basic_def() {
        assert_evals_to!(
            indoc!(
                r#"
                    answer = 42

                    answer
                "#
            ),
            42,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                    pi = 3.14

                    pi
                "#
            ),
            3.14,
            f64
        );
    }

    #[test]
    fn gen_multiple_defs() {
        assert_evals_to!(
            indoc!(
                r#"
                    answer = 42

                    pi = 3.14

                    answer
                "#
            ),
            42,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                    answer = 42

                    pi = 3.14

                    pi
                "#
            ),
            3.14,
            f64
        );
    }

    #[test]
    fn gen_chained_defs() {
        assert_evals_to!(
            indoc!(
                r#"
                    x = i1
                    i3 = i2
                    i1 = 1337
                    i2 = i1
                    y = 12.4

                    i3
                "#
            ),
            1337,
            i64
        );
    }

    #[test]
    fn gen_nested_defs() {
        assert_evals_to!(
            indoc!(
                r#"
                    x = 5

                    answer =
                        i3 = i2

                        nested =
                            a = 1.0
                            b = 5

                            i1

                        i1 = 1337
                        i2 = i1


                        nested

                    # None of this should affect anything, even though names
                    # overlap with the previous nested defs
                    unused =
                        nested = 17

                        i1 = 84.2

                        nested

                    y = 12.4

                    answer
                "#
            ),
            1337,
            i64
        );
    }

    #[test]
    fn gen_basic_fn() {
        assert_evals_to!(
            indoc!(
                r#"
                    always42 : Num.Num Int.Integer -> Num.Num Int.Integer
                    always42 = \num -> 42

                    always42 5
                "#
            ),
            42,
            i64
        );
    }

    #[test]
    fn gen_when_fn() {
        assert_evals_to!(
            indoc!(
                r#"
                    limitedNegate = \num ->
                        when num is
                            1 -> -1
                            -1 -> 1
                            _ -> num

                    limitedNegate 1
                "#
            ),
            -1,
            i64
        );
    }

    #[test]
    fn gen_if_fn() {
        assert_evals_to!(
            indoc!(
                r#"
                    limitedNegate = \num ->
                        if num == 1 then
                            -1
                        else if num == -1 then
                            1
                        else
                            num

                    limitedNegate 1
                "#
            ),
            -1,
            i64
        );
    }

    #[test]
    fn gen_float_eq() {
        assert_evals_to!(
            indoc!(
                r#"
                1.0 == 1.0
                "#
            ),
            true,
            bool
        );
    }

    #[test]
    fn gen_literal_true() {
        assert_evals_to!(
            indoc!(
                r#"
                if True then -1 else 1
                "#
            ),
            -1,
            i64
        );
    }

    #[test]
    fn gen_if_float_fn() {
        assert_evals_to!(
            indoc!(
                r#"
                if True then -1.0 else 1.0
                "#
            ),
            -1.0,
            f64
        );
    }

    #[test]
    fn apply_identity_() {
        assert_evals_to!(
            indoc!(
                r#"
                    identity = \a -> a

                    identity 5
                "#
            ),
            5,
            i64
        );
    }

    #[test]
    fn apply_unnamed_fn() {
        assert_evals_to!(
            indoc!(
                r#"
                    (\a -> a) 5
                "#
            ),
            5,
            i64
        );
    }

    #[test]
    fn gen_add_f64() {
        assert_evals_to!(
            indoc!(
                r#"
                    1.1 + 2.4 + 3
                "#
            ),
            6.5,
            f64
        );
    }

    #[test]
    fn gen_add_i64() {
        assert_evals_to!(
            indoc!(
                r#"
                    1 + 2 + 3
                "#
            ),
            6,
            i64
        );
    }

    #[test]
    fn gen_sub_f64() {
        assert_evals_to!(
            indoc!(
                r#"
                    1.5 - 2.4 - 3
                "#
            ),
            -3.9,
            f64
        );
    }

    #[test]
    fn gen_sub_i64() {
        assert_evals_to!(
            indoc!(
                r#"
                    1 - 2 - 3
                "#
            ),
            -4,
            i64
        );
    }

    #[test]
    fn gen_mul_i64() {
        assert_evals_to!(
            indoc!(
                r#"
                    2 * 4 * 6
                "#
            ),
            48,
            i64
        );
    }

    #[test]
    fn gen_order_of_arithmetic_ops() {
        assert_evals_to!(
            indoc!(
                r#"
                    1 + 3 * 7 - 2
                "#
            ),
            20,
            i64
        );
    }

    #[test]
    fn return_unnamed_fn() {
        assert_evals_to!(
            indoc!(
                r#"
                    alwaysFloatIdentity : Int -> (Float -> Float)
                    alwaysFloatIdentity = \num ->
                        (\a -> a)

                    (alwaysFloatIdentity 2) 3.14
                "#
            ),
            3.14,
            f64
        );
    }

    #[test]
    fn basic_enum() {
        assert_evals_to!(
            indoc!(
                r#"
                Fruit : [ Apple, Orange, Banana ]

                apple : Fruit
                apple = Apple

                orange : Fruit
                orange = Orange

                apple == orange
                "#
            ),
            false,
            bool
        );
    }

    #[test]
    fn when_on_enum() {
        assert_evals_to!(
            indoc!(
                r#"
                Fruit : [ Apple, Orange, Banana ]

                apple : Fruit
                apple = Apple

                when apple is
                    Apple -> 1
                    Banana -> 2
                    Orange -> 3
                "#
            ),
            1,
            i64
        );
    }

    #[test]
    fn applied_tag_nothing() {
        assert_evals_to!(
            indoc!(
                r#"
                Maybe a : [ Just a, Nothing ]

                x : Maybe Int
                x = Nothing

                0x1
                "#
            ),
            1,
            i64
        );
    }
    #[test]
    fn applied_tag_just() {
        assert_evals_to!(
            indoc!(
                r#"
                Maybe a : [ Just a, Nothing ]

                y : Maybe Int
                y = Just 0x4

                0x1
                "#
            ),
            1,
            i64
        );
    }

    #[test]
    fn applied_tag_just_unit() {
        assert_evals_to!(
            indoc!(
                r#"
                Fruit : [ Orange, Apple, Banana ]
                Maybe a : [ Just a, Nothing ]

                orange : Fruit
                orange = Orange

                y : Maybe Fruit
                y = Just orange

                0x1
                "#
            ),
            1,
            i64
        );
    }

    #[test]
    fn when_on_nothing() {
        assert_evals_to!(
            indoc!(
                r#"
                x : [ Nothing, Just Int ]
                x = Nothing

                when x is
                    Nothing -> 0x2
                    Just _ -> 0x1
                "#
            ),
            2,
            i64
        );
    }

    #[test]
    fn when_on_just() {
        assert_evals_to!(
            indoc!(
                r#"
                x : [ Nothing, Just Int ]
                x = Just 41

                when x is
                    Just v -> v + 0x1
                    Nothing -> 0x1
                "#
            ),
            42,
            i64
        );
    }

    #[test]
    fn raw_result() {
        assert_evals_to!(
            indoc!(
                r#"
                x : Result Int Int
                x = Err 41

                x
                "#
            ),
            0,
            i8
        );
    }

    #[test]
    fn when_on_result() {
        assert_evals_to!(
            indoc!(
                r#"
                x : Result Int Int
                x = Err 41

                when x is
                    Err v ->  v + 1
                    Ok _ -> 1
                "#
            ),
            42,
            i64
        );
    }

    #[test]
    fn when_on_these() {
        assert_evals_to!(
            indoc!(
                r#"
                These a b : [ This a, That b, These a b ]

                x : These Int Int
                x = These 0x3 0x2

                when x is
                    These a b -> a + b
                    That v -> 8
                    This v -> v
                "#
            ),
            5,
            i64
        );
    }

    #[test]
    fn match_on_two_values() {
        // this will produce a Chain internally
        assert_evals_to!(
            indoc!(
                r#"
                when Pair 2 3 is
                    Pair 4 3 -> 9
                    Pair a b -> a + b
                "#
            ),
            5,
            i64
        );
    }

    #[test]
    fn pair_with_guard_pattern() {
        assert_evals_to!(
            indoc!(
                r#"
                when Pair 2 3 is
                    Pair 4 _ -> 1
                    Pair 3 _ -> 2
                    Pair a b -> a + b
                "#
            ),
            5,
            i64
        );
    }

    #[test]
    fn result_with_guard_pattern() {
        // This test revealed an issue with hashing Test values
        assert_evals_to!(
            indoc!(
                r#"
            x : Result Int Int
            x = Ok 2

            when x is
                Ok 3 -> 1
                Ok _ -> 2
                Err _ -> 3
            "#
            ),
            2,
            i64
        );
    }

    #[test]
    fn maybe_is_just() {
        assert_evals_to!(
            indoc!(
                r#"
                Maybe a : [ Just a, Nothing ]

                isJust : Maybe a -> Bool
                isJust = \list ->
                    when list is
                        Nothing -> False
                        Just _ -> True

                isJust (Just 42)
                "#
            ),
            true,
            bool
        );
    }

    #[test]
    fn when_on_record() {
        assert_evals_to!(
            indoc!(
                r#"
                when { x: 0x2 } is
                    { x } -> x + 3
                "#
            ),
            5,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                when { x: 0x2, y: 3.14 } is
                    { x: var } -> var + 3
                "#
            ),
            5,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                { x } = { x: 0x2, y: 3.14 }

                x
                "#
            ),
            2,
            i64
        );
    }

    #[test]
    fn record_guard_pattern() {
        assert_evals_to!(
            indoc!(
                r#"
                when { x: 0x2, y: 3.14 } is
                    { x: 0x4 } -> 5
                    { x } -> x + 3
                "#
            ),
            5,
            i64
        );
    }

    #[test]
    fn nested_tag_union() {
        assert_evals_to!(
            indoc!(
                r#"
                Maybe a : [ Nothing, Just a ]

                x : Maybe (Maybe a)
                x = Just (Just 41)

                5
                "#
            ),
            5,
            i64
        );
    }

    #[test]
    fn nested_record_load() {
        assert_evals_to!(
            indoc!(
                r#"
                Maybe a : [ Nothing, Just a ]

                x = { a : { b : 0x5 } }

                y = x.a

                y.b
                "#
            ),
            5,
            i64
        );
    }

    #[test]
    fn nested_pattern_match() {
        assert_evals_to!(
            indoc!(
                r#"
                Maybe a : [ Nothing, Just a ]

                x : Maybe (Maybe Int)
                x = Just (Just 41)

                when x is
                    Just (Just v) -> v + 0x1
                    _ -> 0x1
                "#
            ),
            42,
            i64
        );
    }

    #[test]
    fn or_pattern() {
        assert_evals_to!(
            indoc!(
                r#"
                when 2 is
                    1 | 2 -> 42
                    _ -> 1
                "#
            ),
            42,
            i64
        );
    }

    #[test]
    fn if_guard_pattern_false() {
        assert_evals_to!(
            indoc!(
                r#"
                when 2 is
                    2 if False -> 0
                    _ -> 42
                "#
            ),
            42,
            i64
        );
    }

    #[test]
    fn if_guard_pattern_true() {
        assert_evals_to!(
            indoc!(
                r#"
                when 2 is
                    2 if True -> 42
                    _ -> 0
                "#
            ),
            42,
            i64
        );
    }

    #[test]
    fn if_guard_exhaustiveness() {
        assert_evals_to!(
            indoc!(
                r#"
                when 2 is
                    _ if False -> 0
                    _ -> 42
                "#
            ),
            42,
            i64
        );
    }

    #[test]
    fn if_guard_bind_variable() {
        assert_evals_to!(
            indoc!(
                r#"
                when 10 is
                    x if x == 5 -> 0
                    _ -> 42
                "#
            ),
            42,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                when 10 is
                    x if x == 10 -> 42
                    _ -> 0
                "#
            ),
            42,
            i64
        );
    }

    //    #[test]
    //    fn linked_list_empty() {
    //        assert_evals_to!(
    //            indoc!(
    //                r#"
    //                LinkedList a : [ Cons a (LinkedList a), Nil ]
    //
    //                empty : LinkedList Int
    //                empty = Nil
    //
    //                1
    //                "#
    //            ),
    //            1,
    //            i64
    //        );
    //    }
    //
    //    #[test]
    //    fn linked_list_singleton() {
    //        assert_evals_to!(
    //            indoc!(
    //                r#"
    //                LinkedList a : [ Cons a (LinkedList a), Nil ]
    //
    //                singleton : LinkedList Int
    //                singleton = Cons 0x1 Nil
    //
    //                1
    //                "#
    //            ),
    //            1,
    //            i64
    //        );
    //    }
    //
    //    #[test]
    //    fn linked_list_is_empty() {
    //        assert_evals_to!(
    //            indoc!(
    //                r#"
    //                LinkedList a : [ Cons a (LinkedList a), Nil ]
    //
    //                isEmpty : LinkedList a -> Bool
    //                isEmpty = \list ->
    //                    when list is
    //                        Nil -> True
    //                        Cons _ _ -> False
    //
    //                isEmpty (Cons 4 Nil)
    //                "#
    //            ),
    //            false,
    //            bool
    //        );
    //    }

    #[test]
    fn empty_record() {
        assert_evals_to!(
            indoc!(
                r#"
                v = {}

                1
                "#
            ),
            1,
            i64
        );
    }

    #[test]
    fn unit_type() {
        assert_evals_to!(
            indoc!(
                r#"
                Unit : [ Unit ]

                v : Unit
                v = Unit

                1
                "#
            ),
            1,
            i64
        );
    }

    #[test]
    fn pattern_matching_unit() {
        assert_evals_to!(
            indoc!(
                r#"
                Unit : [ Unit ]

                f : Unit -> Int
                f = \Unit -> 42

                f Unit
                "#
            ),
            42,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                Unit : [ Unit ]

                x : Unit
                x = Unit

                when x is
                    Unit -> 42
                "#
            ),
            42,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                f : {} -> Int
                f = \{} -> 42

                f {}
                "#
            ),
            42,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                when {} is
                    {} -> 42
                "#
            ),
            42,
            i64
        );
    }

    #[test]
    fn basic_record() {
        assert_evals_to!(
            indoc!(
                r#"
                    { y: 17, x: 15, z: 19 }.x
                "#
            ),
            15,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                    { x: 15, y: 17, z: 19 }.y
                "#
            ),
            17,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                    { x: 15, y: 17, z: 19 }.z
                "#
            ),
            19,
            i64
        );
    }

    #[test]
    fn def_record() {
        assert_evals_to!(
            indoc!(
                r#"
                    rec = { y: 17, x: 15, z: 19 }

                    rec.x
                "#
            ),
            15,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                    rec = { x: 15, y: 17, z: 19 }

                    rec.y
                "#
            ),
            17,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                    rec = { x: 15, y: 17, z: 19 }

                    rec.z
                "#
            ),
            19,
            i64
        );
    }

    #[test]
    fn fn_record() {
        assert_evals_to!(
            indoc!(
                r#"
                    getRec = \x -> { y: 17, x, z: 19 }

                    (getRec 15).x
                "#
            ),
            15,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                    rec = { x: 15, y: 17, z: 19 }

                    rec.y
                "#
            ),
            17,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                    rec = { x: 15, y: 17, z: 19 }

                    rec.z
                "#
            ),
            19,
            i64
        );

        assert_evals_to!(
            indoc!(
                r#"
                    rec = { x: 15, y: 17, z: 19 }

                    rec.z + rec.x
                "#
            ),
            34,
            i64
        );
    }

    #[test]
    fn i64_record2_literal() {
        assert_evals_to!(
            indoc!(
                r#"
                   { x: 3, y: 5 }
                "#
            ),
            (3, 5),
            (i64, i64)
        );
    }

    #[test]
    fn true_is_true() {
        assert_evals_to!(
            indoc!(
                r#"
                   bool : [True, False]
                   bool = True

                   bool
                "#
            ),
            true,
            bool
        );
    }

    #[test]
    fn i64_record3_literal() {
        assert_evals_to!(
            indoc!(
                r#"
                   { x: 3, y: 5, z: 17 }
                "#
            ),
            (3, 5, 17),
            (i64, i64, i64)
        );
    }

    #[test]
    fn f64_record2_literal() {
        assert_evals_to!(
            indoc!(
                r#"
                   { x: 3.1, y: 5.1 }
                "#
            ),
            (3.1, 5.1),
            (f64, f64)
        );
    }

    #[test]
    fn f64_record3_literal() {
        assert_evals_to!(
            indoc!(
                r#"
                   { x: 3.1, y: 5.1, z: 17.1 }
                "#
            ),
            (3.1, 5.1, 17.1),
            (f64, f64, f64)
        );
    }

    #[test]
    fn false_is_false() {
        assert_evals_to!(
            indoc!(
                r#"
                   bool : [True, False]
                   bool = False

                   bool
                "#
            ),
            false,
            bool
        );
    }

    // #[test]
    // fn bool_record4_literal() {
    //     assert_evals_to!(
    //         indoc!(
    //             r#"
    //                record : { a : Bool, b : Bool, c : Bool, d : Bool }
    //                record = { a: True, b: True, c : True, d : Bool }

    //                record
    //             "#
    //         ),
    //         (true, false, false, true),
    //         (bool, bool, bool, bool)
    //     );
    // }

    #[test]
    fn i64_record1_literal() {
        assert_evals_to!(
            indoc!(
                r#"
                   { a: 3 }
                "#
            ),
            3,
            i64
        );
    }

    // #[test]
    // fn i64_record9_literal() {
    //     assert_evals_to!(
    //         indoc!(
    //             r#"
    //                { a: 3, b: 5, c: 17, d: 1, e: 9, f: 12, g: 13, h: 14, i: 15 }
    //             "#
    //         ),
    //         (3, 5, 17, 1, 9, 12, 13, 14, 15),
    //         (i64, i64, i64, i64, i64, i64, i64, i64, i64)
    //     );
    // }

    // #[test]
    // fn f64_record3_literal() {
    //     assert_evals_to!(
    //         indoc!(
    //             r#"
    //                { x: 3.1, y: 5.1, z: 17.1 }
    //             "#
    //         ),
    //         (3.1, 5.1, 17.1),
    //         (f64, f64, f64)
    //     );
    // }

    #[test]
    fn f64_record() {
        assert_evals_to!(
            indoc!(
                r#"
                   rec = { y: 17.2, x: 15.1, z: 19.3 }

                   rec.x
                "#
            ),
            15.1,
            f64
        );

        assert_evals_to!(
            indoc!(
                r#"
                   rec = { y: 17.2, x: 15.1, z: 19.3 }

                   rec.y
                "#
            ),
            17.2,
            f64
        );

        assert_evals_to!(
            indoc!(
                r#"
                    rec = { y: 17.2, x: 15.1, z: 19.3 }

                    rec.z
                "#
            ),
            19.3,
            f64
        );
    }
    #[test]
    fn bool_literal() {
        assert_evals_to!(
            indoc!(
                r#"
                x : Bool
                x = True

                x
                "#
            ),
            true,
            bool
        );
    }

    #[test]
    fn tail_call_elimination() {
        assert_evals_to!(
            indoc!(
                r#"
                sum = \n, accum ->
                    when n is
                        0 -> accum
                        _ -> sum (n - 1) (n + accum)

                sum 1_000_000 0
                "#
            ),
            500000500000,
            i64
        );
    }

    #[test]
    fn even_odd() {
        assert_evals_to!(
            indoc!(
                r#"
                even = \n ->
                    when n is
                        0 -> True
                        1 -> False
                        _ -> odd (n - 1)

                odd = \n ->
                    when n is
                        0 -> False
                        1 -> True
                        _ -> even (n - 1)

                odd 5 && even 42
                "#
            ),
            true,
            bool
        );
    }
}
