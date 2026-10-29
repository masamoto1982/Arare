//! AR-VER-214: the command-line interface (AR-LLR-017): `run` prints the
//! stack, `--json` writes Ajisai's value nodes with Arare's width and scale,
//! and the exit status says whether the program ran.

use std::io::Write;
use std::process::{Command, Stdio};

fn arare(args: &[&str], input: &str) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_arare"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn ar_ver_214_run() {
    assert_eq!(
        arare(&["run"], "1 2 ADD [ 1 0 ] 1 SWAP"),
        (
            1,
            String::new(),
            "arare: 1:19: unknown Word SWAP\n".to_string()
        )
    );
    assert_eq!(
        arare(&["run", "-"], "1 2 ADD NIL"),
        (0, "3/1\nNIL(literal)\n".to_string(), String::new())
    );
    assert_eq!(arare(&["run"], "ADD").0, 1);
}

#[test]
fn ar_ver_214_json() {
    let (code, out, _) = arare(
        &["run", "--json"],
        "0.75 8 -4 QUANTIZE 1 0 DIV [ TRUE NIL ]",
    );
    assert_eq!(code, 0);
    assert_eq!(
        out.trim(),
        concat!(
            r#"{"ok":true,"stack":["#,
            r#"{"type":"number","value":{"numerator":"3","denominator":"4"},"width":8,"scale":-4},"#,
            r#"{"type":"number","value":{"numerator":"1","denominator":"0"},"width":8,"scale":0},"#,
            r#"{"type":"block","value":null}]}"#
        )
    );
    let (code, out, _) = arare(&["run", "--json"], "[ TRUE NIL ] 1 RESCALE \"x\"");
    assert_eq!(code, 1);
    assert!(
        out.starts_with(r#"{"ok":false,"error":"1:24: unknown Word \"x\""#),
        "{out}"
    );
    let (_, out, _) = arare(&["run", "--json"], "[ TRUE NIL ] NOT");
    assert_eq!(
        out.trim(),
        r#"{"ok":true,"stack":[{"type":"vector","value":[{"type":"boolean","value":false},{"type":"nil","value":null,"reason":"literal"}]}]}"#
    );
}

#[test]
fn ar_ver_214_misuse() {
    assert_eq!(
        arare(&["version"], "").1,
        format!("arare {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(arare(&[], "").0, 2);
    assert_eq!(arare(&["run", "/no/such/file"], "").0, 2);
}

/// AR-VER-218: `check` (AR-LLR-022) prints each value with its error budget
/// and the program's `field` contract.
#[test]
fn ar_ver_218_check_text() {
    assert_eq!(
        arare(&["check"], "0.1 16 -12 QUANTIZE 2 3 ADD"),
        (
            0,
            "205/2048  ± 1/10240\n5/1  ± 0/1\nfield=closed\n".to_string(),
            String::new()
        )
    );
    let (code, out, _) = arare(
        &["check", "-"],
        "1 0 DIV 1 0.001 8 0 QUANTIZE DIV 1/0 8 0 QUANTIZE",
    );
    assert_eq!(code, 0);
    assert_eq!(
        out,
        "1/0  ± 0/1\n1/0  ± ?\n1/0  ± 0/1\nfield=open: literal wheelPoint, DIV wheelPoint, DIV divisorMayBeZero\n"
    );
    assert_eq!(
        arare(&["check"], "1 SWAP"),
        (
            1,
            String::new(),
            "arare: 1:3: unknown Word SWAP\n".to_string()
        )
    );
}

#[test]
fn ar_ver_218_check_json() {
    let (code, out, _) = arare(&["check", "--json"], "1 3 DIV 1 0.001 8 0 QUANTIZE DIV");
    assert_eq!(code, 0);
    assert_eq!(
        out.trim(),
        concat!(
            r#"{"ok":true,"stack":["#,
            r#"{"type":"number","value":{"numerator":"0","denominator":"1"},"width":8,"scale":0,"budget":"1/2"},"#,
            r#"{"type":"number","value":{"numerator":"1","denominator":"0"},"width":8,"scale":0,"budget":null}],"#,
            r#""field":"open","open":[{"word":"DIV","why":"divisorMayBeZero"}]}"#
        )
    );
    let (code, out, _) = arare(&["check", "--json"], "ADD");
    assert_eq!(code, 1);
    assert!(out.starts_with(r#"{"ok":false,"error":"ADD: "#), "{out}");
    let (_, out, _) = arare(&["check", "--json"], "1/0 8 0 QUANTIZE");
    assert!(
        out.contains(r#""field":"open","open":[{"word":null,"why":"wheelPoint"}]"#),
        "{out}"
    );
}

#[test]
fn ar_ver_218_check_lines() {
    // One JSON document per program, each line a program of its own; a
    // program that stops does not stop the others.
    let (code, out, _) = arare(&["check", "--lines"], "2 3 ADD\nADD\n1 2 LT\n");
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(
        lines[0].ends_with(r#""budget":"0/1"}],"field":"closed","open":[]}"#),
        "{}",
        lines[0]
    );
    assert!(lines[1].starts_with(r#"{"ok":false"#));
    assert!(lines[2].contains(r#"{"type":"boolean","value":true,"budget":"0/1"}"#));
    assert_eq!(arare(&["check", "/no/such/file"], "").0, 2);
}
