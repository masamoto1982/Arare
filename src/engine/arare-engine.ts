// Arare's engine behind the Playground's seam (AR-LLR-034, refining
// AR-HLR-033 and AR-HLR-034).
//
// Arare's engine is a command (rust/crates/arare-wasm, ADR-0013): each
// request instantiates it, hands it source text and reads one JSON answer. It
// keeps nothing between requests, so a session is the list of programs that
// built its stack — the programs run so far that succeeded — and each run
// sends them ahead of its own, separated by NUL, for the engine to run again
// first. Arare's programs are deterministic, so running them again leaves the
// same stack and the same names bound; that is what lets a name bound in one
// Run, or in one step of step mode, be read in the next.
//
// The session's snapshot (`snapshot_stack`) is that list and the stack it
// left, as JSON: restoring it restores both without running anything, which
// is how a run's answer comes back from the worker and how a session is saved
// and reloaded.
//
// Arare has no User Words: the dictionary's User half is always empty, and
// the shell is told so by the engine's descriptor (src/engine/registry.ts).

import type {
    BudgetReport,
    CoreWordInfo,
    ExecuteResult,
    PlaygroundEngine,
    StepPieces,
    UserWord,
    UserWordInfo,
    Value
} from '../wasm-interpreter-types';
import { runWasiCommand } from './wasi';
import catalog from './words.generated.json';

/** One Word as the generated catalog carries it (scripts/generate-word-reference.mjs). */
interface CatalogWord {
    readonly name: string;
    readonly summary: string;
    readonly syntax: string;
    readonly documentation: string;
}

const WORDS: readonly CatalogWord[] = catalog.words;

/** Where an item of a program is written: 1-based lines, columns in characters. */
interface ItemSpan {
    readonly line: number;
    readonly column: number;
    readonly endLine: number;
    readonly endColumn: number;
}

interface EngineError {
    readonly message: string;
    readonly category: string;
    readonly word: string | null;
    readonly line?: number;
    readonly column?: number;
}

/** The engine's answer to `run` (rust/crates/arare-wasm/src/lib.rs). */
type RunAnswer =
    | {
        readonly ok: true;
        readonly stack: Value[];
        readonly field: 'closed' | 'open';
        readonly open: BudgetReport['open'];
        readonly items: ItemSpan[];
    }
    | { readonly ok: false; readonly error: EngineError; readonly items?: ItemSpan[] };

type ItemsAnswer =
    | { readonly ok: true; readonly items: ItemSpan[] }
    | { readonly ok: false; readonly error: EngineError };

/** The session a snapshot carries: the programs that built it and what they left. */
interface Session {
    readonly programs: readonly string[];
    readonly stack: readonly Value[];
    readonly report: BudgetReport | null;
}

const EMPTY: Session = { programs: [], stack: [], report: null };

/** The separator the engine reads between a session's programs. */
const SEPARATOR = '\0';

/**
 * The offset in `source` (UTF-16 code units, as JavaScript strings count) of
 * a 1-based line and a 1-based column counted in characters.
 */
export const offsetOf = (source: string, line: number, column: number): number => {
    let at = 0;
    for (let l = 1; l < line; l++) {
        const next = source.indexOf('\n', at);
        if (next === -1) return source.length;
        at = next + 1;
    }
    for (let c = 1; c < column && at < source.length; c++) {
        const code = source.codePointAt(at)!;
        at += code > 0xffff ? 2 : 1;
    }
    return at;
};

const failure = (error: EngineError): ExecuteResult => ({
    status: 'ERROR',
    error: true,
    message: error.message,
    errorInfo: {
        category: error.category,
        ...(error.word ? { word: error.word } : {}),
        ...(typeof error.line === 'number' ? { line: error.line, column: error.column } : {})
    }
});

/** The field contract as one line of Output. */
export const describeReport = (report: BudgetReport): string =>
    report.field === 'closed'
        ? 'field=closed'
        : `field=open: ${report.open.map((o) => `${o.word ?? 'literal'} ${o.why}`).join(', ')}`;

export class ArareEngine implements PlaygroundEngine {
    private session: Session = EMPTY;

    constructor(private readonly module: WebAssembly.Module) {}

    /** Send one request; the answer is the engine's JSON document. */
    private request<T>(command: string, programs: readonly string[]): T {
        const { exitCode, stdout, stderr } = runWasiCommand(this.module, ['arare-wasm', command], programs.join(SEPARATOR));
        if (exitCode !== 0) throw new Error(`Arare's engine refused the request: ${stderr.trim()}`);
        return JSON.parse(stdout) as T;
    }

    async execute(code: string): Promise<ExecuteResult> {
        const programs = [...this.session.programs, code];
        const answer = this.request<RunAnswer>('run', programs);
        if (!answer.ok) return failure(answer.error);
        const report: BudgetReport = { field: answer.field, open: answer.open };
        this.session = { programs, stack: answer.stack, report };
        return { status: 'OK', output: describeReport(report), budgetReport: report };
    }

    step_pieces(code: string): StepPieces {
        const answer = this.request<ItemsAnswer>('items', [...this.session.programs, code]);
        if (!answer.ok) return { ok: false, error: failure(answer.error) };
        return {
            ok: true,
            pieces: answer.items.map((s) => ({
                start: offsetOf(code, s.line, s.column),
                end: offsetOf(code, s.endLine, s.endColumn)
            }))
        };
    }

    /** The checker's report on the session, or null before anything has run. */
    budget_report(): BudgetReport | null {
        return this.session.report;
    }

    /** The programs the session's stack was built by, oldest first. */
    session_programs(): readonly string[] {
        return this.session.programs;
    }

    host_profile(): string {
        // The engine command has no ceilings of its own; the host's wall-clock
        // guard is the one limit, and the shell reports it beside this.
        return JSON.stringify({ profile: 'arare-playground', limits: {} });
    }

    agent_host_profile(): string {
        throw new Error('Arare has no agent host yet (WORK_ORDER.md §7, Phase 6).');
    }

    reset(): ExecuteResult {
        this.session = EMPTY;
        return { status: 'OK' };
    }

    collect_stack(): Value[] {
        return [...this.session.stack];
    }

    collect_user_words_info(): UserWordInfo[] {
        return [];
    }

    collect_word_identities(): Array<[name: string, id: string]> {
        return [];
    }

    collect_core_words_info(): CoreWordInfo[] {
        return WORDS.map((w) => [w.name, `${w.name} — ${w.summary}`, w.syntax]);
    }

    lookup_word_definition(): string | null {
        return null;
    }

    lookup_word_description(): string | null {
        return null;
    }

    resolve_host_lookup(name: string): { kind: 'documentation' | 'definition'; text: string } | null {
        const word = WORDS.find((w) => w.name === name);
        return word ? { kind: 'documentation', text: word.documentation } : null;
    }

    snapshot_stack(): string {
        return JSON.stringify(this.session);
    }

    restore_stack_snapshot(snapshot_json: string): void {
        const parsed = JSON.parse(snapshot_json) as Partial<Session>;
        if (!Array.isArray(parsed.programs) || !parsed.programs.every((p) => typeof p === 'string')
            || !Array.isArray(parsed.stack)) {
            throw new Error('Not a snapshot of an Arare session.');
        }
        this.session = { programs: parsed.programs, stack: parsed.stack, report: parsed.report ?? null };
    }

    clear_stack(): void {
        this.session = EMPTY;
    }

    restore_user_words(words: UserWord[]): Array<[name: string, reason: string]> {
        return words.map((w) => [w.name, 'Arare has no User Words']);
    }

    set_max_execution_steps(): void {
        // The engine has no step budget; the wall-clock guard stops a long run.
    }
}
