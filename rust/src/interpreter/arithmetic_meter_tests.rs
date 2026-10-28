//! The work meter is charged on the arithmetic, not on the shape it arrived in.
//!
//! The meter used to live in two routes out of the arithmetic dispatch, both
//! reachable only when both operands were scalar-shaped. `2 3 MUL` was priced;
//! `[ 2 ] 3 MUL` was free. Because that is a *shape* axis and not a code-path
//! axis, the tests here run the same arithmetic through scalars, one-element
//! vectors, N-lane vectors and irrational lanes, and require the same answer
//! from the meter each time.
//!
//! The path-invariance case at the end is a regression guard rather than a
//! fix: block, user word and `EXEC` already agreed to the unit before this
//! module existed. It is kept so a future optimization that skips the dispatch
//! entry is caught, and labelled so its green is not read as evidence for the
//! shape fix above it.

use crate::interpreter::runtime_limits::RuntimeLimits;
use crate::interpreter::Interpreter;
use crate::test_support::{charged_by, with_limits};

// ── the meter prices the arithmetic, not the shape it arrived in ───────

/// Assert that `source` is refused by `resource` under an injected ceiling.
macro_rules! assert_refused_by {
    ($source:expr, $limits:expr, $resource:ident, $limit:expr) => {{
        let mut interp = with_limits($limits);
        let err = interp
            .execute($source)
            .await
            .err()
            .unwrap_or_else(|| panic!("`{}` must be refused, it succeeded", $source));
        assert!(
            matches!(
                err,
                crate::error::AjisaiError::ResourceLimitExceeded {
                    resource: crate::error::ResourceLimit::$resource,
                    limit: $limit,
                    ..
                }
            ),
            "`{}` must be refused by {} at {}, got: {err:?}",
            $source,
            stringify!($resource),
            $limit
        );
    }};
}

#[tokio::test]
async fn multiplication_is_charged_in_every_operand_shape() {
    // The meter used to live in two places, both reachable only when both
    // operands were scalar-shaped. Every other route out of the arithmetic
    // dispatch — column kernels, rational broadcast, exact-real broadcast —
    // charged nothing, so `[ 2 ] 3 MUL` was free while `2 3 MUL` was not. A
    // control a pair of brackets switches off is not a control.
    for source in [
        "2 3 MUL",                   // scalar × scalar
        "[ 2 ] [ 3 ] MUL",           // length-1 vectors, matching wraps
        "[ 2 ] 3 MUL",               // vector × scalar, broadcast
        "3 [ 2 ] MUL",               // scalar × vector, broadcast
        "[ 2 2 2 2 2 2 2 2 ] 3 MUL", // N lanes, past the SIMD threshold
        "[ 2 4 6 ] [ 3 5 7 ] MUL",   // N lanes, pairwise
        "2 SQRT [ 3 ] MUL",          // an irrational lane, exact-real broadcast
    ] {
        assert!(
            charged_by(source).await > 0,
            "`{source}` reached the work meter at zero"
        );
    }
}

#[tokio::test]
async fn every_schema_is_charged_in_vector_shape() {
    // Division dispatches through its own broadcast arm, and addition and
    // subtraction through a third; all four have to be on the meter or the
    // ceiling is a claim about `MUL` alone.
    for source in ["[ 6 ] 3 ADD", "[ 6 ] 3 SUB", "[ 6 ] 3 MUL", "[ 6 ] 3 DIV"] {
        assert!(
            charged_by(source).await > 0,
            "`{source}` reached the work meter at zero"
        );
    }
}

#[tokio::test]
async fn every_operand_shape_reaches_the_same_named_ceiling() {
    // Shape decides representation. It must not decide which safety
    // control applies, or whether one applies at all.
    for source in [
        "2 3 MUL",
        "[ 2 ] 3 MUL",
        "[ 2 2 2 2 2 2 2 2 ] 3 MUL",
        "[ 2 4 6 ] [ 3 5 7 ] MUL",
        "2 SQRT [ 3 ] MUL",
    ] {
        assert_refused_by!(
            source,
            RuntimeLimits {
                max_numeric_work: 0,
                ..RuntimeLimits::default()
            },
            NumericWork,
            0
        );
    }
}

#[tokio::test]
async fn a_refusal_by_shape_leaves_the_operands_on_the_stack() {
    // Charged before the operation runs and before its operands are
    // consumed, so the interpreter is still usable afterwards — the same
    // guarantee the scalar-only meter gave. The budget admits a one-lane
    // operation and refuses a three-lane one, which is the whole point:
    // lanes are what is being priced.
    let mut interp = with_limits(RuntimeLimits {
        max_numeric_work: 2,
        ..RuntimeLimits::default()
    });
    interp
        .execute("[ 2 4 6 ] [ 3 5 7 ] MUL")
        .await
        .expect_err("three lanes must not fit a two-unit budget");
    assert!(
        interp.execute("1 1 ADD").await.is_ok(),
        "a limit failure must not poison the next program"
    );
}

#[tokio::test]
async fn vector_accumulator_growth_meets_the_same_ceiling_a_scalar_one_does() {
    // `1 21000 RANGE [ 1 ] [ MUL ] FOLD` returned a 271,233-bit integer
    // under an agent profile declaring 262,144 — charged nothing, no
    // ceiling firing — while the identical fold with a scalar accumulator
    // was refused. The difference was one pair of brackets. Reproduced at a
    // low injected ceiling, so the test builds nothing huge.
    for source in ["1 40 RANGE 1 [ MUL ] FOLD", "1 40 RANGE [ 1 ] [ MUL ] FOLD"] {
        assert_refused_by!(
            source,
            RuntimeLimits {
                max_bigint_bits: 64,
                ..RuntimeLimits::default()
            },
            BigintBits,
            64
        );
    }
}

#[tokio::test]
async fn higher_order_application_charges_what_direct_source_charges() {
    // A regression guard, not a fix: these agreed *before* the shape hole
    // was closed, to the unit. The metering gap was never about `MAP` and
    // `FOLD` — the same fold with a scalar accumulator was charged all
    // along, and it was the vector-shaped one that escaped. Kept so a
    // future optimization that bypasses the dispatch entry is caught here.
    let direct = charged_by("1 20 RANGE 1 [ MUL ] FOLD").await;
    let user_word = charged_by("[ MUL ] 'MULW' DEF 1 20 RANGE 1 [ MULW ] FOLD").await;
    let exec = charged_by("1 20 RANGE 1 [ [ MUL ] EXEC ] FOLD").await;
    assert!(direct > 0, "a 20-step fold must reach the meter");
    assert_eq!(
        direct, user_word,
        "a fold through a user word must cost what the block costs"
    );
    assert_eq!(
        direct, exec,
        "a fold through EXEC must cost what the block costs"
    );
}

#[tokio::test]
async fn ordinary_vector_arithmetic_stays_far_below_the_meter() {
    // Charging lanes must not turn ordinary array work into a refusal.
    for source in [
        "[ 1 2 3 ] 10 ADD",
        "0 999 RANGE 2 MUL",
        "0 999 RANGE 0 [ ADD ] FOLD",
        "[ 1 2 3 ] [ 4 5 6 ] MUL",
    ] {
        let mut interp = Interpreter::new();
        interp
            .execute(source)
            .await
            .expect("ordinary source computes");
        assert!(
            interp.numeric_work_used < 100_000,
            "`{source}` charged {} units, which is not ordinary",
            interp.numeric_work_used
        );
    }
}

async fn result_of(interp: &mut Interpreter, source: &str) -> String {
    match interp.execute(source).await {
        Ok(()) => format!(
            "{}",
            interp
                .get_stack()
                .as_slice()
                .last()
                .expect("a result on the stack")
        ),
        Err(e) => format!("{e:?}"),
    }
}

/// An accumulator that leaves `i64` must promote exactly, not wrap and
/// not saturate. The third case is the one an end-to-end check would
/// miss: no *intermediate* fits, but the total does.
#[tokio::test]
async fn a_fold_past_i64_promotes_exactly() {
    for (source, expected) in [
        (
            "[ 9223372036854775807 1 ] 0 [ ADD ] FOLD",
            "9223372036854775808/1",
        ),
        (
            "[ -9223372036854775808 -1 ] 0 [ ADD ] FOLD",
            "-9223372036854775809/1",
        ),
        (
            "[ 9223372036854775807 1 -1 ] 0 [ ADD ] FOLD",
            "9223372036854775807/1",
        ),
    ] {
        let mut interp = Interpreter::new();
        assert_eq!(
            result_of(&mut interp, source).await,
            expected,
            "`{source}` must promote exactly"
        );
    }
}

/// A fraction's width is read where it lies, without copying its halves out;
/// it must be the width those halves have as `BigInt`s, `Small` and `Big`
/// alike, and a sum or difference of wide fractions must be what the owned
/// pair arithmetic would give.
#[test]
fn width_and_wide_sums_are_what_the_owned_halves_give() {
    use crate::interpreter::runtime_limits::fraction_result_bits;
    use crate::types::fraction::Fraction;
    use num_bigint::BigInt;

    let wide: BigInt = BigInt::from(10).pow(40u32) + 7;
    let fractions = [
        Fraction::new(BigInt::from(0), BigInt::from(1)),
        Fraction::new(BigInt::from(1), BigInt::from(3)),
        Fraction::new(BigInt::from(-7), BigInt::from(2)),
        Fraction::new(BigInt::from(i64::MAX), BigInt::from(1)),
        Fraction::new(BigInt::from(i64::MIN), BigInt::from(3)),
        Fraction::new(wide.clone(), BigInt::from(1)),
        Fraction::new(BigInt::from(5), wide.clone()),
        Fraction::new(-wide.clone(), wide.clone() + 2),
        Fraction::new(wide.clone() * 3, BigInt::from(6)),
    ];
    for f in &fractions {
        assert_eq!(
            fraction_result_bits(f),
            f.numerator().bits().max(f.denominator().bits()),
            "{f:?}"
        );
    }
    for a in &fractions {
        for b in &fractions {
            let (an, ad) = a.to_bigint_pair();
            let (bn, bd) = b.to_bigint_pair();
            let sum = Fraction::new(&an * &bd + &bn * &ad, &ad * &bd);
            let diff = Fraction::new(&an * &bd - &bn * &ad, &ad * &bd);
            assert_eq!(a.add(b), sum, "{a:?} + {b:?}");
            assert_eq!(a.sub(b), diff, "{a:?} - {b:?}");
        }
    }
}

// ── what an irrational operand costs beyond a product ────────────────────

/// `2 SQRT 3 SQRT ADD 5 SQRT ADD …`: the sum of the square roots of the
/// first `k` primes, bound to `X`.
fn radical_sum(k: usize) -> String {
    let primes = [2, 3, 5, 7, 11, 13, 17, 19];
    let mut source = format!("{} SQRT", primes[0]);
    for p in &primes[1..k] {
        source.push_str(&format!(" {p} SQRT ADD"));
    }
    source + " 'X' BIND"
}

/// Dividing by a sum of `k` radicals inverts it by conjugation: `k` levels,
/// each squaring a term map that may hold up to 2^(k-1) terms, so each
/// radical in the divisor multiplies the work by about four. It was priced
/// as the divisor's term count squared, which grows by a few percent per
/// radical, so a twelve-radical divisor ran for forty seconds on a fraction
/// of the budget that was meant to stop it.
#[tokio::test]
async fn dividing_by_an_irrational_is_charged_for_its_conjugation() {
    let mut charges = Vec::new();
    for k in [5, 6, 7] {
        let sum = radical_sum(k);
        let built = charged_by(&format!("{sum} 1")).await;
        charges.push(charged_by(&format!("{sum} 1 X DIV")).await - built);
    }
    for pair in charges.windows(2) {
        assert!(
            pair[1] >= 2 * pair[0],
            "one more radical in the divisor must cost a multiple, got {charges:?}"
        );
    }
}

/// An order comparison on irrational lanes is a subtraction over a common
/// refinement of the two bases, then the sign of what it leaves. It was
/// charged nothing, so `[ 20000 ] X FILL Y LT` ran for a minute on the work
/// it took to build `X` and `Y`. A rational comparison stays free.
#[tokio::test]
async fn an_irrational_comparison_is_charged_as_the_subtraction_it_is() {
    let operands = "2 SQRT 3 SQRT ADD 'X' BIND 5 SQRT 7 SQRT ADD 'Y' BIND";
    let built = charged_by(&format!("{operands} 1")).await;
    for word in ["LT", "GT", "MIN", "MAX", "EQ"] {
        let one = charged_by(&format!("{operands} X Y {word}")).await - built;
        let lanes = charged_by(&format!("{operands} [ 8 ] X FILL Y {word}")).await - built;
        assert!(one > 0, "`X Y {word}` reached the work meter at zero");
        assert!(
            lanes >= 8 * one,
            "`{word}` over 8 lanes: {lanes} < 8 × {one}"
        );
    }
    for source in [
        "1 2 LT",
        "[ 1 2 ] 2 GT",
        "1/2 1/3 MIN",
        "[ 1 2 ] [ 1 2 ] EQ",
    ] {
        assert_eq!(charged_by(source).await, 0, "`{source}`");
    }
}
