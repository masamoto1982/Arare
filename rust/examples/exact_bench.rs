//! Exact-arithmetic comparison suite: the Ajisai side.
//!
//! Run with:  `cargo run --release --example exact_bench [-- FILTER] [--json]`
//!
//! The cases live in `scripts/bench/exact-bench-cases.json`, shared with
//! `scripts/bench/exact_bench.py`, which computes the same values with
//! `fractions.Fraction`, SymPy and python-flint. `scripts/bench/exact-bench.mjs`
//! runs both and reports a time only for a system whose answer matches, so
//! this side prints the answer it timed alongside the time.
//!
//! Each case is timed whole — the RANGE that builds its operands included —
//! as the Python side times its whole loop. The best of `repeats` runs is
//! reported. Like `speed_bench`, this is deliberately not a test: absolute
//! times are a property of one machine.

use ajisai_core::agent::time_after_setup;
use serde_json::{json, Value};

const CASES_JSON: &str = include_str!("../../scripts/bench/exact-bench-cases.json");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let as_json = args.iter().any(|a| a == "--json");
    let filter = args.iter().find(|a| !a.starts_with("--")).cloned();

    let suite: Value = serde_json::from_str(CASES_JSON).expect("exact-bench-cases.json parses");
    let repeats = suite["repeats"].as_u64().unwrap_or(5) as usize;

    let mut rows = Vec::new();
    for case in suite["cases"].as_array().expect("cases array") {
        let name = case["name"].as_str().unwrap();
        if filter.as_deref().is_some_and(|f| !name.contains(f)) {
            continue;
        }
        let n = case["n"].as_u64().unwrap();
        let source = case["source"]
            .as_str()
            .unwrap()
            .replace("{n}", &n.to_string());

        let mut best = f64::INFINITY;
        let mut answer = String::new();
        for _ in 0..repeats {
            let (interp, millis) = time_after_setup("", &source);
            best = best.min(millis);
            let stack = interp.get_stack();
            assert_eq!(stack.len(), 1, "`{name}` must leave one value");
            answer = stack[0].to_string();
        }
        // Untimed: the same value as decimal text, for a system that cannot
        // print the exact normal form.
        let decimal = case["decimalDigits"].as_u64().map(|digits| {
            let (interp, _) = time_after_setup("", &format!("{source} {digits} FORMAT"));
            interp.get_stack()[0]
                .to_string()
                .trim_matches('\'')
                .to_string()
        });
        // Untimed: the value's content identity, which another build of
        // Ajisai (exact-bench-wasm.mjs) checks its own answer against without
        // having to display it.
        let (interp, _) = time_after_setup("", &format!("{source} DIGEST"));
        let digest = interp.get_stack()[0]
            .to_string()
            .trim_matches('\'')
            .to_string();
        if !as_json {
            println!("{name:<14} n={n:<7} {best:>10.2} ms");
        }
        rows.push(json!({
            "name": name, "n": n, "ms": best,
            "answer": answer, "decimal": decimal, "digest": digest,
        }));
    }
    if as_json {
        println!("{}", Value::Array(rows));
    }
}
