//! Agent-facing command-line interface (AR-LLR-017).
//!
//! Quality level QL-C (docs/quality/PLAN_OVERVIEW.md).
//!
//! ```text
//! arare version
//! arare run [--json] [FILE | -]
//! ```
//!
//! `run` executes a program with the reference interpreter and prints the
//! final stack, one value per line; with `--json`, one JSON document whose
//! value nodes follow Ajisai's protocol (`type`, and for a number its exact
//! `numerator` and `denominator`), plus Arare's `width` and `scale`, so the
//! oracle harness can read both languages the same way. Exit status 0 on
//! success, 1 when the program is refused or stops on an error, 2 on misuse.

use arare_interp::{run, Value};
use std::io::Read;
use std::process::ExitCode;

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn node(v: &Value) -> String {
    match v {
        Value::Num(n) => {
            let (p, q) = n.exact();
            format!(
                "{{\"type\":\"number\",\"value\":{{\"numerator\":\"{p}\",\"denominator\":\"{q}\"}},\"width\":{},\"scale\":{}}}",
                n.width().bits(),
                n.scale()
            )
        }
        Value::Bool(b) => format!("{{\"type\":\"boolean\",\"value\":{b}}}"),
        Value::Nil(r) => format!(
            "{{\"type\":\"nil\",\"value\":null,\"reason\":\"{}\"}}",
            r.name()
        ),
        Value::Vector(xs) => format!(
            "{{\"type\":\"vector\",\"value\":[{}]}}",
            xs.iter().map(node).collect::<Vec<_>>().join(",")
        ),
        Value::Quote(_) => "{\"type\":\"block\",\"value\":null}".to_string(),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("version") => {
            println!("arare {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("run") => {
            let json = args.iter().any(|a| a == "--json");
            let path = args
                .iter()
                .skip(1)
                .find(|a| *a != "--json")
                .map(String::as_str)
                .unwrap_or("-");
            let source = if path == "-" {
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s).map(|_| s)
            } else {
                std::fs::read_to_string(path)
            };
            let source = match source {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("arare: cannot read {path}: {e}");
                    return ExitCode::from(2);
                }
            };
            match run(&source) {
                Ok(stack) => {
                    if json {
                        println!(
                            "{{\"ok\":true,\"stack\":[{}]}}",
                            stack.iter().map(node).collect::<Vec<_>>().join(",")
                        );
                    } else {
                        stack.iter().for_each(|v| println!("{v}"));
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    if json {
                        println!("{{\"ok\":false,\"error\":{}}}", json_string(&e.to_string()));
                    } else {
                        eprintln!("arare: {e}");
                    }
                    ExitCode::from(1)
                }
            }
        }
        _ => {
            eprintln!("usage: arare version | arare run [--json] [FILE | -]");
            ExitCode::from(2)
        }
    }
}
