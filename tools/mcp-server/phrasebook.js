// The phrasebook as something a tool call can ask for by name.
//
// `assets/phrasebook.ajisai` is the repository's `docs/phrasebook/
// phrasebook.ajisai`: User Words for phrases programs write again and again,
// each with a verified `#:contract` line and a `#:digest` line naming its
// content identity. A tool call runs one source in a fresh session, so a
// caller who wanted `MEAN` used to paste its definition into every call.
// `words: ["MEAN"]` puts the definition — and those of the phrasebook Words it
// calls — in front of the source instead, and the result's `prelude` names
// each Word added with its digest, so what ran is checkable by content.
//
// Nothing persists between calls: the prelude is part of the one source the
// engine runs, and its bytes count against `sourceBytes` like any other.

import { readFileSync } from "node:fs";

const DEF_LINE = /^\[.*\] '([^']+)' DEF$/;

/**
 * The phrasebook's Words in file order: `{ name, lines, digest, calls }`,
 * where `lines` are the Word's `#:contract` line (when it has one) and its
 * `DEF` line, and `calls` are the other phrasebook Words its body names.
 */
export function parsePhrasebook(text) {
  const lines = text.split("\n");
  const entries = [];
  for (let i = 0; i < lines.length; i += 1) {
    const match = lines[i].match(DEF_LINE);
    if (!match) continue;
    const name = match[1].toUpperCase();
    const contract = lines[i - 1]?.startsWith(`#:contract ${match[1]} `) ? [lines[i - 1]] : [];
    const digestLine = lines[i + 1]?.startsWith(`#:digest ${match[1]} `) ? lines[i + 1] : null;
    if (!digestLine) throw new Error(`phrasebook: ${match[1]} has no #:digest line after its DEF`);
    entries.push({ name, lines: [...contract, lines[i]], digest: digestLine.split(" ")[2], body: lines[i] });
  }
  const names = new Set(entries.map((entry) => entry.name));
  for (const entry of entries) {
    const tokens = entry.body.replace(/'[^']*'/g, " ").toUpperCase().split(/[\s[\]]+/);
    entry.calls = [...new Set(tokens.filter((token) => token !== entry.name && names.has(token)))];
    delete entry.body;
  }
  return entries;
}

let cached;
/** The packaged phrasebook, parsed once. */
export function phrasebook(path) {
  if (!cached) {
    const text = readFileSync(path, "utf8");
    cached = { text, entries: parsePhrasebook(text) };
  }
  return cached;
}

/**
 * The source a call runs for `requested` names: every requested Word and every
 * phrasebook Word it calls, in file order, as `{ text, lineCount, prelude }`;
 * or `{ unknown }` naming the requests the phrasebook does not hold.
 */
export function preludeFor(requested, entries) {
  const byName = new Map(entries.map((entry) => [entry.name, entry]));
  const wanted = requested.map((name) => String(name).toUpperCase());
  const unknown = wanted.filter((name) => !byName.has(name));
  if (unknown.length) return { unknown };
  const needed = new Set();
  const visit = (name) => {
    if (needed.has(name)) return;
    needed.add(name);
    for (const callee of byName.get(name).calls) visit(callee);
  };
  wanted.forEach(visit);
  const chosen = entries.filter((entry) => needed.has(entry.name));
  const lines = chosen.flatMap((entry) => entry.lines);
  return {
    text: lines.length ? `${lines.join("\n")}\n` : "",
    lineCount: lines.length,
    prelude: chosen.map(({ name, digest }) => ({ name, digest })),
  };
}

/**
 * A result as the caller's own source would have produced it: an error's
 * `line` counted from the caller's first line, and nothing the prelude
 * declared or defined reported as if the caller had.
 */
export function withoutPrelude(result, { lineCount, prelude }) {
  const names = new Set(prelude.map((entry) => entry.name));
  if (result.error && typeof result.error.line === "number" && result.error.line > lineCount) {
    result.error = { ...result.error, line: result.error.line - lineCount };
  }
  if (Array.isArray(result.contracts)) {
    result.contracts = result.contracts.filter((contract) => !names.has(String(contract.name).toUpperCase()));
  }
  const decls = result.contractDecls;
  if (decls && typeof decls === "object") {
    const declarations = (decls.declarations ?? []).filter((d) => !names.has(String(d.word).toUpperCase()));
    const findings = (decls.findings ?? []).filter(
      (finding) => ![...names].some((name) => String(finding.message ?? "").toUpperCase().includes(`#:CONTRACT ${name}\``)),
    );
    result.contractDecls = declarations.length || findings.length ? { ...decls, declarations, findings } : null;
  }
  result.prelude = prelude;
  return result;
}
