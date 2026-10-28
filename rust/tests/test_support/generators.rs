//! Semantic-domain generators (Phase 1 foundation).
//!
//! Each strategy yields an **Ajisai source program** that leaves exactly one
//! value of a given semantic domain on the stack. Property-based law tests
//! (`observation_laws.rs` and later phases) consume these so that adding a new
//! law for a new domain is "add a generator, write the equation" — the土台
//! the formalization-expansion roadmap §1.2-(T) asks for.
//!
//! Programs are deliberately small (cheap to run thousands of times) while
//! still covering the boundary values the roadmap §4 calls out: zero, sign,
//! NIL, irrationals, the logical Unknown, and empty/short vectors.
//!
//! Shared test scaffolding: each integration-test binary uses a different
//! subset of these generators, so any single binary sees the rest as dead
//! code. Allow it module-wide rather than annotating every helper.
#![allow(dead_code)]

use proptest::prelude::*;

/// Small signed integers, including zero and both signs.
pub fn small() -> impl Strategy<Value = i64> {
    -20i64..=20
}

/// A nonzero divisor (keeps `_ _ DIV` away from the NIL-projection path when a
/// value is wanted; a separate [`nil_src`] exercises the division-by-zero NIL).
pub fn nonzero() -> impl Strategy<Value = i64> {
    (1i64..=20).prop_flat_map(|n| prop_oneof![Just(n), Just(-n)])
}

/// Pushes a single rational scalar (RawNumber role).
pub fn scalar_src() -> impl Strategy<Value = String> {
    prop_oneof![
        small().prop_map(|n| n.to_string()),
        (small(), nonzero()).prop_map(|(a, b)| format!("{a} {b} DIV")),
        (small(), small()).prop_map(|(a, b)| format!("{a} {b} ADD")),
        (small(), small()).prop_map(|(a, b)| format!("{a} {b} MUL")),
    ]
}

/// Pushes a single definite truth value (Boolean, TruthValue role).
pub fn boolean_src() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("TRUE".to_string()),
        Just("FALSE".to_string()),
        (small(), small()).prop_map(|(a, b)| format!("{a} {b} LT")),
        (small(), small()).prop_map(|(a, b)| format!("{a} {b} EQ")),
        (small(), small()).prop_map(|(a, b)| format!("{a} {b} GT")),
    ]
}

/// Pushes a reasoned NIL via the NIL Projection Rule (a negative radicand,
/// LANG.FAILURE.PROJECT).
pub fn nil_src() -> impl Strategy<Value = String> {
    small().prop_map(|n| format!("{n} {n} MUL -1 MUL -1 ADD SQRT"))
}

/// Radicands that are **not** perfect squares, so `√n` stays a genuine
/// irrational. (Probe finding: `√4`, `√9` collapse to the rationals `2`, `3`,
/// so a comparison of equal `√(square)` decides `true`, not the logical U.)
fn non_square_radicand() -> impl Strategy<Value = i64> {
    prop::sample::select(vec![2i64, 3, 5, 6, 7, 8, 10, 11, 13])
}

/// Pushes an irrational exact-real scalar `√n` (n not a perfect square, so the
/// value is a genuine lazy continued fraction).
pub fn irrational_src() -> impl Strategy<Value = String> {
    non_square_radicand().prop_map(|n| format!("{n} SQRT"))
}

/// Pushes a single vector/tensor literal (1–3 elements).
pub fn vector_src() -> impl Strategy<Value = String> {
    prop::collection::vec(small(), 1..4).prop_map(|xs| {
        let body = xs.iter().map(i64::to_string).collect::<Vec<_>>().join(" ");
        format!("[ {body} ]")
    })
}

/// Pushes a single code-shaped Vector (callable).
pub fn block_src() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("[ 1 ADD ]".to_string()),
        Just("[ DUP MUL ]".to_string()),
        Just("[ ]".to_string()),
    ]
}

// ───────────────────────── Phase 6: names & dictionary ──────────────────────

/// A short user-word name in the runtime's action-object style (already
/// uppercase, so word-name normalization LANG.SOURCE.NORMALIZE is a no-op on it). Kept distinct
/// from every built-in so `DEF` never hits the "cannot redefine built-in" path.
pub fn user_word_name() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec!["INC", "TWICE", "BUMP", "STEP-UP", "ADD-ONE", "GROW"])
}

/// A user-word body that, applied to one scalar, leaves one scalar (a pure
/// `Σ ⇀ Σ` transducer over the top of stack). Paired with a closed-form program
/// `apply(x)` whose observation the defined word must reproduce.
pub fn user_word_body() -> impl Strategy<Value = (&'static str, &'static str)> {
    // (body tokens, equivalent inline program fragment applied after a value)
    prop::sample::select(vec![
        ("1 ADD", "1 ADD"),
        ("2 MUL", "2 MUL"),
        ("3 SUB", "3 SUB"),
        ("0 SUB 1 ADD", "0 SUB 1 ADD"),
        ("10 MUL 1 ADD", "10 MUL 1 ADD"),
    ])
}

/// Union over every semantic domain — the "any well-formed value" generator.
pub fn any_value_src() -> impl Strategy<Value = String> {
    prop_oneof![
        scalar_src(),
        boolean_src(),
        nil_src(),
        irrational_src(),
        vector_src(),
        block_src(),
    ]
}

// ─────────────────────── Phase 7: effects & observation ──────────────────────

/// A short program that leaves a value **and emits no host effect** — a pure /
/// internal-state computation (arithmetic, logic, vectors, dictionary, module
/// import). The outbound `HostEffect` log (π_Eff) stays empty for all of these
/// (probe-confirmed: `IMPORT`/`DEF` mutate Σ but never push to the log; `NOW` /
/// `CSPRNG` are host *reads*, not outbound effects).
pub fn effect_free_src() -> impl Strategy<Value = String> {
    prop_oneof![
        scalar_src(),
        boolean_src(),
        nil_src(),
        irrational_src(),
        vector_src(),
        (small(), small()).prop_map(|(a, b)| format!("[ {a} {b} ] REVERSE")),
        (small(), small()).prop_map(|(a, b)| format!("[ 1 ADD ] 'INC' DEF {a} INC {b} ADD")),
    ]
}

/// A canonical `SERIAL` outbound program (after ``), as
/// `(bare-program, qualified-program)`. Both spellings emit the *same* ordered
/// `serial` effect list (boundary `bare ≡ SERIAL@W`, applied to effectful
/// words). Every program threads a port-id handle so it is well-formed in
/// isolation.
pub fn serial_outbound_call() -> impl Strategy<Value = (String, String)> {
    prop::sample::select(vec![
        ("'P1' OPEN", "'P1' SERIAL@OPEN"),
        (
            "'P1' OPEN 9600 CONFIGURE",
            "'P1' SERIAL@OPEN 9600 SERIAL@CONFIGURE",
        ),
        (
            "'P1' OPEN [ 65 66 ] WRITE",
            "'P1' SERIAL@OPEN [ 65 66 ] SERIAL@WRITE",
        ),
        ("'P1' OPEN FLUSH", "'P1' SERIAL@OPEN SERIAL@FLUSH"),
        ("'P1' OPEN CLOSE", "'P1' SERIAL@OPEN SERIAL@CLOSE"),
    ])
    .prop_map(|(b, q)| (b.to_string(), q.to_string()))
}

// ──────────────────────── Phase 8: child runtimes ───────────────────────────

/// A **deterministic, completing block body** for child-runtime laws: run
/// standalone and via `[ body ] SPAWN AWAIT` it leaves the same final stack
/// (no host effects, no genuine error — domain failures project to NIL and the
/// child still *completes*, probe finding). Used to pin the law
/// "AWAIT observes the child's ⟦body⟧ final configuration".
pub fn completing_block_body() -> impl Strategy<Value = String> {
    prop_oneof![
        (small(), small()).prop_map(|(a, b)| format!("{a} {b} ADD")),
        (small(), small()).prop_map(|(a, b)| format!("{a} {b} MUL")),
        (small(), small()).prop_map(|(a, b)| format!("[ {a} {b} ] REVERSE")),
        small().prop_map(|a| format!("{a} [ 1 ADD ] EXEC")),
        small().prop_map(|a| format!("{a} 0 DIV")), // div-by-zero → a point over zero, still completes
        Just("TRUE FALSE AND".to_string()),
        Just("[ 3 1 2 ] 0 GET".to_string()),
    ]
}

/// A block body that raises a **genuine error** (not a domain projection), so the
/// child terminates `failed`: an unknown word or a stack underflow.
pub fn failing_block_body() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("NOPEWORD".to_string()),
        Just("1 ADD".to_string()), // ADD underflows on a one-item stack
        Just("UNDEFINED-WORD-XYZ".to_string()),
    ]
}

// ───────────────────── Phase 9: records & strings ────────────────────────────

/// A non-empty lowercase word (1–6 letters), used as a string-literal body
/// `'word'`. Kept to `[a-h]` so it never collides with whitespace / quotes and
/// so `CHARS`/`JOIN` round-trip cleanly (an empty string is NIL, LANG.VALUES.NIL).
pub fn ascii_word() -> impl Strategy<Value = String> {
    prop::collection::vec(0u8..8, 1..7)
        .prop_map(|cs| cs.iter().map(|b| (b'a' + b) as char).collect())
}

/// A three-field JSON object `{"a":va,"b":vb,"c":vc}` with integer values, the
/// source for record laws (records are constructed via `JSON@PARSE`; there is
/// no record literal syntax, finding I1).
pub fn record_abc() -> impl Strategy<Value = (i64, i64, i64)> {
    (small(), small(), small())
}
