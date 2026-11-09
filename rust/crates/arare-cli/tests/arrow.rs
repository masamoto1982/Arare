//! AR-VER-236: `arare run` with Arrow (AR-LLR-043): the stack goes out as an
//! Arrow file or stream and comes back in as vectors, first column at the
//! bottom; misuse exits 2 and unreadable or unwritable Arrow exits 1.

use std::io::Write;
use std::process::{Command, Stdio};

fn arare(args: &[&str], input: &str) -> (i32, Vec<u8>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_arare"))
        .arg("run")
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
        out.stdout,
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn temp(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("arare-cli-arrow-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_string_lossy().into_owned()
}

#[test]
fn ar_ver_236_out_and_back_in() {
    let path = temp("a.arrow");
    let (code, out, _) = arare(&["--arrow-out", &path], "[ 1 2 3 ] [ 4 5 NIL ]");
    assert_eq!(code, 0);
    // The stack is still printed when the Arrow goes to a file.
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "[ 1/1 2/1 3/1 ]\n[ 4/1 5/1 NIL(literal) ]\n"
    );
    assert_eq!(std::fs::read(&path).unwrap()[..6], *b"ARROW1");
    let (code, out, err) = arare(&["--arrow-in", &path], "SUB");
    assert_eq!((code, err.as_str()), (0, ""));
    // The first column is at the bottom: [ 1 2 3 ] − [ 4 5 NIL ].
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "[ -3/1 -3/1 NIL(imported) ]\n"
    );
}

#[test]
fn ar_ver_236_stream_to_standard_output() {
    let (code, out, _) = arare(&["--arrow-out", "-"], "[ TRUE ]");
    assert_eq!(code, 0);
    assert_eq!(out[..4], [0xff; 4]);
    assert_eq!(out[out.len() - 8..], [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]);
}

#[test]
fn ar_ver_236_misuse_and_failures() {
    assert_eq!(arare(&["--arrow-in"], "").0, 2);
    assert_eq!(arare(&["--arrow-decimal", "12:0"], "").0, 2);
    assert_eq!(arare(&["--arrow-decimal", "32"], "").0, 2);
    let (code, _, err) = arare(&["--arrow-in", &temp("missing.arrow")], "");
    assert_eq!(code, 1);
    assert!(err.contains("cannot read"), "{err}");
    let junk = temp("junk.arrow");
    std::fs::write(&junk, b"not arrow").unwrap();
    let (code, _, err) = arare(&["--arrow-in", &junk], "");
    assert_eq!(code, 1);
    assert!(err.contains("not Arrow IPC"), "{err}");
    // A stack value with no column, and columns of different lengths.
    let (code, _, err) = arare(&["--arrow-out", &temp("b.arrow")], "[ [ 1 ] ]");
    assert_eq!(code, 1);
    assert!(err.contains("no Arrow column"), "{err}");
    let (code, _, err) = arare(&["--arrow-out", &temp("c.arrow")], "[ 1 ] [ 1 2 ]");
    assert_eq!(code, 1);
    assert!(err.contains("lanes beside"), "{err}");
}
