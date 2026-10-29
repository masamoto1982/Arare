//! AR-VER-216-C: the checker over programs (AR-LLR-020, AR-LLR-021): what it
//! infers of each Word's result, the `field` contract and why it is open,
//! and that it follows the interpreter's stack exactly and the same way on
//! every run.

use arare_budget::{analyze, budget, Abs, Analysis, Info, Open, Why, Q};
use arare_front::{parse, Word};

fn check(src: &str) -> Analysis {
    analyze(&parse(src).unwrap())
}

/// The budget of every value on the final stack, as text.
fn budgets(src: &str) -> Vec<String> {
    check(src)
        .stack
        .iter()
        .map(|a| budget(a).map_or("?".to_string(), |q| q.to_string()))
        .collect()
}

fn opens(src: &str) -> Vec<(Option<Word>, Why)> {
    check(src).open.iter().map(|o| (o.word, o.why)).collect()
}

#[test]
fn ar_ver_216_c_budgets() {
    for (src, want) in [
        ("2 3 ADD", vec!["0/1"]),
        ("0.1 16 -12 QUANTIZE", vec!["1/10240"]),
        ("0.3 8 -4 QUANTIZE 3 MUL", vec!["3/80"]),
        ("1 3 DIV", vec!["1/2"]),
        ("7 1 RESCALE 2 NEG", vec!["1/1", "0/1"]),
        (
            "100 16 NARROW 0.5 8 -1 QUANTIZE 16 -4 QUANTIZE",
            vec!["0/1", "0/1"],
        ),
        // A vector's budget is its largest lane's.
        ("[ 1 2 3 ] 0.1 16 -12 QUANTIZE MUL", vec!["3/10240"]),
        ("[ 1 2 ] [ 3 ] ADD [ 4 ] [ 1 2 ] ADD", vec!["0/1", "0/1"]),
        ("[ ] SUM", vec!["0/1"]),
        ("[ 1 NIL 3 ] SUM", vec!["0/1"]),
        ("[ NIL NIL ] SUM", vec!["0/1"]),
        ("NIL 1 ADD", vec!["0/1"]),
        // Comparisons agree when exact or apart; not when an error may flip them.
        (
            "1 2 LT 0.1 16 -12 QUANTIZE 0.1 16 -12 QUANTIZE LT",
            vec!["0/1", "?"],
        ),
        ("0.1 16 -12 QUANTIZE 5 LT", vec!["0/1"]),
        ("1 0 DIV 2 LT", vec!["0/1"]),
        ("2 2 EQ 0.1 16 -12 QUANTIZE 1 EQ", vec!["0/1", "0/1"]),
        ("0.1 16 -12 QUANTIZE 0.1 16 -11 QUANTIZE EQ", vec!["?"]),
        (
            "TRUE FALSE AND NOT FALSE NIL AND NIL NIL AND",
            vec!["0/1", "0/1", "0/1"],
        ),
    ] {
        assert_eq!(budgets(src), want, "{src}");
    }
}

#[test]
fn ar_ver_216_c_select() {
    // A condition that agrees picks the same branch in both meanings.
    assert_eq!(budgets("1 0.1 16 -12 QUANTIZE TRUE SELECT"), ["1/10240"]);
    assert_eq!(budgets("1 2 NIL SELECT"), ["0/1"]);
    // One that may differ: within the hull [1/10, 1], the worse error plus
    // the hull's width.
    let src = "1 0.1 16 -12 QUANTIZE 0.1 16 -12 QUANTIZE 0.1 16 -11 QUANTIZE EQ SELECT";
    assert_eq!(budgets(src), ["9217/10240"]);
    // Lane by lane, and against vectors of unknown length.
    assert_eq!(
        budgets("[ 1 2 ] [ 3 4 ] [ TRUE FALSE ] SELECT [ 1 ] 0 [ TRUE ] SELECT"),
        ["0/1", "0/1"]
    );
    assert_eq!(
        budgets("[ 1 2 3 ] [ 2 LT ] FILTER [ 0 ] [ 1 2 3 ] [ 2 LT ] FILTER [ 0 LT ] MAP SELECT"),
        ["0/1"]
    );
    assert_eq!(budgets("[ 1 2 ] [ 1 ] 3 ADD [ 4 ] SELECT"), ["?"]);
    assert_eq!(budgets("[ 1 ] [ 2 ] SELECT"), ["?"]);
    // NIL against a number agrees as a join, not as a cross.
    assert_eq!(budgets("NIL 1 TRUE TRUE EQ NOT SELECT"), ["0/1"]);
    let flip = "0.1 16 -12 QUANTIZE 0.1 16 -11 QUANTIZE EQ";
    assert_eq!(budgets(&format!("NIL 1 {flip} SELECT")), ["?"]);
    assert_eq!(budgets(&format!("NIL NIL {flip} SELECT")), ["0/1"]);
    assert_eq!(budgets(&format!("TRUE NIL {flip} SELECT")), ["?"]);
    assert_eq!(budgets(&format!("[ 1 2 ] [ 3 4 ] {flip} SELECT")), ["2/1"]);
    assert_eq!(budgets(&format!("[ 1 2 ] [ 3 ] {flip} SELECT")), ["?"]);
}

#[test]
fn ar_ver_216_c_vectors_of_unknown_length() {
    let a = check("[ 5 1 4 2 ] [ 3 LT ] FILTER");
    match &a.stack[..] {
        [Abs::Hull { max: 4, elem }] => assert!(matches!(**elem, Abs::Num { .. })),
        other => panic!("{other:?}"),
    }
    for (src, want) in [
        ("[ 5 1 4 2 ] [ 3 LT ] FILTER SUM", "0/1"),
        (
            "[ 5 1 4 2 ] [ 3 LT ] FILTER [ 2 MUL ] MAP 0 [ ADD ] FOLD",
            "0/1",
        ),
        ("[ 5 1 4 2 ] [ 3 LT ] FILTER 0.5 8 -1 QUANTIZE ADD", "0/1"),
        ("[ 5 1 ] [ 3 LT ] FILTER [ 1 2 ] ADD", "0/1"),
        ("[ 5 1 ] [ 3 LT ] FILTER [ 1 ] ADD", "0/1"),
        ("[ 5 1 ] [ 3 LT ] FILTER [ 5 1 ] [ 3 LT ] FILTER MUL", "0/1"),
        ("[ ] [ 3 LT ] FILTER", "0/1"),
        ("[ 1 ] [ 3 LT ] FILTER NEG", "0/1"),
        ("[ 1 ] [ 3 LT ] FILTER [ 1 ] EQ", "0/1"),
        ("[ 1 ] [ 3 LT ] FILTER [ 1 ] [ 3 LT ] FILTER EQ", "0/1"),
        ("[ 1 2 ] [ 3 LT ] FILTER [ 0 LT ] MAP [ TRUE ] AND", "0/1"),
        ("[ 1 2 ] [ 3 LT ] FILTER [ 1 LT ] MAP NOT", "0/1"),
        // A FOLD that settles stops early.
        ("[ 1 2 3 ] [ 9 LT ] FILTER 0 [ MUL 0 MUL ] FOLD", "0/1"),
        // MUL in a FOLD of unknown length: each iteration widens the scale.
        (
            "[ 1 2 ] [ 9 LT ] FILTER 0.5 8 -1 QUANTIZE [ MUL ] FOLD",
            "0/1",
        ),
    ] {
        assert_eq!(budgets(src), [want], "{src}");
    }
    // A predicate an error may flip loses the length.
    assert_eq!(
        budgets("[ 1 2 ] 0.1 16 -12 QUANTIZE MUL [ 0.1 16 -12 QUANTIZE LT ] FILTER"),
        ["?"]
    );
    // A NIL among the lanes leaves the sum possibly NIL.
    match &check("[ NIL 1 ] [ 9 LT ] FILTER SUM").stack[..] {
        [x] => assert!(matches!(x, Abs::Num { nil: true, .. } | Abs::Top), "{x:?}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn ar_ver_216_c_field() {
    assert!(check("2 3 ADD 0.1 16 -12 QUANTIZE MUL 3 DIV").closed());
    assert!(check("[ 1 2 3 ] [ 2 MUL ] MAP SUM").closed());
    for (src, want) in [
        ("1 0 DIV", vec![(Some(Word::Div), Why::WheelPoint)]),
        ("1/0 8 0 QUANTIZE", vec![(None, Why::WheelPoint)]),
        ("200 8 NARROW", vec![(Some(Word::Narrow), Why::MayOverflow)]),
        ("2/1 8 7 QUANTIZE 1 ADD", vec![]),
        (
            "300 8 0 QUANTIZE",
            vec![(Some(Word::Quantize), Why::MayOverflow)],
        ),
        ("1.5 8 6 QUANTIZE", vec![]),
        ("1000.5 8 -4 QUANTIZE", vec![(None, Why::MayOverflow)]),
        (
            "1 0.001 8 0 QUANTIZE DIV",
            vec![(Some(Word::Div), Why::DivisorMayBeZero)],
        ),
        (
            "1 0.001 8 0 QUANTIZE DIV 1 ADD",
            vec![
                (Some(Word::Add), Why::Unbounded),
                (Some(Word::Div), Why::DivisorMayBeZero),
            ],
        ),
        (
            "3000000000 3000000000 MUL 3000000000 MUL",
            vec![(Some(Word::Mul), Why::MayOverflow)],
        ),
        (
            "[ 3000000000 1 ] [ 3000000000 1 ] MUL SUM 3000000000 MUL",
            vec![(Some(Word::Mul), Why::MayOverflow)],
        ),
        (
            "[ 3000000000 3000000000 ] [ 2 LT ] FILTER 3000000000 [ MUL ] FOLD",
            vec![(Some(Word::Mul), Why::MayOverflow)],
        ),
        (
            "1 0 DIV NEG",
            vec![
                (Some(Word::Div), Why::WheelPoint),
                (Some(Word::Neg), Why::WheelPoint),
            ],
        ),
    ] {
        assert_eq!(opens(src), want, "{src}");
        assert_eq!(check(src).closed(), want.is_empty(), "{src}");
    }
    assert_eq!(Why::WheelPoint.name(), "wheelPoint");
    assert_eq!(Why::MayOverflow.name(), "mayOverflow");
    assert_eq!(Why::DivisorMayBeZero.name(), "divisorMayBeZero");
    assert_eq!(Why::Unbounded.name(), "unbounded");
    let o = Open {
        word: None,
        why: Why::Unbounded,
    };
    assert_eq!(o, o.clone());
}

#[test]
fn ar_ver_216_c_operands_the_checker_cannot_follow() {
    // A shift, width or exponent computed with an error is not the same in
    // both meanings, so the Word's result is not bounded.
    for (src, word) in [
        ("5 0.3 8 -4 QUANTIZE 10 MUL RESCALE", Word::Rescale),
        ("5 1 3 DIV 16 MUL NARROW", Word::Narrow),
        ("5 1 3 DIV 16 MUL -2 QUANTIZE", Word::Quantize),
        ("5 16 1 3 DIV QUANTIZE", Word::Quantize),
        ("TRUE NEG", Word::Neg),
        ("TRUE 1 ADD", Word::Add),
    ] {
        let a = check(src);
        assert_eq!(budget(&a.stack[0]), None, "{src}");
        assert!(a.open.iter().any(|o| o.word == Some(word)), "{src}");
    }
}

#[test]
fn ar_ver_216_c_kinds_it_does_not_follow() {
    // Malformed programs the interpreter refuses: the checker answers
    // nothing for the Word, never a wrong budget, and never panics.
    for src in [
        "ADD",
        "[ 1 ] [ 1 2 3 ] [ 1 2 ] ADD ADD",
        "1 [ 2 ] MAP",
        "[ 1 2 ] 3 FILTER",
        "[ 1 2 ] [ 1 2 ] FILTER",
        "[ 1 2 ] 0 [ 1 2 ] FOLD",
        "1 0 [ ADD ] FOLD",
        "TRUE SUM",
        "[ TRUE ] SUM",
        "[ 1 2 ] [ DUP ] MAP",
        "1 TRUE AND",
        "1 NOT",
        "1 2 3 SELECT",
        "[ 1 2 ] [ 1 2 ] LT TRUE LT",
        "[ 1 2 ] [ TRUE ] MAP [ 1 ] LT",
    ] {
        if let Ok(p) = parse(src) {
            let a = analyze(&p);
            assert!(a.stack.iter().any(|x| budget(x).is_none()), "{src}");
        }
    }
}

#[test]
fn ar_ver_216_c_follows_the_interpreter() {
    // The same number of values as the interpreter leaves, and the same
    // analysis on every run (WORK_ORDER.md §5.5: the checker is a tool that
    // stands in for verification, so it must be deterministic).
    for src in [
        "1 2 3",
        "[ 1 2 3 ] [ 2 MUL ] MAP [ 3 LT ] FILTER SUM 1 0.5 8 -1 QUANTIZE ADD",
        "1 2 LT 3 4 TRUE SELECT [ 1 2 ] [ 3 4 ] EQ",
        "0.1 16 -12 QUANTIZE 3 [ ADD ] [ 1 2 ] 0 [ ADD ] FOLD",
    ] {
        let a = check(src);
        let stack = arare_interp::run(src).unwrap();
        assert_eq!(a.stack.len(), stack.len(), "{src}");
        assert_eq!(a, check(src));
    }
    // A quote left unread is the same code in both meanings, with no budget.
    let a = check("[ 1 2 ]");
    assert!(matches!(a.stack[..], [Abs::Quote(_)]));
    assert_eq!(budget(&a.stack[0]), None);
    // Nothing known of a number is said as nothing.
    assert_eq!(
        budget(&Abs::Num {
            info: Info::Top,
            nil: false
        }),
        None
    );
    assert_eq!(budget(&Abs::Lanes(vec![Abs::Top])), None);
    assert_eq!(budget(&Abs::Truth { agree: false }), None);
    assert_eq!(budget(&Abs::Nil), Some(Q::ZERO));
}

#[test]
fn ar_ver_216_c_every_shape_each_word_meets() {
    let flip = "0.1 16 -12 QUANTIZE 0.1 16 -11 QUANTIZE EQ";
    let hull = "[ 1 2 ] [ 3 LT ] FILTER";
    for (src, want) in [
        ("5 2 SUB", "0/1"),
        ("1 [ 1 2 ] ADD", "0/1"),
        (&format!("1 {hull} ADD"), "0/1"),
        ("NIL NEG", "0/1"),
        ("[ 1 2 ] NEG", "0/1"),
        ("NIL NOT", "0/1"),
        ("[ TRUE FALSE ] NOT", "0/1"),
        ("NIL 1 EQ", "0/1"),
        ("NIL SUM", "0/1"),
        ("NIL [ 1 ADD ] MAP", "0/1"),
        ("2 1 0 DIV LT", "0/1"),
        (&format!("TRUE {hull} [ 0 LT ] MAP AND"), "0/1"),
        ("TRUE 1 AND", "?"),
        ("1 [ 1 LT ] FILTER", "?"),
        ("5 NIL RESCALE", "?"),
        // SELECT: lanes against a hull, a one-lane vector, and a scalar.
        (&format!("{hull} [ 0 ] [ TRUE ] SELECT"), "0/1"),
        ("[ 1 ] [ 3 4 ] [ TRUE FALSE ] SELECT", "0/1"),
        (&format!("1 2 {hull} [ 0 LT ] MAP SELECT"), "0/1"),
        // Joins of every kind, through a SELECT whose condition agrees.
        ("NIL NIL TRUE SELECT", "0/1"),
        ("TRUE FALSE TRUE SELECT", "0/1"),
        ("NIL TRUE TRUE SELECT", "0/1"),
        ("[ 1 2 ] [ 3 4 ] TRUE SELECT", "0/1"),
        ("[ 1 2 ] [ 3 ] TRUE SELECT", "0/1"),
        ("[ ] [ 3 ] TRUE SELECT", "0/1"),
        (&format!("{hull} [ 5 ] TRUE SELECT"), "0/1"),
        (&format!("{flip} {flip} TRUE SELECT"), "?"),
        // EQ against vectors of unknown length, and across kinds.
        (&format!("{hull} 1 EQ"), "0/1"),
        (&format!("{hull} [ 1 2 ] EQ"), "0/1"),
        (&format!("{hull} {hull} EQ"), "0/1"),
        (&format!("[ 1 ] {flip} EQ"), "?"),
        ("TRUE 1 ADD 1 EQ", "?"),
        ("[ 1 2 ] [ 1 2 ] EQ", "0/1"),
        // A FILTER of a FILTER; a FOLD whose iterations stop agreeing in kind.
        (&format!("{hull} [ 2 LT ] FILTER"), "0/1"),
        (&format!("{hull} TRUE [ ADD ] FOLD"), "?"),
        // A sum that may be NIL, carried through arithmetic.
        ("[ 1 2 ] [ 0 LT ] FILTER SUM 1 ADD", "0/1"),
        // A wheel point flows into a result the checker cannot bound.
        ("1 0 DIV 0.001 8 0 QUANTIZE MUL", "?"),
        // A sum that may be NIL, joined; a vector joined with a scalar.
        ("[ 1 2 ] [ 0 LT ] FILTER SUM 1 TRUE SELECT", "0/1"),
        ("[ 1 ] 1 TRUE SELECT", "?"),
        // Lanes whose equality an error may flip; AND over such a truth.
        (
            "[ 1 ] 0.1 16 -12 QUANTIZE MUL [ 1 ] 0.1 16 -11 QUANTIZE MUL EQ",
            "?",
        ),
        (&format!("{flip} TRUE AND"), "?"),
        (&format!("TRUE {flip} EQ"), "?"),
        (&format!("{flip} TRUE EQ"), "?"),
        (&format!("{flip} 1 EQ"), "?"),
        ("[ 1 2 ] [ 1 ] EQ", "0/1"),
        // The largest lane's budget, wherever it is.
        ("[ 2 1 ] 0.1 16 -12 QUANTIZE MUL", "1/5120"),
    ] {
        assert_eq!(budgets(src), [want], "{src}");
    }
    assert_eq!(
        opens("1 0 DIV 0.001 8 0 QUANTIZE MUL"),
        [
            (Some(Word::Mul), Why::WheelPoint),
            (Some(Word::Div), Why::WheelPoint)
        ]
    );
    // Helpers on values that are not vectors.
    assert_eq!(arare_budget::value::max_len(&Abs::Nil), 0);
    assert_eq!(arare_budget::value::lanes_joined(&Abs::Nil), None);
}
