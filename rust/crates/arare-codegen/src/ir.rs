//! The intermediate representation (AR-LLR-024 and AR-LLR-029, refining
//! AR-HLR-030): a program, for one signature, as a graph of typed nodes.
//!
//! Every node has the width and scale the kernel gives its result — computed
//! by the kernel's own operations on placeholder numbers, so that the two
//! cannot drift — and a bound on the magnitude of its code on the fast path,
//! where every input is a finite number and no lane is NIL. A node either
//! varies lane by lane (`lane`) or is one value; a uniform node computed
//! from a reduction is `late`, known only after the loop.

use arare_kernel::Width;

/// A node, by its index in [`Ir::nodes`].
pub type N = usize;

/// What a node computes, on codes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    /// Scalar input `i`.
    Arg(usize),
    /// The lane of column input `i`.
    Lane(usize),
    /// A finite literal's code.
    Const(i64),
    Bool(bool),
    /// `(a << da) + (b << db)`: ADD after aligning to the smaller exponent.
    Add {
        a: N,
        b: N,
        da: u32,
        db: u32,
    },
    /// `(a << da) − (b << db)`: SUB.
    Sub {
        a: N,
        b: N,
        da: u32,
        db: u32,
    },
    Neg(N),
    Mul(N, N),
    /// `round((a << p) / (b << q))`, half to even: DIV at the dividend's scale.
    Div {
        a: N,
        b: N,
        p: u32,
        q: u32,
    },
    /// RESCALE by `s` bits.
    Rescale(N, u32),
    /// QUANTIZE to an exponent `s` above the operand's: RESCALE, then a fit.
    Up(N, u32),
    /// QUANTIZE to an exponent `s` below the operand's: an exact shift, then a fit.
    Down(N, u32),
    /// NARROW: a fit into the node's width.
    Narrow(N),
    /// `(a << da) < (b << db)`.
    Lt {
        a: N,
        b: N,
        da: u32,
        db: u32,
    },
    /// `(a << da) == (b << db)`.
    Eq {
        a: N,
        b: N,
        da: u32,
        db: u32,
    },
    And(N, N),
    Not(N),
    Select {
        then: N,
        otherwise: N,
        cond: N,
    },
    /// SUM of a lane node over the loop.
    Sum(N),
    /// FOLD's accumulator inside its block: the value carried from the
    /// previous lane, `init` before the first.
    Acc,
    /// FOLD's result after the loop: `acc` starts at `init`, and each lane
    /// replaces it with `step`, the block's result.
    Fold {
        acc: N,
        init: N,
        step: N,
    },
}

/// A node's type: a number of a width and scale, or a truth value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Num { width: Width, scale: i32 },
    Bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Node {
    pub op: Op,
    pub kind: Kind,
    /// The largest magnitude the node's code takes on the fast path, before
    /// any fit; 0 for a truth value.
    pub bound: u128,
    /// Whether the code may leave the node's width: then the fast path checks
    /// it, and a code that does not fit sends the program to the interpreter.
    pub check: bool,
    pub lane: bool,
    pub late: bool,
}

/// A value the program leaves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// One value.
    Scalar(N),
    /// A vector, lane by lane.
    Vector(N),
}

/// A compiled program's inputs, bottom of the stack first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Input {
    Scalar { width: Width, scale: i32 },
    Vector { width: Width, scale: i32 },
}

impl Input {
    /// A signature written as `v8:0 s32:-30 …`: `v` a vector, `s` a scalar,
    /// then the width in bits and the scale exponent.
    pub fn parse_list(text: &str) -> Result<Vec<Input>, String> {
        text.split_whitespace()
            .map(|t| {
                let bad = || format!("`{t}` is not an input such as v8:0 or s32:-30");
                let (shape, rest) = t.split_at_checked(1).ok_or_else(bad)?;
                let (bits, scale) = rest.split_once(':').ok_or_else(bad)?;
                let width = Width::ALL
                    .into_iter()
                    .find(|w| w.bits().to_string() == bits)
                    .ok_or_else(bad)?;
                let scale = scale.parse::<i32>().map_err(|_| bad())?;
                match shape {
                    "v" => Ok(Input::Vector { width, scale }),
                    "s" => Ok(Input::Scalar { width, scale }),
                    _ => Err(bad()),
                }
            })
            .collect()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Ir {
    pub inputs: Vec<Input>,
    pub nodes: Vec<Node>,
    pub outputs: Vec<Output>,
    /// Every SUM.
    pub sums: Vec<Sum>,
}

/// A SUM: its node, the lane node it sums, and the width of the
/// interpreter's running sum after 1, 2, … lanes, up to the width it keeps
/// from then on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sum {
    pub node: N,
    pub elem: N,
    pub widths: Vec<Width>,
}

impl Op {
    /// The nodes this one reads; each comes before it.
    pub fn operands(self) -> Vec<N> {
        match self {
            Op::Arg(_) | Op::Lane(_) | Op::Const(_) | Op::Bool(_) | Op::Acc => vec![],
            Op::Fold { init, step, .. } => vec![init, step],
            Op::Add { a, b, .. }
            | Op::Sub { a, b, .. }
            | Op::Div { a, b, .. }
            | Op::Lt { a, b, .. }
            | Op::Eq { a, b, .. }
            | Op::Mul(a, b)
            | Op::And(a, b) => vec![a, b],
            Op::Neg(a)
            | Op::Rescale(a, _)
            | Op::Up(a, _)
            | Op::Down(a, _)
            | Op::Narrow(a)
            | Op::Not(a)
            | Op::Sum(a) => {
                vec![a]
            }
            Op::Select {
                then,
                otherwise,
                cond,
            } => vec![then, otherwise, cond],
        }
    }
}
