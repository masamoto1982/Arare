//! The engine boundary the Playground drives (AR-LLR-032, refining
//! AR-HLR-034 and AR-HLR-035).
//!
//! Quality level QL-B (docs/quality/PLAN_OVERVIEW.md). The boundary is a
//! command, not a library of exported functions (ADR-0013): built for
//! `wasm32-wasip1`, it reads a request on standard input and writes one JSON
//! document on standard output, and the Playground instantiates it once per
//! request under a small WASI host (`src/engine/wasi.ts`). Exporting
//! functions by hand needs `#[no_mangle]`, which the workspace's
//! `forbid(unsafe_code)` refuses, as it does for the timing program
//! (ADR-0012).
//!
//! A request names a command and carries source text. The text is the
//! session's earlier programs, then the program to answer, separated by NUL
//! characters: the Playground keeps no engine alive between requests, so a
//! session is the programs that built its stack, and each request runs them
//! again before its own (ADR-0013). Their items run first, on the same
//! machine and in the same scope, so a name an earlier program bound is
//! still bound; what the request reports — its items, an error's position —
//! is about the last program alone, with its own line numbers.
//!
//! - `run` runs everything with the reference interpreter and answers the
//!   final stack: each value as the Playground's protocol node
//!   (`spec/host-protocol.schema.json`), a number with its exact rational,
//!   width and scale, a NIL with its reason, a quote with its source text;
//!   each value on the stack with the error budget the checker infers for it
//!   (`arare-budget`); the program's `field` contract; and where each item
//!   of the program is written. A program that is refused or stops answers
//!   `ok: false` with the message, its category, the Word and the position.
//! - `items` only parses, and answers where each item of the program is
//!   written: the pieces the Playground steps through.

#![forbid(unsafe_code)]

mod json;

use arare_budget::{analyze, budget, Abs, Analysis};
use arare_front::{parse_spanned, FrontError, Item, Program, Span};
use arare_interp::{ErrorKind, Machine, Value};
use json::string;

/// The separator between the session's earlier programs and the program a
/// request answers.
pub const SEPARATOR: char = '\0';

/// A request split into the source to run and where its last program
/// starts: the line before it, in the joined source.
struct Joined {
    source: String,
    offset: usize,
}

fn join(input: &str) -> Joined {
    let sections: Vec<&str> = input.split(SEPARATOR).collect();
    let (program, earlier) = sections
        .split_last()
        .expect("split yields one section at least");
    if earlier.is_empty() {
        return Joined {
            source: program.to_string(),
            offset: 0,
        };
    }
    let prelude = earlier.join("\n");
    let offset = prelude.matches('\n').count() + 1;
    Joined {
        source: format!("{prelude}\n{program}"),
        offset,
    }
}

/// A span of the last program, in its own line numbers.
fn span_json(s: &Span, offset: usize) -> String {
    format!(
        "{{\"line\":{},\"column\":{},\"endLine\":{},\"endColumn\":{}}}",
        s.line - offset,
        s.column,
        s.end_line - offset,
        s.end_column
    )
}

/// The spans of the items of the last program, and how many items come
/// before them.
fn own_items(spans: &[Span], offset: usize) -> (usize, String) {
    let before = spans.iter().take_while(|s| s.line <= offset).count();
    let own: Vec<String> = spans[before..]
        .iter()
        .map(|s| span_json(s, offset))
        .collect();
    (before, format!("[{}]", own.join(",")))
}

/// A quote's items as source text, the way they can be written again.
pub fn render(items: &[Item]) -> String {
    let mut out = String::from("[");
    for item in items {
        out.push(' ');
        match item {
            Item::Int(k) => out.push_str(&k.to_string()),
            Item::Num { num, exact: (p, q) } => out.push_str(&format!(
                "{p}/{q} {} {} QUANTIZE",
                num.width().bits(),
                num.scale()
            )),
            Item::Word(w) => out.push_str(w.name()),
            Item::Text(t) => out.push_str(&format!("'{t}'")),
            Item::Name(n) => out.push_str(n),
            Item::Quote(inner) => out.push_str(&render(inner)),
        }
    }
    out.push_str(" ]");
    out
}

/// One value as a protocol node.
pub fn node(v: &Value) -> String {
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
            "{{\"type\":\"nil\",\"value\":null,\"semantics\":{{\"absence\":{{\"reason\":\"{}\"}}}}}}",
            r.name()
        ),
        Value::Vector(xs) => format!(
            "{{\"type\":\"vector\",\"value\":[{}]}}",
            xs.iter().map(node).collect::<Vec<_>>().join(",")
        ),
        Value::Quote(items) => format!("{{\"type\":\"block\",\"value\":{}}}", string(&render(items))),
        Value::Text(name) => format!("{{\"type\":\"string\",\"value\":{}}}", string(name)),
    }
}

/// `node`, with the budget the checker infers for the value: a rational
/// `p/q`, or `null` when it knows nothing.
fn checked_node(v: &Value, abs: Option<&Abs>) -> String {
    let b = abs
        .and_then(budget)
        .map_or("null".to_string(), |q| format!("\"{q}\""));
    let n = node(v);
    format!("{},\"budget\":{b}}}", &n[..n.len() - 1])
}

fn field(a: &Analysis) -> String {
    let exits: Vec<String> = a
        .exits
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
    let contract = if a.closed() { "closed" } else { "leaving" };
    format!(
        "\"field\":\"{contract}\",\"fieldExits\":[{}]",
        exits.join(",")
    )
}

/// The category a host branches on, for an error the interpreter stops with.
pub fn category(kind: &ErrorKind) -> &'static str {
    match kind {
        ErrorKind::StackUnderflow { .. } => "stackUnderflow",
        ErrorKind::Type(_) => "type",
        ErrorKind::Shape { .. } => "shape",
        ErrorKind::ScaleOverflow => "scaleOverflow",
        ErrorKind::BadOperand(_) => "badOperand",
        ErrorKind::BlockResult(_) => "blockResult",
        ErrorKind::Unbound(_) => "unbound",
        ErrorKind::SelfReference(_) => "selfReference",
    }
}

/// An error document. `line` is in the last program's numbering; a position
/// in an earlier program (which ran before, so cannot fail) is reported as
/// it stands.
fn failure(
    message: &str,
    category: &str,
    word: Option<&str>,
    at: Option<(usize, usize)>,
) -> String {
    let word = word.map_or("null".to_string(), |w| format!("\"{w}\""));
    let at = at.map_or(String::new(), |(l, c)| {
        format!(",\"line\":{l},\"column\":{c}")
    });
    format!(
        "{{\"ok\":false,\"error\":{{\"message\":{},\"category\":\"{category}\",\"word\":{word}{at}}}",
        string(message)
    )
}

fn front_failure(e: &FrontError, offset: usize) -> String {
    let line = if e.line > offset {
        e.line - offset
    } else {
        e.line
    };
    // FrontError's Display leads with its own `line:column:`; the message
    // drops it, since the position travels on its own.
    let text = e.to_string();
    let message = text.splitn(3, ':').nth(2).map_or(text.as_str(), str::trim);
    failure(message, "syntax", None, Some((line, e.column))) + "}"
}

/// The `items` command: where each item of the last program is written.
pub fn items(input: &str) -> String {
    let joined = join(input);
    match parse_spanned(&joined.source) {
        Ok((_, spans)) => format!(
            "{{\"ok\":true,\"items\":{}}}",
            own_items(&spans, joined.offset).1
        ),
        Err(e) => front_failure(&e, joined.offset),
    }
}

/// The `run` command.
pub fn run(input: &str) -> String {
    let joined = join(input);
    let (program, spans) = match parse_spanned(&joined.source) {
        Ok(parsed) => parsed,
        Err(e) => return front_failure(&e, joined.offset),
    };
    let (before, items) = own_items(&spans, joined.offset);
    let analysis = analyze(&program);
    let mut machine = Machine::new(Program {
        items: program.items,
    });
    let mut index = 0;
    loop {
        match machine.step() {
            Ok(true) => index += 1,
            Ok(false) => break,
            Err(e) => {
                let span = spans[index];
                let at = (index >= before).then(|| (span.line - joined.offset, span.column));
                let word = e.word.map(|w| w.name());
                return failure(&e.to_string(), category(&e.kind), word, at)
                    + &format!(",\"items\":{items}}}");
            }
        }
    }
    let stack = machine.stack();
    let nodes: Vec<String> = stack
        .iter()
        .enumerate()
        .map(|(i, v)| checked_node(v, analysis.stack.get(i)))
        .collect();
    format!(
        "{{\"ok\":true,\"stack\":[{}],{},\"items\":{items}}}",
        nodes.join(","),
        field(&analysis)
    )
}

/// Answer one request: the command, then the source text.
pub fn respond(command: &str, input: &str) -> Result<String, String> {
    match command {
        "run" => Ok(run(input)),
        "items" => Ok(items(input)),
        "version" => Ok(format!(
            "{{\"engine\":\"arare\",\"version\":\"{}\"}}",
            env!("CARGO_PKG_VERSION")
        )),
        other => Err(format!(
            "unknown command {other:?}; expected run, items or version"
        )),
    }
}
