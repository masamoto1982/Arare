//! The operands and the result of a column kernel: an operand's lanes read
//! straight off a Tensor's columns (or a column the walk computed) or
//! splatted from a scalar, and the result columns. A child module of
//! `dense_kernels` so the kernels themselves read as the laws they compute.

use crate::types::{DenseTensor, Value, ValueData};
use std::sync::Arc;

/// An operand's lanes: columns, or one rational for every lane.
///
/// The columns are held as plain slices, taken off the Tensor once: indexing
/// the `SmallVec` columns themselves re-tests whether they are inline on every
/// lane, which kept the kernels from vectorising and cost more than the
/// arithmetic.
#[derive(Clone, Copy)]
pub(crate) enum Lanes<'a> {
    Columns {
        nums: &'a [i64],
        /// Empty when every lane is an integer and the columns were computed
        /// rather than read off a Tensor: the 1s are implicit.
        dens: &'a [i64],
        /// Every lane is an integer, so the integer kernels apply to the
        /// numerators alone.
        integer: bool,
    },
    Splat(i64, i64),
}

impl Lanes<'_> {
    pub(crate) fn of(value: &Value) -> Option<Lanes<'_>> {
        match &value.data {
            ValueData::Tensor { data, shape } if shape.len() == 1 && !data.is_empty() => {
                Some(Lanes::Columns {
                    nums: &data.numerators,
                    dens: &data.denominators,
                    integer: data.is_pure_integer,
                })
            }
            ValueData::Scalar(f) => {
                let (n, d) = f.extract_i64_pair()?;
                Some(Lanes::Splat(n, d))
            }
            _ => None,
        }
    }

    pub(crate) fn len(self) -> Option<usize> {
        match self {
            Lanes::Columns { nums, .. } => Some(nums.len()),
            Lanes::Splat(..) => None,
        }
    }

    pub(crate) fn integer(self) -> bool {
        match self {
            Lanes::Columns { integer, .. } => integer,
            Lanes::Splat(_, d) => d == 1,
        }
    }

    #[inline(always)]
    pub(crate) fn at(self, i: usize) -> (i64, i64) {
        match self {
            Lanes::Columns {
                nums,
                integer: true,
                ..
            } => (nums[i], 1),
            Lanes::Columns { nums, dens, .. } => (nums[i], dens[i]),
            Lanes::Splat(n, d) => (n, d),
        }
    }

    /// The numerators of `n` lanes as one slice: a column's own, or a
    /// constant written out into `buf`, so a kernel's loop is a plain zip.
    #[inline(always)]
    pub(crate) fn nums<'b>(self, n: usize, buf: &'b mut Vec<i64>) -> &'b [i64]
    where
        Self: 'b,
    {
        match self {
            Lanes::Columns { nums, .. } => &nums[..n],
            Lanes::Splat(v, _) => {
                buf.clear();
                buf.resize(n, v);
                buf
            }
        }
    }
}

/// The lane count two operands pair over: a Tensor's length against a
/// scalar, or the shared length of two Tensors. Anything else — two scalars,
/// two lengths — is the general route's.
pub(crate) fn paired(a: Lanes, b: Lanes) -> Option<usize> {
    match (a.len(), b.len()) {
        (Some(n), None) | (None, Some(n)) => Some(n),
        (Some(n), Some(m)) if n == m => Some(n),
        _ => None,
    }
}

/// A kernel's result columns, built as plain `Vec`s (a push into a
/// `SmallVec` re-tests whether it has spilled) and handed to the Tensor
/// without a copy. A walk keeps one per register and clears it per chunk.
///
/// While every lane is an integer, `dens` stays empty: the 1s are written
/// only when a fraction joins them, or when the Tensor is built.
pub(crate) struct Out {
    pub(crate) nums: Vec<i64>,
    pub(crate) dens: Vec<i64>,
    pub(crate) integer: bool,
}

impl Default for Out {
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

impl Out {
    pub(crate) fn with_capacity(n: usize) -> Self {
        Self {
            nums: Vec::with_capacity(n),
            dens: Vec::with_capacity(n),
            integer: true,
        }
    }

    /// Room for `n` lanes in both columns.
    pub(crate) fn reserve(&mut self, n: usize) {
        self.nums.reserve(n);
        self.dens.reserve(n);
    }

    pub(crate) fn clear(&mut self) {
        self.nums.clear();
        self.dens.clear();
        self.integer = true;
    }

    /// Integer lanes already written to `nums`.
    pub(crate) fn integers(&mut self) {
        self.dens.clear();
        self.integer = true;
    }

    /// Both columns already written, in lowest terms.
    pub(crate) fn settle(&mut self) {
        if self.dens.iter().all(|&d| d == 1) {
            self.integers();
        } else {
            self.integer = false;
        }
    }

    /// The implicit 1s written out, before a fraction joins them.
    #[cold]
    fn fractional(&mut self) {
        self.dens.clear();
        self.dens.resize(self.nums.len(), 1);
        self.integer = false;
    }

    /// A lane appended to both columns, for a kernel that settles the
    /// integer flag once at the end ([`Out::settle`]).
    #[inline(always)]
    pub(crate) fn push_pair(&mut self, num: i64, den: i64) {
        self.nums.push(num);
        self.dens.push(den);
    }

    #[inline(always)]
    pub(crate) fn push(&mut self, num: i64, den: i64) {
        if self.integer {
            if den == 1 {
                self.nums.push(num);
                return;
            }
            self.fractional();
        }
        self.nums.push(num);
        self.dens.push(den);
    }

    /// `n` lanes of `lanes` appended.
    pub(crate) fn extend(&mut self, lanes: Lanes, n: usize) {
        match lanes {
            Lanes::Columns {
                nums,
                integer: true,
                ..
            } => {
                if !self.integer {
                    self.dens.extend(std::iter::repeat_n(1, n));
                }
                self.nums.extend_from_slice(&nums[..n]);
            }
            Lanes::Columns { nums, dens, .. } => {
                if self.integer {
                    self.fractional();
                }
                self.nums.extend_from_slice(&nums[..n]);
                self.dens.extend_from_slice(&dens[..n]);
            }
            Lanes::Splat(num, den) => {
                if self.integer && den != 1 {
                    self.fractional();
                }
                self.nums.extend(std::iter::repeat_n(num, n));
                if !self.integer {
                    self.dens.extend(std::iter::repeat_n(den, n));
                }
            }
        }
    }

    pub(crate) fn lanes(&self) -> Lanes<'_> {
        Lanes::Columns {
            nums: &self.nums,
            dens: &self.dens,
            integer: self.integer,
        }
    }

    pub(crate) fn into_value(mut self) -> Value {
        if self.integer {
            self.dens.clear();
            self.dens.resize(self.nums.len(), 1);
        }
        let shape = vec![self.nums.len()];
        let tensor = DenseTensor::from_columns(self.nums, self.dens, shape.clone(), self.integer);
        Value::new(
            ValueData::Tensor {
                data: Arc::new(tensor),
                shape: Arc::new(shape),
            },
            None,
        )
    }
}
