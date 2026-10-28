//! Test suite for `crate::interpreter::arithmetic`.

#[cfg(test)]
mod ceil_tests {}

#[cfg(test)]
mod round_tests {
    use crate::interpreter::Interpreter;

    #[tokio::test]
    async fn test_round_positive_below_half() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("[ 7/3 ] ROUND").await.unwrap();
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        let result = format!("{}", stack[0]);
        assert_eq!(result, "[ 2/1 ]", "ROUND(7/3) should be 2");
    }

    #[tokio::test]
    async fn test_round_positive_half() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("[ 5/2 ] ROUND").await.unwrap();
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        let result = format!("{}", stack[0]);
        assert_eq!(result, "[ 3/1 ]", "ROUND(5/2) should be 3");
    }

    #[tokio::test]
    async fn test_round_i64_min_numerator_without_overflow() {
        // Regression: the Small(i64::MIN, d) path used i64::abs(), which
        // overflows and panics in debug. Must round without aborting.
        let mut interp = Interpreter::new();
        interp
            .execute("-9223372036854775808/3 ROUND")
            .await
            .unwrap();
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        assert_eq!(format!("{}", stack[0]), "-3074457345618258603/1");
    }

    #[tokio::test]
    async fn test_round_positive_above_half() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("[ 8/3 ] ROUND").await.unwrap();
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        let result = format!("{}", stack[0]);
        assert_eq!(result, "[ 3/1 ]", "ROUND(8/3) should be 3");
    }

    #[tokio::test]
    async fn test_round_negative_below_half() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("[ -7/3 ] ROUND").await.unwrap();
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        let result = format!("{}", stack[0]);
        assert_eq!(result, "[ -2/1 ]", "ROUND(-7/3) should be -2");
    }

    #[tokio::test]
    async fn test_round_negative_half() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("[ -5/2 ] ROUND").await.unwrap();
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        let result = format!("{}", stack[0]);
        assert_eq!(result, "[ -3/1 ]", "ROUND(-5/2) should be -3");
    }

    #[tokio::test]
    async fn test_round_negative_above_half() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("[ -8/3 ] ROUND").await.unwrap();
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        let result = format!("{}", stack[0]);
        assert_eq!(result, "[ -3/1 ]", "ROUND(-8/3) should be -3");
    }

    #[tokio::test]
    async fn test_round_positive_integer() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("[ 6/3 ] ROUND").await.unwrap();
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        let result = format!("{}", stack[0]);
        assert_eq!(result, "[ 2/1 ]", "ROUND(6/3) should be 2");
    }

    #[tokio::test]
    async fn test_round_negative_integer() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("[ -6/3 ] ROUND").await.unwrap();
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        let result = format!("{}", stack[0]);
        assert_eq!(result, "[ -2/1 ]", "ROUND(-6/3) should be -2");
    }

    #[tokio::test]
    async fn test_round_operation_target_stack_error() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        let result = interp.execute("[ 1 2 3 ] .. ROUND").await;
        assert!(result.is_err(), "ROUND should not support Stack mode (..)");
    }

    #[tokio::test]
    async fn test_round_of_nil_passes_nil_through() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("NIL").await.unwrap();
        interp
            .execute("ROUND")
            .await
            .expect("ROUND of NIL should succeed and produce NIL");
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        assert!(stack[0].is_nil(), "ROUND of NIL should yield NIL");
    }
}

#[cfg(test)]
mod num_tests {
    use crate::interpreter::Interpreter;

    #[tokio::test]
    async fn test_num_lifts_over_a_vector_and_projects_per_element() {
        // A Vector where one String is read lifts NUM over its elements
        // (LANG.COLLECTIONS.LIFT); an unparsable element projects its own NIL.
        let mut interp = Interpreter::new();
        interp.execute("[ '1' 'hello' ] NUM").await.unwrap();
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        assert_eq!(stack[0].to_string(), "[ 1/1 NIL ]");
    }

    #[tokio::test]
    async fn test_num_same_structure_error_stack_restoration() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("[ 42 ]").await.unwrap();
        let result = interp.execute("NUM").await;

        assert!(result.is_err(), "NUM should error on number vector [42]");
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1, "Stack should be restored after error");
    }

    #[tokio::test]
    async fn test_num_error_in_one_element_restores_the_whole_operand() {
        let mut interp = Interpreter::new();
        interp.execute("[ '1' TRUE ]").await.unwrap();
        let result = interp.execute("NUM").await;
        assert!(result.is_err());
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1, "the lifted operand is restored whole");
        assert_eq!(stack[0].to_string(), "[ '1' TRUE ]");
    }

    #[tokio::test]
    async fn test_num_operation_target_stack_error() {
        let mut interp = Interpreter::new();
        interp.execute("").await.unwrap();
        interp.execute("[ '42' ] [ '123' ]").await.unwrap();
        let result = interp.execute(".. NUM").await;
        assert!(result.is_err());
        let stack = interp.get_stack();
        assert_eq!(
            stack.len(),
            2,
            "Stack should remain unchanged after Stack mode error"
        );
    }
}

#[cfg(test)]
mod interval_tests {
    use crate::interpreter::Interpreter;

    #[tokio::test]
    async fn test_sqrt_exact_cases() {
        let mut interp = Interpreter::new();
        interp.execute("4 SQRT").await.unwrap();
        assert_eq!(format!("{}", interp.get_stack()[0]), "2/1");

        let mut interp = Interpreter::new();
        interp.execute("9/16 SQRT").await.unwrap();
        assert_eq!(format!("{}", interp.get_stack()[0]), "3/4");
    }
}

#[cfg(test)]
mod nil_passthrough_tests {

    use crate::test_support::run;

    #[tokio::test]
    async fn add_with_nil_left_yields_nil() {
        let interp = run("NIL 3 ADD").await;
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        assert!(stack[0].is_nil(), "got {}", stack[0]);
    }

    #[tokio::test]
    async fn add_with_nil_right_yields_nil() {
        let interp = run("3 NIL ADD").await;
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1);
        assert!(stack[0].is_nil(), "got {}", stack[0]);
    }

    #[tokio::test]
    async fn sub_mul_div_with_nil_yield_nil() {
        let interp = run("NIL 5 SUB").await;
        assert!(interp.get_stack()[0].is_nil());
        let interp = run("NIL 5 MUL").await;
        assert!(interp.get_stack()[0].is_nil());
        let interp = run("NIL 5 DIV").await;
        assert!(interp.get_stack()[0].is_nil());
    }

    #[tokio::test]
    async fn div_by_nil_does_not_raise_division_by_zero() {
        let interp = run("5 NIL DIV").await;
        assert!(interp.get_stack()[0].is_nil());
    }

    #[tokio::test]
    async fn comparisons_with_nil_yield_nil() {
        let interp = run("NIL 3 LT").await;
        assert!(interp.get_stack()[0].is_nil());
        let interp = run("NIL NIL EQ").await;
        assert!(interp.get_stack()[0].is_nil());
        let interp = run("NIL 3 GT").await;
        assert!(interp.get_stack()[0].is_nil());
        let interp = run("NIL 3 EQ NOT").await;
        assert!(interp.get_stack()[0].is_nil());
    }

    #[tokio::test]
    async fn divide_then_add_propagates_nil_through_pipeline() {
        // The scalar law: `-10 SQRT` projects to NIL, and the NIL survives the
        // `ADD` that follows it.
        let interp = run("-10 SQRT 1 ADD").await;
        let stack = interp.get_stack();
        assert!(
            stack.last().unwrap().is_nil(),
            "expected NIL on top of stack after divide and add; got {}",
            stack.last().unwrap()
        );

        // Lifted over a vector, that law applies per lane
        // (LANG.COLLECTIONS.LIFT): the negative radicand empties its own
        // lane, and the lane -- not the vector around it -- is what carries
        // the NIL onward. This case used to assert the whole value went NIL,
        // which is the collapse the lane law forbids: `[ 4 ] SQRT` answers
        // `[ 2/1 ]`, so `[ -10 ] SQRT` answers `[ NIL ]`.
        let interp = run("[ -10 ] SQRT 1 ADD").await;
        let stack = interp.get_stack();
        let result = stack.last().unwrap();
        let lanes = result
            .as_vector_view()
            .expect("a lifted divide answers with a vector");
        assert_eq!(lanes.len(), 1, "expected one lane; got {result}");
        assert!(
            lanes[0].is_nil(),
            "expected the lane to stay NIL after divide and add; got {result}"
        );
    }

    #[tokio::test]
    async fn a_fallback_can_replace_a_nil_that_passed_through() {
        let interp = run("-10 SQRT 1 ADD 'S' BIND 0 S S NIL? SELECT").await;
        let stack = interp.get_stack();
        assert_eq!(stack.len(), 1, "the choice leaves exactly one value");
        assert!(
            !stack.last().unwrap().is_nil(),
            "the fallback should have been chosen; got {}",
            stack.last().unwrap()
        );
    }
}

#[cfg(test)]
mod ai_first_comparison_tests {

    use crate::test_support::{bool_of, run};
    // Tests for the AI-first comparison primitive GT. It mirrors
    // LT / EQ and exists so an automated producer can emit the relation
    // that matches its intent directly rather than rewriting it as a
    // negation or operand swap.

    // ── canonical-name parity with LT/EQ ─────────────────────────────────

    #[tokio::test]
    async fn gt_canonical_name_returns_true_when_strictly_greater() {
        let interp = run("2 1 GT").await;
        assert!(bool_of(&interp));
    }

    #[tokio::test]
    async fn gt_returns_false_on_equal_values() {
        let interp = run("1 1 GT").await;
        assert!(!bool_of(&interp));
    }

    #[tokio::test]
    async fn eq_not_canonical_name_returns_true_when_different() {
        let interp = run("1 2 EQ NOT").await;
        assert!(bool_of(&interp));
    }

    #[tokio::test]
    async fn eq_not_returns_false_when_equal() {
        let interp = run("3 3 EQ NOT").await;
        assert!(!bool_of(&interp));
    }

    // ── exact rational comparison ────────────────────────────────────────

    #[tokio::test]
    async fn gt_compares_fractions_exactly() {
        let interp = run("7/2 17/5 GT").await;
        // 7/2 = 35/10, 17/5 = 34/10, so 7/2 > 17/5.
        assert!(bool_of(&interp));
    }

    // ── EQ NOT structural equality on vectors ───────────────────────────────

    #[tokio::test]
    async fn eq_not_returns_false_for_structurally_equal_vectors() {
        let interp = run("[ 1 2 3 ] [ 1 2 3 ] EQ NOT").await;
        assert!(!bool_of(&interp));
    }

    #[tokio::test]
    async fn eq_not_returns_true_for_structurally_different_vectors() {
        let interp = run("[ 1 2 3 ] [ 1 2 4 ] EQ NOT").await;
        assert!(bool_of(&interp));
    }
    // ── NIL passthrough for the new ops (contract: nil_policy = Passthrough)

    #[tokio::test]
    async fn gt_with_nil_left_yields_nil() {
        let interp = run("NIL 1 GT").await;
        assert!(interp.get_stack()[0].is_nil());
    }

    #[tokio::test]
    async fn eq_not_with_two_nils_yields_nil() {
        // EQ is NIL-passthrough and NOT keeps UNKNOWN, so NIL NIL EQ NOT is NIL — *not* FALSE.
        // (NIL is an absence value, not a member of an equivalence class.)
        let interp = run("NIL NIL EQ NOT").await;
        assert!(interp.get_stack()[0].is_nil());
    }

    // ── stack-mode sequence properties ───────────────────────────────────
}

#[cfg(test)]
mod ordering_decision_tests {

    use crate::test_support::{bool_of, run};
    // The ordering Words decide every pair of numbers (LANG.VALUES.EXACT);
    // a NIL operand passes through.

    // ── Regression: every ordering decides on rational operands ──────────

    #[tokio::test]
    async fn lt_decides_on_rational_pair() {
        let interp = run("1/2 2/3 LT").await;
        assert!(bool_of(&interp));
    }

    #[tokio::test]
    async fn gt_decides_on_negative_left() {
        let interp = run("-3/2 1/2 GT").await;
        assert!(!bool_of(&interp));
    }

    // ── NIL passthrough is unchanged ─────────────────────────────────────

    #[tokio::test]
    async fn lt_with_left_nil_passes_nil_through() {
        let interp = run("NIL 1 LT").await;
        assert!(interp.get_stack()[0].is_nil());
    }

    #[tokio::test]
    async fn lt_with_right_nil_passes_nil_through() {
        let interp = run("1 NIL LT").await;
        assert!(interp.get_stack()[0].is_nil());
    }
}

/// `EQ` decides every pair of numbers (LANG.VALUES.EXACT): rational operands
/// by `Fraction` equality, anything reaching the algebraic field through the
/// total `ExactReal::cmp_exact`.
#[cfg(test)]
mod eq_decision_tests {

    use crate::test_support::{bool_of, run};
    use crate::types::exact::ExactReal;
    use crate::types::fraction::Fraction;
    use num_bigint::BigInt;

    // ── Regression: EQ still decides on rationals ───────────────────

    #[tokio::test]
    async fn eq_decides_value_equal_reduced_rationals() {
        let interp = run("2/4 1/2 EQ").await;
        assert!(bool_of(&interp));
    }

    #[tokio::test]
    async fn eq_decides_unequal_rationals() {
        let interp = run("1/2 2/3 EQ").await;
        assert!(!bool_of(&interp));
    }

    #[tokio::test]
    async fn eq_not_decides_unequal_rationals() {
        let interp = run("1/2 2/3 EQ NOT").await;
        assert!(bool_of(&interp));
    }

    #[tokio::test]
    async fn eq_not_decides_equal_reduced_rationals() {
        let interp = run("2/4 1/2 EQ NOT").await;
        assert!(!bool_of(&interp));
    }

    #[tokio::test]
    async fn eq_decides_large_rationals() {
        let interp = run("355/113 355/113 EQ").await;
        assert!(bool_of(&interp));
    }

    #[tokio::test]
    async fn eq_decides_negative_vs_positive() {
        let interp = run("-1/2 1/2 EQ").await;
        assert!(!bool_of(&interp));
    }

    // ── NIL passthrough is unchanged ─────────────────────────────────────

    #[tokio::test]
    async fn eq_with_left_nil_passes_nil_through() {
        let interp = run("NIL 1 EQ").await;
        assert!(interp.get_stack()[0].is_nil());
    }

    #[tokio::test]
    async fn eq_not_with_right_nil_passes_nil_through() {
        let interp = run("1 NIL EQ NOT").await;
        assert!(interp.get_stack()[0].is_nil());
    }

    // ── ExactReal-level dispatch boundary ────────────────────────────────
    //
    // These cover the exact comparison that `pairwise_eq` /
    // `scalar_pair_eq` route through whenever at least one operand is
    // non-Rational: total over the field.

    fn rational(n: i64, d: i64) -> ExactReal {
        ExactReal::Rational(Fraction::new(BigInt::from(n), BigInt::from(d)))
    }

    #[test]
    fn exact_real_cmp_decides_equal_rationals() {
        assert_eq!(
            rational(2, 4).cmp_exact(&rational(1, 2)),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn exact_real_cmp_decides_unequal_rationals() {
        assert_eq!(
            rational(1, 2).cmp_exact(&rational(2, 3)),
            Some(std::cmp::Ordering::Less)
        );
    }

    #[test]
    fn exact_real_cmp_decides_rational_vs_algebraic_sqrt() {
        // √2 is irrational; it can never equal 7/5 (or any rational), and
        // the algebraic comparison decides the order exactly.
        let sqrt_two =
            ExactReal::from_sqrt_rational(Fraction::new(BigInt::from(2), BigInt::from(1)))
                .expect("sqrt(2) constructible");
        assert_eq!(
            sqrt_two.cmp_exact(&rational(7, 5)),
            Some(std::cmp::Ordering::Greater)
        );
    }
}
