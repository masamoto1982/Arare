//! Compiles `programs.txt` with arare-codegen into `$OUT_DIR/compiled.rs`
//! (AR-LLR-025): every program the translation validation and the
//! benchmarks run, as the compiled route runs it. Nothing generated is
//! committed; the build makes it from the list each time.

use arare_codegen::{emit, Input, PRELUDE};
use std::fmt::Write;

fn main() {
    let list = std::fs::read_to_string("programs.txt").expect("programs.txt");
    let mut out = String::from(PRELUDE);
    let mut table = String::from("/// Every compiled program: its name, signature, source, the compiled\n/// function and its fast path alone.\npub const PROGRAMS: &[Program] = &[\n");
    for line in list
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let mut cells = line.split('|').map(str::trim);
        let (name, sig, source) = (
            cells.next().expect("a name"),
            cells.next().expect("a signature"),
            cells.next().expect("a source"),
        );
        let inputs = Input::parse_list(sig).unwrap_or_else(|e| panic!("{name}: {e}"));
        out.push_str(&emit(name, source, &inputs));
        let _ = writeln!(table, "    Program {{ name: {name:?}, signature: {sig:?}, source: {source:?}, run: {name}, fast: {name}_fast }},");
    }
    table.push_str("];\n");
    out.push_str(&table);
    let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("compiled.rs");
    std::fs::write(path, out).unwrap();
    println!("cargo:rerun-if-changed=programs.txt");
}
