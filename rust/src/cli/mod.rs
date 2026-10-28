//! Headless `ajisai` CLI: the agent-facing write → run → read-structured-error
//! loop, entirely in a terminal.
//!
//! Commands (see `docs/dev/agent-cli-output-contract.md` for the JSON
//! output contract):
//!
//! ```text
//! ajisai run <file.ajisai>                 # human-readable
//! ajisai check <file.ajisai>               # tokenize + parse + resolve, no execution
//! ajisai contract <file.ajisai>            # report inferred word contracts, no execution
//! ajisai agent <operation> <file.ajisai>   # the one JSON boundary for host adapters
//!   operations: compute, check, infer-contracts
//! ajisai test <file-or-dir> [--json]       # execute `#@` host test directives
//! ajisai repl [--json]                     # persistent interactive session
//! ajisai version [--json]
//! ```
//!
//! Exit codes: 0 = success, 1 = language error (diagnosis emitted),
//! 2 = CLI usage error. `agent` (and `test`/`repl`/`version` with `--json`)
//! write exactly one JSON document to stdout and nothing else (pipe-safe);
//! usage errors go to stderr. `run`, `check` and `contract` are the same
//! operations for a person to read: their machine-readable form is `agent`,
//! so they take no `--json` of their own.
//!
//! This module is observational: it feeds source text to the existing
//! interpreter and serializes the existing diagnostic structures. It defines
//! no language semantics (canonical source: `SPECIFICATION.html`).

mod repl;
#[cfg(test)]
mod step_limit_tests;
mod test_runner;

use crate::agent::api as agent_api;
use crate::agent::report::{self, Report};
use crate::agent::{block_on, contract};

/// The command line's options.
pub(crate) struct Opts {
    pub json: bool,
    /// `agent`: print the envelope compact, byte-identical to the WASM entry
    /// points, instead of indented.
    pub compact: bool,
    /// `compute`: execution step budget override. `None` keeps the
    /// interpreter default.
    pub step_limit: Option<usize>,
    /// `agent compute`: which resource ceilings apply.
    pub limits: LimitProfile,
}

/// The resource ceilings an `agent compute` runs under: the tighter profile
/// for untrusted, generated programs (the default), or the interpreter's own
/// defaults, which `run` uses for a program its author trusts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LimitProfile {
    Agent,
    Trusted,
}

const USAGE: &str = "Usage: ajisai <command> [options]

Commands:
  run <file.ajisai> [--step-limit <N>]
                                  Execute a program file
  check <file.ajisai>             Tokenize, parse and resolve only, and check
                                  each `#:contract` declaration against the
                                  contract inferred from the Core Words it
                                  calls (no execution)
  contract <file.ajisai>          Report each user word's inferred contract
                                  (inputs, outputs, field) plus a paste-ready
                                  `#:contract` line (no execution)
  agent <operation> <file.ajisai|-> [--limits <agent|trusted>] [--step-limit <N>]
                                  The one source-to-JSON host boundary. Operations:
                                  compute, check, infer-contracts. `-`
                                  reads the program from standard input, so an
                                  embedding host needs no temporary file
  test <file-or-dir> [--json]     Run test files, checking each program against
                                  its `#@` directive comments (status/stack/
                                  output/error). Exit 1 if any test fails
  repl [--json]                   Interactive session; stack and definitions
                                  persist. :help for commands, :quit to leave
  version [--json]                Print version information

Options:
  --json                          With `test`/`repl`/`version`: emit JSON
                                  (pipe-safe). `agent` always emits JSON
                                  and takes no `--json`
  --compact                       With `agent`: emit the envelope as compact
                                  JSON with no trailing newline, the exact
                                  bytes the WASM agent entry points return
                                  (default: indented for reading)
  --limits <agent|trusted>        With `agent compute`: the
                                  resource ceilings. `agent` (default) is the
                                  tighter profile for untrusted, generated
                                  programs; `trusted` is the interpreter
                                  default `run` uses
  --step-limit <N>                With `run`/`agent compute`:
                                  override the execution step
                                  budget. N is a positive integer; default:
                                  the host's derived step budget
                                  (interpreter::DEFAULT_MAX_EXECUTION_STEPS,
                                  docs/dev/mcp-host-profiles.md). A host
                                  safety control, not a language semantic

Exit codes:
  0  success
  1  language error (structured diagnosis emitted)
  2  CLI usage error";

/// CLI entry point. Returns the process exit code.
pub fn run(args: &[String]) -> i32 {
    let Some((command, rest)) = args.split_first() else {
        eprintln!("{}", USAGE);
        return 2;
    };
    let mut json = false;
    let mut compact = false;
    let mut limits = LimitProfile::Agent;
    let mut limits_given = false;
    let mut step_limit: Option<usize> = None;
    let mut positional: Vec<&str> = Vec::new();
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--compact" => compact = true,
            "--limits" => match iter.next().map(String::as_str) {
                Some("agent") => (limits, limits_given) = (LimitProfile::Agent, true),
                Some("trusted") => (limits, limits_given) = (LimitProfile::Trusted, true),
                _ => {
                    eprintln!("--limits expects `agent` or `trusted`\n\n{}", USAGE);
                    return 2;
                }
            },
            "--step-limit" => match iter.next().and_then(|value| value.parse::<usize>().ok()) {
                Some(parsed) if parsed > 0 => step_limit = Some(parsed),
                _ => {
                    eprintln!("--step-limit expects a positive integer\n\n{}", USAGE);
                    return 2;
                }
            },
            // A bare `-` is the conventional name for standard input, not an
            // option; every command that takes a path accepts it as one.
            "-" => positional.push("-"),
            flag if flag.starts_with('-') => {
                eprintln!("Unknown option: {}\n\n{}", flag, USAGE);
                return 2;
            }
            path => positional.push(path),
        }
    }
    let opts = Opts {
        json,
        compact,
        step_limit,
        limits,
    };
    // A flag the command does not read would be accepted and silently
    // ignored — `run file --limits agent` running under the trusted ceilings
    // regardless — so it is refused instead.
    let agent_op = |op: &str| command == "agent" && positional.first() == Some(&op);
    let misplaced = [
        ("--limits", limits_given, agent_op("compute")),
        (
            "--step-limit",
            step_limit.is_some(),
            command == "run" || agent_op("compute"),
        ),
        (
            "--json",
            json,
            matches!(command.as_str(), "test" | "repl" | "version"),
        ),
        ("--compact", compact, command == "agent"),
    ];
    for (flag, given, applies) in misplaced {
        if given && !applies {
            let where_ = if command == "agent" {
                format!("agent {}", positional.first().copied().unwrap_or(""))
            } else {
                command.to_string()
            };
            eprintln!("`{flag}` does not apply to `{where_}`\n\n{USAGE}");
            return 2;
        }
    }
    match (command.as_str(), positional.as_slice()) {
        ("run", [path]) => cmd_run(path, &opts),
        ("check", [path]) => cmd_check(path),
        ("contract", [path]) => cmd_contract(path),
        ("agent", [operation, path]) => cmd_agent(operation, path, &opts),
        ("test", [path]) => test_runner::cmd_test(path, &opts),
        ("repl", []) => repl::cmd_repl(&opts),
        ("version", []) => cmd_version(json),
        _ => {
            eprintln!("{}", USAGE);
            2
        }
    }
}

fn cmd_version(json: bool) -> i32 {
    let version = env!("CARGO_PKG_VERSION");
    if json {
        let doc = serde_json::json!({
            "schemaVersion": report::SCHEMA_VERSION,
            "status": "ok",
            "version": version,
            "specificationVersion": crate::kernel::generated::SPECIFICATION_VERSION,
        });
        println!("{}", pretty(&doc));
    } else {
        println!(
            "ajisai {} (specification {})",
            version,
            crate::kernel::generated::SPECIFICATION_VERSION
        );
    }
    0
}

/// The program at `path`, or `None` once the reason it cannot be read is on
/// stderr — the caller exits 2, a usage error.
fn read_source(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .map_err(|e| eprintln!("ajisai: cannot read {}: {}", path, e))
        .ok()
}

fn cmd_run(path: &str, opts: &Opts) -> i32 {
    let Some(source) = read_source(path) else {
        return 2;
    };

    let response = block_on(agent_api::compute(
        &source,
        agent_api::ComputeOptions {
            step_limit: opts.step_limit,
            ..agent_api::ComputeOptions::default()
        },
    ));
    emit(response.report());
    response.exit_code()
}

fn cmd_check(path: &str) -> i32 {
    let Some(source) = read_source(path) else {
        return 2;
    };
    // The same check `agent check` answers, read out for a person: one
    // implementation, two renderings.
    let response = agent_api::check(&source);
    let report = response.report();
    if report.error.is_some() {
        emit(report);
        return response.exit_code();
    }
    let status = if report.status == "ok" { "ok" } else { "fail" };
    println!("{}: {}", status, path);
    let findings = report
        .contract_decls
        .as_ref()
        .and_then(|decls| decls["findings"].as_array());
    for finding in findings.into_iter().flatten() {
        eprintln!(
            "  [{}] {}",
            finding["severity"].as_str().unwrap_or(""),
            finding["message"].as_str().unwrap_or("")
        );
    }
    response.exit_code()
}

/// `ajisai contract <file>`: report each user word's inferred contract
/// (`interpreter::word_contract`), the reporting companion to `check`
/// Registers definitions without executing any word body or top-level code. Observational — a well-formed file always exits 0.
fn cmd_contract(path: &str) -> i32 {
    let Some(source) = read_source(path) else {
        return 2;
    };
    {
        let reports = contract::contracts_json(&source);
        let reports = reports.as_array().map(Vec::as_slice).unwrap_or_default();
        if reports.is_empty() {
            println!("{}: no user words defined", path);
        }
        for r in reports {
            println!(
                "{} : inputs={} outputs={} field={}",
                r["name"].as_str().unwrap_or(""),
                r["inputs"],
                r["outputs"],
                r["field"].as_str().unwrap_or("")
            );
            if let Some(exits) = r["fieldExits"].as_array().filter(|e| !e.is_empty()) {
                let exits: Vec<&str> = exits.iter().filter_map(|e| e.as_str()).collect();
                println!("    leaves the field at: {}", exits.join(", "));
            }
            println!("    {}", r["suggested"].as_str().unwrap_or(""));
        }
    }
    0
}

/// The one JSON boundary consumed by host adapters: every operation returns
/// the same top-level envelope shape.
fn cmd_agent(operation: &str, path: &str, opts: &Opts) -> i32 {
    // `-` reads the program from standard input. A host adapter that already
    // holds the source in memory should not have to invent a temporary file to
    // hand it over: writing one needs a writable temporary directory, leaves
    // the program on disk for as long as the call runs, and buys nothing the
    // pipe does not already give.
    let source = if path == "-" {
        use std::io::Read;
        let mut buffer = String::new();
        match std::io::stdin().read_to_string(&mut buffer) {
            Ok(_) => buffer,
            Err(e) => {
                eprintln!("ajisai: cannot read standard input: {}", e);
                return 2;
            }
        }
    } else {
        let Some(source) = read_source(path) else {
            return 2;
        };
        source
    };
    let (document, exit_code) = match operation {
        "compute" => {
            let response = block_on(agent_api::compute(&source, compute_options(opts)));
            let exit_code = response.exit_code();
            (response.into_json(), exit_code)
        }
        "check" => {
            let response = agent_api::check(&source);
            (response.to_json(), response.exit_code())
        }
        "infer-contracts" => {
            let response = agent_api::infer_contracts(&source);
            (response.to_json(), response.exit_code())
        }
        _ => {
            eprintln!("unknown agent operation: {operation}");
            return 2;
        }
    };
    if opts.compact {
        // The bytes the WASM entry points return (`agent_compute` and its
        // siblings answer `Value::to_string()`), so a host that caps a
        // response by its size caps both backends at the same result.
        use std::io::Write;
        let mut out = std::io::stdout().lock();
        if serde_json::to_writer(&mut out, &document).is_ok() {
            let _ = out.flush();
        }
    } else {
        println!("{}", pretty(&document));
    }
    exit_code
}

/// The ceilings an `agent compute` runs under, from the
/// command line: the step budget and the chosen limit profile.
fn compute_options(opts: &Opts) -> agent_api::ComputeOptions {
    match opts.limits {
        LimitProfile::Agent => agent_api::ComputeOptions::agent(opts.step_limit),
        LimitProfile::Trusted => agent_api::ComputeOptions {
            step_limit: opts.step_limit,
            ..agent_api::ComputeOptions::default()
        },
    }
}

fn emit(report: &Report) {
    for line in &report.output {
        println!("{}", line);
    }
    if report.status == "ok" {
        if report.stack_display.is_empty() {
            println!("stack: (empty)");
        } else {
            println!("stack: {}", report.stack_display.join(" "));
        }
        return;
    }
    if let Some(message) = &report.message {
        eprintln!("error: {}", message);
    }
    if let Some(error) = &report.error {
        if let Some(word) = &error.word {
            eprintln!("  in {}", word);
        }
        if let Some(span) = error.position {
            eprintln!("  at line {}, column {}", span.line, span.column);
        }
    }
}

fn pretty(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}
