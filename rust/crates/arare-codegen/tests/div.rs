//! AR-VER-222: the vectorizable DIV of the fast path (AR-LLR-026) is the
//! kernel's round-half-even quotient on every operand pair it is used for:
//! every int8 pair, ties and near-ties built around each quotient, and
//! seeded random operands up to its limits.

use arare_codegen::rt::{div_f64, round_div, DIV_F64_DIVIDEND, DIV_F64_DIVISOR};

fn same(n: i64, d: i64) {
    let want = round_div(n as i128, d as i128);
    let got = div_f64(n as f64, d as f64);
    assert_eq!(got as i128, want, "{n} / {d}");
    assert_eq!(got, got.trunc());
}

#[test]
fn ar_ver_222_every_int8_pair() {
    for n in -128..=127 {
        for d in (-128..=127).filter(|&d| d != 0) {
            same(n, d);
        }
    }
}

#[test]
fn ar_ver_222_ties_and_near_ties() {
    let top = DIV_F64_DIVIDEND as i64 - 1;
    for d in [
        1i64,
        2,
        3,
        7,
        1000,
        65535,
        1 << 20,
        (1 << 31) - 1,
        (1 << 40) + 3,
        DIV_F64_DIVISOR as i64 - 1,
    ] {
        for q in [0i64, 1, 2, 3, 1 << 10, (1 << 21) - 1, 1 << 30] {
            // n = q·d + d/2 + δ: the tie and its neighbours.
            for delta in -2..=2i64 {
                for sign in [1i64, -1] {
                    let n = (q as i128 * d as i128 + d as i128 / 2 + delta as i128) * sign as i128;
                    if n.unsigned_abs() < DIV_F64_DIVIDEND {
                        same(n as i64, d);
                        same(n as i64, -d);
                    }
                }
            }
        }
        same(top, d);
        same(-top, d);
    }
}

#[test]
fn ar_ver_222_random() {
    let mut s = 20261103u64;
    for _ in 0..2_000_000 {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let n = ((s as i64) >> (12 + (s >> 58) as u32 % 40))
            .clamp(-(DIV_F64_DIVIDEND as i64 - 1), DIV_F64_DIVIDEND as i64 - 1);
        let t = s.rotate_left(29);
        let d = (t as i64) >> (11 + (t >> 59) as u32 % 50);
        if d == 0 || d.unsigned_abs() as u128 >= DIV_F64_DIVISOR {
            continue;
        }
        same(n, d);
    }
}

/// The `i32` RESCALE is the kernel's on every `i32` code it can be given:
/// every shift, around every power of two and on seeded random codes.
#[test]
fn ar_ver_222_rescale_i32() {
    use arare_codegen::rt::{rescale_code, rescale_i32};
    let mut s = 99u64;
    for shift in 0..=40u32 {
        let mut ks: Vec<i32> = vec![0, 1, -1, i32::MAX, i32::MIN + 1, i32::MIN];
        for e in 0..31 {
            for d in -3..=3 {
                ks.push(
                    (1i64 << e)
                        .saturating_add(d)
                        .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                );
                ks.push(
                    (-(1i64 << e))
                        .saturating_add(d)
                        .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                );
            }
        }
        for _ in 0..20_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ks.push(s as i32);
        }
        for k in ks {
            assert_eq!(
                rescale_i32(k, shift) as i64,
                rescale_code(k as i64, shift),
                "{k} >> {shift}"
            );
        }
    }
}
