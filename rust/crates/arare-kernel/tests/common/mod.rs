//! Conversions between the kernel's values and the specification's.
#![allow(dead_code)]

use arare_kernel::{Num, Val, Width};
use arare_spec::W;

pub fn to_spec(v: Val) -> W {
    match v {
        Val::Fin(k) => W::Fin(k as i128),
        Val::Inf => W::Inf,
        Val::Bot => W::Bot,
    }
}

pub fn m(w: Width) -> i128 {
    w.half_range()
}

/// Every value width `w` holds, in code order: `0/0`, the finite range, `1/0`.
pub fn values(w: Width) -> impl Iterator<Item = Val> {
    let max = w.max_finite();
    core::iter::once(Val::Bot)
        .chain((-max..=max).map(Val::Fin))
        .chain(core::iter::once(Val::Inf))
}

pub fn num(v: Val, w: Width, e: i32) -> Num {
    Num::new(v, w, e).expect("a valid number")
}

pub fn spec_truth(t: arare_spec::Truth) -> arare_kernel::Truth {
    match t {
        arare_spec::Truth::False => arare_kernel::Truth::False,
        arare_spec::Truth::Unknown => arare_kernel::Truth::Unknown,
        arare_spec::Truth::True => arare_kernel::Truth::True,
    }
}
