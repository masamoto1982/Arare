//! `ajisai repl` — an interactive read-eval-print loop (Phase 8A).
//!
//! The REPL keeps one stateful interpreter across lines, so the User Words
//! and the stack persist within a session — the same production Core
//! the `run` command drives, never the Python reference. The evaluation core
//! (`ReplSession`) is a pure function of `(session, line) -> ReplResponse` with
//! no I/O, so it is testable without a terminal; the terminal driver
//! (`run_repl`) is a thin shell over it. Prompts and banners go to stderr, so
//! stdout carries only results and stays pipe-safe (mirroring `agent compute`).
//!
//! Lines beginning with `:` are REPL *meta-commands* (`:quit`, `:reset`,
//! `:help`), handled by the host and kept strictly separate from Ajisai
//! surface syntax — they are not language words and never reach the interpreter.

use std::io::{BufRead, Write};

use crate::interpreter::{HostEffect, Interpreter};

use super::Opts;
use crate::agent::{block_on, stack_display};

/// Outcome kind of evaluating one REPL line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplStatus {
    Ok,
    Error,
}

impl ReplStatus {
    fn as_str(self) -> &'static str {
        match self {
            ReplStatus::Ok => "ok",
            ReplStatus::Error => "error",
        }
    }
}

/// Structured result of evaluating one line — the testable surface, free of I/O.
#[derive(Debug, Clone)]
pub(crate) struct ReplResponse {
    pub status: ReplStatus,
    /// The full stack after the line, bottom to top, as display strings.
    pub stack_display: Vec<String>,
    /// `PRINT` payloads produced by *this* line only, in order.
    pub output: Vec<String>,
    /// Error display string when `status` is `Error`.
    pub message: Option<String>,
}

/// A REPL meta-command, or a line to evaluate.
enum Line<'a> {
    Eval(&'a str),
    Quit,
    Reset,
    Help,
    Unknown(&'a str),
    Blank,
}

fn classify(line: &str) -> Line<'_> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Line::Blank;
    }
    if let Some(cmd) = trimmed.strip_prefix(':') {
        return match cmd.trim() {
            "quit" | "q" | "exit" => Line::Quit,
            "reset" => Line::Reset,
            "help" | "h" | "?" => Line::Help,
            other => Line::Unknown(other),
        };
    }
    Line::Eval(line)
}

/// A stateful REPL session over one persistent interpreter.
pub(crate) struct ReplSession {
    interp: Interpreter,
}

impl ReplSession {
    pub(crate) fn new() -> Self {
        Self {
            interp: Interpreter::new(),
        }
    }

    /// Evaluate one source line against the persistent session and report the
    /// resulting stack plus any output the line produced. Pure w.r.t. I/O.
    pub(crate) fn eval(&mut self, line: &str) -> ReplResponse {
        // Host effects accumulate on the interpreter across lines; take only the
        // slice this line appended so the REPL shows per-line output.
        let effects_before = self.interp.host_effects().len();
        let result = block_on(self.interp.execute(line));
        // Drain the legacy output buffer so it does not bleed into later lines;
        // the structured effect log is the source of truth for `output`.
        let _ = self.interp.collect_output();

        let output: Vec<String> = self.interp.host_effects()[effects_before..]
            .iter()
            .map(|effect| match effect {
                HostEffect::Print(payload) => payload.clone(),
            })
            .collect();
        let stack_display = stack_display(&self.interp);

        match result {
            Ok(()) => ReplResponse {
                status: ReplStatus::Ok,
                stack_display,
                output,
                message: None,
            },
            Err(err) => ReplResponse {
                status: ReplStatus::Error,
                stack_display,
                output,
                message: Some(err.to_string()),
            },
        }
    }

    /// Clear all session state (`:reset`).
    pub(crate) fn reset(&mut self) {
        // Clearing cannot fail; ignore the Result to keep the meta-command total.
        let _ = self.interp.execute_reset();
    }
}

const HELP: &str = "REPL commands:\n  \
    :help   show this help\n  \
    :reset  clear the stack and the User Words\n  \
    :quit   leave the REPL (Ctrl-D also works)\n\
Anything else is evaluated as Ajisai; the stack and definitions persist.";

/// Render one response to `out` (stdout). In JSON mode, one document per line;
/// in text mode, the output lines then the stack, one value per line.
fn render_response<W: Write>(out: &mut W, resp: &ReplResponse, json: bool) -> std::io::Result<()> {
    if json {
        let doc = serde_json::json!({
            "status": resp.status.as_str(),
            "stackDisplay": resp.stack_display,
            "output": resp.output,
            "message": resp.message,
        });
        writeln!(out, "{}", doc)
    } else {
        for payload in &resp.output {
            writeln!(out, "{}", payload)?;
        }
        if let Some(message) = &resp.message {
            writeln!(out, "error: {}", message)?;
        }
        if resp.stack_display.is_empty() {
            writeln!(out, "(empty stack)")
        } else {
            writeln!(out, "{}", resp.stack_display.join(" "))
        }
    }
}

/// Drive the REPL over `reader` (lines) and `out` (results). Prompts, the
/// banner, and help go to `err` so `out` stays pipe-safe. Returns when the
/// input ends or `:quit` is read.
pub(crate) fn run_repl<R: BufRead, W: Write, E: Write>(
    reader: R,
    mut out: W,
    mut err: E,
    opts: &Opts,
) -> std::io::Result<()> {
    let mut session = ReplSession::new();
    writeln!(err, "ajisai repl — :help for commands, :quit to leave.")?;

    for line in reader.lines() {
        let line = line?;
        match classify(&line) {
            Line::Blank => {}
            Line::Quit => break,
            Line::Help => writeln!(err, "{}", HELP)?,
            Line::Reset => {
                session.reset();
                writeln!(err, "session reset.")?;
            }
            Line::Unknown(cmd) => {
                writeln!(err, "unknown REPL command ':{}' — try :help", cmd)?;
            }
            Line::Eval(source) => {
                let resp = session.eval(source);
                render_response(&mut out, &resp, opts.json)?;
                out.flush()?;
            }
        }
    }
    Ok(())
}

/// `ajisai repl`: wire the driver to the process's stdin/stdout/stderr. The
/// banner, prompts, and help go to stderr so stdout stays pipe-safe.
pub(crate) fn cmd_repl(opts: &Opts) -> i32 {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    match run_repl(stdin.lock(), stdout.lock(), stderr.lock(), opts) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("ajisai repl: I/O error: {}", e);
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval_all(lines: &[&str]) -> Vec<ReplResponse> {
        let mut session = ReplSession::new();
        lines.iter().map(|line| session.eval(line)).collect()
    }

    #[test]
    fn stack_persists_across_lines() {
        let r = eval_all(&["[ 2 ] [ 3 ] ADD", "[ 10 ] MUL"]);
        assert_eq!(r[0].status, ReplStatus::Ok);
        assert_eq!(r[0].stack_display, vec!["[ 5/1 ]".to_string()]);
        // The second line multiplies the persisted [ 5 ] by [ 10 ].
        assert_eq!(r[1].stack_display, vec!["[ 50/1 ]".to_string()]);
    }

    #[test]
    fn definitions_persist_across_lines() {
        let r = eval_all(&["[ [ 2 ] MUL ] 'DBL' DEF", "[ 21 ] DBL"]);
        assert_eq!(r[1].status, ReplStatus::Ok);
        assert_eq!(r[1].stack_display, vec!["[ 42/1 ]".to_string()]);
    }

    #[test]
    fn output_is_per_line_not_cumulative() {
        let r = eval_all(&["[ 1 ] PRINT", "[ 2 ] PRINT"]);
        assert_eq!(r[0].output, vec!["[ 1/1 ]".to_string()]);
        // Only this line's PRINT, not the previous one's.
        assert_eq!(r[1].output, vec!["[ 2/1 ]".to_string()]);
    }

    #[test]
    fn error_reports_message_and_keeps_session_usable() {
        let mut session = ReplSession::new();
        let bad = session.eval("NOSUCHWORD");
        assert_eq!(bad.status, ReplStatus::Error);
        assert!(bad.message.is_some());
        // The session survives the error and keeps evaluating.
        let ok = session.eval("[ 7 ]");
        assert_eq!(ok.status, ReplStatus::Ok);
        assert_eq!(ok.stack_display, vec!["[ 7/1 ]".to_string()]);
    }

    #[test]
    fn reset_clears_the_stack() {
        let mut session = ReplSession::new();
        session.eval("[ 1 ] [ 2 ] [ 3 ]");
        session.reset();
        let after = session.eval("");
        assert!(after.stack_display.is_empty(), "reset clears the stack");
    }

    #[test]
    fn driver_is_pipe_safe_and_json_per_line() {
        let input = b"[ 2 ] [ 3 ] ADD\n:quit\n" as &[u8];
        let mut out = Vec::new();
        let mut err = Vec::new();
        let opts = Opts {
            json: true,
            compact: false,
            step_limit: None,
            limits: crate::cli::LimitProfile::Agent,
        };
        run_repl(input, &mut out, &mut err, &opts).unwrap();
        let stdout = String::from_utf8(out).unwrap();
        // One JSON document, and the banner went to stderr, not stdout.
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines.len(), 1, "one result line on stdout");
        let doc: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(doc["status"], "ok");
        assert_eq!(doc["stackDisplay"][0], "[ 5/1 ]");
        assert!(String::from_utf8(err).unwrap().contains("ajisai repl"));
    }
}
