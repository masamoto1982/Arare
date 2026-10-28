//! A declared ceiling that another ceiling always pre-empts is a claim, not a
//! control.
//!
//! The three internal-cost ceilings are not independent dials. Building a large
//! *exact* value requires work — not as an implementation artifact but as what
//! exactness means, since every digit and every term was actually computed — so
//! each size ceiling has a minimum work cost to reach it:
//!
//! ```text
//! work_to_reach(size ceiling) < max_numeric_work
//! ```
//!
//! A profile that violates this declares a ceiling nothing can ever hit. Its
//! `mcp.limits` entry still refuses the program, but under the wrong name, and
//! the name is what an agent repairs from: "you exceeded the work budget" and
//! "stop multiplying surds like that" are different instructions.
//!
//! `max_algebraic_terms` was 4,096 under the MCP profile and violated it. The
//! doubling that would first exceed 4,096 terms charges 16,780,400 units
//! against a 10,000,000 budget, so `numericWork` answered every time and the
//! term ceiling had never fired in its life. That went unnoticed through
//! three rounds of external review because nothing checked it. This is what
//! checks it.
//!
//! Until 2026-08-14 this module checked one profile: `LOCAL_AGENT_RUNTIME_LIMITS`
//! (MCP / `ajisai agent`). The playground / `ajisai run` default was silently
//! assumed to be fine because it was strictly more generous — but "more
//! generous" is not "live": the same day's re-derivation of
//! `DEFAULT_MAX_NUMERIC_WORK` (`docs/dev/host-profile-derivation-handoff.md`
//! §4) shrank the playground's work budget enough that its own
//! `max_algebraic_terms` (100,000 at the time) stopped being reachable —
//! exactly the failure mode this module exists to catch, just on the other
//! profile. Every liveness property below is now asserted for **both**
//! profiles, with each profile supplying its own witness sizes (the two
//! profiles' ceilings differ by construction, so the source that crosses one
//! does not cross the other).

use crate::agent::api::{compute, ComputeOptions, LOCAL_AGENT_RUNTIME_LIMITS};
use crate::interpreter::RuntimeLimits;

/// One profile under test, plus the witness sizes that exercise it. The
/// witnesses are not portable between profiles: each is the smallest size
/// this session measured that crosses (or stays under) *this* profile's
/// own declared ceiling, per `docs/dev/host-profile-derivation-handoff.md`
/// §4 and the re-measurement it required.
struct Profile {
    name: &'static str,
    limits: RuntimeLimits,
    /// `None` uses `Interpreter::new()`'s own default, which is correct
    /// for the playground/native profile (that default *is* the profile
    /// after the 2026-08-14 re-derivation) but not for MCP, which pins
    /// its own `executionSteps` explicitly at every real call site
    /// (`tools/mcp-server/index.js` `LIMITS.executionSteps`) independent
    /// of whatever the interpreter default happens to be.
    step_limit: Option<usize>,
    /// Cascade factor count whose term count first exceeds
    /// `max_algebraic_terms`.
    algebraic_terms_over: usize,
    /// Cascade factor count whose term count stays at or under
    /// `max_algebraic_terms`.
    algebraic_terms_under: usize,
    /// Widening-chain multiplication count whose bit width first exceeds
    /// `max_bigint_bits`.
    bigint_bits_over: usize,
    /// Widening-chain multiplication count whose bit width stays under
    /// `max_bigint_bits`.
    bigint_bits_under: usize,
    /// Cascade factor count used as the repeated unit for the cumulative
    /// `numericWork` witness — must stay comfortably under
    /// `algebraic_terms_under` so no single repeat trips the term
    /// ceiling.
    cumulative_cascade_factors: usize,
    /// Repeat count of `cumulative_cascade_factors` that first crosses
    /// `max_numeric_work`.
    cumulative_cascade_reps: usize,
    /// `RANGE` upper bound (element count minus one) for an all-distinct
    /// vector at the `materializedElements` ceiling.
    collection_vector_max: usize,
    /// Repeated-`UNIQUE` pass count over `collection_vector_max` that
    /// first crosses `max_collection_work`.
    collection_reps_to_cross: usize,
}

fn mcp_profile() -> Profile {
    Profile {
        name: "mcp",
        limits: LOCAL_AGENT_RUNTIME_LIMITS,
        // Every real MCP call site threads this explicitly; see the
        // field doc above.
        step_limit: Some(100_000),
        algebraic_terms_over: 10, // 1,024 terms, > 512
        algebraic_terms_under: 9, // 512 terms, <= 512
        bigint_bits_over: 20,     // > 262,144 bits
        bigint_bits_under: 19,    // < 262,144 bits
        cumulative_cascade_factors: 8,
        cumulative_cascade_reps: 20, // 524,560 units each
        collection_vector_max: 99_998,
        collection_reps_to_cross: 6,
    }
}

/// The playground / `ajisai run` default profile, re-derived
/// 2026-08-14 (`docs/dev/host-profile-derivation-handoff.md` §4) and
/// re-derived again the same day once the first attempt's native
/// calibration was found not to be a valid proxy for the WASM engine
/// this profile actually governs (`docs/dev/
/// host-profile-derivation-2026-08-14.md` §10). `algebraic_terms_*` and
/// `bigint_bits_*` are unchanged by that: the term/bit thresholds a
/// cascade or widening chain crosses are fixed by the pricing formula,
/// not by which host measured the work budget's floor rate — only the
/// *margin* moved. `cumulative_cascade_reps` and
/// `collection_reps_to_cross` did move, since the work budgets
/// themselves did. See `docs/dev/mcp-host-profiles.md` for how each
/// ceiling was derived.
fn playground_profile() -> Profile {
    Profile {
        name: "playground",
        limits: RuntimeLimits::default(),
        step_limit: None,
        algebraic_terms_over: 14,  // 16,384 terms, > 10,000
        algebraic_terms_under: 13, // 8,192 terms, <= 10,000
        bigint_bits_over: 74,      // > 1,000,000 bits
        bigint_bits_under: 73,     // < 1,000,000 bits
        cumulative_cascade_factors: 8,
        cumulative_cascade_reps: 594, // 524,560 units each
        collection_vector_max: 999_997,
        collection_reps_to_cross: 42,
    }
}

/// `(√p₁+√q₁)·(√p₂+√q₂)·…` — the term count doubles with every factor, and
/// it is the only way to grow one. 25 pairs: the playground profile's
/// `algebraic_terms_over` (14) needs more distinct prime pairs than the
/// original 12-pair MCP witness carried.
fn algebraic_cascade(factors: usize) -> String {
    const PAIRS: [(u32, u32); 25] = [
        (2, 3),
        (5, 7),
        (11, 13),
        (17, 19),
        (23, 29),
        (31, 37),
        (41, 43),
        (47, 53),
        (59, 61),
        (67, 71),
        (73, 79),
        (83, 89),
        (97, 101),
        (103, 107),
        (109, 113),
        (127, 131),
        (137, 139),
        (149, 151),
        (157, 163),
        (167, 173),
        (179, 181),
        (191, 193),
        (197, 199),
        (211, 223),
        (227, 229),
    ];
    assert!(
        factors <= PAIRS.len(),
        "algebraic_cascade({factors}) needs more prime pairs than the {} available",
        PAIRS.len()
    );
    let mut source = String::new();
    for (index, (left, right)) in PAIRS.iter().take(factors).enumerate() {
        source.push_str(&format!("{left} SQRT {right} SQRT ADD"));
        if index > 0 {
            source.push_str(" MUL");
        }
        source.push(' ');
    }
    source
}

/// Widening by repeated multiplication — the only way to grow an integer,
/// and quadratic in the width it reaches by construction.
fn widening_chain(multiplications: usize) -> String {
    format!(
        "[ {} MUL ] 'M' DEF 1{}",
        "9".repeat(4096),
        " M".repeat(multiplications)
    )
}

/// Run under `profile` and report which ceiling answered, plus what the
/// numeric- and collection-work meters read when it did.
async fn refused_by(profile: &Profile, source: &str) -> (String, u64, u64) {
    let report = compute(
        source,
        ComputeOptions {
            step_limit: profile.step_limit,
            runtime_limits: Some(profile.limits),
            ..ComputeOptions::default()
        },
    )
    .await
    .to_json();
    let resource = report["error"]["resource"]
        .as_str()
        .unwrap_or("<none: the program succeeded>")
        .to_string();
    let numeric_spent = report["resourceUsage"]["numericWork"].as_u64().unwrap_or(0);
    let collection_spent = report["resourceUsage"]["collectionWork"]
        .as_u64()
        .unwrap_or(0);
    (resource, numeric_spent, collection_spent)
}

#[tokio::test]
async fn the_algebraic_term_ceiling_can_be_the_one_that_fires() {
    // One doubling past the ceiling, on the only path that grows a term
    // count. Both profiles: each supplies the factor count that first
    // exceeds its own `max_algebraic_terms`.
    for profile in [mcp_profile(), playground_profile()] {
        let (resource, spent, _) =
            refused_by(&profile, &algebraic_cascade(profile.algebraic_terms_over)).await;
        assert_eq!(
            resource, "algebraicTerms",
            "[{}] growing past `max_algebraic_terms` must be refused by \
             name, not by whichever ceiling happens to be cheaper to \
             reach. The work meter read {spent} of {} at that point; if \
             `numericWork` answered instead, the term ceiling is \
             declared past what the work budget can pay for and one of \
             the two has to move.",
            profile.name, profile.limits.max_numeric_work
        );
    }
}

#[tokio::test]
async fn the_bigint_width_ceiling_can_be_the_one_that_fires() {
    for profile in [mcp_profile(), playground_profile()] {
        let (resource, spent, _) =
            refused_by(&profile, &widening_chain(profile.bigint_bits_over)).await;
        assert_eq!(
            resource, "bigintBits",
            "[{}] growing past `max_bigint_bits` must be refused by \
             name. The work meter read {spent} of {} at that point; \
             building an N-limb integer costs about N²/4 \
             limb-operations by construction, so this pair is the \
             closest of the three and the first to break if either \
             price is re-measured.",
            profile.name, profile.limits.max_numeric_work
        );
    }
}

#[tokio::test]
async fn a_value_at_each_ceiling_is_still_allowed() {
    // The other half of a live ceiling: it has to admit what it declares,
    // or it is not the number it says it is.
    for profile in [mcp_profile(), playground_profile()] {
        for (name, source) in [
            (
                "algebraicTerms",
                algebraic_cascade(profile.algebraic_terms_under),
            ),
            ("bigintBits", widening_chain(profile.bigint_bits_under)),
        ] {
            let (resource, spent, _) = refused_by(&profile, &source).await;
            assert_eq!(
                resource, "<none: the program succeeded>",
                "[{}] a value just inside `{name}` must be allowed, and \
                 this one was refused by `{resource}` with the work \
                 meter at {spent} of {}",
                profile.name, profile.limits.max_numeric_work
            );
        }
    }
}

#[tokio::test]
async fn the_work_ceiling_is_still_reachable_once_the_size_ceilings_bind() {
    // Tightening a size ceiling must not make `numericWork` unreachable in
    // turn — the three have to be orderable, not merely ordered. Repeating
    // a cascade that stays inside `algebraicTerms` accumulates work without
    // growing any single value, which is exactly what a cumulative ceiling
    // is for.
    for profile in [mcp_profile(), playground_profile()] {
        let repeated = format!(
            "[ {} ] 'C' DEF{}",
            algebraic_cascade(profile.cumulative_cascade_factors),
            " C".repeat(profile.cumulative_cascade_reps)
        );
        let (resource, spent, _) = refused_by(&profile, &repeated).await;
        assert_eq!(
            resource, "numericWork",
            "[{}] cumulative work with no single value growing must \
             reach the work ceiling; got `{resource}` at {spent} of {}",
            profile.name, profile.limits.max_numeric_work
        );
    }
}

#[tokio::test]
async fn the_collection_ceiling_is_reachable_inside_the_materialization_one() {
    // The same liveness property, on the axis the collection meter added.
    // `max_collection_work` has to be reachable by a vector the
    // `materializedElements` ceiling admits — otherwise the collection
    // ceiling is declared past what the host will ever let a program build.
    // The ordering here runs the other way from the size ceilings above:
    // the *work* ceiling has to bind first, because a vector this size is
    // legal and it is what is done to it that is not.
    //
    // `0 99999 RANGE UNIQUE` was the MCP witness before the
    // de-quadraticization follow-up (`Value: Hash` turned the scan family's
    // O(n×distinct) linear scan into an O(n)-average `HashMap` lookup):
    // 100,000 all-distinct machine words now costs about 3.4M of the 20M
    // budget in one pass, nowhere near the ceiling. An algebraic element
    // costs enough per element to reach the ceiling in one pass, but
    // `Algebraic::hash`'s interval refinement is genuinely expensive in an
    // unoptimized build (this test runs under `cargo test`, not
    // `--release`) — a single-pass algebraic witness here once ran in
    // 40 seconds. Repeating `UNIQUE` on the same all-distinct vector
    // instead accumulates the same budget in cheap machine-word passes:
    // each repeat re-scans the same elements (still all distinct, so
    // `UNIQUE` is a no-op on the values and re-charges close to the same
    // amount each time), and the last pass crosses the budget partway
    // through — still inside `materializedElements` (built once) and
    // `executionSteps` (a handful of steps either way). The playground
    // profile needs far more repeats than MCP's 6 (its
    // `materializedElements`/`collectionWork` are both larger, and not by
    // the same factor — see `mcp-host-profiles.md`), which is why this is
    // driven by `profile.collection_reps_to_cross` rather than a literal
    // source string.
    for profile in [mcp_profile(), playground_profile()] {
        let source = format!(
            "0 {} RANGE{}",
            profile.collection_vector_max,
            " UNIQUE".repeat(profile.collection_reps_to_cross)
        );
        let (resource, _, collection_spent) = refused_by(&profile, &source).await;
        assert_eq!(
            resource,
            "collectionWork",
            "[{}] {} passes over a `materializedElements`-legal vector \
             must cross the collection-work ceiling and be refused by \
             name; got `{resource}` with the collection meter at {} of {}",
            profile.name,
            profile.collection_reps_to_cross,
            collection_spent,
            profile.limits.max_collection_work
        );
    }
}

#[tokio::test]
async fn the_materialization_ceiling_is_still_what_bounds_a_bare_vector() {
    // And the other direction: building the largest permitted vector, and
    // walking it once, must stay inside the collection budget. If a linear
    // Word over a legal vector could not run, the collection price would be
    // bounding the wrong thing.
    for profile in [mcp_profile(), playground_profile()] {
        let max_element_index = profile.limits.max_materialized_elements - 1;
        let source = format!("0 {max_element_index} RANGE REVERSE LENGTH");
        let (resource, _, _) = refused_by(&profile, &source).await;
        assert_eq!(
            resource, "<none: the program succeeded>",
            "[{}] one linear pass over the largest permitted vector must \
             be affordable; got `{resource}`",
            profile.name
        );
    }
}
