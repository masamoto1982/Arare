//! AR-VER-231: the Playground's engine boundary (AR-LLR-032). A request's
//! programs run in order on one machine, and the answer is about the last
//! one: its stack with every value's width, scale and budget, its field
//! contract and where its items are written; an error with its category,
//! Word and position in the last program's own lines.

use arare_wasm::{items, node, render, respond, run};

const NUL: &str = "\0";

#[test]
fn ar_ver_231_a_number_with_width_scale_and_budget() {
    assert_eq!(
        run("1/3 8 -6 QUANTIZE [ 1 2 ] MUL SUM"),
        concat!(
            r#"{"ok":true,"stack":[{"type":"number","value":{"numerator":"63","denominator":"64"},"width":32,"scale":-6,"budget":"1/64"}],"#,
            r#""field":"closed","fieldExits":[],"decided":true,"undecided":[],"#,
            r#""items":[{"line":1,"column":1,"endLine":1,"endColumn":18},{"line":1,"column":19,"endLine":1,"endColumn":26},"#,
            r#"{"line":1,"column":27,"endLine":1,"endColumn":30},{"line":1,"column":31,"endLine":1,"endColumn":34}]}"#
        )
    );
}

#[test]
fn ar_ver_231_every_kind_of_value() {
    let out = run("TRUE NIL 1 0 DIV [ 1 2 ] [ 3 4 ] ADD [ 2 MUL ] 'X'");
    for part in [
        r#"{"type":"boolean","value":true,"budget":"0/1"}"#,
        r#"{"type":"nil","value":null,"semantics":{"absence":{"reason":"literal"}},"budget":"0/1"}"#,
        r#"{"type":"number","value":{"numerator":"1","denominator":"0"},"width":8,"scale":0,"budget":"0/1"}"#,
        r#"{"type":"vector","value":[{"type":"number","value":{"numerator":"4","denominator":"1"},"width":16,"scale":0},{"type":"number","value":{"numerator":"6","denominator":"1"},"width":16,"scale":0}],"budget":"0/1"}"#,
        r#"{"type":"block","value":"[ 2 MUL ]","budget":null}"#,
        r#"{"type":"string","value":"X","budget":"0/1"}"#,
        r#""field":"leaving","fieldExits":[{"word":"DIV","why":"wheelPoint"}]"#,
    ] {
        assert!(out.contains(part), "{part} missing from {out}");
    }
    assert!(
        run("[ NIL ] SUM").contains(r#""reason":"emptyAggregate""#),
        "a NIL keeps its reason"
    );
}

#[test]
fn ar_ver_231_quotes_render_as_source() {
    let (program, _) = arare_front::parse_spanned(
        "[ 1 -2 0.5 8 -1 QUANTIZE 1/0 8 0 QUANTIZE [ ADD ] 'N' BIND N ]",
    )
    .unwrap();
    let arare_front::Item::Quote(items) = &program.items[0] else {
        panic!("a quote")
    };
    let text = render(items);
    assert_eq!(
        text,
        "[ 1 -2 1/2 8 -1 QUANTIZE 1/0 8 0 QUANTIZE [ ADD ] 'N' BIND N ]"
    );
    assert_eq!(
        arare_front::parse(&text).unwrap(),
        program,
        "the text parses back to the same quote"
    );
    assert_eq!(
        node(&arare_interp::run("[ ]").unwrap()[0]),
        r#"{"type":"block","value":"[ ]"}"#
    );
}

#[test]
fn ar_ver_231_a_session_runs_its_earlier_programs_first() {
    // The stack and the names an earlier program left carry over.
    let out = run(&format!("5 'N' BIND{NUL}N N ADD"));
    assert!(
        out.starts_with(
            r#"{"ok":true,"stack":[{"type":"number","value":{"numerator":"10","denominator":"1"}"#
        ),
        "{out}"
    );
    // Only the last program's items are reported, in its own lines.
    assert!(
        out.ends_with(r#""items":[{"line":1,"column":1,"endLine":1,"endColumn":2},{"line":1,"column":3,"endLine":1,"endColumn":4},{"line":1,"column":5,"endLine":1,"endColumn":8}]}"#),
        "{out}"
    );
    let multi = run(&format!("1 2{NUL}3 # c{NUL}\nADD\nADD"));
    assert!(multi.contains(r#""numerator":"6""#), "{multi}");
    assert!(
        multi.ends_with(r#""items":[{"line":2,"column":1,"endLine":2,"endColumn":4},{"line":3,"column":1,"endLine":3,"endColumn":4}]}"#),
        "{multi}"
    );
}

#[test]
fn ar_ver_231_errors_say_where() {
    assert_eq!(
        run("1\n  ADD"),
        concat!(
            r#"{"ok":false,"error":{"message":"ADD: needs 2 operand(s), the stack has 1","category":"stackUnderflow","word":"ADD","line":2,"column":3},"#,
            r#""items":[{"line":1,"column":1,"endLine":1,"endColumn":2},{"line":2,"column":3,"endLine":2,"endColumn":6}]}"#
        )
    );
    assert_eq!(
        run(&format!("1{NUL}2 FOO")),
        r#"{"ok":false,"error":{"message":"unknown Word FOO","category":"syntax","word":null,"line":1,"column":3}}"#
    );
    assert_eq!(
        run("N"),
        r#"{"ok":false,"error":{"message":"unknown Word N","category":"syntax","word":null,"line":1,"column":1}}"#
    );
    for (src, category) in [
        ("TRUE 1 ADD", "type"),
        ("[ 1 2 ] [ 1 2 3 ] ADD", "shape"),
        ("[ [ 1 2 ] [ 3 ] ] [ 1 2 ] ADD", "shape"),
        ("[ 1 2 ADD ] 1 ADD", "type"),
        ("[ 1 ] [ 'K' BIND K ] MAP K", "unbound"),
        ("1 40 RESCALE", "scaleOverflow"),
        ("1 7 NARROW", "badOperand"),
        ("[ 1 2 ] [ DROP ] MAP", "syntax"),
        ("[ 1 2 ] [ 1 1 ] MAP", "blockResult"),
        ("[ X ] 'X' BIND", "selfReference"),
        ("[ 'Y' BIND ] MAP Y", "stackUnderflow"),
    ] {
        let out = run(src);
        assert!(
            out.contains(&format!(r#""category":"{category}""#)),
            "{src}: {out}"
        );
    }
}

#[test]
fn ar_ver_231_items_and_commands() {
    assert_eq!(
        items(&format!("1{NUL}0.5 8 -1 QUANTIZE [ 1 ]")),
        r#"{"ok":true,"items":[{"line":1,"column":1,"endLine":1,"endColumn":18},{"line":1,"column":19,"endLine":1,"endColumn":24}]}"#
    );
    assert_eq!(
        items("[ 1"),
        r#"{"ok":false,"error":{"message":"a [ is never closed","category":"syntax","word":null,"line":1,"column":3}}"#
    );
    assert_eq!(respond("items", "1").unwrap(), items("1"));
    assert_eq!(respond("run", "1").unwrap(), run("1"));
    assert!(respond("version", "")
        .unwrap()
        .starts_with(r#"{"engine":"arare","version":""#));
    assert!(respond("compile", "").is_err());
    assert!(
        run("'a\"b\\c'").contains(r#"a\"b\\c"#) || run("'ab'").contains(r#""value":"ab""#),
        "text is escaped as JSON"
    );
}
