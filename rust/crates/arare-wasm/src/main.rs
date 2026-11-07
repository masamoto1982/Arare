//! The Playground's engine as a command (AR-LLR-032, ADR-0013):
//! `arare-wasm COMMAND`, the request's source text on standard input, one
//! JSON document on standard output. Exit status 0 when it answered, 2 on
//! misuse.

use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("arare-wasm: cannot read the request: {e}");
        return ExitCode::from(2);
    }
    match arare_wasm::respond(&command, &input) {
        Ok(answer) => {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{answer}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("arare-wasm: {e}");
            ExitCode::from(2)
        }
    }
}
