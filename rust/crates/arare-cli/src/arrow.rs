//! `arare run` with Arrow input and output (AR-LLR-043, ADR-0014).
//!
//! `--arrow-in FILE` reads an Arrow IPC file or stream and pushes each column
//! as a vector before the program runs, the first column at the bottom.
//! `--arrow-out FILE` writes the final stack, its blocks read as data, as an
//! Arrow IPC file, one column per value; `--arrow-out -` writes the stream
//! format to standard output instead of the stack. `--arrow-decimal W:E`
//! gives the width and exponent a decimal field Arare did not write is read
//! at (ADR-0004).

use arare_arrow::{read, to_columns, to_values, write_file, write_stream, Batch, ReadOptions};
use arare_interp::{Machine, RunError, Value};
use arare_kernel::Width;
use std::io::Write;

/// The Arrow flags of `arare run`; each takes a value.
pub const FLAGS: [&str; 3] = ["--arrow-in", "--arrow-out", "--arrow-decimal"];

#[derive(Default)]
pub struct Options {
    pub input: Option<String>,
    pub output: Option<String>,
    pub decimal: Option<(Width, i32)>,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Options, String> {
        let value = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| {
                    args.get(i + 1)
                        .cloned()
                        .ok_or(format!("{flag} needs a value"))
                })
                .transpose()
        };
        let decimal = match value("--arrow-decimal")? {
            None => None,
            Some(s) => {
                let bad =
                    || format!("--arrow-decimal takes WIDTH:EXPONENT, such as 32:-8, not {s}");
                let (w, e) = s.split_once(':').ok_or_else(bad)?;
                let w = Width::ALL
                    .into_iter()
                    .find(|x| x.bits().to_string() == w)
                    .ok_or_else(bad)?;
                Some((w, e.parse().map_err(|_| bad())?))
            }
        };
        Ok(Options {
            input: value("--arrow-in")?,
            output: value("--arrow-out")?,
            decimal,
        })
    }

    /// The vectors `--arrow-in` names, or none.
    pub fn inputs(&self) -> Result<Vec<Value>, String> {
        let Some(path) = &self.input else {
            return Ok(Vec::new());
        };
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        let batch = read(
            &bytes,
            ReadOptions {
                decimal: self.decimal,
            },
        )
        .map_err(|e| format!("{path}: {e}"))?;
        Ok(to_values(&batch.columns))
    }

    /// Writes the stack where `--arrow-out` says.
    pub fn write(&self, path: &str, stack: &[Value]) -> Result<(), String> {
        let batch = Batch {
            columns: to_columns(stack).map_err(|e| e.to_string())?,
        };
        if path == "-" {
            let bytes = write_stream(&batch).map_err(|e| e.to_string())?;
            return std::io::stdout()
                .write_all(&bytes)
                .map_err(|e| format!("cannot write the stream: {e}"));
        }
        let bytes = write_file(&batch).map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| format!("cannot write {path}: {e}"))
    }
}

/// Runs `source` on `inputs`; with `data`, the final stack has its blocks
/// read as data, as Arrow output needs.
pub fn run(source: &str, inputs: Vec<Value>, data: bool) -> Result<Vec<Value>, RunError> {
    let program = arare_front::parse(source).map_err(RunError::Front)?;
    let mut m = Machine::with_stack(program, inputs);
    m.run().map_err(RunError::Interp)?;
    if data {
        return m.data_stack().map_err(RunError::Interp);
    }
    Ok(m.stack().to_vec())
}
