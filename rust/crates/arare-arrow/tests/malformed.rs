//! AR-VER-234: the reader treats its input as untrusted (AR-LLR-041). Every
//! prefix of a valid stream and file, and many seeded byte flips of them, is
//! read to an answer or an error, never a panic; and a few malformed inputs
//! are refused with the error that names them.

use arare_arrow::{read, write_file, write_stream, Batch, Column, Error, NumColumn, ReadOptions};
use arare_kernel::{Truth, Val, Width};

fn sample() -> Batch {
    Batch {
        columns: vec![
            (
                "t".into(),
                Column::Truth(vec![Truth::True, Truth::Unknown, Truth::False]),
            ),
            (
                "n".into(),
                Column::Num(NumColumn {
                    width: Width::W32,
                    scale: -4,
                    lanes: vec![Some(Val::Fin(-17)), Some(Val::Inf), None],
                }),
            ),
            (
                "w".into(),
                Column::Num(NumColumn {
                    width: Width::W64,
                    scale: -30,
                    lanes: vec![Some(Val::Bot), Some(Val::Fin(1)), Some(Val::Fin(2))],
                }),
            ),
        ],
    }
}

#[test]
fn ar_ver_234_every_prefix_is_an_answer_or_an_error() {
    let b = sample();
    for bytes in [write_stream(&b).unwrap(), write_file(&b).unwrap()] {
        for n in 0..bytes.len() {
            let _ = read(&bytes[..n], ReadOptions::default());
        }
    }
}

#[test]
fn ar_ver_234_byte_flips_never_panic() {
    let b = sample();
    let mut state = 0x6d61_6c66_6f72_6d65u64;
    for bytes in [write_stream(&b).unwrap(), write_file(&b).unwrap()] {
        for _ in 0..20_000 {
            let mut m = bytes.clone();
            for _ in 0..1 + (state % 4) {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let at = (state >> 8) as usize % m.len();
                m[at] ^= 1 << (state % 8);
            }
            let _ = read(&m, ReadOptions::default());
        }
    }
}

#[test]
fn ar_ver_234_refusals() {
    assert!(matches!(
        read(&[], ReadOptions::default()),
        Err(Error::Malformed(_))
    ));
    assert!(matches!(
        read(b"ARROW1\0\0", ReadOptions::default()),
        Err(Error::Malformed(_))
    ));
    // A stream that ends right after the end-of-stream marker has no schema.
    assert!(matches!(
        read(
            &[0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0],
            ReadOptions::default()
        ),
        Err(Error::Malformed(_))
    ));
    // A file whose closing magic is cut.
    let f = write_file(&sample()).unwrap();
    assert!(matches!(
        read(&f[..f.len() - 1], ReadOptions::default()),
        Err(Error::Malformed(_))
    ));
}
