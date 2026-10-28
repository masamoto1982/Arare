//! The model as a table (AR-LLR-001, verification case AR-VER-201): the
//! executable specification's answers over the grid `formal/Table.lean`
//! prints from the Lean model, in the same format, so that the Lean job of
//! CI can diff the two line by line.

use crate::{
    decode, encode, eq_t, filter_t, fit, lt_t, rescale, round_div, round_div_pos, select_t, sum_t,
    truth_of, Truth, W,
};
use std::io::{self, Write};

fn show_o(x: Option<W>) -> String {
    x.map_or_else(|| "N".to_string(), |w| w.to_string())
}

fn v8() -> Vec<W> {
    (-128i128..=127)
        .filter(|&c| c != -127)
        .map(|c| decode(128, c))
        .collect()
}

fn sum_inputs() -> Vec<Vec<Option<W>>> {
    let atoms = [
        None,
        Some(W::Fin(0)),
        Some(W::Fin(1)),
        Some(W::Fin(-1)),
        Some(W::Inf),
        Some(W::Bot),
    ];
    let mut all = vec![vec![]];
    all.extend(atoms.iter().map(|&a| vec![a]));
    for &a in &atoms {
        for &b in &atoms {
            all.push(vec![a, b]);
        }
    }
    for &a in &atoms {
        for &b in &atoms {
            for &c in &atoms {
                all.push(vec![a, b, c]);
            }
        }
    }
    all
}

/// Writes the table `formal/Table.lean` prints, in the same format.
pub fn write_table(out: &mut impl Write) -> io::Result<()> {
    let v = v8();
    for &x in &v {
        for &y in &v {
            writeln!(
                out,
                "A {x} {y} {} {} {} {} {} {}",
                x.add(y),
                x.sub(y),
                x.mul(y),
                x.div(y),
                eq_t(x, y),
                lt_t(x, y)
            )?;
        }
    }
    for &x in &v {
        writeln!(out, "G {x} {} {}", x.neg(), fit(128, x))?;
    }
    for k in -126i128..=126 {
        for s in 0..=8u32 {
            writeln!(out, "R {k} {s} {}", rescale(k, s))?;
        }
    }
    for n in -40i128..=40 {
        for d in 1i128..=12 {
            writeln!(out, "P {n} {d} {}", round_div_pos(n, d))?;
        }
        for d in (-12i128..=12).filter(|&d| d != 0) {
            writeln!(out, "D {n} {d} {}", round_div(n, d))?;
        }
    }
    for k in -300i128..=300 {
        writeln!(out, "T {k} {}", fit(128, W::Fin(k)))?;
    }
    for c in -128i128..=127 {
        writeln!(
            out,
            "C {c} {} {}",
            decode(128, c),
            encode(128, decode(128, c))
        )?;
    }
    for a in Truth::ALL {
        writeln!(out, "N {a} {}", a.not())?;
        for b in Truth::ALL {
            writeln!(out, "K {a} {b} {} {}", a.and(b), a.or(b))?;
        }
    }
    for xs in sum_inputs() {
        let shown: Vec<String> = xs.iter().map(|&x| show_o(x)).collect();
        writeln!(out, "S {} {}", shown.join(","), show_o(sum_t(&xs)))?;
        let kept = filter_t(|x: &Option<W>| x.map(|w| lt_t(w, W::Fin(0))), &xs);
        let kept: Vec<String> = kept.iter().map(|&x| show_o(x)).collect();
        writeln!(out, "L {} {}", shown.join(","), kept.join(","))?;
    }
    for c in [
        None,
        Some(Truth::False),
        Some(Truth::Unknown),
        Some(Truth::True),
    ] {
        let shown = c.map_or_else(|| "N".to_string(), |t: Truth| t.to_string());
        writeln!(
            out,
            "Q {shown} {} {}",
            select_t(c, 1, 2),
            truth_of(c).rank()
        )?;
    }
    out.flush()
}
