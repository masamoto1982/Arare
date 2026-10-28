import { createHash } from 'node:crypto';
import { readJson, readText, writeOrCheck } from './lib/common.mjs';

const fragments = new Map([
  ['presentation-profile', readText('spec/gui-semantics.md')],
]);

let content = readText('spec/language-semantics.md');
for (const [id, fragment] of fragments) {
  const marker = `<!-- INCLUDE:${id} -->`;
  if (!content.includes(marker)) throw new Error(`Missing specification marker: ${marker}`);
  content = content.replace(marker, fragment.trimEnd());
}
if (/<!-- INCLUDE:/.test(content)) throw new Error('Unresolved specification include marker');

// ------------------------------------------------- Japanese clause summaries
//
// The specification is canonical in English alone, while the design record
// (docs/dev/) and the Reference are written from Japanese first. Nothing
// checked that the two languages still said the same thing about a clause.
// docs/i18n/clause-summaries.ja.json gives every LANG.* clause a short
// Japanese summary, recorded against a digest of the clause's English text —
// the rule docs/i18n/word-summaries.ja.json already follows for each Word —
// so a clause rewritten in English fails here until its summary is revisited.
// The summary is embedded after the clause's heading, folded, and carries no
// authority: the English clause it summarises is the specification.

const CLAUSE_HEADING = /^<h3 id="([^"]+)">(LANG\.[A-Z]+\.[A-Z]+) — [^<]*<\/h3>$/;

/** Every clause as `{ id, anchor, text, headingLine }`, text up to the next heading. */
function clausesOf(source) {
  const lines = source.split('\n');
  const clauses = [];
  for (let i = 0; i < lines.length; i += 1) {
    const match = lines[i].match(CLAUSE_HEADING);
    if (!match) continue;
    let end = i + 1;
    while (end < lines.length && !/^<h[23][ >]/.test(lines[end])) end += 1;
    clauses.push({ anchor: match[1], id: match[2], headingLine: i, text: lines.slice(i, end).join('\n') });
  }
  return clauses;
}

const digestOf = (text) => createHash('sha256').update(text).digest('hex').slice(0, 16);
const escapeHtml = (text) => text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
const inlineCode = (text) => escapeHtml(text).replace(/`([^`]+)`/g, '<code>$1</code>');

const summaries = readJson('docs/i18n/clause-summaries.ja.json').entries;
const clauses = clausesOf(content);
const problems = [];
for (const id of Object.keys(summaries)) {
  if (!clauses.some((clause) => clause.id === id)) problems.push(`docs/i18n/clause-summaries.ja.json summarises ${id}, which the specification does not define`);
}
const lines = content.split('\n');
for (const clause of [...clauses].reverse()) {
  const summary = summaries[clause.id];
  const source = digestOf(clause.text);
  if (!summary) {
    problems.push(`${clause.id} has no Japanese summary in docs/i18n/clause-summaries.ja.json (source ${source})`);
    continue;
  }
  if (summary.source !== source) {
    problems.push(`the English text of ${clause.id} changed; revisit its Japanese summary in docs/i18n/clause-summaries.ja.json and set its source to ${source}`);
  }
  lines.splice(
    clause.headingLine + 1,
    0,
    `<details class="clause-ja" lang="ja"><summary>日本語の要約（非正典）</summary><p>${inlineCode(summary.summary)}</p></details>`,
  );
}
if (problems.length) {
  for (const problem of problems) console.error(`[specification] ${problem}`);
  process.exit(1);
}
content = lines.join('\n');

const generated = readText('spec/specification.template.html').replace('{{SPECIFICATION_CONTENT}}', content);
writeOrCheck(
  'specification',
  [{
    path: 'SPECIFICATION.html',
    content: generated,
    stale: 'SPECIFICATION.html is stale. Run npm run specification:generate and commit the result.',
  }],
  { current: 'SPECIFICATION.html is up to date.', wrote: 'wrote SPECIFICATION.html.' },
);
