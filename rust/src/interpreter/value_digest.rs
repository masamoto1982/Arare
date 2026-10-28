//! A value's digest: the `DIGEST` Word's answer for anything that is not a
//! Word, and the leaf of a User Word's content identity (LANG.VALUES.DENOTATION).
//! Equal values digest alike whatever built them. Four representation traps
//! make that easy to get wrong without a test noticing:
//!
//!  * **Algebraic normal forms are not canonical.** `8 SQRT` keeps the basis
//!    `{8}`; `2 SQRT 2 SQRT ADD` keeps `{2}` — equal values, disagreeing
//!    `normal_form_terms()`. Hashing the normal form would make this Phase's
//!    founding example (`8 SQRT` against `2 SQRT 2 SQRT ADD`) fail. `impl Hash for Algebraic`
//!    (`types/exact/algebraic.rs`) already solves this by hashing
//!    `floor(value * 2^bits)`, doubling `bits` until the bounding interval's
//!    floor is decided; this module reruns that exact loop at that impl's own
//!    `HASH_KEY_BITS` precision (`DIGEST_ALGEBRAIC_BITS`), not a larger one —
//!    see `encode_algebraic`'s doc comment for why a larger precision is not
//!    just unnecessary but actively wrong for a value that can carry
//!    hundreds of terms. This gives "equal ⇒ same digest" but not the
//!    converse: two distinct algebraic numbers closer together than
//!    `2^-DIGEST_ALGEBRAIC_BITS` fold to the same key. Documented as a
//!    residual, not silently assumed away.
//!  * **`Vector` and `Tensor` can be the same value.** A rectangular numeric
//!    `Vector` equals the `Tensor` holding the same lanes
//!    (`tensor_eq_vector`), so encoding by `ValueData` variant would answer
//!    differently for two equal values. Both are encoded through
//!    `Value::len()` / `Value::child(i)` instead, which is representation
//!    agnostic — but `Scalar`/`ExactScalar` also report `len() == 1` and
//!    `child(0) == self`, so those two are matched as their own leaves before
//!    a collection encoder ever runs, or encoding them would recurse forever.
//!  * **`absence`'s reason is meaning.** `PartialEq for Value` always reads
//!    the NIL reason, so the digest does too.
//!  * **`stackDisplay` is not the value.** It is a rendering, and for a
//!    Word it is the body's text; the digest always encodes the value itself,
//!    so two spellings of one value digest alike.

use num_bigint::{BigInt, Sign};
use num_traits::One;

use crate::interpreter::word_identity::content_digest;
use crate::types::exact::algebraic::floor_scaled;
use crate::types::exact::{Algebraic, ExactReal};
use crate::types::fraction::Fraction;
use crate::types::{Value, ValueData};

/// Bits of fractional precision an algebraic scalar's digest key is taken at
/// — deliberately `types::exact::algebraic::HASH_KEY_BITS`, the same
/// precision `impl Hash for Algebraic` already uses, not a larger one; see
/// `encode_algebraic`. Gives "equal ⇒ same digest" (soundness); does not
/// separate two distinct algebraic numbers closer together than
/// `2^-DIGEST_ALGEBRAIC_BITS` — see the module comment.
pub(crate) const DIGEST_ALGEBRAIC_BITS: u32 = 64;

/// Version tag for this byte grammar. Bump it if the grammar ever changes —
/// a digest is not a compatible value across a tag change.
pub(crate) const VALUE_DIGEST_SCHEMA_TAG: &[u8] = b"AJISAI-VAL-1";

/// The digest of one value's denotation. Equal values digest alike whatever
/// built them.
pub(crate) fn value_digest(value: &Value) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(VALUE_DIGEST_SCHEMA_TAG);
    encode_value(&mut bytes, value);
    content_digest(&bytes)
}

fn write_u64(bytes: &mut Vec<u8>, n: u64) {
    bytes.extend_from_slice(&n.to_be_bytes());
}

fn write_str(bytes: &mut Vec<u8>, s: &str) {
    write_u64(bytes, s.len() as u64);
    bytes.extend_from_slice(s.as_bytes());
}

/// A `BigInt`, written as its signed radix-16 spelling. Never downcast to a
/// machine integer — the whole point is that arbitrary-precision values stay
/// exact through the encoding.
fn write_sint(bytes: &mut Vec<u8>, i: &BigInt) {
    write_str(bytes, &i.to_str_radix(16));
}

/// [`write_sint`] for an integer that fits an `i128`: the same length-prefixed
/// radix-16 spelling `BigInt::to_str_radix(16)` gives — lowercase digits, a
/// leading `-` for a negative, `0` for zero.
fn write_small_sint(bytes: &mut Vec<u8>, i: i128) {
    let mut digits = [0u8; 33];
    let mut at = digits.len();
    let mut magnitude = i.unsigned_abs();
    loop {
        at -= 1;
        digits[at] = b"0123456789abcdef"[(magnitude % 16) as usize];
        magnitude /= 16;
        if magnitude == 0 {
            break;
        }
    }
    if i < 0 {
        at -= 1;
        digits[at] = b'-';
    }
    write_u64(bytes, (digits.len() - at) as u64);
    bytes.extend_from_slice(&digits[at..]);
}

/// Encode one value. Leaves are matched directly on `ValueData` first — this
/// is what keeps `Scalar`/`ExactScalar` (whose `len()`/`child()` describe a
/// one-element self-loop) from ever reaching the `Vector`/`Tensor` recursion.
/// `Vector` and `Tensor` share one arm precisely because they must encode
/// identically when they are equal: it walks `Value::len()` / `Value::child`,
/// never the raw variant payload.
fn encode_value(bytes: &mut Vec<u8>, value: &Value) {
    match &value.data {
        ValueData::Truths(t) => encode_value(bytes, &t.to_vector()),
        ValueData::Nil(_) => {
            bytes.push(b'N');
            let reason = value
                .nil_reason()
                .map(|r| r.as_protocol_str())
                .unwrap_or("");
            write_str(bytes, reason);
            // An ABSENT NIL is identified by its Text (LANG.VALUES.NIL), so
            // the detail is part of the identity, exactly as `Value::hash`
            // and `PartialEq` treat it.
            write_str(bytes, value.absence_detail().unwrap_or(""));
        }
        ValueData::Boolean(b) => {
            bytes.push(b'B');
            bytes.push(u8::from(*b));
        }
        ValueData::Text(s) => {
            bytes.push(b'S');
            write_str(bytes, s);
        }
        // A Symbol digests as its bare name under its own tag, distinct from
        // ValueData::Text's 'S' tag.
        ValueData::Symbol(name) => {
            bytes.push(b'Y');
            write_str(bytes, name);
        }
        // A Record digests as its two sequences under its own tag, so a
        // Record and the Vector of its pairs never collide.
        ValueData::Record(record) => {
            bytes.push(b'R');
            write_u64(bytes, record.len() as u64);
            for (key, value) in record.entries() {
                encode_value(bytes, key);
                encode_value(bytes, value);
            }
        }
        ValueData::Scalar(f) => encode_rational(bytes, f),
        ValueData::ExactScalar(exact) => match &**exact {
            ExactReal::Rational(f) => encode_rational(bytes, f),
            ExactReal::Algebraic(alg) => encode_algebraic(bytes, alg),
        },
        ValueData::Vector(_) | ValueData::Tensor { .. } => {
            bytes.push(b'V');
            let len = value.len();
            write_u64(bytes, len as u64);
            for i in 0..len {
                let child = value
                    .child(i)
                    .expect("i < value.len() always has a child for Vector/Tensor");
                encode_value(bytes, &child);
            }
        }
    }
}

/// A rational scalar as it is held: every `Fraction` is canonical — lowest
/// terms, a positive denominator or one of the two points over zero — so
/// equal values land on identical bytes however each was built
/// (`fraction_lowest_terms_tests` holds the arithmetic to it).
fn encode_rational(bytes: &mut Vec<u8>, f: &Fraction) {
    bytes.push(b'Q');
    match &f.repr {
        // Spelled in place: the radix-16 digits the `BigInt` arm writes,
        // without building two `BigInt`s and two strings per lane.
        crate::types::fraction::FractionRepr::Small(n, d) => {
            write_small_sint(bytes, i128::from(*n));
            write_small_sint(bytes, i128::from(*d));
        }
        crate::types::fraction::FractionRepr::Big(big) => {
            debug_assert!(
                big.denominator.sign() == Sign::Plus
                    && crate::types::fraction_arithmetic::balanced_bigint_gcd(
                        &big.numerator,
                        &big.denominator
                    )
                    .is_one(),
                "a Big fraction is held in lowest terms with a positive denominator"
            );
            write_sint(bytes, &big.numerator);
            write_sint(bytes, &big.denominator);
        }
    }
}

/// An algebraic scalar, keyed by `floor(value * 2^DIGEST_ALGEBRAIC_BITS)` —
/// the same representation-independent enclosure key `impl Hash for
/// Algebraic` uses (`HASH_KEY_BITS`), at that same precision rather than a
/// larger one of this module's own. `Algebraic` never holds a rational
/// (rationals demote eagerly), so the value is never exactly on a `2^-bits`
/// grid line and the loop below always terminates.
///
/// Reusing `HASH_KEY_BITS` rather than choosing a larger precision is load
/// bearing, not just tidy: this loop's cost is `O(terms)` big-integer square
/// roots at `~2*bits` bits each, and a legitimately reachable value can carry
/// up to `max_algebraic_terms` (512) terms. Keying at 512 bits instead of 64
/// made one such 256-term stack value's digest cost tens of milliseconds in
/// a release build and several hundred in a debug one — multiplied across a
/// stack of them, enough to make an unrelated resource-limit regression test
/// time out in CI. `HASH_KEY_BITS` is not a lesser precision chosen for
/// speed at correctness's expense: it is the precision `UNIQUE` / `GROUP` /
/// `TALLY` already trust for exactly this soundness property everywhere in
/// the interpreter, so reusing it does not introduce a new residual — it
/// reuses the one already load-bearing elsewhere. The residual this leaves
/// (two distinct algebraic numbers within `2^-64` of each other digest
/// alike) is documented, not hidden.
fn encode_algebraic(bytes: &mut Vec<u8>, alg: &Algebraic) {
    let target_bits = u64::from(DIGEST_ALGEBRAIC_BITS);
    let mut bits = target_bits + 8;
    let key = loop {
        let (lo, hi) = alg.bounds(bits);
        let lo_key = floor_scaled(&lo, target_bits);
        let hi_key = floor_scaled(&hi, target_bits);
        if lo_key == hi_key {
            break lo_key;
        }
        debug_assert!(
            bits < (1 << 24),
            "algebraic digest key failed to converge — an irrational value \
             should never sit exactly on a precision grid line"
        );
        bits *= 2;
    };
    bytes.push(b'A');
    write_sint(bytes, &key);
}

#[cfg(test)]
#[path = "value_digest_small_tests.rs"]
mod small_tests;

#[cfg(test)]
#[path = "value_digest_tests.rs"]
mod value_digest_tests;
