//! The collection meter prices what a Word does to the elements, not how many
//! of them there are.
//!
//! Element count was the obvious answer and the measurements ruled it out
//! (`examples/collection_word_calibration`, summarized in
//! `docs/dev/mcp-host-profiles.md`): the same `UNIQUE` over the
//! same 16,000 elements costs 0.52 ms or 682 ms depending only on how many
//! distinct values the data holds, an element that is itself a vector is not one
//! unit of work, and an algebraic element costs five hundred times what a
//! machine word does to compare. So the tests here move one axis at a time —
//! count, distinctness, element width, element shape, element domain — and
//! require the charge to move with the axis that actually costs something.
//!
//! The shape-invariance cases are the collection family's version of what
//! `arithmetic_meter_tests` established for arithmetic: a safety control whose
//! price turns on an unobservable representation decision is not a control.

use crate::error::{AjisaiError, ResourceLimit};
use crate::interpreter::runtime_limits::RuntimeLimits;
use crate::interpreter::Interpreter;

/// Collection work charged by a source that runs successfully under limits
/// loose enough not to interfere.
async fn charged_by(source: &str) -> u64 {
    let mut interp = Interpreter::new();
    interp
        .execute(source)
        .await
        .unwrap_or_else(|e| panic!("`{source}` must compute, got: {e:?}"));
    interp.collection_work_used()
}

/// Collection work charged by `word` alone, with `setup` left on the stack
/// beforehand.
///
/// Building the operand is itself charged — materializing 1,000 elements
/// costs what materializing them costs — so a comparison between two Words
/// has to exclude it, or it measures the `RANGE` they share. `execute`
/// keeps the stack across calls and resets the counters, which is exactly
/// the split needed.
async fn charged_by_word(setup: &str, word: &str) -> u64 {
    let mut interp = Interpreter::new();
    interp
        .execute(setup)
        .await
        .unwrap_or_else(|e| panic!("setup `{setup}` must compute, got: {e:?}"));
    interp
        .execute(word)
        .await
        .unwrap_or_else(|e| panic!("`{word}` must compute, got: {e:?}"));
    interp.collection_work_used()
}

/// The resource that refused `source` under `max_collection_work`, or
/// `None` if it succeeded.
async fn refused_under(source: &str, budget: u64) -> Option<ResourceLimit> {
    let mut interp = Interpreter::new();
    interp.set_runtime_limits(RuntimeLimits {
        max_collection_work: budget,
        ..RuntimeLimits::default()
    });
    match interp.execute(source).await {
        Ok(()) => None,
        Err(AjisaiError::ResourceLimitExceeded { resource, .. }) => Some(resource),
        Err(other) => panic!("`{source}` must succeed or hit a ceiling, got: {other:?}"),
    }
}

// ── the axis that element count cannot see ─────────────────────────────

#[tokio::test]
async fn a_scan_is_priced_by_the_element_count_not_the_distinct_count() {
    // 4,000 elements either way — one distinct value on one side, 4,000 on the
    // other. Before the de-quadraticization follow-up this was the opposite
    // assertion: `UNIQUE` scanned every distinct value found so far, so the
    // same element count charged 100x more when nothing repeated. `Value: Hash`
    // turned that scan into one hash-and-lookup per element, so the charge now
    // tracks element count; distinctness only adds the small, linear cost of
    // copying every new value into the result.
    let uniform = charged_by("0 3999 RANGE [ 0 MUL ] MAP UNIQUE").await;
    let distinct = charged_by("0 3999 RANGE UNIQUE").await;
    assert!(
        distinct < uniform * 3,
        "distinctness must no longer dominate the price the way the O(n×d) \
         linear scan made it: {distinct} distinct against {uniform} uniform"
    );
}

#[tokio::test]
async fn tally_and_group_are_priced_like_the_scan_they_share() {
    // `UNIQUE`, `TALLY` and `GROUP` run one scan between them, so a program
    // cannot get the quadratic for free by asking for it under a different
    // name.
    let unique = charged_by("0 999 RANGE UNIQUE").await;
    let tally = charged_by("0 999 RANGE TALLY").await;
    assert_eq!(unique, tally, "one scan, however it is spelled");

    let group = charged_by("0 999 RANGE 0 999 RANGE GROUP").await;
    assert!(
        group > unique,
        "`GROUP` scans the keys and copies the values, so it cannot cost \
         less than `UNIQUE` over the same keys: {group} against {unique}"
    );
}

#[tokio::test]
async fn an_element_that_is_a_vector_costs_what_its_leaves_cost() {
    // Equality on a nested element is a loop over that element, so "one
    // element" is only a unit of work when the elements are scalars.
    let flat = charged_by("0 199 RANGE UNIQUE").await;
    let nested = charged_by("0 199 RANGE [ 1 16 RANGE ADD ] MAP UNIQUE").await;
    assert!(
        nested > flat * 8,
        "sixteen leaves per element must cost more than one: {nested} \
         against {flat}"
    );
}

#[tokio::test]
async fn an_algebraic_element_costs_more_than_a_machine_word() {
    // Deciding `√2 == √3` rebases two radical bases; deciding `2 == 3`
    // compares two machine words. `ALGEBRAIC_ELEMENT_UNITS` prices one
    // such decision at 512 against 1, and the de-quadraticization
    // follow-up reuses it for the cost of finding an algebraic element's
    // hash bucket (`Algebraic::hash` runs the same kind of interval
    // refinement `cmp` does) — so algebraic still costs far more per
    // element, just not the >100x the old quadratic scan measured, since
    // the per-element copy cost (shared by both sides, dominated by the
    // width-independent `COLLECTION_COPY_UNITS`) no longer gets diluted
    // by an O(n) amplification that hit both sides equally.
    let rational = charged_by_word("2 121 RANGE", "UNIQUE").await;
    let algebraic = charged_by_word("2 121 RANGE [ SQRT ] MAP", "UNIQUE").await;
    assert!(
        algebraic > rational * 30,
        "an algebraic element must be priced as one: {algebraic} against \
         {rational}"
    );
}

#[tokio::test]
async fn a_wide_element_costs_more_than_a_narrow_one() {
    // Same element count and same distinct count on both sides; only the
    // width of an element moves. Multiplying by the wide literal is what
    // makes the elements genuine BigInts rather than machine words — the
    // step where the measured cost jumps twelvefold.
    let narrow = charged_by_word("1 200 RANGE [ 1/101 MUL FLOOR 1 ADD ] MAP", "UNIQUE").await;
    let wide = charged_by_word(
        &format!(
            "1 200 RANGE [ 1/101 MUL FLOOR 1 ADD {} MUL ] MAP",
            "9".repeat(512)
        ),
        "UNIQUE",
    )
    .await;
    assert!(
        wide > narrow,
        "an equality test walks limbs, so width has to be in the price: \
         {wide} against {narrow}"
    );
}

// ── shape invariance ───────────────────────────────────────────────────

#[tokio::test]
async fn the_price_does_not_turn_on_how_the_vector_is_stored() {
    // Whether a vector of small integers is held as boxed children or as a
    // dense tensor is an optimization decision, unobservable from the
    // language (LANG.AUTHORITY.FREEDOM). A ceiling that fired for one
    // representation and not the other would make it observable — the same
    // hole the arithmetic meter was closed for.
    let literal = charged_by_word("[ 1 2 3 4 5 6 7 8 ]", "REVERSE").await;
    let generated = charged_by_word("1 8 RANGE", "REVERSE").await;
    assert_eq!(
        literal, generated,
        "eight elements is eight elements: {literal} against {generated}"
    );
}

#[tokio::test]
async fn a_word_that_reads_a_count_is_not_charged_for_the_vector() {
    // `LENGTH` answers from the header. It used to deep-copy the whole
    // element vector to call `.len()` on the copy — 18 ms at 100,000
    // elements — which is the reason this test exists as well as the reason
    // the charge is zero.
    assert_eq!(
        charged_by_word("1 1000 RANGE", "LENGTH").await,
        0,
        "an O(1) Word must add nothing to the meter"
    );
}

#[tokio::test]
async fn a_selection_is_priced_on_what_it_selects() {
    // `GET` and `TAKE` copy a part. Pricing them by the operand's length
    // would charge for elements they never touch, which is the difference
    // between a ceiling and a tax on holding a large vector.
    let whole = charged_by_word("1 1000 RANGE", "REVERSE").await;
    let part = charged_by_word("1 1000 RANGE", "3 TAKE").await;
    assert!(
        part * 10 < whole,
        "taking three of a thousand must cost far less than touching all \
         thousand: {part} against {whole}"
    );
}

// ── the ceiling ────────────────────────────────────────────────────────

#[tokio::test]
async fn the_quadratic_scan_is_refused_by_name() {
    assert_eq!(
        refused_under("0 4999 RANGE UNIQUE", 100_000).await,
        Some(ResourceLimit::CollectionWork),
        "a scan past the budget must name the budget it passed"
    );
}

#[tokio::test]
async fn a_refusal_leaves_the_stack_as_the_program_left_it() {
    // The scan family is the one place either meter charges inside its
    // loop, so it is the one place a refusal can happen with the operand
    // already taken. It has to go back.
    // The budget has to admit the `RANGE` — building 5,800 elements costs
    // 98,600 units — and refuse the scan on its own: 5,800 all-distinct
    // elements is 104,400 units, over budget in a fresh `execute` call
    // (the counter resets between calls, so the two charges are not
    // cumulative here — see `charged_by_word`).
    let mut interp = Interpreter::new();
    interp.set_runtime_limits(RuntimeLimits {
        max_collection_work: 100_000,
        ..RuntimeLimits::default()
    });
    interp
        .execute("0 5799 RANGE")
        .await
        .expect("building the vector is inside the budget for this test");
    let before = interp.get_stack().len();

    let err = interp
        .execute("UNIQUE")
        .await
        .expect_err("the scan must be refused");
    assert!(matches!(
        err,
        AjisaiError::ResourceLimitExceeded {
            resource: ResourceLimit::CollectionWork,
            ..
        }
    ));
    assert_eq!(
        interp.get_stack().len(),
        before,
        "a refused Word must leave the operand where the program put it"
    );
}

#[tokio::test]
async fn a_scan_refusal_says_how_far_it_got_and_that_size_fits() {
    // The property the whole `progress` field exists for. `observed` on a
    // meter charged as it goes is always a hair over `limit` — it cannot
    // say how much smaller the input has to be, and a real model reading it
    // proportionally retried 100,000 elements as 99,999 and failed again.
    // `completed` is the answer rather than a hint at it: the budget bought
    // exactly that many elements, so re-running with that many must succeed.
    let budget = 250_000;
    let mut interp = Interpreter::new();
    interp.set_runtime_limits(RuntimeLimits {
        max_collection_work: budget,
        ..RuntimeLimits::default()
    });
    let err = interp
        .execute("0 9999 RANGE UNIQUE")
        .await
        .expect_err("ten thousand distinct values is past this budget");
    let AjisaiError::ResourceLimitExceeded {
        observed, progress, ..
    } = err
    else {
        panic!("expected a resource-limit refusal, got {err:?}");
    };
    let progress = progress.expect("an incrementally charged refusal reports its progress");
    assert_eq!(progress.total, 10_000);
    assert_eq!(progress.unit, "elements");
    assert!(
        progress.completed > 0 && progress.completed < progress.total,
        "the scan stopped part way, at {} of {}",
        progress.completed,
        progress.total
    );
    // The defect this replaced, stated as an assertion: `observed` alone
    // looks like a rounding error however far over the request was.
    let overshoot = observed.expect("a work ceiling reports what it spent") - budget;
    assert!(
        overshoot * 100 < budget,
        "observed overshoots by under 1% of the budget, which is exactly \
         why it cannot be read proportionally: {overshoot}"
    );

    // And the advertised size really does fit.
    let mut retry = Interpreter::new();
    retry.set_runtime_limits(RuntimeLimits {
        max_collection_work: budget,
        ..RuntimeLimits::default()
    });
    retry
        .execute(&format!("0 {} RANGE UNIQUE", progress.completed - 1))
        .await
        .unwrap_or_else(|e| {
            panic!(
                "retrying with the {} elements the refusal advertised must succeed, got {e:?}",
                progress.completed
            )
        });
}

#[tokio::test]
async fn a_pre_charged_refusal_reports_no_progress() {
    // The other half: a copy or a sort is charged in full before it runs,
    // so its `observed` already includes the whole operation's cost and
    // says how far over the request was on its own. Inventing a progress
    // figure there would be reporting a measurement nothing took.
    let mut interp = Interpreter::new();
    interp.set_runtime_limits(RuntimeLimits {
        max_collection_work: 1_000,
        ..RuntimeLimits::default()
    });
    let err = interp
        .execute("0 9999 RANGE")
        .await
        .expect_err("materializing ten thousand elements is past this budget");
    let AjisaiError::ResourceLimitExceeded { progress, .. } = err else {
        panic!("expected a resource-limit refusal, got {err:?}");
    };
    assert!(progress.is_none(), "a pre-charge has no partial progress");
}

#[tokio::test]
async fn the_budget_admits_what_it_declares() {
    // The other half of a live ceiling. A scan whose charge is inside the
    // budget has to run, or the number is not the number it says it is.
    assert_eq!(
        refused_under("0 99 RANGE UNIQUE", 1_000_000).await,
        None,
        "a hundred distinct elements is 5,000 probes and must be allowed"
    );
}

#[tokio::test]
async fn every_charging_word_charges_something() {
    // A Word that loops over its operand and charges nothing is the hole
    // this meter was added to close, and the way it comes back is a new
    // Word landing in the family without a call to the meter.
    for source in [
        "1 32 RANGE",
        "1 32 RANGE REVERSE",
        "1 32 RANGE 4 TAKE",
        "1 32 RANGE [ 1 2 ] CONCAT",
        "1 32 RANGE 0 7 PUT",
        "1 32 RANGE [ 0 1 ] GET",
        "1 32 RANGE SORT",
        "1 32 RANGE ORDER",
        "1 32 RANGE UNIQUE",
        "1 32 RANGE TALLY",
        "1 32 RANGE 1 32 RANGE GROUP",
        "1 32 RANGE 1 32 RANGE 2 COLLECT ZIP",
        "1 32 RANGE -1 BSEARCH",
        "[ 4 4 ] 0 FILL",
    ] {
        assert!(
            charged_by(source).await > 0,
            "`{source}` touches elements and charged nothing"
        );
    }
}

// ── SORT's dense route is priced as SORT's comparison route ─────────────
//
// `SORT` over a flat pure-integer dense buffer sorts its numerator column
// instead of materializing a boxed `Value` per lane and ordering a
// permutation through the exact comparison. That is a representation
// decision, and this module's subject is a price that must not turn on one.
//
// Measured as a *delta*, not as a total: the two programs that put the same
// integers on the stack in different representations do so through different
// prefixes, and a total would be comparing those prefixes as much as the
// sort. `CONCAT` with a boxed operand (the empty literal) builds with the
// non-promoting constructor, so it leaves a nested `Vector` where `REVERSE` of
// a range leaves a dense `Tensor`.

/// Collection work charged by `source`, which must compute.
async fn collection_work_of(source: &str) -> u64 {
    let mut interp = Interpreter::new();
    let mut limits = *interp.runtime_limits();
    limits.max_materialized_elements = 10_000_000;
    limits.max_collection_work = u64::MAX;
    interp.set_runtime_limits(limits);
    interp
        .execute(source)
        .await
        .unwrap_or_else(|e| panic!("`{source}` must compute, got: {e:?}"));
    interp.collection_work_used()
}

/// What appending `word` to `prefix` costs on its own.
async fn charge_of_appending(word: &str, prefix: &str) -> u64 {
    let without = collection_work_of(prefix).await;
    let with = collection_work_of(&format!("{prefix} {word}")).await;
    with - without
}

/// The same integers, as a flat dense `Tensor` and as a nested `Vector`.
/// `REVERSE` of a range leaves the former; `CONCAT` with the empty literal
/// does not promote, so it leaves the latter. `modulo` sets how many distinct values the data holds,
/// which is the axis a hash-keyed scan's price actually turns on.
fn dense_and_nested(n: usize, modulo: Option<usize>) -> (String, String) {
    let half = n / 2;
    // `[ m n DIV MUL FLOOR ] MAP` sends 0..n-1 onto exactly 0..m-1.
    let fold = modulo.map_or(String::new(), |m| format!(" [ {m} {n} DIV MUL FLOOR ] MAP"));
    (
        format!("0 {} RANGE{fold} REVERSE", n - 1),
        format!(
            "0 {} RANGE{fold} {half} {} RANGE{fold} CONCAT [ ] CONCAT",
            half - 1,
            n - 1
        ),
    )
}

#[tokio::test]
async fn sorting_costs_the_same_whichever_representation_holds_the_elements() {
    for n in [8usize, 100, 1000] {
        let half = n / 2;
        // Dense: a range reversed is a flat `Tensor`.
        let dense = format!("0 {} RANGE REVERSE", n - 1);
        // Nested: `CONCAT` with a boxed operand does not promote, so this
        // stays a `Vector`.
        let nested = format!(
            "0 {} RANGE {half} {} RANGE CONCAT [ ] CONCAT",
            half - 1,
            n - 1
        );
        assert_eq!(
            charge_of_appending("SORT", &dense).await,
            charge_of_appending("SORT", &nested).await,
            "sorting {n} integers must cost the same held densely as held nested"
        );
    }
}

/// And the price still moves with the axis that costs something: a sort is
/// `n⌈log₂n⌉` comparisons, so ten times the elements costs more than ten
/// times as much.
#[tokio::test]
async fn sorting_more_elements_costs_more_than_linearly() {
    let small = charge_of_appending("SORT", "0 99 RANGE REVERSE").await;
    let large = charge_of_appending("SORT", "0 999 RANGE REVERSE").await;
    assert!(
        large > small.saturating_mul(10),
        "1000 elements charged {large} against 100 elements' {small},              which is at most linear — the log factor is missing"
    );
}

/// `UNIQUE` and `TALLY` share one hash-keyed scan, and it too has a dense
/// route now — keying the `i64` column rather than a boxed `Value` per lane.
/// Unlike the sort's, this scan charges *as it goes*, and how often it
/// charges for retaining an element depends on how many distinct values the
/// data holds. So the parity is checked across that axis as well as across
/// size: `[ m n DIV MUL FLOOR ] MAP` sets the distinct count to `m`.
#[tokio::test]
async fn a_hash_keyed_scan_costs_the_same_whichever_representation_holds_it() {
    for (n, modulo) in [(8usize, 2usize), (100, 4), (1000, 7), (1000, 1000)] {
        let (dense, nested) = dense_and_nested(n, Some(modulo));
        for word in ["UNIQUE", "TALLY"] {
            assert_eq!(
                charge_of_appending(word, &dense).await,
                charge_of_appending(word, &nested).await,
                "`{word}` over {n} integers with {modulo} distinct values must \
                 cost the same held densely as held nested"
            );
        }
    }
}
