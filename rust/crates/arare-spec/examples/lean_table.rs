//! Prints the specification's table to standard output for the Lean job of
//! CI to diff against `formal/Table.lean` (AR-LLR-001, AR-VER-201).

use std::io::{self, BufWriter};

fn main() -> io::Result<()> {
    arare_spec::write_table(&mut BufWriter::new(io::stdout().lock()))
}
