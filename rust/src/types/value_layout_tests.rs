//! How wide a `Value` is allowed to be.
//!
//! A `Value` is copied and moved constantly — every stack slot, every element
//! of an AoS `Vector`, every intermediate a Word builds — so its width is a
//! multiplier on almost all memory traffic the runtime generates. It used to be
//! 344 bytes around a 72-byte `ValueData`, because `Option<AbsenceMetadata>`
//! inlined a 256-byte `DebugDiagnosis`: a summary string, an evidence list, a
//! next-check list and a candidate list, reserved in full on every value
//! whether or not it was absent. A vector of 262,144 numbers moved 90 MB to
//! carry 19 MB of numerators, and `0 262143 RANGE` took 89.8 ms; boxed, the
//! same line takes 9.9 ms.
//!
//! These are layout assertions, not wall clocks (`work_meter_calibration_tests`
//! says why timing tests do not belong in CI). They pin the shape that made the
//! difference: the absence envelope is a *pointer* on a present value, not a
//! reserved diagnosis.

use crate::semantic::AbsenceMetadata;
use crate::types::{Value, ValueData};
use std::mem::size_of;

/// The payload plus the cached nesting depth, rounded to alignment: four
/// words. A NIL's reason is boxed inside `ValueData::Nil` and an irrational
/// inside `ValueData::ExactScalar`, so neither widens a present number — a
/// `Fraction` (three words with its tag's spare values naming the other
/// variants) is the widest thing a value holds inline. Anything wider means
/// something rare was inlined into every value again.
#[test]
fn a_value_is_four_words() {
    let payload = size_of::<ValueData>();
    let value = size_of::<Value>();
    assert!(
        payload <= 3 * size_of::<u64>() && value <= 4 * size_of::<u64>(),
        "Value is {value} bytes around a {payload}-byte ValueData: a rare \
         form (a NIL's reason, an irrational) must stay behind a pointer, \
         because this width multiplies every stack slot and every AoS lane"
    );
}

/// Stated separately from the budget above so a regression names its cause.
/// `AbsenceMetadata` is a reason and a *pointer* to a declared detail, which
/// is only ever present on a `userDeclared` NIL.
#[test]
fn absence_metadata_holds_its_detail_behind_a_thin_pointer() {
    let metadata = size_of::<AbsenceMetadata>();
    assert!(
        metadata <= 2 * size_of::<usize>(),
        "AbsenceMetadata is {metadata} bytes: it must not widen its detail \
         to a fat pointer"
    );
}

/// The numbers a `ValueData` holds stay three words: a rational's two
/// machine words and its tag, with anything wider — a `BigInt` pair, an
/// algebraic basis and term map — behind a pointer. Those were inline once,
/// and made every `Value` 104 bytes to hold what nearly every value never
/// does.
#[test]
fn numbers_hold_their_rare_wide_forms_behind_a_pointer() {
    use crate::types::exact::ExactReal;
    use crate::types::fraction::Fraction;
    assert!(size_of::<Fraction>() <= 3 * size_of::<u64>());
    assert!(size_of::<ExactReal>() <= 3 * size_of::<u64>());
    assert!(size_of::<ValueData>() <= 3 * size_of::<u64>());
    assert!(size_of::<Value>() <= 4 * size_of::<u64>());
}
