//! Unified resource-control limits for internal computation cost (CS5).
//!
//! The execution-step budget (`Interpreter::max_execution_steps`, charged once
//! per word in `execute_builtin.rs`) prices *how many words* run, but not the
//! expensive work performed **inside** a single word: algebraic term×term
//! products, reciprocal/conjugate recursion, sign/bounds precision doubling,
//! BigInt blow-up, huge materializations, and huge numeric-literal parses. A
//! word that loops internally counts as one step, so those costs bypass the
//! step budget entirely. Ajisai must stay exact but return a **diagnosable**
//! runtime failure at a resource ceiling rather than an approximation,
//! wraparound, panic, OOM, or WASM trap.
//!
//! [`RuntimeLimits`] gathers those ceilings in one place. It lives on the
//! interpreter (and every child runtime inherits a copy — it is **not** a
//! global), and small limits can be injected in tests to fire a guard without
//! actually allocating or computing anything huge.
//!
//! Limits are a safety control, never value semantics: conformance results must
//! not depend on a specific limit value, and all conformance must pass under
//! the documented defaults (LANG.MACHINE.LIMITS).

use crate::error::{AjisaiError, ResourceLimit, Result};
use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;

pub use super::host_profile_defaults::*;

/// Width of the machine word the bignum arithmetic underneath actually works
/// in. Work is priced in limb×limb products because that is what a bignum
/// multiply costs.
pub const WORK_LIMB_BITS: u64 = 64;

/// Limbs needed to hold `bits`, never zero — every operation costs at least one.
///
/// This is the unit the work meter counts in, and the reason it counts in this
/// unit rather than in operations: the meter used to charge a *term-pair*
/// count, which prices the loop and ignores the size of the numbers inside it.
/// Multiplying two 4096-digit integers and two single-digit ones both charged
/// one unit while differing by orders of magnitude in cost, so the meter could
/// not bound the thing it exists to bound.
pub fn work_limbs(bits: u64) -> u64 {
    bits.div_ceil(WORK_LIMB_BITS).max(1)
}

/// What one algebraic term pair costs, in limb-multiply units.
///
/// A Tier 1 term pair is not one bignum multiply. It multiplies two
/// coefficients *and* two radicands, then square-free-decomposes the product
/// against a basis that grows with the number of distinct primes in play, and
/// inserts the result into an ordered term map. Measured against the rational
/// path — which is one bignum multiply and therefore the meter's natural unit —
/// a term pair costs roughly three orders of magnitude more. Without this
/// factor the two paths ran on the same meter at wildly different prices, so a
/// ceiling calibrated for one was meaningless for the other.
///
/// Empirical, and rounded to a power of two. It was chosen by measuring both
/// paths on one container: the rational chain charged ~80,000 units/ms and the
/// algebraic cascade ~56 units/ms, a ratio near 1,400. The exact figure is a
/// property of that machine and this `num-bigint`, and the meter only needs to
/// be right to within the order of magnitude that keeps a ceiling meaningful
/// for both paths. It is deliberately *not* pinned by a timing test: a wall
/// clock in CI would be flaky, and a stale constant is a worse failure than an
/// unpinned one only if nobody re-measures — so re-measure when either path's
/// representation changes.
pub const ALGEBRAIC_PAIR_UNITS: u64 = 1_024;

/// What one term of an algebraic *sum* costs, in limb-multiply units.
///
/// A sum does not multiply term pairs; it merges two term maps, refining the
/// basis only where the operands' radicals share a factor and carrying a term
/// over only where its monomial holds a radical the merge split
/// (`types/exact/algebraic_sum.rs`). It was priced with
/// [`ALGEBRAIC_PAIR_UNITS`] per term until that path was rebuilt, and then ran
/// an order of magnitude cheaper per unit than every other path, so a sum the
/// host could finish in milliseconds was refused for `numericWork`.
///
/// Measured like its partner, and kept a power of two: chosen so the dearest
/// sum there is — one whose every addend splits a radical that every monomial
/// of the sum shares — still charges no slower than the floor path the host
/// profiles are derived from, with margin, on native and on WASM
/// (`examples/work_meter_calibration.rs`, `scripts/wasm-profile-calibration.mjs`).
pub const ALGEBRAIC_SUM_TERM_UNITS: u64 = 256;

/// The work a binary exact-arithmetic operation of these two operand widths
/// costs, in limb-multiply units.
///
/// Every schema is priced limb×limb, including addition and subtraction. That
/// looks wrong for a moment and is the whole point: Ajisai's `ADD` is *rational*
/// addition, not integer addition. `a/b ADD c/d` cross-multiplies into
/// `(ad ADD cb)/(bd)` and then normalizes by a gcd — three multiplications and a
/// Euclid, none of them linear in the wider operand.
///
/// Pricing it as linear was not a rounding error. Measured on the reference
/// container with the literal parsed once (`examples/work_meter_calibration`),
/// a 4096-digit rational addition takes 745 µs — *twice* what a multiplication
/// of the same width takes — and used to be charged 213 units against that
/// multiplication's ~430,000. A meter that prices the cheaper operation four
/// thousand times higher than the dearer one is not bounding anything; it was
/// possible to spend the whole `wallTimeMs` budget on additions while the work
/// meter read a fraction of a percent.
///
/// Deliberately an upper bound on the real cost and deliberately cheap to
/// compute: it is charged before the operation runs, so a runaway computation
/// is refused rather than measured. The bound is loosest for very wide
/// multiplications, where `num-bigint` switches to Karatsuba and beats the
/// limb×limb estimate — over-charging is the safe direction, and
/// `examples/work_meter_calibration` is where to see by how much.
pub fn binary_numeric_work(left_bits: u64, right_bits: u64) -> u64 {
    work_limbs(left_bits).saturating_mul(work_limbs(right_bits))
}

/// What the work meter needs to know about one operand, independent of the
/// shape it arrived in.
///
/// The meter used to read a `Fraction` and stop there, which made it a meter on
/// the *representation* rather than on the arithmetic: `2 3 MUL` was charged and
/// `[ 2 ] 3 MUL` was free, because the second one leaves the scalar path and
/// every other path charged nothing. Whether an operand is stored as a scalar,
/// a one-element vector or an N-lane tensor is an internal decision
/// (LANG.AUTHORITY.FREEDOM says it is unobservable), and a safety control whose
/// price turns on an unobservable decision is not a control. A broadcast
/// performs one operation per lane, so lanes are what it is charged for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperandWork {
    /// Numeric leaves the operand carries. A scalar is one lane.
    pub lanes: u64,
    /// The widest leaf, in bits.
    pub bits: u64,
    /// The largest algebraic term count among the leaves, or 0 when every leaf
    /// is rational.
    pub terms: u64,
    /// The largest radical basis among the leaves, or 0 when every leaf is
    /// rational: how deep the conjugation that inverts one goes.
    pub basis: u64,
}

impl OperandWork {
    /// The measure of a single rational leaf.
    pub const fn leaf(bits: u64) -> Self {
        Self {
            lanes: 1,
            bits,
            terms: 0,
            basis: 0,
        }
    }

    /// Combine sibling leaves: lanes add, width and term count take the worst.
    pub fn join(self, other: Self) -> Self {
        Self {
            lanes: self.lanes.saturating_add(other.lanes),
            bits: self.bits.max(other.bits),
            terms: self.terms.max(other.terms),
            basis: self.basis.max(other.basis),
        }
    }
}

/// The work a binary arithmetic operation costs across every lane a broadcast
/// will touch.
///
/// `pair_units` is what one lane pair costs *above* the bignum operation: 1 for
/// a rational pair, and the term-pair count times [`ALGEBRAIC_PAIR_UNITS`] for
/// an algebraic one. A scalar operation is `lanes = 1` of this, so shape
/// changes nothing about the price.
pub fn broadcast_numeric_work(left: OperandWork, right: OperandWork, pair_units: u64) -> u64 {
    // Broadcast repeats the narrower operand across the wider one, so the
    // wider lane count is how many operations actually run.
    let lanes = left.lanes.max(right.lanes).max(1);
    binary_numeric_work(left.bits, right.bits)
        .saturating_mul(pair_units)
        .saturating_mul(lanes)
}

/// What one boxed element copy costs, in collection units.
///
/// A collection unit is the cheapest thing this meter counts: one equality
/// probe between two machine-word scalars, measured at 5.8 ns for a 16,000
/// element `UNIQUE` and 9.1 ns at 100,000 — call it 6 ns.
///
/// Copying one element costs 80 ns at 4,000 elements and 389 ns at 100,000; the
/// spread is the allocator and the cache, not the copy. 16 units (~96 ns) sits
/// inside that range and is the smallest power of two that keeps the dearest
/// measured copy — 100,000 elements through `REVERSE`, 38.9 ms — charging
/// 43,700 units/ms, above the 30,800 units/ms floor of the scan family and well
/// above the 14,465 units/ms floor the arithmetic meter already accepts on an
/// unbounded path.
///
/// Empirical, and re-measured by `examples/collection_word_calibration`.
pub const COLLECTION_COPY_UNITS: u64 = 16;

/// What comparing one *algebraic* element costs, in collection units.
///
/// `Algebraic::eq` is `Algebraic::cmp`: deciding it rebases both sides over a
/// common refinement of their radical bases, which is nothing like a limb
/// compare. Measured at 3.0 µs through `UNIQUE` and 3.3 µs through `SORT`
/// (after correcting for Rust's sort detecting the already-ascending run that
/// `2 n RANGE { SQRT } MAP` produces) — 500 to 550 units at 6 ns each.
///
/// Distinct from [`ALGEBRAIC_PAIR_UNITS`], which prices an algebraic *product*.
/// A product and a comparison are different operations and the measurements
/// disagree by a factor of two; one constant serving both would be a rounding
/// of two numbers into one.
pub const ALGEBRAIC_ELEMENT_UNITS: u64 = 512;

/// The units a collection Word is charged for touching one element, given the
/// measure of the operand it came from.
///
/// Two prices, because the measurements are two different shapes. Copying is
/// `constant + width`: duplicating a 213-limb BigInt costs 3.4x what
/// duplicating a machine word does, not 213x, because the wide part is a
/// memcpy. Comparing is `width`: an equality test walks limbs until they
/// differ, so it is the width that is traversed, not a constant plus it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElementCost {
    /// Numeric leaves one element carries. 1 for a scalar; `k` for an element
    /// that is itself a `k`-element vector, because an equality test on that
    /// element is a loop over `k` leaves and a copy of it copies `k` of them.
    pub leaves: u64,
    /// What one leaf costs to look at: limbs for a rational, and
    /// `terms × ALGEBRAIC_ELEMENT_UNITS` for an algebraic, whichever is dearer.
    pub width: u64,
}

impl ElementCost {
    /// The cost of one element in a collection of `element_count` elements
    /// whose combined measure is `work`.
    ///
    /// `OperandWork::lanes` counts leaves across the whole operand, so the
    /// leaves *per element* are that divided by the element count — which is
    /// how a vector of 4,000 sixty-four-element rows is told apart from a
    /// vector of 256,000 scalars, two operands with the same leaf count and a
    /// 64x difference in what one element costs to compare.
    pub fn measure(work: OperandWork, element_count: usize) -> Self {
        let elements = (element_count as u64).max(1);
        let width = work_limbs(work.bits).max(work.terms.saturating_mul(ALGEBRAIC_ELEMENT_UNITS));
        Self {
            leaves: (work.lanes / elements).max(1),
            width: width.max(1),
        }
    }

    /// What one equality probe or order comparison against one element costs.
    pub fn probe(self) -> u64 {
        self.leaves.saturating_mul(self.width)
    }

    /// What copying one element into a result costs.
    pub fn copy(self) -> u64 {
        self.leaves
            .saturating_mul(COLLECTION_COPY_UNITS.saturating_add(self.width))
    }

    /// What copying `count` elements costs.
    pub fn copies(self, count: usize) -> u64 {
        self.copy().saturating_mul(count as u64)
    }

    /// What a comparison sort over `count` elements costs at its worst case.
    ///
    /// `n⌈log₂n⌉` is the bound, not the count Rust's sort actually performs —
    /// it detects sorted runs and can finish in `n`. A safety control prices
    /// the worst case, because the input that reaches it is the one that did
    /// not have a sorted run.
    pub fn comparison_sort(self, count: usize) -> u64 {
        let n = count as u64;
        let depth = u64::from(n.max(1).ilog2()) + 1;
        self.probe()
            .saturating_mul(n)
            .saturating_mul(depth)
            .saturating_add(self.copies(count))
    }
}

/// Operand width for the work meter on an exact (Tier 0 or Tier 1) value:
/// the wider of what it stores in coefficients and in radicands.
pub fn exact_work_bits(value: &ExactReal) -> u64 {
    match value {
        ExactReal::Algebraic(a) => a.max_coefficient_bits().max(a.max_radicand_bits()),
        other => other.max_coefficient_bits(),
    }
}

/// Operand width for the work meter, without paying to measure a small one.
///
/// `Fraction` keeps machine-word values in a `Small` representation, which is
/// the overwhelming majority of ordinary arithmetic; asking a `BigInt` for its
/// bit length on every add in a hot loop would cost more than the guard saves.
/// A small fraction is one limb by definition.
pub fn fraction_work_bits(f: &Fraction) -> u64 {
    if f.is_small() {
        return 1;
    }
    fraction_result_bits(f)
}

/// The width a fraction actually occupies: the wider of its two halves.
pub fn fraction_result_bits(f: &Fraction) -> u64 {
    // Read where it lies: `numerator()` answers an owned `BigInt`, so asking a
    // wide value for its halves copied both just to count their bits.
    f.magnitude_bits()
}

/// Unified internal-computation-cost ceilings (CS5).
///
/// This deliberately does **not** include the execution-step budget, which
/// remains the adjacent `Interpreter::max_execution_steps` field: the step
/// budget prices word count, whereas `RuntimeLimits` prices the per-word
/// internal work the step budget cannot see. The two together are the
/// interpreter's complete runtime-safety envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLimits {
    /// Max elements a single generative built-in may materialize in one call.
    pub max_materialized_elements: usize,
    /// Max byte length of a source program handed to `execute` (checked before
    /// tokenization).
    pub max_source_bytes: usize,
    /// Max digit count of a single numeric literal (checked before the BigInt
    /// parse builds the value).
    pub max_numeric_literal_digits: usize,
    /// Max accumulated internal numeric work units. Consumed by the work meter
    /// in the CS5 follow-up.
    pub max_numeric_work: u64,
    /// Max accumulated collection work units: the element copies, comparisons
    /// and equality probes performed *inside* collection Words, which the step
    /// budget cannot see because each such Word is one step however much it
    /// does.
    pub max_collection_work: u64,
    /// Max bit length of a BigInt arithmetic result. Consumed by the work meter
    /// in the CS5 follow-up.
    pub max_bigint_bits: u64,
    /// Max algebraic-term count of a single algebraic (normal-form)
    /// value. Consumed by the work meter in the CS5 follow-up.
    pub max_algebraic_terms: usize,
    /// Max container nesting of one value (`Value::nesting`). Every walk
    /// over a value — comparing, rendering, hashing, encoding, broadcasting —
    /// descends one native frame per level, so a value nested past what the
    /// host's call stack holds would abort the process instead of failing.
    pub max_nesting_depth: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_materialized_elements: DEFAULT_MAX_MATERIALIZED_ELEMENTS,
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
            max_numeric_literal_digits: DEFAULT_MAX_NUMERIC_LITERAL_DIGITS,
            max_numeric_work: DEFAULT_MAX_NUMERIC_WORK,
            max_collection_work: DEFAULT_MAX_COLLECTION_WORK,
            max_bigint_bits: DEFAULT_MAX_BIGINT_BITS,
            max_algebraic_terms: DEFAULT_MAX_ALGEBRAIC_TERMS,
            max_nesting_depth: DEFAULT_MAX_NESTING_DEPTH,
        }
    }
}

impl RuntimeLimits {
    /// Reject a source program larger than `max_source_bytes`, before
    /// tokenization. Returns a diagnosable `AjisaiError` (never a panic/OOM).
    pub fn check_source_bytes(&self, byte_len: usize) -> Result<()> {
        if byte_len > self.max_source_bytes {
            return Err(AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::SourceBytes,
                limit: self.max_source_bytes as u64,
                observed: Some(byte_len as u64),
                progress: None,
            });
        }
        Ok(())
    }

    /// Reject a value nested more than `max_nesting_depth` containers deep.
    pub fn check_nesting_depth(&self, nesting: usize) -> Result<()> {
        if nesting > self.max_nesting_depth {
            return Err(AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::NestingDepth,
                limit: self.max_nesting_depth as u64,
                observed: Some(nesting as u64),
                progress: None,
            });
        }
        Ok(())
    }

    /// Reject a numeric literal with more than `max_numeric_literal_digits`
    /// digits, before the BigInt parse builds the value. `digit_len` counts
    /// digit characters only (sign, radix point, and separators excluded).
    pub fn check_numeric_literal_digits(&self, digit_len: usize) -> Result<()> {
        if digit_len > self.max_numeric_literal_digits {
            return Err(AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::NumericLiteralDigits,
                limit: self.max_numeric_literal_digits as u64,
                observed: Some(digit_len as u64),
                progress: None,
            });
        }
        Ok(())
    }

    /// Reject an exact (Tier 1 algebraic) arithmetic result whose size crosses
    /// the internal-computation ceilings: `term_count` past
    /// `max_algebraic_terms` (multiplicative term explosion, e.g. repeatedly
    /// multiplying distinct `√p`), or `coeff_bits` past `max_bigint_bits`
    /// (BigInt blow-up). Bounds *accumulation* so operands feeding the next
    /// operation stay sane; the per-operation work is bounded separately by the
    /// work meter's pre-charge. Each ceiling reports itself by name through
    /// `ResourceLimitExceeded`: "this value carries too many algebraic terms"
    /// and "this coefficient is too many bits wide" are separately declared
    /// limits, and folding both into the step budget's category left a host
    /// unable to say which of the two an agent had actually hit.
    pub fn check_algebraic_size(&self, term_count: usize, coeff_bits: u64) -> Result<()> {
        if term_count > self.max_algebraic_terms {
            return Err(AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::AlgebraicTerms,
                limit: self.max_algebraic_terms as u64,
                observed: Some(term_count as u64),
                progress: None,
            });
        }
        self.check_bigint_bits(coeff_bits)
    }

    /// Reject an arithmetic result wider than `max_bigint_bits`.
    ///
    /// Split out of `check_algebraic_size` because it is not an algebraic
    /// concern: a plain rational product blows a coefficient up exactly the
    /// same way, and used to reach this ceiling through no path at all. A
    /// repeated-multiplication chain was bounded only by the *step* budget,
    /// which prices word count — so four hundred multiplications, 0.4% of that
    /// budget, could spend tens of seconds building a multi-megabyte integer
    /// while every size ceiling stayed silent.
    pub fn check_bigint_bits(&self, bits: u64) -> Result<()> {
        if bits > self.max_bigint_bits {
            return Err(AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::BigintBits,
                limit: self.max_bigint_bits,
                observed: Some(bits),
                progress: None,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_byte_ceiling_fires_at_a_low_injected_limit() {
        let limits = RuntimeLimits {
            max_source_bytes: 8,
            ..RuntimeLimits::default()
        };
        assert!(
            limits.check_source_bytes(8).is_ok(),
            "at the limit is allowed"
        );
        let err = limits
            .check_source_bytes(9)
            .expect_err("over the limit must error");
        assert!(err.to_string().contains("exceeds the limit"));
    }

    #[test]
    fn numeric_literal_digit_ceiling_fires_at_a_low_injected_limit() {
        let limits = RuntimeLimits {
            max_numeric_literal_digits: 4,
            ..RuntimeLimits::default()
        };
        assert!(limits.check_numeric_literal_digits(4).is_ok());
        let err = limits
            .check_numeric_literal_digits(5)
            .expect_err("over the digit limit must error");
        assert!(err.to_string().contains("exceeds the limit"));
    }
}

#[cfg(kani)]
#[path = "runtime_limits_proofs.rs"]
mod proofs;
