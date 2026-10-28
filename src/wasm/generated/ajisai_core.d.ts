/* tslint:disable */
/* eslint-disable */

export class AjisaiInterpreter {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * The agent profile — what the MCP server and `ajisai agent` apply — in
     * the same shape as `host_profile()`, so the playground can show the
     * ceilings a program meets when it leaves the browser beside its own.
     * They differ by 10x to 120x on purpose (LANG.MACHINE.LIMITS), and a
     * program tried here and then run through MCP met that difference as a
     * surprise, because only the Reference and the MCP README said so.
     */
    agent_host_profile(): string;
    /**
     * Discard every value on the stack, leaving the dictionary, the output
     * and every other piece of session state untouched.
     *
     * A REPL keeps its stack between runs, which is right, and until now the
     * only way to get rid of a leftover intermediate was the full reset — and
     * that takes the User dictionary with it. Clearing values is not a
     * language operation (no Word does it, and none should: a program's own
     * values are its own business), so it belongs here, on the host, where the
     * person at the keyboard is the one asking.
     */
    clear_stack(): void;
    collect_core_words_info(): any;
    collect_stack(): any;
    collect_user_words_info(): any;
    /**
     * Content identity of each user word, as `[name, id]` pairs. The host
     * uses these to deduplicate identical definitions on import and to key
     * shared word groups by content rather than by name.
     */
    collect_word_identities(): any;
    execute(code: string): Promise<any>;
    /**
     * The resource ceilings this interpreter is actually running under, as
     * JSON, under the same names every other Ajisai host publishes them by —
     * literally the same, since `interpreter::limit_profile` is the one place
     * the ceiling set is enumerated.
     *
     * LANG.MACHINE.LIMITS makes limits a host safety control rather than value
     * semantics, so two conforming hosts legitimately disagree about them —
     * and they do: the playground runs the interpreter defaults while the MCP
     * agent profile is an order of magnitude tighter. That is only a trap for
     * someone who prototypes here and runs there while neither host says what
     * it applies. Read from the live interpreter rather than from a constant,
     * so what is displayed is what is enforced.
     */
    host_profile(): string;
    lookup_word_definition(name: string): any;
    /**
     * A User Word's `#:contract`-derived description, for a host affordance
     * like the Dictionary panel's hover. `execute` refuses a source whose
     * declaration inference disproves, as `check` does.
     */
    lookup_word_description(name: string): any;
    constructor();
    reset(): any;
    /**
     * Answer the host's lookup of `name` against the current dictionary.
     *
     * This is a *query*, not a run. Looking a Word up used to be the Word
     * `LOOKUP`, which meant asking what `ADD` does went through `execute` and
     * came back on a side channel that no evaluation rule read. The host asks
     * here instead, so nothing about a lookup touches the stack, the
     * dictionary, or the output buffer.
     *
     * Returns `{ kind: "documentation" | "definition", text }`, or `NULL` for a
     * name the dictionary does not hold — the caller reports the unknown name
     * itself, since it is the one that read it off the input.
     */
    resolve_host_lookup(name: string): any;
    /**
     * Restore a stack from a `snapshot_stack` payload, reinstating exact
     * values (CodeBlock, ExactScalar, …).
     */
    restore_stack_snapshot(snapshot_json: string): void;
    /**
     * Restore saved User Words, and name the entries that could not be
     * restored as `[name, reason]` pairs.
     *
     * Restoring skips an unreadable entry rather than raising, which is what
     * keeps the rest of a dictionary (`restore_user_word_definitions`); the
     * skipped entries come back here instead of being thrown, since a throw
     * would abort the host's own post-restore work and leave the session
     * holding a half-restored dictionary. The host used to learn only the
     * *names* that did not arrive, by comparing what it asked for against
     * the dictionary afterwards — which could not see a refused
     * redefinition (the old body is still there, so the name is present) and
     * could not say why anything was left out. The `Err` case is a list that
     * does not deserialize at all.
     */
    restore_user_words(words_js: any): any;
    /**
     * Override the execution step budget (water level, LANG.MACHINE.LIMITS) for
     * subsequent executions. A runtime safety control, not a language
     * semantic: the host may raise or lower it; never calling this keeps the
     * interpreter's own `DEFAULT_MAX_EXECUTION_STEPS`. A zero or non-positive
     * value is ignored so a malformed host call cannot disable the safety
     * budget entirely.
     *
     * The default's *value* is deliberately not restated here. It is derived
     * from the host time budget and has already moved once (it was 100,000);
     * every doc comment that spelled the number out went on claiming the old
     * one, on both sides of the wasm boundary. The constant is that number's
     * single representation, so this names it instead.
     */
    set_max_execution_steps(steps: number): void;
    /**
     * The one stack format persistence accepts (LANG.OBSERVATION.FIREWALL). Unlike
     * `collect_stack`, which serializes the *observation* wire format (a
     * CodeBlock shows as `nil`, an ExactScalar as a marked rational
     * approximation), this captures the exact value so `restore_stack_snapshot`
     * returns identical values. The two surfaces are deliberately distinct:
     * observation is lossy-but-honest, persistence is lossless. Restoring the
     * observation format is not offered — it would silently downgrade exact
     * values. The payload is an opaque JSON string produced by
     * `crate::types::value_persist`.
     */
    snapshot_stack(): string;
}

/**
 * Parse and resolve `source` without executing it; also verifies declared
 * `#:contract` declarations conservatively, matching `ajisai agent check`.
 */
export function agent_check(source: string): string;

/**
 * Execute one Ajisai source document under the same tightened
 * agent-profile runtime limits the native `ajisai agent compute` CLI
 * applies. `step_limit` overrides the default execution step budget when
 * positive; `0` or omitted keeps the interpreter default.
 */
export function agent_compute(source: string, step_limit?: number | null): Promise<string>;

/**
 * Infer machine-readable contracts for user-defined Words without
 * executing their bodies, matching `ajisai agent infer-contracts`.
 */
export function agent_infer_contracts(source: string): string;

/**
 * Benchmark hook for `scripts/bench/speed-bench-wasm.mjs`: run `source` on a fresh
 * interpreter with every priced ceiling lifted (`agent::unbounded_interpreter`,
 * the same one `rust/examples/speed_bench.rs` times natively) and answer the
 * final stack depth. Nothing is converted to JS, so the caller's clock sees
 * the computation and not the marshalling of its result.
 */
export function bench_execute(source: string): Promise<number>;

/**
 * Install console_error_panic_hook so any panic on the WASM side
 * surfaces in the browser console with a JS-friendly stack trace
 * instead of an opaque `RuntimeError: unreachable executed` trap.
 * Idempotent (`set_once`). Called from the TS loader exactly once
 * right after wasm-bindgen `init`.
 */
export function init_panic_hook(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_ajisaiinterpreter_free: (a: number, b: number) => void;
    readonly agent_check: (a: number, b: number) => [number, number];
    readonly agent_compute: (a: number, b: number, c: number) => any;
    readonly agent_infer_contracts: (a: number, b: number) => [number, number];
    readonly ajisaiinterpreter_agent_host_profile: (a: number) => [number, number];
    readonly ajisaiinterpreter_clear_stack: (a: number) => void;
    readonly ajisaiinterpreter_collect_core_words_info: (a: number) => any;
    readonly ajisaiinterpreter_collect_stack: (a: number) => any;
    readonly ajisaiinterpreter_collect_user_words_info: (a: number) => any;
    readonly ajisaiinterpreter_collect_word_identities: (a: number) => any;
    readonly ajisaiinterpreter_execute: (a: number, b: number, c: number) => any;
    readonly ajisaiinterpreter_host_profile: (a: number) => [number, number];
    readonly ajisaiinterpreter_lookup_word_definition: (a: number, b: number, c: number) => any;
    readonly ajisaiinterpreter_lookup_word_description: (a: number, b: number, c: number) => any;
    readonly ajisaiinterpreter_new: () => number;
    readonly ajisaiinterpreter_reset: (a: number) => any;
    readonly ajisaiinterpreter_resolve_host_lookup: (a: number, b: number, c: number) => any;
    readonly ajisaiinterpreter_restore_stack_snapshot: (a: number, b: number, c: number) => [number, number];
    readonly ajisaiinterpreter_restore_user_words: (a: number, b: any) => [number, number, number];
    readonly ajisaiinterpreter_set_max_execution_steps: (a: number, b: number) => void;
    readonly ajisaiinterpreter_snapshot_stack: (a: number) => [number, number];
    readonly bench_execute: (a: number, b: number) => any;
    readonly init_panic_hook: () => void;
    readonly wasm_bindgen_413a9fa07f7179f1___convert__closures_____invoke___wasm_bindgen_413a9fa07f7179f1___JsValue__core_7d5f0a2ba6a62c33___result__Result_____wasm_bindgen_413a9fa07f7179f1___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_413a9fa07f7179f1___convert__closures_____invoke___js_sys_402f77b0a9aba5cd___Function_fn_wasm_bindgen_413a9fa07f7179f1___JsValue_____wasm_bindgen_413a9fa07f7179f1___sys__Undefined___js_sys_402f77b0a9aba5cd___Function_fn_wasm_bindgen_413a9fa07f7179f1___JsValue_____wasm_bindgen_413a9fa07f7179f1___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
