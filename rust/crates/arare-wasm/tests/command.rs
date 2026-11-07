//! AR-VER-231: the command itself (AR-LLR-032): the request on standard
//! input, the command in its first argument, one JSON line on standard
//! output; misuse is exit status 2.

use std::io::Write;
use std::process::{Command, Stdio};

fn call(args: &[&str], input: &str) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_arare-wasm"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the command starts");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("the request is written");
    let out = child.wait_with_output().expect("the command ends");
    (
        out.status.code().expect("an exit status"),
        String::from_utf8(out.stdout).expect("UTF-8"),
        String::from_utf8(out.stderr).expect("UTF-8"),
    )
}

#[test]
fn ar_ver_231_the_command_answers_one_line() {
    let (code, out, _) = call(&["run"], "2 3 ADD");
    assert_eq!(code, 0);
    assert_eq!(out, format!("{}\n", arare_wasm::run("2 3 ADD")));
    let (code, out, _) = call(&["items"], "2 3 ADD");
    assert_eq!(
        (code, out.trim()),
        (0, arare_wasm::items("2 3 ADD").as_str())
    );
}

#[test]
fn ar_ver_231_misuse_is_status_2() {
    let (code, out, err) = call(&["compile"], "1");
    assert_eq!(code, 2);
    assert_eq!(out, "");
    assert!(err.contains("unknown command"), "{err}");
    let (code, _, err) = call(&[], "1");
    assert_eq!(code, 2, "{err}");
}
