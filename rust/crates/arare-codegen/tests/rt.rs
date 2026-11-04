//! AR-VER-224: what compiled code runs with (AR-LLR-023): columns checked
//! once when they are made, the two facts the fast path reads (a wheel point,
//! a NIL lane), the values compiled code answers, and the fallback to the
//! interpreter.

use arare_codegen::rt::{self, Arg, Codes, Column, ColumnError, Out};
use arare_interp::{Reason, Value};
use arare_kernel::{Num, Val, Width};

#[test]
fn ar_ver_224_columns() {
    let c = Column::new(Width::W8, -2, Codes::I8(vec![1, -3, 126]), None).unwrap();
    assert_eq!(
        (c.width(), c.scale(), c.len(), c.is_empty()),
        (Width::W8, -2, 3, false)
    );
    assert!(!c.has_points() && !c.has_nils());
    assert_eq!(
        c.plain_codes(Width::W8, -2),
        Some(&Codes::I8(vec![1, -3, 126]))
    );
    assert_eq!(c.plain_codes(Width::W8, 0), None, "another scale");
    assert_eq!(c.plain_codes(Width::W16, -2), None, "another width");
    assert_eq!(c.lane(1), Some(Val::Fin(-3)));
    // Points and NIL are found when the column is made.
    let p = Column::new(Width::W8, 0, Codes::I8(vec![1, 127, -128]), None).unwrap();
    assert!(p.has_points() && !p.has_nils());
    assert_eq!(p.plain_codes(Width::W8, 0), None);
    assert_eq!((p.lane(1), p.lane(2)), (Some(Val::Inf), Some(Val::Bot)));
    let n = Column::new(Width::W16, 0, Codes::I16(vec![5, 0, 7]), Some(vec![0b101])).unwrap();
    assert!(n.has_nils() && !n.has_points());
    assert_eq!(n.lane(1), None);
    assert_eq!(n.plain_codes(Width::W16, 0), None);
    // A full validity bitmap is no NIL at all.
    let full = Column::new(Width::W16, 0, Codes::I16(vec![5, 6]), Some(vec![0b11])).unwrap();
    assert!(!full.has_nils() && full.plain_codes(Width::W16, 0).is_some());
    // A NIL lane's code is not read.
    assert!(Column::new(Width::W8, 0, Codes::I8(vec![-127, 1]), Some(vec![0b10])).is_ok());
    // Refusals.
    assert_eq!(
        Column::new(Width::W8, 0, Codes::I16(vec![1]), None),
        Err(ColumnError::WidthMismatch)
    );
    assert_eq!(
        Column::new(Width::W8, 0, Codes::I8(vec![0, -127]), None),
        Err(ColumnError::NotACode(1))
    );
    assert_eq!(
        Column::new(Width::W8, 0, Codes::I8(vec![0]), Some(vec![])),
        Err(ColumnError::ValidityLength)
    );
    assert_eq!(
        Column::new(Width::W8, 40, Codes::I8(vec![0]), None),
        Err(ColumnError::Scale)
    );
    // Every width's codes and accessor.
    for (w, codes) in [
        (Width::W8, Codes::I8(vec![3])),
        (Width::W16, Codes::I16(vec![3])),
        (Width::W32, Codes::I32(vec![3])),
        (Width::W64, Codes::I64(vec![3])),
    ] {
        let c = Column::new(w, 1, codes.clone(), None).unwrap();
        assert_eq!(c.lane(0), Some(Val::Fin(3)));
        assert_eq!(c.plain_codes(w, 1), Some(&codes));
        assert_eq!(codes.get(0), 3);
    }
    let empty = Column::new(Width::W32, 0, Codes::I32(vec![]), None).unwrap();
    assert!(empty.is_empty() && Codes::I32(vec![]).is_empty());
}

#[test]
fn ar_ver_224_values() {
    let c = Column::of_values(Width::W8, 0, &[Some(Val::Fin(2)), None, Some(Val::Inf)]).unwrap();
    assert!(c.has_nils() && c.has_points());
    let num = |k: i64| Value::Num(Num::new(Val::Fin(k), Width::W8, 0).unwrap());
    assert_eq!(
        c.to_value(),
        Value::Vector(vec![
            num(2),
            Value::Nil(Reason::Literal),
            Value::Num(Num::new(Val::Inf, Width::W8, 0).unwrap())
        ])
    );
    // Every width's codes are made from values.
    for w in [Width::W8, Width::W16, Width::W32, Width::W64] {
        let c = Column::of_values(w, 0, &[Some(Val::Fin(-3)), None]).unwrap();
        assert_eq!(
            (c.width(), c.lane(0), c.lane(1)),
            (w, Some(Val::Fin(-3)), None)
        );
        assert_eq!(Arg::Column(c.clone()).to_value(), c.to_value());
    }
    let a = Arg::Num(Num::int(5).unwrap());
    assert_eq!(a.to_value(), Value::Num(Num::int(5).unwrap()));
    assert_eq!(a.plain_num(Width::W8, 0), Some(5));
    assert_eq!(a.plain_num(Width::W16, 0), None);
    assert_eq!(a.plain_num(Width::W8, 1), None);
    assert!(a.column().is_none());
    let point = Arg::Num(Num::new(Val::Bot, Width::W8, 0).unwrap());
    assert_eq!(point.plain_num(Width::W8, 0), None);
    let col = Arg::Column(c.clone());
    assert!(col.column().is_some() && col.plain_num(Width::W8, 0).is_none());
    assert_eq!(Out::Column(c.clone()).to_value(), c.to_value());
    assert_eq!(
        Out::Bools(vec![true, false]).to_value(),
        Value::Vector(vec![Value::Bool(true), Value::Bool(false)])
    );
    assert_eq!(rt::truth(true).to_value(), Value::Bool(true));
    assert_eq!(
        rt::num(7, Width::W16, 2).to_value(),
        Value::Num(Num::new(Val::Fin(7), Width::W16, 2).unwrap())
    );
    let computed = Column::computed(Width::W16, 0, Codes::I16(vec![1, 2]));
    assert_eq!(
        computed.to_value(),
        Value::Vector(vec![
            Value::Num(Num::new(Val::Fin(1), Width::W16, 0).unwrap()),
            Value::Num(Num::new(Val::Fin(2), Width::W16, 0).unwrap())
        ])
    );
}

#[test]
fn ar_ver_224_sum_and_fallback() {
    let widths = [Width::W16, Width::W32, Width::W64];
    assert_eq!(
        rt::sum(0, 0, &widths, 0),
        Some(Out::Value(Value::Nil(Reason::EmptyAggregate)))
    );
    assert_eq!(rt::sum(1, 5, &widths, 0), Some(rt::num(5, Width::W16, 0)));
    assert_eq!(rt::sum(9, 5, &widths, 0), Some(rt::num(5, Width::W64, 0)));
    assert_eq!(
        rt::sum(1, 40000, &widths, 0),
        None,
        "beyond the width of one lane"
    );
    assert_eq!(rt::sum(9, i128::MAX, &widths, 0), None, "beyond int64");
    assert!(rt::sum_fits(10, 100, 1000));
    assert!(!rt::sum_fits(11, 100, 1000));
    assert!(!rt::sum_fits(usize::MAX, u128::MAX, u128::MAX));
    // The fallback is the interpreter on the inputs, errors included.
    let args = [
        Arg::Num(Num::int(2).unwrap()),
        Arg::Num(Num::int(3).unwrap()),
    ];
    let out = rt::fallback("ADD", &args).unwrap();
    assert_eq!(
        out[0].to_value(),
        Value::Num(Num::new(Val::Fin(5), Width::W16, 0).unwrap())
    );
    assert!(rt::fallback("ADD ADD", &args).is_err());
}
