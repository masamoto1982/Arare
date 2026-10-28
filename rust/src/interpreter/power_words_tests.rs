//! Behavioral probes for the number-closing Words `POW` `GCD` `RATIO` — the
//! tier each answer lands in, the projections the contracts declare, and the
//! lifting every arithmetic Word shares (LANG.VALUES.EXACT,
//! LANG.COLLECTIONS.LIFT).

use crate::error::{AjisaiError, ResourceLimit};
use crate::interpreter::runtime_limits::RuntimeLimits;
use crate::test_support::{charged_by, error_of, top, top_nil_reason, with_limits};

#[tokio::test]
async fn pow_stays_exact_where_the_field_holds_the_answer() {
    assert_eq!(top("2 10 POW").await, "1024/1");
    assert_eq!(top("2 -2 POW").await, "1/4");
    assert_eq!(top("-3 3 POW").await, "-27/1");
    assert_eq!(top("0 0 POW").await, "1/1");
    assert_eq!(top("4 1/2 POW").await, "2/1");
    assert_eq!(top("9/4 -3/2 POW").await, "8/27");
    assert_eq!(top("0 1/2 POW").await, "0/1");
    assert_eq!(top("2 1/2 POW 2 SQRT EQ").await, "TRUE");
    assert_eq!(top("2 3/2 POW 2 SQRT 2 MUL EQ").await, "TRUE");
    assert_eq!(top("2 SQRT 2 POW").await, "2/1");
    assert_eq!(top("2 SQRT -2 POW").await, "1/2");
    assert_eq!(top("2 SQRT 3 POW 2 SQRT 2 MUL EQ").await, "TRUE");
    assert_eq!(
        top("2 SQRT 3 SQRT ADD -1 POW 3 SQRT 2 SQRT SUB EQ").await,
        "TRUE"
    );
    assert_eq!(top("[ 1 2 3 ] 2 POW").await, "[ 1/1 4/1 9/1 ]");
    assert_eq!(top("2 [ 1 2 3 ] POW").await, "[ 2/1 4/1 8/1 ]");
}

/// Every other exponent leaves the field, and POW says so rather than
/// answer with something no comparison could decide: a root other than a
/// square root, a square root of an irrational base, an irrational
/// exponent — even where the answer happens to be rational (`8 1/3 POW`).
#[tokio::test]
async fn pow_projects_an_exponent_outside_the_field() {
    for code in [
        "8 1/3 POW",
        "2 1/3 POW",
        "27/8 -2/3 POW",
        "2 SQRT 1/2 POW",
        "2 2 SQRT POW",
        "1 2 SQRT POW",
        "0 2 SQRT POW",
    ] {
        assert_eq!(
            top(&format!("{code} NIL-REASON")).await,
            "'domainMiss'",
            "`{code}`"
        );
    }
    assert_eq!(
        top("[ 4 8 ] 1/3 POW 1 GET NIL-REASON").await,
        "'domainMiss'"
    );
}

#[tokio::test]
async fn pow_projects_what_has_no_value() {
    // Division is total, so a zero base under a negative exponent is the
    // reciprocal of zero: `1/0`.
    assert_eq!(top("0 -1 POW").await, "1/0");
    assert_eq!(top("0 -1/2 POW").await, "1/0");
    assert_eq!(top("1/0 -1 POW").await, "0/1");
    assert_eq!(top("1/0 2 POW").await, "1/0");
    assert_eq!(top("-1/0 2 POW").await, "1/0");
    // `0/0` absorbs every operation (LANG.VALUES.EXACT), POW included, in
    // either operand — the exponent 0 and the exponent `0/0` too, where the
    // empty product and the domain miss would otherwise answer.
    assert_eq!(top("0/0 3 POW").await, "0/0");
    assert_eq!(top("0/0 1/2 POW").await, "0/0");
    assert_eq!(top("0/0 0 POW").await, "0/0");
    assert_eq!(top("0/0 -1 POW").await, "0/0");
    assert_eq!(top("2 0/0 POW").await, "0/0");
    assert_eq!(top("0/0 0/0 POW").await, "0/0");
    assert_eq!(top("2 SQRT 0/0 POW").await, "0/0");
    // The empty product is 1 for every other base, the points over zero
    // included: nothing is multiplied, so no pair over zero reaches it.
    assert_eq!(top("1/0 0 POW").await, "1/1");
    assert_eq!(top("-1/0 0 POW").await, "1/1");
    assert_eq!(top("2 1/0 POW NIL-REASON").await, "'domainMiss'");
    assert_eq!(top("2 -1/0 POW NIL-REASON").await, "'domainMiss'");
    assert_eq!(top("-8 1/3 POW NIL-REASON").await, "'domainMiss'");
    assert_eq!(top("-2 1/2 POW NIL-REASON").await, "'domainMiss'");
    assert_eq!(top("-2 SQRT 3/2 POW NIL-REASON").await, "'domainMiss'");
    assert_eq!(error_of("2 1000000000 POW").await, "resourceLimitExceeded");
    assert_eq!(top("NIL 2 POW").await, "NIL");
    assert_eq!(error_of("'x' 2 POW").await, "nonNumeric");
    assert_eq!(error_of("[ 1 2 ] [ 1 2 3 ] POW").await, "shapeMismatch");
    assert_eq!(
        top("2 'A' BIND 3 'B' BIND A B A B POW").await,
        "2/1 3/1 8/1"
    );
}

#[tokio::test]
async fn gcd_and_ratio_read_the_rationals() {
    assert_eq!(top("12 18 GCD").await, "6/1");
    assert_eq!(top("-12 18 GCD").await, "6/1");
    assert_eq!(top("0 0 GCD").await, "0/1");
    assert_eq!(top("7 0 GCD").await, "7/1");
    assert_eq!(top("[ 12 9 ] 6 GCD").await, "[ 6/1 3/1 ]");
    assert_eq!(top("1/2 4 GCD NIL-REASON").await, "'domainMiss'");
    assert_eq!(top("2 SQRT 4 GCD NIL-REASON").await, "'domainMiss'");
    assert_eq!(error_of("'a' 4 GCD").await, "nonNumeric");
    assert_eq!(top("6/4 RATIO").await, "[ 3/1 2/1 ]");
    assert_eq!(top("-3 RATIO").await, "[ -3/1 1/1 ]");
    assert_eq!(top("0 RATIO").await, "[ 0/1 1/1 ]");
    assert_eq!(
        top("[ 1/2 3/4 ] RATIO").await,
        "[ [ 1/1 2/1 ] [ 3/1 4/1 ] ]"
    );
    assert_eq!(top("2 SQRT RATIO NIL-REASON").await, "'domainMiss'");
    assert_eq!(error_of("'a' RATIO").await, "nonNumeric");
    // RATIO then DIV is the identity on a rational.
    assert_eq!(
        top("6/4 RATIO 0 GET 6/4 RATIO 1 GET DIV 3/2 EQ").await,
        "TRUE"
    );
}

#[tokio::test]
async fn the_numeric_words_lift_over_records() {
    assert_eq!(
        top("[ 'a' 'b' ] [ 2 3 ] RECORD 2 POW").await,
        "[ 'a' 'b' ] [ 4/1 9/1 ] RECORD"
    );
    assert_eq!(
        top("[ 'a' 'b' ] [ 12 9 ] RECORD 6 GCD").await,
        "[ 'a' 'b' ] [ 6/1 3/1 ] RECORD"
    );
    assert_eq!(
        top("[ 'a' ] [ 1/2 ] RECORD RATIO").await,
        "[ 'a' ] [ [ 1/1 2/1 ] ] RECORD"
    );
    assert_eq!(
        top("[ 'a' ] [ 4 ] RECORD 1/2 POW").await,
        "[ 'a' ] [ 2/1 ] RECORD"
    );
}

// ── POW and GCD are on the work meter and under the size ceilings ──────

/// The product of the primes below 200, a 272-bit square-free integer: its
/// root is one term with coefficient 1, so only the radicand says how wide
/// the root's powers are.
const PRIMORIAL_200: &str =
    "7799922041683461553249199106329813876687996789903550945093032474868511536164700810";

/// What `source` leaves on top under `limits`: `Ok(Some(reason))` for a
/// reasoned NIL, `Ok(None)` for any other value, or the error it raised.
async fn under(limits: RuntimeLimits, source: &str) -> Result<Option<String>, AjisaiError> {
    let mut interp = with_limits(limits);
    interp.execute(source).await?;
    Ok(top_nil_reason(&interp).map(|reason| reason.as_protocol_str().to_string()))
}

/// The ceiling `source` is refused under, under `limits`; `None` when it
/// answers.
async fn refused_by(limits: RuntimeLimits, source: &str) -> Option<ResourceLimit> {
    match with_limits(limits).execute(source).await {
        Ok(()) => None,
        Err(AjisaiError::ResourceLimitExceeded { resource, .. }) => Some(resource),
        Err(e) => panic!("`{source}` must answer or be refused by name, got {e}"),
    }
}

fn bits(max_bigint_bits: u64) -> RuntimeLimits {
    RuntimeLimits {
        max_bigint_bits,
        ..RuntimeLimits::default()
    }
}

fn terms(max_algebraic_terms: usize) -> RuntimeLimits {
    RuntimeLimits {
        max_algebraic_terms,
        ..RuntimeLimits::default()
    }
}

/// `POW` is repeated multiplication (LANG.VALUES.EXACT), and its contract
/// prices it as the products it is. It was charged nothing, so a power was
/// the one way to build a wide number the work meter never saw.
#[tokio::test]
async fn pow_is_charged_as_the_products_it_performs() {
    assert!(charged_by("3 2 POW").await > 0, "a power is work");
    assert!(
        charged_by("3 20000 POW").await > charged_by("3 2000 POW").await,
        "a wider power costs more"
    );
    assert!(
        charged_by("[ 3 3 3 3 ] 20000 POW").await >= 4 * charged_by("3 20000 POW").await,
        "every lane is a power of its own"
    );
    assert!(
        charged_by("2 SQRT 3 SQRT ADD 5 SQRT ADD 40 POW").await
            > charged_by("2 SQRT 3 SQRT ADD 5 SQRT ADD 2 POW").await,
        "an algebraic power is charged for its term products"
    );
    let err = under(
        RuntimeLimits {
            max_numeric_work: 1_000,
            ..RuntimeLimits::default()
        },
        "3 100000 POW",
    )
    .await
    .expect_err("a power past the work budget is refused");
    assert!(
        matches!(
            err,
            AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::NumericWork,
                ..
            }
        ),
        "{err:?}"
    );
}

/// A power past `bigintBits` or `algebraicTerms` is refused before it is
/// computed, by the ceiling's own name — the refusal `MUL` makes for the
/// same width after computing it, made first. It used to be computed whole,
/// and only the operation after it, if any, met the ceiling; and through
/// engine 1.0.0-beta.1 it projected a NIL where every other ceiling raised.
#[tokio::test]
async fn a_power_past_the_size_ceilings_is_refused_before_it_is_computed() {
    assert_eq!(under(bits(1_000), "2 900 POW").await.unwrap(), None);
    assert_eq!(
        refused_by(bits(1_000), "2 1100 POW").await,
        Some(ResourceLimit::BigintBits)
    );
    assert_eq!(
        refused_by(bits(1_000), "1/3 -700 POW").await,
        Some(ResourceLimit::BigintBits)
    );
    // √P has coefficient 1; its tenth power is P⁵, 1,360 bits wide.
    assert_eq!(
        refused_by(bits(1_000), &format!("{PRIMORIAL_200} SQRT 10 POW")).await,
        Some(ResourceLimit::BigintBits)
    );
    assert_eq!(
        under(bits(1_000), &format!("{PRIMORIAL_200} SQRT 6 POW"))
            .await
            .unwrap(),
        None
    );
    // A sum of four radicals powers into the 8 monomials an even product of
    // them can reach; a sum of two only ever into 2, either way up.
    let four = "2 SQRT 3 SQRT ADD 5 SQRT ADD 7 SQRT ADD";
    assert_eq!(
        refused_by(terms(4), &format!("{four} 6 POW")).await,
        Some(ResourceLimit::AlgebraicTerms)
    );
    assert_eq!(
        under(terms(8), &format!("{four} 6 POW")).await.unwrap(),
        None
    );
    assert_eq!(
        under(terms(2), "2 SQRT 3 SQRT ADD 40 POW").await.unwrap(),
        None
    );
    assert_eq!(
        under(terms(2), "2 SQRT 3 SQRT ADD -40 POW").await.unwrap(),
        None
    );
}

/// The operands of a refused power stay on the stack, as every refusal
/// leaves them, and the ceiling's name, value and the observed width are
/// reported, so an agent can tell the power to shrink from the host to
/// change.
#[tokio::test]
async fn a_refused_power_names_its_ceiling_and_keeps_its_operands() {
    let mut interp = with_limits(bits(1_000));
    let err = interp
        .execute("[ 1 2 ] 1100 POW")
        .await
        .expect_err("a lane past bigintBits refuses the whole lift");
    assert!(
        matches!(
            err,
            AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::BigintBits,
                limit: 1_000,
                observed: Some(1_101),
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(interp.get_stack().len(), 2, "both operands put back");
}

/// `GCD` is Euclid on the operands' limbs, priced at least as `ADD` is on
/// the same pair; it was charged nothing.
#[tokio::test]
async fn gcd_is_charged_like_the_arithmetic_beside_it() {
    let a = format!("{}1", "9".repeat(600));
    let c = format!("{}3", "7".repeat(600));
    let gcd = charged_by(&format!("{a} {c} GCD")).await;
    assert!(gcd > 0, "a gcd is work");
    assert!(gcd >= charged_by(&format!("{a} {c} ADD")).await);
    assert!(charged_by(&format!("[ {a} {a} {a} ] {c} GCD")).await >= 3 * gcd);
}
