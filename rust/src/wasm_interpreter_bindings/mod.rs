// `wasm-bindgen` expands `#[wasm_bindgen]` items into generated glue that
// contains `unsafe`, so this module re-permits `unsafe_code` over the crate-root
// `#![deny(unsafe_code)]` (structural-memory-safety roadmap Phase 4). No
// hand-written `unsafe` lives here; the allow only covers macro-generated code.
#![allow(unsafe_code)]

//! The browser playground's stateful session (`AjisaiInterpreter`), the value
//! conversion into the `JsValue` shapes it returns, and the one-shot agent
//! entry points. Dictionary and stack-state methods live in
//! `wasm_interpreter_state`.
//!
//! `js_sys::Reflect::set(...).unwrap()` in `set_js_prop`: every target is a
//! plain JS object this module just created with `js_sys::Object::new()`, so
//! the usual failure causes (a Proxy handler, a frozen object) cannot occur.
//! Should one ever fail anyway, console_error_panic_hook puts the stack trace
//! in the browser console.

use crate::agent::report::{error_json, ErrorInfo};
use crate::agent::{api, contract};
use crate::error::ErrorCategory;
use crate::interpreter::Interpreter;
use crate::types::Value;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

mod wasm_interpreter_state;

/// Install console_error_panic_hook so any panic on the WASM side
/// surfaces in the browser console with a JS-friendly stack trace
/// instead of an opaque `RuntimeError: unreachable executed` trap.
/// Idempotent (`set_once`). Called from the TS loader exactly once
/// right after wasm-bindgen `init`.
#[wasm_bindgen]
pub fn init_panic_hook() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct AjisaiInterpreter {
    interpreter: Interpreter,
}

fn set_js_prop(obj: &js_sys::Object, key: &str, value: &JsValue) {
    js_sys::Reflect::set(obj, &JsValue::from_str(key), value).unwrap();
}

/// A `serde_json` rendering as the plain JS value it describes: the error
/// this boundary returns is rendered by the agent report's own serializer
/// and converted here, so the GUI reads what an agent reads.
fn json_to_js(value: serde_json::Value) -> JsValue {
    use serde::Serialize as _;
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .expect("a serde_json value always converts to a JS value")
}

impl Default for AjisaiInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl AjisaiInterpreter {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let interp = Interpreter::new();
        AjisaiInterpreter {
            interpreter: interp,
        }
    }

    /// The resource ceilings this interpreter is actually running under, as
    /// JSON, under the same names every other Ajisai host publishes them by —
    /// literally the same, since `interpreter::limit_profile` is the one place
    /// the ceiling set is enumerated.
    ///
    /// LANG.MACHINE.LIMITS makes limits a host safety control rather than value
    /// semantics, so two conforming hosts legitimately disagree about them —
    /// and they do: the playground runs the interpreter defaults while the MCP
    /// agent profile is an order of magnitude tighter. That is only a trap for
    /// someone who prototypes here and runs there while neither host says what
    /// it applies. Read from the live interpreter rather than from a constant,
    /// so what is displayed is what is enforced.
    #[wasm_bindgen]
    pub fn host_profile(&self) -> String {
        let limits = self.interpreter.runtime_limits();
        let step_limit = self.interpreter.max_execution_steps();
        serde_json::json!({
            "profile": "browser-playground",
            "limits": crate::interpreter::limit_profile::to_json(limits, step_limit),
        })
        .to_string()
    }
    /// The agent profile — what the MCP server and `ajisai agent` apply — in
    /// the same shape as `host_profile()`, so the playground can show the
    /// ceilings a program meets when it leaves the browser beside its own.
    /// They differ by 10x to 120x on purpose (LANG.MACHINE.LIMITS), and a
    /// program tried here and then run through MCP met that difference as a
    /// surprise, because only the Reference and the MCP README said so.
    #[wasm_bindgen]
    pub fn agent_host_profile(&self) -> String {
        serde_json::json!({
            "profile": "mcp-local-stdio",
            "limits": api::ComputeOptions::agent_limit_profile(),
        })
        .to_string()
    }
    #[wasm_bindgen]
    pub async fn execute(&mut self, code: &str) -> Result<JsValue, JsValue> {
        let obj = js_sys::Object::new();

        // LANG.CONTRACT.CHECK: a program whose `#:contract` declaration is
        // violated is not executed, here as in every other host.
        if let Some(check) = contract::declared_contract_check(code) {
            if check.violated {
                let (message, word) = contract::violation_message(&check);
                let info = ErrorInfo::of(ErrorCategory::ContractViolation, word);
                set_js_prop(&obj, "status", &("ERROR".into()));
                set_js_prop(&obj, "message", &(message.into()));
                set_js_prop(&obj, "error", &(true.into()));
                set_js_prop(&obj, "output", &("".into()));
                set_js_prop(&obj, "errorInfo", &json_to_js(error_json(&info)));
                return Ok(obj.into());
            }
        }

        match self.interpreter.execute(code).await {
            Ok(()) => {
                set_js_prop(&obj, "status", &("OK".into()));
                let output = self.interpreter.collect_output();
                // This host reads the text buffer; the structured effect log
                // carries the same emissions and is drained with it, so a
                // long session does not keep every payload it ever printed.
                self.interpreter.take_host_effects();
                set_js_prop(&obj, "output", &(output.into()));
                set_js_prop(&obj, "userWords", &(self.collect_user_words_for_state()));
            }
            Err(e) => {
                let error_msg = e.to_string();
                set_js_prop(&obj, "status", &("ERROR".into()));
                set_js_prop(&obj, "message", &(error_msg.into()));
                set_js_prop(&obj, "error", &(true.into()));
                // Whatever the program printed before it failed is part of the
                // report, not something the failure erases: `PRINT` is the
                // language's trace tool and the run that ends in an error is
                // exactly the run whose trace is wanted. The native CLI has
                // always emitted it (`completed_run_report` carries `output`
                // down the Err arm too); only this host dropped it, so a
                // multi-line program that failed late showed the error and
                // nothing else. Draining the buffer here also stops the
                // orphaned output from surfacing at the head of the next run.
                set_js_prop(&obj, "output", &(self.interpreter.collect_output().into()));
                self.interpreter.take_host_effects();
                // The failure's category, Word and position travel in the
                // shape the CLI's `error` has — never parsed back out of
                // `message`, which is display text.
                let info = ErrorInfo::of_run(&self.interpreter, &e);
                set_js_prop(&obj, "errorInfo", &json_to_js(error_json(&info)));
                // An ERROR result carries no `userWords`, which is the
                // protocol's way of saying the run committed nothing to the
                // dictionary. The run said otherwise while it was going: every
                // `DEF` it reached printed `Defined word:`, and those lines are
                // in the `output` above, still claiming a Word the host is
                // about to discard. Naming what was discarded is what turns the
                // report back into a true one.
                let changes = self.interpreter.dictionary_changes_this_run();
                if !changes.is_empty() {
                    let names = js_sys::Array::new();
                    for name in changes {
                        names.push(&JsValue::from_str(name));
                    }
                    set_js_prop(&obj, "discardedDictionaryChanges", &names);
                }
            }
        }
        Ok(obj.into())
    }

    #[wasm_bindgen]
    pub fn reset(&mut self) -> JsValue {
        self.reset_runtime()
    }

    fn reset_runtime(&mut self) -> JsValue {
        let obj = js_sys::Object::new();

        let outcome = self.interpreter.execute_reset();

        match outcome {
            Ok(()) => {
                set_js_prop(&obj, "status", &("OK".into()));
                set_js_prop(&obj, "output", &("System reinitialized.".into()));
                set_js_prop(&obj, "stack", &(self.collect_stack()));
                set_js_prop(&obj, "userWords", &(self.collect_user_words_for_state()));
            }
            Err(e) => {
                set_js_prop(&obj, "status", &("ERROR".into()));
                set_js_prop(&obj, "message", &(e.to_string().into()));
                set_js_prop(&obj, "error", &(true.into()));
            }
        }
        obj.into()
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct UserWordData {
    pub(crate) name: String,
    pub(crate) definition: Option<String>,
    /// The `#:contract`-derived hover text (see `execute_def::set_word_
    /// description`), round-tripped through save/export so it survives a
    /// restore rather than existing only for the session that typed it.
    #[serde(default)]
    pub(crate) description: Option<String>,
}

/// The stack as the JS array the playground draws: the bounded view of each
/// value (`agent::stack_view`), every Vector cut to the elements the
/// playground can draw, with what it left out stated about the whole value.
///
/// Rendered as one JSON text and handed to the JS engine's own parser. It used
/// to be every element, built node by node through `js_sys` at about 7 µs per
/// element: a run that left `0 999999 RANGE` on the stack spent some seven
/// seconds converting it, against a few milliseconds computing it.
fn stack_to_js(stack: &[Value]) -> JsValue {
    let text = crate::agent::stack_view::stack_view_json(stack).to_string();
    js_sys::JSON::parse(&text).expect("serde_json writes valid JSON")
}

// ── One-shot agent entry points ──────────────────────────────────────────
//
// The host-neutral agent boundary (`crate::agent`), returning the same JSON
// envelope the native `ajisai agent <operation>` CLI emits
// (`docs/dev/agent-cli-output-contract.md`), serialized as a JSON string so a
// Node host parses it identically to the native CLI's stdout — no bespoke
// per-host JS object shape, no separate normalizer.
//
// Deliberately separate from `AjisaiInterpreter`: that struct is the browser
// playground's stateful, step-mode session API and keeps its own JS-object
// result shape. These create one fresh interpreter per call, mirroring the
// native CLI's one-process-per-call model.

/// The agent-profile ceilings every one-shot call runs under. `step_limit`
/// overrides the default execution step budget when positive; `0` or omitted
/// keeps the interpreter default.
fn agent_options(step_limit: Option<u32>) -> api::ComputeOptions {
    api::ComputeOptions::agent(step_limit.filter(|&n| n > 0).map(|n| n as usize))
}

/// Execute one Ajisai source document under the same tightened
/// agent-profile runtime limits the native `ajisai agent compute` CLI
/// applies. `step_limit` overrides the default execution step budget when
/// positive; `0` or omitted keeps the interpreter default.
#[wasm_bindgen]
pub async fn agent_compute(source: &str, step_limit: Option<u32>) -> String {
    api::compute(source, agent_options(step_limit))
        .await
        .into_json()
        .to_string()
}

/// Parse and resolve `source` without executing it; also verifies declared
/// `#:contract` declarations conservatively, matching `ajisai agent check`.
#[wasm_bindgen]
pub fn agent_check(source: &str) -> String {
    api::check(source).to_json().to_string()
}

/// Infer machine-readable contracts for user-defined Words without
/// executing their bodies, matching `ajisai agent infer-contracts`.
#[wasm_bindgen]
pub fn agent_infer_contracts(source: &str) -> String {
    api::infer_contracts(source).to_json().to_string()
}

/// Benchmark hook for `scripts/bench/speed-bench-wasm.mjs`: run `source` on a fresh
/// interpreter with every priced ceiling lifted (`agent::unbounded_interpreter`,
/// the same one `rust/examples/speed_bench.rs` times natively) and answer the
/// final stack depth. Nothing is converted to JS, so the caller's clock sees
/// the computation and not the marshalling of its result.
#[wasm_bindgen]
pub async fn bench_execute(source: &str) -> Result<usize, String> {
    let mut interp = crate::agent::unbounded_interpreter();
    interp.execute(source).await.map_err(|e| e.to_string())?;
    Ok(interp.get_stack().len())
}
