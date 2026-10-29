//! Prints the checker's table to standard output for the Lean job of CI to
//! diff against `formal/BudgetTable.lean` (AR-LLR-019, AR-VER-215).

use std::io::{self, BufWriter};

fn main() -> io::Result<()> {
    arare_budget::table::write_table(&mut BufWriter::new(io::stdout().lock()))
}
