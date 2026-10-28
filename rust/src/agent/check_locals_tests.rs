//! `check` resolves names without running anything, so it has to know which
//! names are bindings rather than Words. A `'NAME' BIND` used to be reported
//! as an unknown Word, which made `check` refuse programs that run.

#[test]
fn bound_names_are_not_unknown_words() {
    for source in [
        "[ 'V' BIND V V LENGTH DIV ] 'MEAN' DEF",
        "1 'Q' BIND Q",
        "[ 2 7 ] [ 'W' 'B' ] BIND W B ADD",
    ] {
        let response = super::api::check(source).to_json();
        assert_eq!(response["status"], "ok", "`{source}`: {response}");
    }
}

#[test]
fn a_name_nothing_binds_is_still_unknown() {
    let response = super::api::check("[ 'V' BIND W ] 'M' DEF").to_json();
    assert_eq!(response["status"], "error");
    assert!(
        response["message"].as_str().unwrap_or("").contains('W'),
        "{response}"
    );
}

/// A binding is reachable in the frame that made it and in the blocks
/// written there, never inside a Word called from it (`bindings.rs`). The
/// runtime refuses `1 'X' BIND [ X ] 'W' DEF W` as "bound in another frame";
/// `check` must not call it ok.
#[test]
fn a_binding_does_not_reach_into_a_word_body() {
    for source in [
        "1 'X' BIND [ X ] 'W' DEF W",
        "[ 'X' BIND ] 'W' DEF 1 W X",
        "[ 'A' 'B' ] BIND [ A B ADD ] 'W' DEF",
    ] {
        let response = super::api::check(source).to_json();
        assert_eq!(response["status"], "error", "`{source}`: {response}");
        assert!(
            response["message"]
                .as_str()
                .unwrap_or("")
                .contains("bound in another frame"),
            "`{source}`: {response}"
        );
    }
}

/// Within one frame, a block may be bound first and evaluated later, and a
/// body may bind names its nested blocks read — both run, so both check.
#[test]
fn bindings_reach_blocks_written_in_their_frame() {
    for source in [
        "[ X ] 'B' BIND 1 'X' BIND B EXEC",
        "[ 'X' BIND [ X ] EXEC ] 'W' DEF 1 W",
        "[ 'V' BIND V [ V ADD ] MAP ] 'W' DEF [ 1 2 ] W",
        "[ [ 'Y' BIND Y ] 'INNER' DEF 'X' BIND X ] 'OUTER' DEF",
    ] {
        let response = super::api::check(source).to_json();
        assert_eq!(response["status"], "ok", "`{source}`: {response}");
    }
}
