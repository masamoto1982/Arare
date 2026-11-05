//! The performance requirements (AR-PERF-001, 002, 004, 005 and 006,
//! WORK_ORDER.md §6), measured on the compiled route against a baseline in
//! the same run: a hand-written Rust loop, or Ajisai's native build at the
//! commit UPSTREAM.md records (verification case AR-VER-221).
//!
//! `cargo run --release` prints the table; `-- --check` also exits 1 when a
//! requirement is not met, which is how CI runs it; `-- --json` prints the
//! figures as JSON. Each candidate is timed in interleaved rounds and its
//! best round is kept, so that a slow moment of the machine falls on every
//! candidate alike. Ajisai's inputs are built in its untimed setup, as its
//! own benchmarks do, and where both answers are exact they must agree.

use ajisai_core::agent::time_after_setup;
use arare_codegen::rt::rescale_i32;
use arare_compiled::{program, Arg, Codes, Column, Out};
use arare_kernel::{Num, Width};
use std::hint::black_box;
use std::time::Instant;

/// The best time, in milliseconds, of `rounds` interleaved runs of each.
/// Every other round runs them in the reverse order, so that what one
/// candidate leaves behind (freed memory, a cold cache) falls on each alike.
fn race(rounds: usize, fs: &mut [&mut dyn FnMut()]) -> Vec<f64> {
    let mut best = vec![f64::MAX; fs.len()];
    for round in 0..rounds {
        for k in 0..fs.len() {
            let i = if round % 2 == 0 { k } else { fs.len() - 1 - k };
            let t = Instant::now();
            fs[i]();
            best[i] = best[i].min(t.elapsed().as_secs_f64() * 1e3);
        }
    }
    best
}

fn ajisai_ms(setup: &str, source: &str, rounds: usize) -> (f64, String) {
    let mut best = f64::MAX;
    let mut answer = String::new();
    for _ in 0..rounds {
        let (interp, ms) = time_after_setup(setup, source);
        best = best.min(ms);
        answer = interp
            .get_stack()
            .last()
            .map(|v| v.to_string())
            .unwrap_or_default();
    }
    (best, answer)
}

fn col8(scale: i32, codes: Vec<i8>) -> Arg {
    Arg::Column(Column::new(Width::W8, scale, Codes::I8(codes), None).unwrap())
}

fn col16(scale: i32, codes: Vec<i16>) -> Arg {
    Arg::Column(Column::new(Width::W16, scale, Codes::I16(codes), None).unwrap())
}

fn run(name: &str, args: &[Arg]) -> Vec<Out> {
    (program(name).unwrap().run)(black_box(args)).unwrap()
}

/// The exact value of a compiled scalar result, as Ajisai prints a number.
fn exact(out: &[Out]) -> String {
    match out.last().map(Out::to_value) {
        Some(arare_interp_value) => format!("{arare_interp_value}"),
        None => String::new(),
    }
}

/// Whether `|ours − exact| ≤ num/den`, each `p/q` as printed, in big
/// integers.
fn within_budget(ours: &str, exact: &str, num: i128, den: i128) -> bool {
    use num_bigint::BigInt;
    let rat = |s: &str| -> (BigInt, BigInt) {
        let (p, q) = s.split_once('/').unwrap_or((s, "1"));
        (p.parse().unwrap(), q.parse().unwrap())
    };
    let ((a, b), (p, q)) = (rat(ours), rat(exact));
    // |a/b − p/q| ≤ num/den  ⇔  |a·q − p·b|·den ≤ num·b·q, with b, q, den > 0.
    let diff = (&a * &q - &p * &b).magnitude().clone();
    BigInt::from(diff) * BigInt::from(den) <= BigInt::from(num) * b * q
}

struct Row {
    id: &'static str,
    what: &'static str,
    figure: f64,
    unit: &'static str,
    limit: &'static str,
    ok: bool,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--check");
    let json = args.iter().any(|a| a == "--json");
    let mut rows = Vec::new();

    // AR-PERF-001: the int8 dot product over 1,000,000 lanes.
    let n = 1_000_000;
    let a: Vec<i8> = (0..n).map(|i| ((i * 7 % 253) as i32 - 126) as i8).collect();
    let b: Vec<i8> = (0..n)
        .map(|i| ((i * 11 % 253) as i32 - 126) as i8)
        .collect();
    let dot_args = [col8(0, a.clone()), col8(0, b.clone())];
    let hand = |x: &[i8], y: &[i8]| -> i64 {
        x.iter()
            .zip(y)
            .map(|(&p, &q)| (p as i32 * q as i32) as i64)
            .sum()
    };
    let t = race(
        50,
        &mut [
            &mut || {
                black_box(run("dot8", &dot_args));
            },
            &mut || {
                black_box(hand(black_box(&a), black_box(&b)));
            },
        ],
    );
    let setup = "0 999999 RANGE 'I' BIND I 7 MUL 'X' BIND X X 253 DIV FLOOR 253 MUL SUB 126 SUB I 11 MUL 'Y' BIND Y Y 253 DIV FLOOR 253 MUL SUB 126 SUB";
    let (aj, aj_answer) = ajisai_ms(setup, "MUL 0 [ ADD ] FOLD", 3);
    let ours = exact(&run("dot8", &dot_args));
    assert_eq!(
        ours,
        format!("{}/1", hand(&a, &b)),
        "the compiled dot product is exact"
    );
    assert_eq!(
        aj_answer, ours,
        "Ajisai and Arare agree on the integer dot product"
    );
    rows.push(Row {
        id: "AR-PERF-001",
        what: "int8 dot product, 1,000,000 lanes: compiled / hand-written loop",
        figure: t[0] / t[1],
        unit: "×",
        limit: "≤ 1.5",
        ok: t[0] / t[1] <= 1.5,
    });
    rows.push(Row {
        id: "AR-PERF-001",
        what: "int8 dot product: Ajisai native / compiled",
        figure: aj / t[0],
        unit: "×",
        limit: "≥ 20",
        ok: aj / t[0] >= 20.0,
    });

    // AR-PERF-002: the small-rational dot product, in fixed point: the
    // thirds and quarters of Ajisai's case quantized at int8, exponent −5.
    let ra: Vec<i8> = (0..n)
        .map(|i| ((((i % 7) + 1) * 32 + 1) / 3) as i8)
        .collect();
    let rb: Vec<i8> = (0..n).map(|i| (((i % 5) + 1) * 8) as i8).collect();
    let rat_args = [col8(-5, ra), col8(-5, rb)];
    let t = race(
        50,
        &mut [&mut || {
            black_box(run("ratdot", &rat_args));
        }],
    );
    let setup = "0 999999 RANGE 'A' BIND A A 7 DIV FLOOR 7 MUL SUB 1 ADD 3 DIV A A 5 DIV FLOOR 5 MUL SUB 1 ADD 4 DIV";
    let (aj, _) = ajisai_ms(setup, "MUL 0 [ ADD ] FOLD", 3);
    rows.push(Row {
        id: "AR-PERF-002",
        what: "small-rational dot product: Ajisai native / compiled fixed point",
        figure: aj / t[0],
        unit: "×",
        limit: "≥ 300",
        ok: aj / t[0] >= 300.0,
    });

    // AR-PERF-003: twenty steps of the logistic map, x' = (13/4)·x·(1 − x)
    // from x = 1/2, against Ajisai's exact x₂₀ — a rational of about a
    // million bits — within the error budget the checker infers for the
    // same program.
    let steps = col8(0, (1..=20).collect());
    let t = race(
        200,
        &mut [&mut || {
            for _ in 0..1000 {
                black_box(run("logistic", black_box(std::slice::from_ref(&steps))));
            }
        }],
    );
    let per_run_us = t[0]; // 1000 runs in t[0] ms
    let source = program("logistic").unwrap().source;
    let lanes: Vec<String> = (1..=20).map(|k| k.to_string()).collect();
    let analysis = arare_budget::check::analyze(
        &arare_front::parse(&format!("[ {} ] {source}", lanes.join(" "))).unwrap(),
    );
    let budget = arare_budget::value::budget(analysis.stack.last().unwrap())
        .expect("the checker bounds the logistic map");
    let (aj_ms, aj_answer) = ajisai_ms(
        "",
        "1 20 RANGE 1/2 [ 'I' BIND 'X' BIND 13/4 X MUL 1 X SUB MUL ] FOLD",
        1,
    );
    let ours = exact(&run("logistic", std::slice::from_ref(&steps)));
    let within = within_budget(&ours, &aj_answer, budget.num(), budget.den());
    assert!(
        within,
        "the compiled logistic map {ours} is not within {budget} of Ajisai's x₂₀"
    );
    rows.push(Row {
        id: "AR-PERF-003",
        what: "logistic map, 20 steps, compiled, within its budget of Ajisai's exact x₂₀",
        figure: per_run_us,
        unit: "µs",
        limit: "≤ 1",
        ok: per_run_us <= 1.0 && within,
    });
    eprintln!("(Ajisai native takes {aj_ms:.0} ms for the exact x₂₀)");

    // AR-PERF-004: the harmonic sum to k = 20,000.
    let one = Arg::Num(Num::new(arare_kernel::Val::Fin(1 << 30), Width::W32, -30).unwrap());
    let ks = col16(0, (1..=20000).map(|k| k as i16).collect());
    let harm_args = [one, ks];
    let t = race(
        200,
        &mut [&mut || {
            black_box(run("harmonic", &harm_args));
        }],
    );
    let (aj, _) = ajisai_ms("1 20000 RANGE", "'K' BIND 1 K DIV 0 [ ADD ] FOLD", 3);
    rows.push(Row {
        id: "AR-PERF-004",
        what: "harmonic sum, k = 20,000: Ajisai native / compiled",
        figure: aj / t[0],
        unit: "×",
        limit: "≥ 1000",
        ok: aj / t[0] >= 1000.0,
    });

    // AR-PERF-005: top-level scalar Words, per Word.
    let p = program("scalars").unwrap();
    let words = arare_front::parse(p.source)
        .unwrap()
        .items
        .iter()
        .filter(|i| matches!(i, arare_front::Item::Word(_)))
        .count();
    let reps = 100_000;
    let inputs: Vec<[Arg; 2]> = (0..64)
        .map(|i| {
            [
                Arg::Num(
                    Num::int(i % 7 + 1)
                        .unwrap()
                        .quantize(Width::W16, 0)
                        .unwrap(),
                ),
                Arg::Num(
                    Num::int(i % 5 + 2)
                        .unwrap()
                        .quantize(Width::W16, 0)
                        .unwrap(),
                ),
            ]
        })
        .collect();
    let t = race(
        20,
        &mut [&mut || {
            for r in 0..reps {
                black_box((p.run)(black_box(&inputs[r % 64])).unwrap());
            }
        }],
    );
    let per_word_ns = t[0] * 1e6 / (reps * words) as f64;
    rows.push(Row {
        id: "AR-PERF-005",
        what: "top-level scalar Word, compiled, per Word",
        figure: per_word_ns,
        unit: "ns",
        limit: "≤ 2",
        ok: per_word_ns <= 2.0,
    });

    // AR-PERF-006: the wheel points and NIL, against the same kernel
    // without them — memory-bound (int8 ADD over 16,000,000 lanes) and
    // compute-bound (a chain of fixed-point steps over 4,096 lanes in cache).
    let m = 16_000_000;
    let a: Vec<i8> = (0..m).map(|i| ((i * 7 % 253) as i32 - 126) as i8).collect();
    let b: Vec<i8> = (0..m)
        .map(|i| ((i * 13 % 253) as i32 - 126) as i8)
        .collect();
    let add_args = [col8(0, a.clone()), col8(0, b.clone())];
    let plain_add = |x: &[i8], y: &[i8]| -> Vec<i16> {
        x.iter()
            .zip(y)
            .map(|(&p, &q)| p as i16 + q as i16)
            .collect()
    };
    // Each run writes a fresh 32 MB output, whose page faults vary on a
    // shared machine; enough rounds that the best of each sees a quiet one.
    let t = race(
        60,
        &mut [
            &mut || {
                black_box(run("add8", &add_args));
            },
            &mut || {
                black_box(plain_add(black_box(&a), black_box(&b)));
            },
        ],
    );
    rows.push(Row {
        id: "AR-PERF-006",
        what: "memory-bound int8 ADD: with points and NIL / without",
        figure: t[0] / t[1],
        unit: "×",
        limit: "≤ 1.1",
        ok: t[0] / t[1] <= 1.1,
    });
    let x: Vec<i16> = (0..16384)
        .map(|i| ((i * 37) % 2000) as i16 - 1000)
        .collect();
    let chain_args = [col16(-8, x.clone())];
    // The same six fixed-point steps, hand-written on plain codes.
    let plain_chain = |x: &[i16]| -> Vec<i16> {
        x.iter()
            .map(|&v| {
                let mut v = v as i32;
                for _ in 0..6 {
                    v = rescale_i32(v * 3 + 256, 2);
                    v = rescale_i32(v - 64, 1);
                }
                v as i16
            })
            .collect()
    };
    let compiled = run("chain16", &chain_args);
    let want = Out::Column(Column::computed(
        Width::W16,
        -8,
        Codes::I16(plain_chain(&x)),
    ));
    assert_eq!(
        compiled[0].to_value(),
        want.to_value(),
        "the hand-written chain computes the program"
    );
    let t = race(
        1000,
        &mut [
            &mut || {
                black_box(run("chain16", &chain_args));
            },
            &mut || {
                black_box(plain_chain(black_box(&x)));
            },
        ],
    );
    rows.push(Row {
        id: "AR-PERF-006",
        what: "compute-bound fixed-point chain: with points and NIL / without",
        figure: t[0] / t[1],
        unit: "×",
        limit: "≤ 1.1",
        ok: t[0] / t[1] <= 1.1,
    });

    if json {
        let items: Vec<String> = rows
            .iter()
            .map(|r| format!("{{\"id\":\"{}\",\"what\":\"{}\",\"figure\":{:.4},\"unit\":\"{}\",\"limit\":\"{}\",\"ok\":{}}}", r.id, r.what, r.figure, r.unit, r.limit, r.ok))
            .collect();
        println!("[{}]", items.join(","));
    } else {
        for r in &rows {
            println!(
                "{} {:<66} {:>10.3} {:<2} {:<7} {}",
                r.id,
                r.what,
                r.figure,
                r.unit,
                r.limit,
                if r.ok { "ok" } else { "MISSED" }
            );
        }
    }
    if check && rows.iter().any(|r| !r.ok) {
        std::process::exit(1);
    }
}
