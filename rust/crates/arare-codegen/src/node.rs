//! Emitting one node (AR-LLR-025, AR-LLR-026): the `let` that computes it in
//! the integer type its bound needs, and the check that sets `trap` when
//! its code can leave its width.

use crate::emit::rty;
use crate::ir::{Ir, Kind, Op, N};
use crate::rt;
use std::fmt::Write;

pub(crate) struct Emit<'a> {
    pub(crate) ir: &'a Ir,
    pub(crate) out: String,
}

impl Emit<'_> {
    pub(crate) fn line(&mut self, indent: usize, text: &str) {
        let _ = writeln!(self.out, "{}{}", "    ".repeat(indent), text);
    }

    /// The type a node's value is held in.
    pub(crate) fn ty(&self, n: N) -> &'static str {
        match self.ir.nodes[n].kind {
            Kind::Bool => "bool",
            Kind::Num { .. } => rty(self.ir.nodes[n].bound),
        }
    }

    /// A node's bound once its check has passed: within its width.
    pub(crate) fn bound(&self, n: N) -> u128 {
        self.ir.nodes[n].bound.min(self.max(n))
    }

    pub(crate) fn max(&self, n: N) -> u128 {
        match self.ir.nodes[n].kind {
            Kind::Num { width, .. } => width.max_finite() as u128,
            Kind::Bool => 0,
        }
    }

    /// A fit check: the code must lie in the node's finite range.
    pub(crate) fn fit(&mut self, indent: usize, n: N) {
        let node = self.ir.nodes[n];
        if node.bound > self.max(n) {
            let m = self.max(n);
            self.line(
                indent,
                &format!("trap |= (v{n} < -{m} || v{n} > {m}) as u8;"),
            );
        }
    }

    pub(crate) fn node(&mut self, indent: usize, n: N) {
        let node = self.ir.nodes[n];
        let t = self.ty(n);
        let shl = |x: N, s: u32, c: &str| {
            if s == 0 {
                format!("(v{x} as {c})")
            } else {
                format!("((v{x} as {c}) << {s})")
            }
        };
        let expr = match node.op {
            Op::Arg(i) => format!("s{i} as {t}"),
            Op::Lane(i) => format!("x{i}[i] as {t}"),
            Op::Const(c) => format!("{c} as {t}"),
            Op::Bool(b) => format!("{b}"),
            Op::Add { a, b, da, db } => format!("{} + {}", shl(a, da, t), shl(b, db, t)),
            Op::Sub { a, b, da, db } => format!("{} - {}", shl(a, da, t), shl(b, db, t)),
            Op::Neg(a) => format!("-(v{a} as {t})"),
            Op::Mul(a, b) => format!("(v{a} as {t}) * (v{b} as {t})"),
            Op::Div { a, b, p, q } => {
                let dividend = self.bound(a).saturating_mul(1u128 << p);
                let divisor = self.bound(b).saturating_mul(1u128 << q);
                let c = rty(dividend.max(divisor));
                self.line(indent, &format!("let d{n} = {};", shl(b, q, c)));
                self.line(indent, &format!("trap |= (d{n} == 0) as u8;"));
                self.line(
                    indent,
                    &format!("let d{n} = if d{n} == 0 {{ 1 }} else {{ d{n} }};"),
                );
                if dividend < rt::DIV_F64_DIVIDEND && divisor < rt::DIV_F64_DIVISOR {
                    format!("rt::div_f64({} as f64, d{n} as f64) as {t}", shl(a, p, c))
                } else {
                    format!(
                        "rt::round_div({} as i128, d{n} as i128) as {t}",
                        shl(a, p, c)
                    )
                }
            }
            Op::Rescale(a, s) | Op::Up(a, s) => {
                if self.bound(a) <= i32::MAX as u128 {
                    format!("rt::rescale_i32(v{a} as i32, {s}) as {t}")
                } else {
                    format!("rt::rescale_code(v{a} as i64, {s}) as {t}")
                }
            }
            Op::Down(a, s) => shl(a, s, t),
            Op::Narrow(a) => format!("v{a} as {t}"),
            Op::Lt { a, b, da, db } | Op::Eq { a, b, da, db } => {
                let sym = if matches!(node.op, Op::Lt { .. }) {
                    "<"
                } else {
                    "=="
                };
                if self.ir.nodes[a].kind == Kind::Bool {
                    format!("v{a} {sym} v{b}")
                } else {
                    let c = rty((self.bound(a).saturating_mul(1u128 << da))
                        .max(self.bound(b).saturating_mul(1u128 << db)));
                    format!("{} {sym} {}", shl(a, da, c), shl(b, db, c))
                }
            }
            Op::And(a, b) => format!("v{a} & v{b}"),
            Op::Not(a) => format!("!v{a}"),
            Op::Select {
                then,
                otherwise,
                cond,
            } => {
                if node.kind == Kind::Bool {
                    format!("if v{cond} {{ v{then} }} else {{ v{otherwise} }}")
                } else {
                    format!("if v{cond} {{ v{then} as {t} }} else {{ v{otherwise} as {t} }}")
                }
            }
            // The loop has summed the lanes into `t{n}`.
            Op::Sum(_) => format!("t{n} as {t}"),
        };
        self.line(indent, &format!("let v{n}: {t} = {expr};"));
        self.fit(indent, n);
    }
}
