// Types for oracle-compare.mjs (AR-LLR-037).

export interface Rat { readonly p: bigint; readonly q: bigint }

export type Normal =
    | { kind: 'number'; p: bigint; q: bigint; budget?: Rat | null }
    | { kind: 'boolean'; value: boolean; budget?: Rat | null }
    | { kind: 'nil'; budget?: Rat | null }
    | { kind: 'vector'; items: Normal[]; budget?: Rat | null }
    | { kind: string; budget?: Rat | null };

export type Verdict = 'within' | 'outside' | 'noBudget' | 'missing';

export interface ComparedValue {
    readonly arare: string | null;
    readonly ajisai: string | null;
    readonly budget: string | null;
    readonly difference: string | null;
    readonly verdict: Verdict;
}

export const PRELUDE: string;
export function rat(text: string): Rat;
export function norm(node: unknown): Normal;
export function show(v: Normal): string;
export function showRat(r: Rat): string;
export function ratLe(a: Rat, b: Rat): boolean;
export function within(a: Normal, j: Normal, tol: Rat): boolean;
export function difference(a: Normal, j: Normal): Rat | null;
export function compareStacks(arare: readonly unknown[], ajisai: readonly unknown[]): ComparedValue[];
