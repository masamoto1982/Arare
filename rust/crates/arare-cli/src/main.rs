//! Agent-facing command-line interface (AR-LLR-017, AR-LLR-022).
//!
//! Quality level QL-C (docs/quality/PLAN_OVERVIEW.md).
//!
//! ```text
//! arare version
//! arare run [--json] [FILE | -]
//! arare check [--json] [--lines] [FILE | -]
//! ```
//!
//! `run` executes a program with the reference interpreter and prints the
//! final stack, one value per line; with `--json`, one JSON document whose
//! value nodes follow Ajisai's protocol (`type`, and for a number its exact
//! `numerator` and `denominator`), plus Arare's `width` and `scale`, so the
//! oracle harness can read both languages the same way. Exit status 0 on
//! success, 1 when the program is refused or stops on an error, 2 on misuse.
//!
//! `check` runs the program the same way, then the error-budget checker
//! (`arare-budget`): each value is printed with its budget, the most its
//! numbers may differ from the exact result Ajisai computes, and the program
//! with its `field` contract, `closed` or `open` and why. With `--json`, each
//! value node carries `"budget"`: a rational, or `null` when the checker
//! knows nothing. With `--lines`, every input line is a program of its own and
//! gets a JSON document of its own, for the oracle harness's thousands of
//! programs (AR-VER-217); the exit status is then 0 unless input cannot be read.

use arare_budget::{analyze, budget, Abs, Analysis};
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

/// `node`, with the budget `abs` gives the value.
fn checked_node(v: &Value, abs: &Abs) -> String {
    let b = budget(abs).map_or("null".to_string(), |q| format!("\"{q}\""));
    let n = node(v);
    format!("{},\"budget\":{b}}}", &n[..n.len() - 1])
}

fn field(a: &Analysis) -> String {
    let open: Vec<String> = a
        .open
        .iter()
        .map(|o| {
            format!(
                "{{\"word\":{},\"why\":\"{}\"}}",
                o.word
                    .map_or("null".to_string(), |w| format!("\"{}\"", w.name())),
                o.why.name()
            )
        })
        .collect();
    let contract = if a.closed() { "closed" } else { "open" };
    format!("\"field\":\"{contract}\",\"open\":[{}]", open.join(","))
}

/// One program checked: a JSON document, or the text lines, and whether it ran.
fn check(source: &str, json: bool) -> (String, bool) {
    let stack = match run(source) {
        Ok(stack) => stack,
        Err(e) if json => {
            return (
                format!("{{\"ok\":false,\"error\":{}}}", json_string(&e.to_string())),
                false,
            )
        }
        Err(e) => return (format!("arare: {e}"), false),
    };
    let program = arare_front::parse(source).expect("the program ran, so it parses");
    let a = analyze(&program);
    if json {
        let nodes: Vec<String> = stack
            .iter()
            .zip(&a.stack)
            .map(|(v, x)| checked_node(v, x))
            .collect();
        return (
            format!(
                "{{\"ok\":true,\"stack\":[{}],{}}}",
                nodes.join(","),
                field(&a)
            ),
            true,
        );
    }
    let mut out: Vec<String> = stack
        .iter()
        .zip(&a.stack)
        .map(|(v, x)| match budget(x) {
            Some(q) => format!("{v}  ± {q}"),
            None => format!("{v}  ± ?"),
        })
        .collect();
    if a.closed() {
        out.push("field=closed".to_string());
    } else {
        let why: Vec<String> = a
            .open
            .iter()
            .map(|o| {
                format!(
                    "{} {}",
                    o.word.map_or("literal", |w| w.name()),
                    o.why.name()
                )
            })
            .collect();
        out.push(format!("field=open: {}", why.join(", ")));
    }
    (out.join("\n"), true)
}

fn read(path: &str) -> Result<String, ExitCode> {
    let source = if path == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).map(|_| s)
    } else {
        std::fs::read_to_string(path)
    };
    source.map_err(|e| {
        eprintln!("arare: cannot read {path}: {e}");
        ExitCode::from(2)
    })
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
            let source = match read(path) {
                Ok(s) => s,
                Err(code) => return code,
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
        Some("check") => {
            let flags = ["--json", "--lines"];
            let lines = args.iter().any(|a| a == "--lines");
            let json = lines || args.iter().any(|a| a == "--json");
            let path = args
                .iter()
                .skip(1)
                .find(|a| !flags.contains(&a.as_str()))
                .map(String::as_str)
                .unwrap_or("-");
            let source = match read(path) {
                Ok(s) => s,
                Err(code) => return code,
            };
            if lines {
                source
                    .lines()
                    .for_each(|l| println!("{}", check(l, true).0));
                return ExitCode::SUCCESS;
            }
            let (out, ok) = check(&source, json);
            if ok || json {
                println!("{out}");
            } else {
                eprintln!("{out}");
            }
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        _ => {
            eprintln!(
                "usage: arare version | arare run [--json] [FILE | -] | arare check [--json] [--lines] [FILE | -]"
            );
            ExitCode::from(2)
        }
    }
}
