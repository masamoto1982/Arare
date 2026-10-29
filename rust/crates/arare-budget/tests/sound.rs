//! AR-VER-216: the Rust checker is sound against the kernel itself. Over a
//! grid of literals — integers, rationals that round and that do not, values
//! out of their width, the wheel points — every Word and every pair, and two
//! levels deep on part of the grid, what the checker infers is held against
//! what `arare-kernel` computes and the exact value computed with exact
//! rationals: the claim `Arare.Budget.Sound` makes, checked case by case
//! (AR-HLR-027, AR-LLR-019).

use arare_budget::info::{self, Info};
use arare_budget::Q;
use arare_kernel::{Num, Val, Width};

/// The exact meaning: Lean's `X`.
#[derive(Clone, Copy, PartialEq, Debug)]
enum X {
    Fin(Q),
    Inf,
    Bot,
}

fn x_add(a: X, b: X) -> X {
    match (a, b) {
        (X::Bot, _) | (_, X::Bot) | (X::Inf, X::Inf) => X::Bot,
        (X::Inf, _) | (_, X::Inf) => X::Inf,
        (X::Fin(p), X::Fin(q)) => X::Fin(p.add(q).unwrap()),
    }
}

fn x_neg(a: X) -> X {
    match a {
        X::Fin(p) => X::Fin(p.neg()),
        x => x,
    }
}

fn x_mul(a: X, b: X) -> X {
    match (a, b) {
        (X::Bot, _) | (_, X::Bot) => X::Bot,
        (X::Inf, X::Inf) => X::Inf,
        (X::Inf, X::Fin(p)) | (X::Fin(p), X::Inf) => {
            if p.is_zero() {
                X::Bot
            } else {
                X::Inf
            }
        }
        (X::Fin(p), X::Fin(q)) => X::Fin(p.mul(q).unwrap()),
    }
}

fn x_div(a: X, b: X) -> X {
    match (a, b) {
        (X::Bot, _) | (_, X::Bot) | (X::Inf, X::Inf) => X::Bot,
        (X::Inf, X::Fin(_)) => X::Inf,
        (X::Fin(_), X::Inf) => X::Fin(Q::ZERO),
        (X::Fin(p), X::Fin(q)) if q.is_zero() => {
            if p.is_zero() {
                X::Bot
            } else {
                X::Inf
            }
        }
        (X::Fin(p), X::Fin(q)) => X::Fin(p.mul(q.inv()).unwrap()),
    }
}

/// One case: what the checker infers, the exact value, the machine's number.
#[derive(Clone, Copy)]
struct Case {
    info: Info,
    exact: X,
    machine: Num,
}

/// `Arare.Budget.Sound`.
fn sound(c: &Case) -> bool {
    match c.info {
        Info::Top => true,
        Info::Inf => c.exact == X::Inf && c.machine.val() == Val::Inf,
        Info::Bot => c.exact == X::Bot && c.machine.val() == Val::Bot,
        Info::Num(r) => {
            let (X::Fin(q), Val::Fin(k)) = (c.exact, c.machine.val()) else {
                return false;
            };
            let s = c.machine.scale();
            let m = Q::int(k as i128).unwrap().mul(Q::pow2(s).unwrap()).unwrap();
            let d = m.sub(q).unwrap();
            r.smin <= s
                && s <= r.smax
                && r.lo.le(q)
                && q.le(r.hi)
                && r.err.neg().le(d)
                && d.le(r.err)
        }
    }
}

const LITS: [(i64, i64, Width, i32); 22] = [
    (0, 1, Width::W8, 0),
    (1, 1, Width::W8, 0),
    (-1, 1, Width::W8, 0),
    (5, 1, Width::W8, 0),
    (-126, 1, Width::W8, 0),
    (300, 1, Width::W16, 0),
    (1_000_000_007, 1, Width::W32, 0),
    (3_000_000_000_000_000_000, 1, Width::W64, 0),
    (1, 3, Width::W16, -8),
    (-2, 3, Width::W16, -8),
    (1, 10, Width::W16, -12),
    (-5, 7, Width::W32, -20),
    (5, 2, Width::W8, -1),
    (7, 4, Width::W8, 0),
    (1, 1000, Width::W8, 0),
    (-1, 1000, Width::W16, -4),
    (3, 1, Width::W8, 2),
    (200, 1, Width::W8, 0),
    (1, 0, Width::W8, 0),
    (0, 0, Width::W8, 0),
    (9, 8, Width::W8, -3),
    (-37, 16, Width::W16, -2),
];

fn literal((p, q, w, e): (i64, i64, Width, i32)) -> Case {
    let machine = Num::from_ratio(p, q, w, e).unwrap();
    let exact = match (p, q) {
        (0, 0) => X::Bot,
        (_, 0) => X::Inf,
        _ => X::Fin(Q::new(p as i128, q as i128).unwrap()),
    };
    Case {
        info: info::lit(p, q, w, e, machine.val()),
        exact,
        machine,
    }
}

/// Each case under the unary Words, where the kernel accepts them.
fn unary(c: Case) -> Vec<Case> {
    let mut out = vec![
        c,
        Case {
            info: info::neg(c.info),
            exact: x_neg(c.exact),
            machine: c.machine.neg(),
        },
    ];
    for s in [1, 3, 9] {
        if let Ok(m) = c.machine.rescale(s) {
            out.push(Case {
                info: info::rescale(c.info, s),
                exact: c.exact,
                machine: m,
            });
        }
    }
    for w in [Width::W8, Width::W16] {
        out.push(Case {
            info: info::narrow(c.info, w),
            exact: c.exact,
            machine: c.machine.narrow(w),
        });
    }
    for (w, e) in [
        (Width::W16, -4),
        (Width::W8, 0),
        (Width::W32, -12),
        (Width::W8, 3),
    ] {
        out.push(Case {
            info: info::quantize(c.info, w, e),
            exact: c.exact,
            machine: c.machine.quantize(w, e).unwrap(),
        });
    }
    out
}

/// The four arithmetic Words on two cases, where the kernel accepts them.
fn binary(a: &Case, b: &Case) -> Vec<Case> {
    let mut out = vec![
        Case {
            info: info::add(a.info, b.info),
            exact: x_add(a.exact, b.exact),
            machine: a.machine.add(b.machine),
        },
        Case {
            info: info::sub(a.info, b.info),
            exact: x_add(a.exact, x_neg(b.exact)),
            machine: a.machine.sub(b.machine),
        },
        Case {
            info: info::div(a.info, b.info),
            exact: x_div(a.exact, b.exact),
            machine: a.machine.div(b.machine),
        },
    ];
    if let Ok(m) = a.machine.mul(b.machine) {
        out.push(Case {
            info: info::mul(a.info, b.info),
            exact: x_mul(a.exact, b.exact),
            machine: m,
        });
    }
    out
}

fn level1() -> Vec<Case> {
    LITS.iter().copied().map(literal).flat_map(unary).collect()
}

#[test]
fn ar_ver_216_every_word_on_the_grid() {
    let xs = level1();
    let (mut checked, mut bounded) = (0, 0);
    for c in &xs {
        assert!(sound(c), "unsound unary case: {:?}", c.info);
        checked += 1;
    }
    for a in &xs {
        for b in &xs {
            for c in binary(a, b) {
                assert!(
                    sound(&c),
                    "unsound: {:?} from {:?} and {:?}",
                    c.info,
                    a.info,
                    b.info
                );
                checked += 1;
                bounded += c.info.is_num() as usize;
            }
        }
    }
    // The grid is not vacuous: most results are bounded.
    assert!(checked > 100_000, "{checked}");
    assert!(bounded * 2 > checked, "{bounded} of {checked}");
}

#[test]
fn ar_ver_216_two_levels_deep() {
    let lits: Vec<Case> = LITS.iter().copied().map(literal).collect();
    let pairs: Vec<Case> = lits
        .iter()
        .flat_map(|a| lits.iter().flat_map(move |b| binary(a, b)))
        .collect();
    for a in pairs.iter().step_by(7) {
        for b in &lits {
            for c in binary(a, b).into_iter().chain(binary(b, a)) {
                assert!(sound(&c), "unsound: {:?}", c.info);
                for u in unary(c) {
                    assert!(sound(&u), "unsound: {:?}", u.info);
                }
            }
        }
    }
}

#[test]
fn ar_ver_216_join_and_cross() {
    // A join is right about either side; a cross bound is right when the
    // machine takes one side and the exact meaning the other.
    let xs = level1();
    for a in xs.iter().step_by(3) {
        for b in xs.iter().step_by(5) {
            let j = info::join(a.info, b.info);
            let x = info::cross(a.info, b.info);
            for (info, exact, machine) in [
                (j, a.exact, a.machine),
                (j, b.exact, b.machine),
                (x, a.exact, a.machine),
                (x, b.exact, b.machine),
                (x, b.exact, a.machine),
                (x, a.exact, b.machine),
            ] {
                assert!(
                    sound(&Case {
                        info,
                        exact,
                        machine
                    }),
                    "{info:?}"
                );
            }
        }
    }
}
