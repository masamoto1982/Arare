//! AR-VER-223: lowering and emission (AR-LLR-024, AR-LLR-025). Every Word
//! the fast path covers lowers to the node the kernel's widths and scales
//! give, every program it does not cover is refused with a reason, and the
//! emitted text is deterministic, with a fast path exactly when lowering
//! succeeds. The compiled programs themselves are held to the interpreter by
//! AR-VER-220.

use arare_codegen::ir::{Input, Kind, Op, Output};
use arare_codegen::{emit, lower};
use arare_kernel::Width;

const V8: Input = Input::Vector {
    width: Width::W8,
    scale: 0,
};
const S8: Input = Input::Scalar {
    width: Width::W8,
    scale: 0,
};
const V16: Input = Input::Vector {
    width: Width::W16,
    scale: 0,
};

fn kind(source: &str, inputs: &[Input]) -> Kind {
    let ir = lower(source, inputs).unwrap_or_else(|e| panic!("{source}: {e}"));
    let n = match ir.outputs.last().expect("an output") {
        Output::Scalar(n) | Output::Vector(n) => *n,
    };
    ir.nodes[n].kind
}

fn num(width: Width, scale: i32) -> Kind {
    Kind::Num { width, scale }
}

#[test]
fn ar_ver_223_widths_and_scales_are_the_kernels() {
    for (source, inputs, want) in [
        ("ADD", vec![V8, V8], num(Width::W16, 0)),
        ("SUB", vec![V8, V8], num(Width::W16, 0)),
        ("MUL", vec![V8, V8], num(Width::W16, 0)),
        ("DIV", vec![V8, V8], num(Width::W8, 0)),
        ("NEG", vec![V8], num(Width::W8, 0)),
        ("3 RESCALE", vec![V16], num(Width::W16, 3)),
        ("8 NARROW", vec![V16], num(Width::W8, 0)),
        ("8 -2 QUANTIZE", vec![V16], num(Width::W8, -2)),
        ("16 2 QUANTIZE", vec![V8], num(Width::W16, 2)),
        ("SUM", vec![V8], num(Width::W64, 0)),
        ("[ 2 MUL ] MAP", vec![V8], num(Width::W16, 0)),
        ("0.5 8 -1 QUANTIZE ADD", vec![V8], num(Width::W16, -1)),
        ("LT", vec![V8, S8], Kind::Bool),
        ("EQ", vec![S8, S8], Kind::Bool),
        ("TRUE EQ", vec![S8], Kind::Bool),
        ("LT TRUE EQ", vec![S8, S8], Kind::Bool),
        ("LT NOT TRUE AND", vec![V8, V8], Kind::Bool),
        ("LT SELECT", vec![V8, V8, V8, V8], num(Width::W8, 0)),
        ("LT SELECT", vec![S8, S8, S8, S8], num(Width::W8, 0)),
        ("LT SELECT", vec![V8, V8, S8, S8], num(Width::W8, 0)),
        ("TRUE FALSE 1 2 LT SELECT", vec![], Kind::Bool),
    ] {
        assert_eq!(kind(source, &inputs), want, "{source}");
    }
}

#[test]
fn ar_ver_223_bounds_and_checks() {
    // int8 × int8 needs no check; int64 + int64 may overflow and is checked.
    let ir = lower("MUL", &[V8, V8]).unwrap();
    let mul = ir.nodes.last().unwrap();
    assert_eq!(
        (mul.op, mul.bound, mul.check),
        (Op::Mul(0, 1), 126 * 126, false)
    );
    let w64 = Input::Vector {
        width: Width::W64,
        scale: 0,
    };
    let ir = lower("ADD", &[w64, w64]).unwrap();
    assert!(ir.nodes.last().unwrap().check);
    // Aligning scales shifts the operand with the larger exponent.
    let ir = lower(
        "ADD",
        &[
            Input::Vector {
                width: Width::W8,
                scale: -2,
            },
            Input::Vector {
                width: Width::W16,
                scale: 3,
            },
        ],
    )
    .unwrap();
    assert!(matches!(
        ir.nodes.last().unwrap().op,
        Op::Add { da: 0, db: 5, .. }
    ));
    // DIV is always checked: its divisor may be zero.
    let ir = lower("DIV", &[V8, V8]).unwrap();
    assert!(ir.nodes.last().unwrap().check);
    // SUM records the widths of the running sum until they settle.
    let ir = lower("SUM", &[V8]).unwrap();
    assert_eq!(
        ir.sums[0].widths,
        vec![Width::W8, Width::W16, Width::W32, Width::W64]
    );
    // A lane node and a uniform one.
    let ir = lower("2 MUL", &[V8]).unwrap();
    assert!(ir.nodes.last().unwrap().lane);
    let ir = lower("SUM 2 MUL", &[V8]).unwrap();
    let last = ir.nodes.last().unwrap();
    assert!(last.late && !last.lane);
}

#[test]
fn ar_ver_223_refusals() {
    for (source, inputs, why) in [
        ("SWAP", vec![], "front end"),
        ("ADD", vec![V8], "too few operands"),
        ("[ 1 2 ] ADD", vec![V8], "a vector written as a quote"),
        ("1/0 8 0 QUANTIZE", vec![], "a literal wheel point"),
        ("NIL", vec![], "a literal NIL"),
        ("[ 3 LT ] FILTER", vec![V8], "FILTER and FOLD"),
        ("0 [ ADD ] FOLD", vec![V8], "FILTER and FOLD"),
        ("SUM MUL", vec![V8, V8], "second pass"),
        ("SUM", vec![S8], "SUM of a scalar"),
        ("[ SUM ] MAP", vec![V8], "SUM of a scalar"),
        ("[ 1 ] MAP", vec![S8], "MAP over a scalar"),
        ("1 MAP", vec![V8], "MAP without a block"),
        ("[ 1 2 ] MAP", vec![V8], "does not leave one scalar"),
        ("EQ", vec![V8, V8], "EQ of vectors"),
        ("EQ", vec![S8, V8], "EQ of vectors"),
        ("AND", vec![S8, S8], "AND of numbers"),
        ("1 2 LT 3 AND", vec![], "AND of numbers"),
        ("NOT", vec![S8], "NOT of a number"),
        ("TRUE ADD", vec![S8], "a truth value where a number"),
        (
            "LT SELECT",
            vec![V8, S8, S8, S8],
            "between a vector and a scalar",
        ),
        ("1 SELECT", vec![S8, S8], "SELECT on a number"),
        (
            "0.5 8 -1 QUANTIZE TRUE SELECT",
            vec![S8],
            "different widths or scales",
        ),
        ("[ 1 ]", vec![], "a quote left on the stack"),
        ("[ 1 ] 2 ADD", vec![], "a vector written as a quote"),
        ("DUP2", vec![], "front end"),
        ("63 RESCALE", vec![V8], "outside 0 to 62"),
        ("0.5 8 -1 QUANTIZE RESCALE", vec![V8], "not an integer"),
        ("2 3 ADD RESCALE", vec![V8], "a computed operand"),
        ("9 NARROW", vec![V8], "a width other than"),
        ("8 40 QUANTIZE", vec![V8], "exponent outside"),
        ("[ 2 ] RESCALE", vec![V8], "a vector written as a quote"),
        ("RESCALE", vec![V8, V8], "a vector where a literal"),
        (
            "1 RESCALE",
            vec![Input::Vector {
                width: Width::W8,
                scale: 31,
            }],
            "leaves ±31",
        ),
        (
            "MUL",
            vec![
                Input::Vector {
                    width: Width::W8,
                    scale: 20,
                },
                Input::Vector {
                    width: Width::W8,
                    scale: 20,
                },
            ],
            "leaves ±31",
        ),
        (
            "8 -31 QUANTIZE 8 -31 QUANTIZE 2 MUL",
            vec![Input::Vector {
                width: Width::W8,
                scale: 5,
            }],
            "",
        ),
        (
            "ADD",
            vec![Input::Scalar {
                width: Width::W8,
                scale: 40,
            }],
            "input scale",
        ),
        ("9223372036854775806 RESCALE", vec![V8], "outside 0 to 62"),
        (
            "9223372036854775807.0 64 31 QUANTIZE RESCALE",
            vec![V8],
            "out of range",
        ),
    ] {
        match lower(source, &inputs) {
            Err(e) => assert!(e.to_string().contains(why), "{source}: {e}"),
            Ok(_) => assert!(why.is_empty(), "{source} lowered"),
        }
    }
}

#[test]
fn ar_ver_223_emission() {
    // Deterministic, a fast path exactly when lowering succeeds, the source
    // kept verbatim for the fallback.
    for (source, inputs) in [
        ("MUL SUM", vec![V8, V8]),
        ("[ 3 LT ] FILTER", vec![V8]),
        ("ADD", vec![V8, V8]),
    ] {
        let a = emit("k", source, &inputs);
        assert_eq!(a, emit("k", source, &inputs));
        assert!(a.contains(&format!("rt::fallback({source:?}, args)")));
        assert!(a.contains("pub fn k_fast(") && a.contains("pub fn k("));
        assert_eq!(
            a.contains("No fast path"),
            lower(source, &inputs).is_err(),
            "{source}"
        );
    }
    // The shapes the emitter chooses between.
    let dot = emit("k", "MUL SUM", &[V8, V8]);
    assert!(dot.contains("let mut p") && dot.contains("rt::sum(n,"));
    let add = emit("k", "ADD", &[V8, V8]);
    assert!(add.contains(".extend((0..len).map("));
    let two = emit("k", "ADD NEG", &[V8, V8, V8]);
    assert!(two.contains("extend_from_slice"));
    let mixed = emit("k", "SUM", &[V8, V8]);
    assert!(mixed.contains("extend_from_slice") && mixed.contains("rt::sum("));
    let bools = emit("k", "LT", &[V8, V8]);
    assert!(bools.contains("Out::Bools"));
    let bools2 = emit("k", "LT LT", &[V8, V8, V8]);
    assert!(bools2.contains("Out::Bools") || bools2.contains("No fast path"));
    let lanes_and_bools = emit("k", "LT", &[V8, V8, V8]);
    assert!(lanes_and_bools.contains("Out::Bools") && lanes_and_bools.contains("Out::Column"));
    let scalar = emit("k", "ADD 3 MUL LT", &[S8, S8, S8]);
    assert!(scalar.contains("rt::truth("));
    let narrow = emit("k", "0.5 8 -1 QUANTIZE MUL 8 NARROW", &[V16]);
    assert!(narrow.contains("trap |="));
    let wide = emit(
        "k",
        "DIV",
        &[
            Input::Vector {
                width: Width::W64,
                scale: 0,
            },
            V8,
        ],
    );
    assert!(wide.contains("rt::round_div("));
    let fast_div = emit("k", "DIV", &[V8, V8]);
    assert!(fast_div.contains("rt::div_f64("));
    let wide_rescale = emit(
        "k",
        "2 RESCALE",
        &[Input::Vector {
            width: Width::W64,
            scale: 0,
        }],
    );
    assert!(wide_rescale.contains("rt::rescale_code("));
    let down = emit("k", "16 -3 QUANTIZE", &[V8]);
    assert!(down.contains("<< 3"));
    let up = emit("k", "8 2 QUANTIZE", &[V16]);
    assert!(up.contains("rt::rescale_i32("));
    let cmp = emit(
        "k",
        "EQ",
        &[
            Input::Scalar {
                width: Width::W8,
                scale: 3,
            },
            S8,
        ],
    );
    assert!(cmp.contains("=="));
    let sel = emit("k", "LT SELECT", &[S8, S8, S8, S8]);
    assert!(sel.contains("if v"));
    let selb = emit("k", "TRUE FALSE 1 2 LT SELECT", &[]);
    assert!(selb.contains("if v"));
    let none = emit("k", "1 2 ADD", &[]);
    assert!(none.contains("let n = 0usize;"));
    let w64 = Input::Vector {
        width: Width::W64,
        scale: 0,
    };
    let big_divisor = emit("k", "DIV", &[V8, w64]);
    assert!(big_divisor.contains("rt::round_div("));
    let truth_eq = emit("k", "LT TRUE EQ", &[S8, S8]);
    assert!(truth_eq.contains("== v"));
    let zero = emit("k", "0 RESCALE", &[V8]);
    assert!(zero.contains("rt::rescale_i32("));
    // A shift that saturates the bound: the code is checked.
    let w15 = Input::Vector {
        width: Width::W64,
        scale: 15,
    };
    let huge = emit("k", "MUL 64 -31 QUANTIZE", &[w15, w15]);
    assert!(huge.contains("trap |=") && huge.contains("i128"));
    let after = emit("k", "SUM 2 MUL", &[V8]);
    assert!(after.contains("rt::num("));
}

#[test]
fn ar_ver_223_signatures() {
    assert_eq!(
        Input::parse_list("v8:0 s32:-30 v64:5").unwrap(),
        vec![
            V8,
            Input::Scalar {
                width: Width::W32,
                scale: -30
            },
            Input::Vector {
                width: Width::W64,
                scale: 5
            }
        ]
    );
    for bad in ["x8:0", "v9:0", "v8", "v8:a", "", "v:1"] {
        if bad.is_empty() {
            assert_eq!(Input::parse_list(bad).unwrap(), vec![]);
        } else {
            assert!(Input::parse_list(bad).is_err(), "{bad}");
        }
    }
}
