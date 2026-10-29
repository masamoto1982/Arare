//! The checker printed over the grid of `formal/BudgetTable.lean`, for the
//! Lean job of CI to diff against the Lean checker (AR-LLR-019, AR-VER-215).

use crate::info::{self, Info};
use arare_kernel::{Num, Width};
use std::io::{self, Write};

fn show(i: Info) -> String {
    match i {
        Info::Top => "T".to_string(),
        Info::Inf => "I".to_string(),
        Info::Bot => "B".to_string(),
        Info::Num(r) => format!("N {} {} {} {} {}", r.lo, r.hi, r.err, r.smin, r.smax),
    }
}

/// A literal of the grid: the exact `p/q`, its width and exponent.
const LITS: [(i64, i64, Width, i32); 17] = [
    (0, 1, Width::W8, 0),
    (1, 1, Width::W8, 0),
    (-1, 1, Width::W8, 0),
    (3, 1, Width::W8, 0),
    (-7, 1, Width::W8, 0),
    (126, 1, Width::W8, 0),
    (40000, 1, Width::W32, 0),
    (1, 3, Width::W16, -8),
    (-5, 7, Width::W16, -4),
    (1, 10, Width::W16, -12),
    (5, 2, Width::W8, -1),
    (7, 4, Width::W8, 0),
    (1, 1000, Width::W8, 0),
    (3, 1, Width::W8, 2),
    (200, 1, Width::W8, 0),
    (1, 0, Width::W8, 0),
    (0, 0, Width::W8, 0),
];

fn literal((p, q, w, e): (i64, i64, Width, i32)) -> Info {
    let code = Num::from_ratio(p, q, w, e).expect("the grid's exponents are in range");
    info::lit(p, q, w, e, code.val())
}

/// Each literal under the seven unary Words, in the Lean grid's order.
fn level1() -> Vec<Info> {
    LITS.iter()
        .flat_map(|&l| {
            let x = literal(l);
            [
                x,
                info::neg(x),
                info::rescale(x, 1),
                info::rescale(x, 3),
                info::narrow(x, Width::W8),
                info::quantize(x, Width::W16, -4),
                info::quantize(x, Width::W8, 0),
            ]
        })
        .collect()
}

pub fn write_table(out: &mut impl Write) -> io::Result<()> {
    let xs = level1();
    for (i, x) in xs.iter().enumerate() {
        writeln!(out, "L {i} {}", show(*x))?;
    }
    for (i, &a) in xs.iter().enumerate() {
        for (j, &b) in xs.iter().enumerate() {
            let r = [
                info::add(a, b),
                info::sub(a, b),
                info::mul(a, b),
                info::div(a, b),
                info::join(a, b),
            ];
            let shown: Vec<String> = r.into_iter().map(show).collect();
            writeln!(out, "P {i} {j} {}", shown.join(" | "))?;
        }
    }
    Ok(())
}
