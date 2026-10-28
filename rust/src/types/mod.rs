mod dense_columns;
mod lehmer_gcd;
mod small_divisor;
pub(crate) use self::dense_columns::ScalarColumns;
#[cfg(test)]
mod dense_columns_tests;
pub mod display;
pub mod exact;
pub mod fraction;
pub(crate) mod fraction_arithmetic;
#[cfg(test)]
mod fraction_arithmetic_tests;
mod fraction_extended;
#[cfg(test)]
mod fraction_gcd_tests;
#[cfg(test)]
mod fraction_lowest_terms_tests;
#[cfg(test)]
mod fraction_mcdc_tests;
mod fraction_order;
mod fraction_word_term;
pub(crate) mod group_key;
pub mod record;
pub(crate) mod small_rational;
pub mod stack;
mod value_absence;
mod value_children;
mod value_identity;
mod value_tensor;
// The lossless persistence codec is consumed only by the wasm boundary
// (`snapshot_stack` / `restore_stack_snapshot`) and by its own native property
// tests. Gating it on `any(test, feature = "wasm")` keeps a plain native build
// free of dead code while still running the round-trip tests under `cargo test`.
#[cfg(test)]
mod number_literal_tests;
#[cfg(test)]
mod tensor_lanes_tests;
mod tensor_storage;
#[cfg(test)]
mod tensor_storage_tests;
#[cfg(test)]
mod value_hash_tests;
#[cfg(test)]
mod value_layout_tests;
#[cfg(any(test, feature = "wasm"))]
pub(crate) mod value_persist;
#[cfg(test)]
mod value_persist_tests;
pub(crate) mod value_protocol;
#[cfg(test)]
mod value_protocol_tests;

mod token_text;
pub use self::token_text::{Name, NumberLiteral};
pub(crate) mod truth_column;
pub use self::truth_column::TruthColumn;

use self::fraction::Fraction;
pub use self::record::{RecordBuildError, RecordData};
pub use self::stack::Stack;
pub use self::tensor_storage::{Column, DenseTensor, Dims};
use self::value_identity::{dense_flatten, tensor_eq_vector};
use crate::semantic::{AbsenceMetadata, NilAbsence};
use crate::types::exact::ExactReal;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ValueData {
    /// A definite logical truth value, `true` or `false`
    /// (LANG.VALUES.TRUTH). A Boolean is a data-plane value distinct from any
    /// number: `TRUE` is not the scalar `1` and `FALSE` is not the scalar `0`,
    /// so `TRUE 1 EQ` is false.
    Boolean(bool),
    Scalar(Fraction),
    /// An exact real value outside the rationals: an algebraic normal form
    /// over `SQRT` (`Σ cᵢ√mᵢ`). Constructed only by
    /// `Value::from_exact_real`; use `as_scalar()` for the rational fast path.
    /// Boxed: it is the rare value, and inline it would widen every one
    /// (`value_layout_tests`).
    ExactScalar(Box<ExactReal>),
    Vector(Arc<Vec<Value>>),
    Tensor {
        data: Arc<DenseTensor>,
        shape: Arc<Vec<usize>>,
    },
    /// An absent value, with why (LANG.VALUES.NIL), held on the one variant
    /// that has a reason rather than beside every value (`value_layout_tests`).
    /// A quotient by zero is a Scalar over zero.
    Nil(NilAbsence),
    /// A bare name in code position — a Word reference that is data until
    /// something executes it, a value in its own right (LANG.VALUES.DISJOINT).
    Symbol(Arc<str>),
    /// A String: a sequence of Unicode scalar values (LANG.VALUES.DISJOINT).
    /// Holding the content directly makes the domain the tag, so
    /// `'A' [ 65 ] EQ` is false.
    Text(Arc<str>),
    /// A Record: an insertion-ordered keyed correspondence, the seventh
    /// disjoint domain (LANG.RECORDS.STRUCTURE). It is not a Vector — no
    /// Vector Word accepts it and no Record Word accepts a Vector — and it
    /// has no literal: `RECORD` is its only constructor.
    Record(Arc<RecordData>),
    /// A flat Vector of truth values held a byte a lane (`truth_column`): a
    /// representation of the Vector it stands for, equal to it and read as
    /// it by everything that does not read the bytes directly.
    Truths(Arc<TruthColumn>),
}

impl PartialEq for ValueData {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValueData::Boolean(a), ValueData::Boolean(b)) => a == b,
            (ValueData::Scalar(a), ValueData::Scalar(b)) => a == b,
            (ValueData::ExactScalar(a), ValueData::ExactScalar(b)) => a == b,
            (ValueData::Vector(a), ValueData::Vector(b)) => a == b,
            (
                ValueData::Tensor {
                    data: a_data,
                    shape: a_shape,
                },
                ValueData::Tensor {
                    data: b_data,
                    shape: b_shape,
                },
            ) => a_data == b_data && a_shape == b_shape,
            (ValueData::Vector(v), ValueData::Tensor { data, shape })
            | (ValueData::Tensor { data, shape }, ValueData::Vector(v)) => {
                tensor_eq_vector(data, shape, v)
            }
            (ValueData::Nil(_), ValueData::Nil(_)) => true,
            // A Symbol equals a Symbol with the same name, and nothing else —
            // in particular not a Text of the same spelling: `[ FOO ] { FOO }
            // EQ` and `[ FOO ] [ 'FOO' ] EQ` are both false (LANG.VALUES.
            // DISJOINT).
            (ValueData::Symbol(a), ValueData::Symbol(b)) => a == b,
            // A String equals a String with the same codepoints, and nothing
            // else. It never meets the Vector/Tensor arms above, so `'A'` and
            // `[ 65 ]` now fall to the catch-all and answer false, as
            // LANG.VALUES.DISJOINT requires.
            (ValueData::Text(a), ValueData::Text(b)) => a == b,
            // A Record equals a Record with the same key sequence and the
            // same value sequence (LANG.VALUES.DENOTATION), and nothing else.
            (ValueData::Record(a), ValueData::Record(b)) => a == b,
            (ValueData::Truths(a), ValueData::Truths(b)) => a == b,
            (ValueData::Truths(t), other) | (other, ValueData::Truths(t)) => {
                ValueData::Vector(Arc::new(t.to_values())) == *other
            }
            _ => false,
        }
    }
}

/// Discriminant tags for [`ValueData`]'s `Hash`. `DENSE` is shared by
/// `Vector` and `Tensor` deliberately: it is what makes a rectangular
/// numeric `Vector` hash the same as the `Tensor` holding the same data,
/// matching the cross-representation equality `value_identity` grants
/// them. `STRUCT_VECTOR` is everything else — a `Vector` that is not a
/// dense-tensor lookalike, hashed structurally instead.
const HASH_TAG_BOOLEAN: u8 = 0;
const HASH_TAG_SCALAR: u8 = 1;
const HASH_TAG_EXACT_SCALAR: u8 = 2;
const HASH_TAG_DENSE: u8 = 3;
const HASH_TAG_STRUCT_VECTOR: u8 = 4;
const HASH_TAG_NIL: u8 = 5;
const HASH_TAG_SYMBOL: u8 = 6;
const HASH_TAG_TEXT: u8 = 7;
const HASH_TAG_RECORD: u8 = 8;

impl std::hash::Hash for ValueData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ValueData::Boolean(b) => {
                state.write_u8(HASH_TAG_BOOLEAN);
                b.hash(state);
            }
            ValueData::Scalar(f) => {
                state.write_u8(HASH_TAG_SCALAR);
                f.hash(state);
            }
            ValueData::ExactScalar(e) => {
                state.write_u8(HASH_TAG_EXACT_SCALAR);
                e.hash(state);
            }
            ValueData::Vector(v) => match dense_flatten(v) {
                Some((shape, leaves)) => {
                    state.write_u8(HASH_TAG_DENSE);
                    shape.hash(state);
                    leaves.hash(state);
                }
                None => {
                    state.write_u8(HASH_TAG_STRUCT_VECTOR);
                    v.hash(state);
                }
            },
            ValueData::Truths(t) => ValueData::Vector(Arc::new(t.to_values())).hash(state),
            ValueData::Tensor { data, shape } => {
                state.write_u8(HASH_TAG_DENSE);
                shape.hash(state);
                let leaves: Vec<Fraction> = data.to_fractions();
                leaves.hash(state);
            }
            ValueData::Nil(_) => state.write_u8(HASH_TAG_NIL),
            ValueData::Symbol(name) => {
                state.write_u8(HASH_TAG_SYMBOL);
                name.hash(state);
            }
            ValueData::Text(s) => {
                state.write_u8(HASH_TAG_TEXT);
                s.hash(state);
            }
            ValueData::Record(record) => {
                state.write_u8(HASH_TAG_RECORD);
                record.hash(state);
            }
        }
    }
}

#[derive(Debug)]
pub struct Value {
    pub data: ValueData,
    /// How many containers deep this value nests, computed once when the
    /// value is built (`Value::new`). Private so that no value can be built
    /// around it: every construction goes through `Value::new` (or
    /// `Value::leaf`, whose nesting is known without asking), and every
    /// in-place change of `data` recomputes it (`value_children.rs`).
    nesting: u32,
}

impl Value {
    /// A value from its parts. The one general constructor: every other one
    /// but `leaf` goes through here, so a value's nesting is always the one
    /// its data has.
    #[inline]
    pub fn new(data: ValueData, absence: Option<AbsenceMetadata>) -> Self {
        let data = match (data, absence) {
            (ValueData::Nil(_), Some(absence)) => ValueData::Nil(NilAbsence::of(absence)),
            (data, absence) => {
                debug_assert!(absence.is_none(), "only a NIL carries an absence");
                data
            }
        };
        let nesting = data.nesting();
        Self { data, nesting }
    }

    /// How many containers deep this value nests: 0 for anything that is not
    /// a container, and one more than its deepest element for a Vector or a
    /// Record (a dense tensor nests as deep as its rank). This is what
    /// LANG.MACHINE.LIMITS' nesting ceiling bounds; it counts Records, which
    /// `DEPTH` does not, because every walk over a value descends into both.
    ///
    /// Constant time: the figure is kept on the value, so checking a value
    /// against the ceiling never walks it.
    #[inline]
    pub fn nesting(&self) -> u32 {
        self.nesting
    }

    /// A Vector of Booleans, one per truth, held as a truth column
    /// (`truth_column`): a byte a lane rather than a `Value`. Built for
    /// comparisons over a whole Tensor (`dense_kernels`), whose answers run
    /// to millions of lanes.
    pub(crate) fn from_truths(truths: impl Iterator<Item = bool>) -> Self {
        let codes = truths.map(u8::from).collect();
        Self::from_truth_column(TruthColumn::known(codes))
    }

    /// Raise the kept nesting to at least `nesting`, after a child was added
    /// in place.
    pub(crate) fn raise_nesting_to(&mut self, nesting: u32) {
        self.nesting = self.nesting.max(nesting);
    }

    /// Recompute the nesting after `data` was changed in place.
    pub(crate) fn refresh_nesting(&mut self) {
        self.nesting = self.data.nesting();
    }
}

impl ValueData {
    /// The nesting of a value holding this data, from its children's kept
    /// figures — one level of work, never a walk.
    fn nesting(&self) -> u32 {
        let deepest = |children: &mut dyn Iterator<Item = &Value>| {
            children.map(Value::nesting).max().unwrap_or(0)
        };
        match self {
            ValueData::Vector(children) => deepest(&mut children.iter()).saturating_add(1),
            ValueData::Record(record) => {
                deepest(&mut record.keys().iter().chain(record.values().iter())).saturating_add(1)
            }
            ValueData::Tensor { shape, .. } => u32::try_from(shape.len()).unwrap_or(u32::MAX),
            ValueData::Truths(_) => 1,
            ValueData::Boolean(_)
            | ValueData::Scalar(_)
            | ValueData::ExactScalar(_)
            | ValueData::Nil(_)
            | ValueData::Symbol(_)
            | ValueData::Text(_) => 0,
        }
    }
}

impl PartialEq for Value {
    /// Value equality, which must agree with the `EQ` Word: this relation is
    /// what decides nested elements once `EQ` reaches a Vector, so a
    /// disagreement between the two is observable from the language.
    ///
    /// The NIL reason is part of it. LANG.VALUES.NIL makes the reason "the
    /// entire observable content of a NIL", so two NILs are the same value
    /// exactly when their reasons agree. `ValueData`'s own equality ignores
    /// the reason a `Nil` holds, so comparing `data` alone decides every pair
    /// of NILs equal — it once did here, and `[ NIL ] [ 'x' ] [ NUM ] MAP EQ` answered TRUE for a
    /// `literal` absence against an `invalidEncoding` one.
    ///
    /// Nothing else is: a value carries no record of how it was made
    /// (LANG.VALUES.DENOTATION), so two values with the same data and the same
    /// NIL reason are the same value.
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.nil_reason() == other.nil_reason()
            && self.absence_detail() == other.absence_detail()
    }
}

/// No value here is ever NaN-like (every numeric tier is exact), so `eq` is
/// a true equivalence relation and this marker is safe.
impl Eq for Value {}

/// Hashes exactly what `PartialEq` compares — `data`, then the NIL reason and
/// the detail a `userDeclared` reason carries —
/// and nothing `PartialEq` does not. Required
/// for `UNIQUE` / `TALLY` / `GROUP` to key a `HashMap<Value, _>` rather than
/// re-scan the accumulated result for every element (CS5 collection-word
/// de-quadraticization).
impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.data.hash(state);
        self.nil_reason().hash(state);
        self.absence_detail().hash(state);
    }
}

#[derive(Debug, PartialEq, Hash)]
pub enum Token {
    Number(NumberLiteral),
    String(Arc<str>),
    Symbol(Name),
    VectorStart,
    VectorEnd,
    /// A value carried into a token sequence whole. Source never produces
    /// one: it is how the bridge that runs a Vector as code
    /// (`interpreter::value_as_code`) hands over an element no source text
    /// denotes — a Record, a NIL together with its reason, an exact
    /// irrational — so running the Vector pushes that element unchanged.
    Value(Box<Value>),
}

/// Written out rather than derived so that it inlines: the tokenizer copies a
/// token for every repeated lexeme, and the derived copy was an out-of-line
/// call per token.
impl Clone for Token {
    #[inline(always)]
    fn clone(&self) -> Self {
        match self {
            Token::Number(literal) => Token::Number(literal.clone()),
            Token::String(text) => Token::String(Arc::clone(text)),
            Token::Symbol(name) => Token::Symbol(name.clone()),
            Token::VectorStart => Token::VectorStart,
            Token::VectorEnd => Token::VectorEnd,
            Token::Value(value) => Token::Value(value.clone()),
        }
    }
}

impl Token {
    /// A `Number` token from a lexeme, parsed once here.
    pub fn number(lexeme: &str) -> Self {
        Token::Number(NumberLiteral::from_lexeme(lexeme))
    }

    /// A `Number` token for a rational already in hand — see
    /// [`NumberLiteral::from_value`].
    pub(crate) fn number_from_value(value: Fraction) -> Self {
        Token::Number(NumberLiteral::from_value(value))
    }
}

#[derive(Debug, Clone)]
pub struct WordDefinition {
    /// The body's tokens, in order. Whitespace, line breaks included, is not
    /// part of it (LANG.SOURCE.TEXT).
    pub body: Arc<[Token]>,
    pub is_builtin: bool,
    pub description: Option<String>,
    pub dependencies: HashSet<String>,
    /// Every uppercased Symbol name written anywhere in the body — including
    /// one that does not currently resolve to anything, unlike
    /// `dependencies`. A forward reference to a not-yet-defined word is
    /// absent from `dependencies` until that word is defined (`DEF` adds the
    /// edge then) but must be visible here from the start, or a two-step
    /// mutual recursion (define A naming B, then define B naming A) would slip
    /// past the DEF-time acyclicity check that `dependencies` alone cannot
    /// support.
    pub text_references: HashSet<String>,
    pub registration_order: u64,
    /// The body compiled for the engine, on its first call
    /// (`Interpreter::body_code`).
    pub(crate) code: std::sync::OnceLock<Arc<crate::interpreter::engine::Code>>,
    /// The generated registry entry for a Core Word, held rather than looked up
    /// again.
    ///
    /// `None` for a User Word. Dispatch resolves a name to this definition and
    /// then, for a Core Word, used to find the *same* Word a second time by
    /// string: `generated_word(name)` is a linear scan of the 65-entry registry
    /// comparing names, and it ran once per element — 220,000 `memcmp` calls and
    /// 6.25% of the instructions of a 20,000-element `[ ABS ] MAP`, to re-answer
    /// a question resolution had already answered. `builtins::register` has the
    /// entry in hand when it builds each definition, so it puts it here.
    pub generated: Option<&'static crate::kernel::generated::GeneratedWord>,
}
