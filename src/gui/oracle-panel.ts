// The error-budget panel and the comparison with Ajisai (AR-LLR-037,
// refining AR-HLR-035).
//
// After every change to an Arare session's stack, the panel states the
// program's field contract (AR-HLR-028) and, for each value on the stack,
// Arare's value, the budget the checker infers for it (AR-HLR-027), Ajisai's
// exact value on the same session, the distance between them, and whether
// Arare's value lies within its budget — the comparison CI makes over its
// corpus (ADR-0007), here on whatever the reader wrote.

import { compareStacks, type ComparedValue } from '../engine/oracle-compare.mjs';
import type { OracleAnswer } from '../engine/ajisai-oracle';
import type { BudgetReport, Value } from '../wasm-interpreter-types';

/** What the panel shows, before it becomes DOM. */
export interface ComparisonView {
    readonly field: string;
    readonly rows: readonly ComparedValue[];
    readonly oracleError: string | null;
    readonly summary: string;
}

const VERDICT_TEXT: Record<ComparedValue['verdict'], string> = {
    within: 'within budget',
    outside: 'outside budget',
    noBudget: 'no budget',
    missing: 'missing'
};

export const describeVerdict = (verdict: ComparedValue['verdict']): string => VERDICT_TEXT[verdict];

/** The field contract, as the panel's first line. */
export const describeField = (report: BudgetReport | null): string => {
    if (report === null) return 'field: —';
    if (report.field === 'closed') return 'field=closed: no overflow, no division by zero, no wheel point';
    return `field=open: ${report.open.map((o) => `${o.word ?? 'literal'} (${o.why})`).join(', ')}`;
};

/** Everything the panel shows for a stack, the checker's report and Ajisai's answer. */
export const buildComparisonView = (
    stack: readonly Value[],
    report: BudgetReport | null,
    oracle: OracleAnswer
): ComparisonView => {
    const field = describeField(report);
    if (!oracle.ok) {
        return { field, rows: compareStacks(stack, []), oracleError: oracle.error, summary: `Ajisai refused the session: ${oracle.error}` };
    }
    const rows = compareStacks(stack, oracle.stack);
    const outside = rows.filter((r) => r.verdict === 'outside').length;
    const within = rows.filter((r) => r.verdict === 'within').length;
    const summary = rows.length === 0
        ? 'The stack is empty.'
        : outside > 0
            ? `${outside} value(s) outside their budget. Arare departs from Ajisai on purpose where a branch meets UNKNOWN, an aggregate meets NIL, or a value overflows to 1/0 (ADR-0007).`
            : `${within} of ${rows.length} value(s) within their budget of Ajisai's exact value.`;
    return { field, rows, oracleError: null, summary };
};

const cell = (tag: 'td' | 'th', text: string, className?: string): HTMLTableCellElement => {
    const el = document.createElement(tag);
    el.textContent = text;
    if (className) el.className = className;
    return el;
};

/** Draw `view` into `target`, replacing what was there. */
export const renderComparisonView = (target: HTMLElement, view: ComparisonView): void => {
    const field = document.createElement('p');
    field.className = 'oracle-field';
    field.textContent = view.field;
    const summary = document.createElement('p');
    summary.className = 'oracle-summary';
    summary.textContent = view.summary;
    const table = document.createElement('table');
    table.className = 'oracle-table';
    const head = document.createElement('tr');
    for (const h of ['#', 'Arare', '± budget', 'Ajisai', '|Δ|', '']) head.appendChild(cell('th', h));
    table.createTHead().appendChild(head);
    const body = table.createTBody();
    // Top of the stack first, as the Stack area draws it.
    [...view.rows].reverse().forEach((row, i) => {
        const tr = document.createElement('tr');
        tr.appendChild(cell('td', String(view.rows.length - i)));
        tr.appendChild(cell('td', row.arare ?? '—'));
        tr.appendChild(cell('td', row.budget ?? '?'));
        tr.appendChild(cell('td', row.ajisai ?? '—'));
        tr.appendChild(cell('td', row.difference ?? '—'));
        tr.appendChild(cell('td', describeVerdict(row.verdict), `oracle-verdict oracle-verdict-${row.verdict}`));
        body.appendChild(tr);
    });
    target.replaceChildren(field, summary, ...(view.rows.length > 0 ? [table] : []));
};
