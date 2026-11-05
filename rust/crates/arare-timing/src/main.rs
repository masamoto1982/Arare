//! Times the compiled kernels of the benchmarks (AR-VER-226), for AR-PERF-007:
//! the same source, built natively and for `wasm32-wasip1` with SIMD128, is
//! run by `scripts/check-wasm-speed.mjs`, which compares the two.
//!
//! Each kernel runs on the inputs `rust/bench` gives it, in rounds of a fixed
//! number of runs; the best round counts, so a slow moment of the machine
//! does not. It prints one line per kernel: the name and the nanoseconds of
//! one run.

use arare_compiled::{program, Arg, Codes, Column, Out};
use arare_kernel::{Num, Val, Width};
use std::hint::black_box;
use std::time::Instant;

fn col8(scale: i32, codes: Vec<i8>) -> Arg {
    Arg::Column(Column::new(Width::W8, scale, Codes::I8(codes), None).expect("valid codes"))
}

fn col16(scale: i32, codes: Vec<i16>) -> Arg {
    Arg::Column(Column::new(Width::W16, scale, Codes::I16(codes), None).expect("valid codes"))
}

fn run(name: &str, args: &[Arg]) -> Vec<Out> {
    (program(name).expect("a compiled program").run)(black_box(args)).expect("no error")
}

/// The nanoseconds of one run of `name` on `args`: the best of `rounds`
/// rounds of `reps` runs.
fn time(name: &str, args: &[Arg], rounds: usize, reps: usize) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..rounds {
        let t = Instant::now();
        for _ in 0..reps {
            black_box(run(name, args));
        }
        best = best.min(t.elapsed().as_secs_f64() * 1e9 / reps as f64);
    }
    best
}

fn main() {
    let n = 1_000_000;
    let a: Vec<i8> = (0..n).map(|i| ((i * 7 % 253) - 126) as i8).collect();
    let b: Vec<i8> = (0..n).map(|i| ((i * 11 % 253) - 126) as i8).collect();
    let ra: Vec<i8> = (0..n)
        .map(|i| ((((i % 7) + 1) * 32 + 1) / 3) as i8)
        .collect();
    let rb: Vec<i8> = (0..n).map(|i| (((i % 5) + 1) * 8) as i8).collect();
    let one = Num::new(Val::Fin(1 << 30), Width::W32, -30).expect("1 at 2^-30");
    let chain: Vec<i16> = (0..16384)
        .map(|i| ((i * 37) % 2000) as i16 - 1000)
        .collect();
    let kernels: Vec<(&str, Vec<Arg>, usize, usize)> = vec![
        ("dot8", vec![col8(0, a.clone()), col8(0, b.clone())], 20, 1),
        ("ratdot", vec![col8(-5, ra), col8(-5, rb)], 20, 1),
        ("add8", vec![col8(0, a), col8(0, b)], 20, 1),
        (
            "harmonic",
            vec![
                Arg::Num(one),
                col16(0, (1..=20000).map(|k| k as i16).collect()),
            ],
            50,
            1,
        ),
        ("chain16", vec![col16(-8, chain)], 200, 1),
        ("logistic", vec![col8(0, (1..=20).collect())], 50, 1000),
    ];
    for (name, args, rounds, reps) in &kernels {
        println!("{name} {:.1}", time(name, args, *rounds, *reps));
    }
}
