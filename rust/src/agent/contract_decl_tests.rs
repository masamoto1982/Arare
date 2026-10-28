//! Tests for `#:contract` declarations (LANG.CONTRACT.CHECK): each is
//! verified, cannot be verified, or is violated, and a violation stops the
//! run before anything executes.

use crate::agent::api::check;
use serde_json::Value as Json;

fn contract_decls(source: &str) -> Json {
    check(source).to_json()["contractDecls"].clone()
}

fn exit_code(source: &str) -> i32 {
    check(source).exit_code()
}

#[test]
fn a_correct_declaration_is_verified() {
    for source in [
        "[ [ 1 2 ] ] 'PAIR' DEF\n#:contract PAIR inputs=0 outputs=1",
        "[ [ 2 MUL ] MAP ] 'D' DEF\n#:contract D inputs=1 outputs=1 field=closed",
        "[ 2 DIV ] 'H' DEF\n#:contract H field=closed",
    ] {
        let decls = contract_decls(source);
        assert_eq!(decls["outcome"], "value", "{source}: {decls}");
        assert_eq!(decls["declarations"][0]["outcome"], "value");
        assert_eq!(exit_code(source), 0, "{source}");
    }
}

#[test]
fn a_wrong_declaration_is_violated() {
    for source in [
        "[ 1 ADD ] 'W' DEF\n#:contract W inputs=2",
        "[ 1 ADD ] 'W' DEF\n#:contract W outputs=0",
        "[ 0 DIV ] 'W' DEF\n#:contract W field=closed",
    ] {
        let decls = contract_decls(source);
        assert_eq!(decls["outcome"], "error", "{source}: {decls}");
        assert_eq!(decls["findings"][0]["severity"], "error");
        assert_eq!(exit_code(source), 1, "{source}");
    }
}

#[test]
fn what_the_inference_cannot_decide_is_a_note() {
    for source in [
        "[ 2 COLLECT ] 'W' DEF\n#:contract W inputs=2",
        "[ MAP ] 'W' DEF\n#:contract W field=closed",
        "[ 1 ] 'W' DEF [ 2 ] 'W' DEF\n#:contract W outputs=1",
        "[ 1 ] 'W' 'X' BIND X DEF\n#:contract W outputs=1",
    ] {
        let decls = contract_decls(source);
        assert_eq!(decls["outcome"], "nil", "{source}: {decls}");
        assert_eq!(decls["findings"][0]["severity"], "note");
        assert_eq!(exit_code(source), 0, "{source}");
    }
}

#[test]
fn a_malformed_or_dangling_directive_is_an_error() {
    for source in [
        "[ 1 ] 'W' DEF\n#:contract W purity=pure",
        "[ 1 ] 'W' DEF\n#:contract W inputs=-1",
        "[ 1 ] 'W' DEF\n#:contract W field=finite",
        "#:contract",
        "#:contract NOWHERE inputs=0",
    ] {
        assert_eq!(contract_decls(source)["outcome"], "error", "{source}");
        assert_eq!(exit_code(source), 1, "{source}");
    }
}

#[test]
fn the_worst_declaration_decides_the_outcome() {
    let source = "[ 1 ] 'A' DEF [ 2 COLLECT ] 'B' DEF\n\
                  #:contract A outputs=1\n#:contract B inputs=2";
    assert_eq!(contract_decls(source)["outcome"], "nil");
    let source = format!("{source}\n#:contract A outputs=3");
    assert_eq!(contract_decls(&source)["outcome"], "error");
}

#[tokio::test]
async fn compute_refuses_a_violated_declaration_before_running() {
    let source = "[ 'ran' PRINT 1 ADD ] 'W' DEF\n#:contract W inputs=2\n1 W";
    let json = crate::agent::api::compute(source, Default::default())
        .await
        .to_json();
    assert_eq!(json["error"]["category"], "contractViolation");
    assert_eq!(json["error"]["word"], "W");
    assert_eq!(json["output"], serde_json::json!([]));
}
