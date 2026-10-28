//! Dense numeric tensor storage.
//!
//! Invariant: storage validity and shape are representation
//! concerns; semantic interpretation remains owned by `Value`. A dense tensor
//! holds numbers and nothing else — a NIL is not a number and never a lane
//! (`dense_columns`) — so the columns are the whole of what it stores.

mod lanes;

use super::fraction::Fraction;

/// A column of lanes, inline for up to four lanes, so building a short one
/// — a seed `[ 0 ]`, a row a nested walk reads out of a matrix — costs
/// nothing beyond the tensor; and a shape, inline for one axis.
pub type Column = smallvec::SmallVec<[i64; 4]>;
pub type Dims = smallvec::SmallVec<[usize; 1]>;

/// A dense numeric tensor in struct-of-arrays form.
///
/// Every lane is a reduced pair with a non-negative denominator, written from
/// a normalized `Fraction` and read back without re-deriving the normal form.
/// A denominator of 0 is a lane like any other: one of the two points over
/// zero (`fraction_extended`), `1/0` or `0/0`, whose numerator is 1 or 0.
/// Such a lane is not an integer lane, so a pure-integer tensor holds
/// none, and the integer kernels that read numerators alone never meet one.
///
/// Two tensors are one value exactly when their columns and shapes are, which
/// is why `PartialEq` is derived: reduced pairs compare as pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseTensor {
    pub numerators: Column,
    pub denominators: Column,
    pub shape: Dims,
    pub is_pure_integer: bool,
    /// Whether the lanes ascend, worked out the first time a search asks.
    pub(crate) ascending: Ascending,
}

/// A tensor's lanes never change once built, so whether they ascend is
/// decided once: `BSEARCH` lifted over its keys, or run again on a bound
/// table, reads the answer instead of re-reading every lane. It is part of
/// no value: two tensors compare by their lanes alone.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ascending(std::sync::OnceLock<bool>);

impl Ascending {
    pub(crate) fn get_or_init(&self, decide: impl FnOnce() -> bool) -> bool {
        *self.0.get_or_init(decide)
    }
}

impl PartialEq for Ascending {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Ascending {}

impl DenseTensor {
    /// Assemble a tensor from already-separated columns, each lane a reduced
    /// pair with a non-negative denominator.
    pub fn from_columns(
        numerators: impl Into<Column>,
        denominators: impl Into<Column>,
        shape: impl Into<Dims>,
        is_pure_integer: bool,
    ) -> Self {
        let (numerators, denominators) = (numerators.into(), denominators.into());
        debug_assert_eq!(numerators.len(), denominators.len());
        debug_assert!(
            !is_pure_integer || denominators.iter().all(|&d| d == 1),
            "a pure-integer tensor has denominator 1 in every lane"
        );
        Self {
            numerators,
            denominators,
            shape: shape.into(),
            is_pure_integer,
            ascending: Ascending::default(),
        }
    }

    /// Assemble a tensor from columns of *unverified* provenance.
    ///
    /// [`Self::from_columns`] takes its columns as already being in lowest
    /// terms with a non-negative denominator, which every in-process path
    /// satisfies because it writes lanes from `Fraction`s that have been
    /// normalized. A restored session is the one place that is not true: the
    /// columns come off the wire, so a payload could carry `4/2`, a negative
    /// denominator, or `100/0`, and nothing in the decode path looked. So the
    /// check runs here, once per lane per restore: every lane is reduced,
    /// one over zero to `1/0` or `0/0`.
    pub fn from_untrusted_columns(
        numerators: Vec<i64>,
        denominators: Vec<i64>,
        shape: impl Into<Dims>,
        is_pure_integer: bool,
    ) -> Self {
        let mut numerators = numerators;
        let mut denominators = denominators;
        for index in 0..numerators.len().min(denominators.len()) {
            let normalized =
                Fraction::create_from_i128(numerators[index] as i128, denominators[index] as i128);
            if let Some((n, d)) = normalized.extract_i64_pair() {
                numerators[index] = n;
                denominators[index] = d;
            }
        }
        // `is_pure_integer` is the caller's claim about the same columns, so it
        // is recomputed rather than believed: a payload claiming purity for a
        // lane like `1/2` would otherwise send every integer fast path down a
        // route its own guard had cleared.
        let is_pure_integer = is_pure_integer && denominators.iter().all(|&d| d == 1);
        Self::from_columns(numerators, denominators, shape, is_pure_integer)
    }

    /// Build from rationals. `None` when a lane does not fit two machine
    /// words, or when `shape` does not name exactly the lanes given.
    pub fn from_fractions(data: Vec<Fraction>, shape: Vec<usize>) -> Option<Self> {
        let expected_len = if shape.is_empty() {
            0
        } else {
            shape.iter().product()
        };
        if expected_len != data.len() {
            return None;
        }

        let mut numerators = Column::with_capacity(data.len());
        let mut denominators = Column::with_capacity(data.len());
        let mut is_pure_integer = true;
        for fraction in data {
            let (numerator, denominator) = fraction.extract_i64_pair()?;
            numerators.push(numerator);
            denominators.push(denominator);
            is_pure_integer &= denominator == 1;
        }

        Some(Self::from_columns(
            numerators,
            denominators,
            shape,
            is_pure_integer,
        ))
    }

    /// This flat buffer with its lanes in reverse order, columns and all.
    ///
    /// Rearranging lanes is a representation concern, so it happens here
    /// rather than by unpacking the tensor into boxed `Value`s, reversing
    /// those, and re-densifying: two columns of `i64` reverse in place.
    ///
    /// Flat buffers only: the caller checks rank, because reversing a rank-2
    /// tensor reverses its *rows*, and a row is a stride rather than a lane.
    pub fn reversed_lanes(&self) -> Self {
        let mut numerators = self.numerators.clone();
        numerators.reverse();
        let mut denominators = self.denominators.clone();
        denominators.reverse();
        Self::from_columns(
            numerators,
            denominators,
            self.shape.clone(),
            self.is_pure_integer,
        )
    }

    /// Build a 1-D pure-integer dense tensor directly from `i64` numerators,
    /// without routing through `Fraction`. The denominator is implicitly
    /// `1`, avoiding the `Vec<i64> → Vec<Fraction> → re-densify` round-trip
    /// (`Value::from_int_tensor`).
    pub fn from_integers(numerators: Vec<i64>) -> Self {
        let len = numerators.len();
        let denominators = smallvec::smallvec![1; len];
        Self {
            numerators: numerators.into(),
            denominators,
            shape: smallvec::smallvec![len],
            is_pure_integer: true,
            ascending: Ascending::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.numerators.len()
    }

    pub fn is_empty(&self) -> bool {
        self.numerators.is_empty()
    }

    /// `true` when every lane is a rational — no lane is one of the two
    /// points over zero. The pair kernels that compute on `(i64, i64)` pairs
    /// with a positive denominator ask this before borrowing the columns; a
    /// pure-integer tensor answers it without a scan.
    pub fn all_finite(&self) -> bool {
        self.is_pure_integer || !self.denominators.contains(&0)
    }

    pub fn iter(&self) -> impl Iterator<Item = Fraction> + '_ {
        (0..self.len()).map(|index| self.fraction_at(index))
    }

    /// The lane at `index` as the `Fraction` its columns denote.
    ///
    /// The columns are already in lowest terms with a non-negative
    /// denominator — see `from_columns` — so this reads them rather than
    /// re-deriving them. It used to go through `Fraction::new`, which widened
    /// both halves to `BigInt`, narrowed them straight back, and ran a
    /// Euclidean gcd to reach the normal form they were stored in: two
    /// allocations and a 128-bit division loop, per lane, every time a lane
    /// was read. Reading a lane is what `MAP`, `FILTER` and `FOLD` do once
    /// per element.
    pub fn fraction_at(&self, index: usize) -> Fraction {
        Fraction::from_normalized_pair(self.numerators[index], self.denominators[index])
    }

    pub fn to_fractions(&self) -> Vec<Fraction> {
        self.iter().collect()
    }
}
