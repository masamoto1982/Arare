//! AR-VER-220: translation validation. Every compiled program answers what
//! the reference interpreter answers on the same inputs (AR-HLR-030): over
//! every pair of int8 codes for the lane-wise Words, and on seeded random and
//! boundary inputs at every width for every program, wheel points, NIL lanes
//! and mismatched columns included. Where the inputs are finite the fast path
//! must be the one that answered, so the comparison is of compiled code, not
//! of the interpreter with itself.

use arare_compiled::{program, Arg, Codes, Column, Out, PROGRAMS};
use arare_interp::{run_on, Value};
use arare_kernel::{Num, Val, Width};

fn interpret(source: &str, args: &[Arg]) -> Result<Vec<Value>, String> {
    run_on(source, args.iter().map(Arg::to_value).collect()).map_err(|e| e.to_string())
}

fn values(out: Result<Vec<Out>, arare_compiled::RunError>) -> Result<Vec<Value>, String> {
    out.map(|o| o.iter().map(Out::to_value).collect())
        .map_err(|e| e.to_string())
}

fn column(width: Width, scale: i32, codes: Vec<i64>) -> Column {
    let codes = match width {
        Width::W8 => Codes::I8(codes.iter().map(|&c| c as i8).collect()),
        Width::W16 => Codes::I16(codes.iter().map(|&c| c as i16).collect()),
        Width::W32 => Codes::I32(codes.iter().map(|&c| c as i32).collect()),
        Width::W64 => Codes::I64(codes),
    };
    Column::new(width, scale, codes, None).unwrap()
}

/// Every int8 code but the unused one; the finite ones only when `finite`.
fn codes8(finite: bool) -> Vec<i64> {
    (-128..=127)
        .filter(|&c| c != -127 && (!finite || (-126..=126).contains(&c)))
        .collect()
}

fn check(name: &str, args: &[Arg], fast: bool) {
    let p = program(name).unwrap_or_else(|| panic!("no program {name}"));
    let want = interpret(p.source, args);
    assert_eq!(values((p.run)(args)), want, "{name}");
    if fast {
        let out = (p.fast)(args).unwrap_or_else(|| panic!("{name}: the fast path did not apply"));
        assert_eq!(values(Ok(out)), want, "{name} (fast path)");
    }
}

#[test]
fn ar_ver_220_every_int8_pair() {
    for (finite, fast) in [(true, true), (false, false)] {
        let codes = codes8(finite);
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        for &x in &codes {
            for &y in &codes {
                xs.push(x);
                ys.push(y);
            }
        }
        let args = [
            Arg::Column(column(Width::W8, 0, xs)),
            Arg::Column(column(Width::W8, 0, ys)),
        ];
        for name in ["add", "sub", "mul", "lt", "dot8", "add8", "broadcast2"] {
            if program(name).is_some() && program(name).unwrap().signature == "v8:0 v8:0" {
                check(name, &args, fast);
            }
        }
        // DIV traps on a zero divisor: the fast path declines, the
        // interpreter answers.
        check("div", &args, false);
    }
}

#[test]
fn ar_ver_220_every_int8_code() {
    let args = [Arg::Column(column(Width::W8, 0, codes8(true)))];
    for name in ["neg", "down", "mapped"] {
        check(name, &args, true);
    }
    let args = [Arg::Column(column(Width::W8, 0, codes8(false)))];
    for name in ["neg", "down", "mapped"] {
        check(name, &args, false);
    }
}

/// A seeded xorshift sequence.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// A finite code of `w`, often at a boundary.
    fn finite(&mut self, w: Width) -> i64 {
        let m = w.max_finite();
        match self.below(8) {
            0 => m,
            1 => -m,
            2 => 0,
            3 => 1,
            4 => -1,
            5 => (self.below(5) as i64) - 2,
            _ => {
                let r = self.next() as i128;
                (r.rem_euclid(2 * m as i128 + 1) - m as i128) as i64
            }
        }
    }
}

fn parse_sig(sig: &str) -> Vec<(bool, Width, i32)> {
    sig.split_whitespace()
        .map(|t| {
            let (bits, scale) = t[1..].split_once(':').unwrap();
            let w = Width::ALL
                .into_iter()
                .find(|w| w.bits().to_string() == bits)
                .unwrap();
            (t.starts_with('v'), w, scale.parse().unwrap())
        })
        .collect()
}

/// Inputs for a signature: `n` lanes per vector; `wild` adds wheel points
/// and NIL lanes.
fn inputs(rng: &mut Rng, sig: &str, n: usize, wild: bool) -> Vec<Arg> {
    parse_sig(sig)
        .into_iter()
        .map(|(vector, w, s)| {
            let lane = |rng: &mut Rng| -> Option<Val> {
                if wild && rng.below(10) == 0 {
                    return [None, Some(Val::Inf), Some(Val::Bot)][rng.below(3) as usize];
                }
                Some(Val::Fin(rng.finite(w)))
            };
            if vector {
                let lanes: Vec<Option<Val>> = (0..n).map(|_| lane(rng)).collect();
                Arg::Column(Column::of_values(w, s, &lanes).unwrap())
            } else {
                let v = match lane(rng) {
                    Some(v) => v,
                    None => Val::Bot,
                };
                Arg::Num(Num::new(v, w, s).unwrap())
            }
        })
        .collect()
}

#[test]
fn ar_ver_220_every_program_random_and_boundary() {
    // TV_ROUNDS raises the number of rounds for a longer local run.
    let rounds: usize = std::env::var("TV_ROUNDS")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(6);
    let mut rng = Rng(20261103);
    let mut fast_runs = 0;
    for p in PROGRAMS {
        for n in [0, 1, 2, 3, 4, 5, 64, 1000, 4097] {
            for wild in [false, true] {
                for _ in 0..rounds {
                    let args = inputs(&mut rng, p.signature, n, wild);
                    let want = interpret(p.source, &args);
                    assert_eq!(values((p.run)(&args)), want, "{} {:?}", p.name, p.signature);
                    if let Some(out) = (p.fast)(&args) {
                        let tame = args.iter().all(|a| match a {
                            Arg::Column(c) => !c.has_points() && !c.has_nils(),
                            Arg::Num(x) => matches!(x.val(), Val::Fin(_)),
                        });
                        assert!(
                            tame,
                            "{}: the fast path ran on a wheel point or a NIL",
                            p.name
                        );
                        assert_eq!(values(Ok(out)), want, "{} (fast path)", p.name);
                        fast_runs += 1;
                    }
                }
            }
        }
    }
    assert!(
        fast_runs > 500,
        "the fast path answered only {fast_runs} times"
    );
}

#[test]
fn ar_ver_220_fast_path_applies() {
    // On finite inputs of the declared widths and enough lanes, every
    // program with a fast path takes it.
    let mut rng = Rng(7);
    for p in PROGRAMS {
        if [
            "filtered",
            "twopass",
            "div",
            "scalediv",
            "widediv",
            "scalardiv",
            "harmonic",
        ]
        .contains(&p.name)
        {
            continue;
        }
        let args = inputs(&mut rng, p.signature, 8, false);
        // Inputs that could overflow a checked width may trap; the others
        // must not.
        let traps = ["widest", "widestmul", "narrow", "up", "chain16", "scalars"].contains(&p.name);
        assert!(traps || (p.fast)(&args).is_some(), "{}", p.name);
    }
    // No fast path: the compiled program is the interpreter.
    for name in ["filtered", "twopass"] {
        let args = inputs(&mut rng, program(name).unwrap().signature, 8, false);
        assert!((program(name).unwrap().fast)(&args).is_none());
    }
}

#[test]
fn ar_ver_220_inputs_the_fast_path_declines() {
    let p = program("add").unwrap();
    let a = column(Width::W8, 0, vec![1, 2, 3, 4]);
    // A column of another width or scale, of another length, or a scalar
    // where a vector is expected: the interpreter answers.
    for args in [
        vec![
            Arg::Column(a.clone()),
            Arg::Column(column(Width::W16, 0, vec![1, 2, 3, 4])),
        ],
        vec![
            Arg::Column(a.clone()),
            Arg::Column(column(Width::W8, 1, vec![1, 2, 3, 4])),
        ],
        vec![
            Arg::Column(a.clone()),
            Arg::Column(column(Width::W8, 0, vec![1])),
        ],
        vec![Arg::Column(a.clone()), Arg::Num(Num::int(3).unwrap())],
        vec![Arg::Column(a.clone())],
    ] {
        assert!((p.fast)(&args).is_none());
        assert_eq!(values((p.run)(&args)), interpret(p.source, &args));
    }
}
