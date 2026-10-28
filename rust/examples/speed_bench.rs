//! Wall-clock speed suite for the interpreter's hot paths.
//!
//! Run with:  `cargo run --release --example speed_bench [-- FILTER] [--json]`
//!
//! The cases live in `scripts/bench/speed-bench-cases.json`, shared with
//! `scripts/bench/speed-bench-wasm.mjs`, which times the same programs on the
//! WebAssembly build. Each case builds its operand with `setup`, then times
//! `source` alone (`agent::time_after_setup`), so the timed region is the
//! computation and not the RANGE that feeds it. The best of `repeats` runs is
//! reported: the minimum is the run least disturbed by the machine, which is
//! what a before/after comparison of one change wants.
//!
//! Like `work_meter_calibration`, this is deliberately not a test: absolute
//! times are a property of one machine. The cases cover each route a program
//! can take — the column walk over a block, its general tier, the column
//! kernels, the engine at top level and inside unfused blocks, User Word
//! calls, BigInt arithmetic — so a change to one
//! route shows up in its own row.

use ajisai_core::agent::time_after_setup;
use serde_json::Value;

const CASES_JSON: &str = include_str!("../../scripts/bench/speed-bench-cases.json");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let filter = args.iter().find(|a| !a.starts_with("--")).cloned();

    let suite: Value = serde_json::from_str(CASES_JSON).expect("speed-bench-cases.json parses");
    let repeats = suite["repeats"].as_u64().unwrap_or(5) as usize;

    let mut rows = Vec::new();
    for case in suite["cases"].as_array().expect("cases array") {
        let name = case["name"].as_str().unwrap();
        if let Some(f) = &filter {
            if !name.contains(f.as_str()) {
                continue;
            }
        }
        let setup = case["setup"].as_str().unwrap();
        let source = case["source"]
            .as_str()
            .unwrap()
            .repeat(case["repeat"].as_u64().unwrap_or(1) as usize);
        let elements = case["elements"].as_u64().unwrap() as f64;

        let mut best = f64::INFINITY;
        for _ in 0..repeats {
            let (interp, millis) = time_after_setup(setup, &source);
            std::hint::black_box(interp.get_stack().len());
            best = best.min(millis);
        }
        let ns_per = best * 1.0e6 / elements;
        if !json {
            println!("{name:<22} {best:>10.2} ms {ns_per:>10.1} ns/elem");
        }
        rows.push(format!(
            "  {{\"name\":\"{name}\",\"ms\":{best:.3},\"nsPerElem\":{ns_per:.2}}}"
        ));
    }
    if json {
        println!("[\n{}\n]", rows.join(",\n"));
    }
}
