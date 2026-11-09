//! AR-VER-233: what Arare writes as Arrow it reads back unchanged
//! (AR-LLR-039, AR-LLR-040, AR-LLR-042; the Phase 6 exit condition): every
//! width, every exponent from −31 to 31, wheel points and NIL lanes, in the
//! stream and the file format; and the stack's values through columns.

use arare_arrow::{
    read, to_columns, to_values, write_file, write_stream, Batch, Column, NumColumn, ReadOptions,
};
use arare_interp::{Machine, Reason, Value};
use arare_kernel::{Truth, Val, Width};

/// xorshift64*, seeded, so a failure reproduces.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn lane(r: &mut Rng, w: Width) -> Option<Val> {
    let max = w.max_finite();
    match r.below(10) {
        0 => None,
        1 => Some(Val::Inf),
        2 => Some(Val::Bot),
        3 => Some(Val::Fin(max)),
        4 => Some(Val::Fin(-max)),
        5 => Some(Val::Fin(0)),
        _ => {
            let span = (max as u64).wrapping_mul(2).wrapping_add(1);
            Some(Val::Fin((r.below(span) as i64).wrapping_sub(max)))
        }
    }
}

fn batch(r: &mut Rng, len: usize) -> Batch {
    let mut columns = Vec::new();
    for w in Width::ALL {
        for e in -31..=31 {
            let lanes = (0..len).map(|_| lane(r, w)).collect();
            columns.push((
                format!("w{}e{e}", w.bits()),
                Column::Num(NumColumn {
                    width: w,
                    scale: e,
                    lanes,
                }),
            ));
        }
    }
    let truth = (0..len).map(|_| Truth::ALL[r.below(3) as usize]).collect();
    columns.push(("truth".to_string(), Column::Truth(truth)));
    Batch { columns }
}

#[test]
fn ar_ver_233_every_width_and_exponent_round_trips() {
    let mut r = Rng(0x0a7a_7e00_2026_1109);
    for len in [0, 1, 7, 64, 65, 200] {
        let b = batch(&mut r, len);
        for bytes in [write_stream(&b).unwrap(), write_file(&b).unwrap()] {
            assert_eq!(
                read(&bytes, ReadOptions::default()).unwrap(),
                b,
                "{len} lanes"
            );
        }
    }
}

#[test]
fn ar_ver_233_decimal_widths() {
    // Decimal256 only past 38 digits: width 64 at an exponent of −28 or below.
    let b = Batch {
        columns: vec![
            (
                "a".into(),
                Column::Num(NumColumn {
                    width: Width::W64,
                    scale: -27,
                    lanes: vec![Some(Val::Fin(Width::W64.max_finite()))],
                }),
            ),
            (
                "b".into(),
                Column::Num(NumColumn {
                    width: Width::W64,
                    scale: -28,
                    lanes: vec![Some(Val::Fin(-Width::W64.max_finite()))],
                }),
            ),
        ],
    };
    let bytes = write_file(&b).unwrap();
    assert_eq!(read(&bytes, ReadOptions::default()).unwrap(), b);
}

#[test]
fn ar_ver_233_lengths_must_agree() {
    let b = Batch {
        columns: vec![
            ("a".into(), Column::Truth(vec![Truth::True])),
            ("b".into(), Column::Truth(vec![])),
        ],
    };
    assert!(write_stream(&b).is_err());
}

#[test]
fn ar_ver_233_stack_values_through_columns() {
    let stack = run("[ 1 2 NIL ] [ 1 0 DIV 0 0 DIV 3 ] [ TRUE NIL FALSE ] [ 1 1000 NIL ]").unwrap();
    let columns = to_columns(&stack).unwrap();
    let bytes = write_file(&Batch {
        columns: columns.clone(),
    })
    .unwrap();
    let back = read(&bytes, ReadOptions::default()).unwrap();
    assert_eq!(back.columns, columns);
    let values = to_values(&back.columns);
    // Same values; NIL lanes come back with the reason that they were read.
    let imported = Value::Nil(Reason::Imported);
    assert_eq!(
        values[0],
        Value::Vector(vec![
            stack_lane(&stack[0], 0),
            stack_lane(&stack[0], 1),
            imported.clone()
        ])
    );
    assert_eq!(values[1], stack[1]);
    assert_eq!(
        values[2],
        Value::Vector(vec![Value::Bool(true), imported, Value::Bool(false)])
    );
    // Lanes of differing widths share the widest.
    let Column::Num(c) = &columns[3].1 else {
        panic!("a number column")
    };
    assert_eq!((c.width, c.scale), (Width::W16, 0));
}

#[test]
fn ar_ver_233_scalars_and_empty_vectors() {
    let stack = run("5 TRUE NIL").unwrap();
    let back = read(
        &write_stream(&Batch {
            columns: to_columns(&stack).unwrap(),
        })
        .unwrap(),
        ReadOptions::default(),
    )
    .unwrap();
    let values = to_values(&back.columns);
    assert_eq!(values[0], Value::Vector(vec![stack[0].clone()]));
    assert_eq!(values[1], Value::Vector(vec![Value::Bool(true)]));
    assert_eq!(values[2], Value::Vector(vec![Value::Nil(Reason::Imported)]));
    assert_eq!(Reason::Imported.name(), "imported");
    let empty = to_columns(&run("[ ]").unwrap()).unwrap();
    assert_eq!(empty[0].1, Column::Truth(vec![]));
}

/// The stack a program leaves, its blocks read as data.
fn run(source: &str) -> Result<Vec<Value>, arare_interp::Error> {
    let mut m = Machine::new(arare_front::parse(source).unwrap());
    m.run()?;
    m.data_stack()
}

fn stack_lane(v: &Value, i: usize) -> Value {
    match v {
        Value::Vector(xs) => xs[i].clone(),
        _ => panic!("a vector"),
    }
}

#[test]
fn ar_ver_233_values_without_columns() {
    for source in ["[ 1 TRUE ]", "[ [ 1 ] ]", "[ 1 ] 'X'"] {
        let stack = run(source).unwrap();
        assert!(to_columns(&stack).is_err(), "{source}");
    }
    // A block that does not read as data stops before any column is made.
    assert!(run("[ ADD ]").is_err());
}
