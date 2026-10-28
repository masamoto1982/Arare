#!/usr/bin/env node
// Generate SKILL.md — the thin "read this, then write Ajisai" protocol for AI
// agents — from machine sources, never by hand.
//
// Inputs:
//   - docs/word-manifest.json            (the surface inventory gate: §9)
//   - spec/words.json                    (canonical Word documentation)
//   - curated snippet data in this file  (§6 examples, §7 errors, §8 forbidden)
//
// Every snippet is executed through the real `ajisai` CLI and the *actual*
// `--json` output (stackDisplay / output / error fields) is embedded.
// If language behavior changes, regeneration changes SKILL.md and the
// `check:skill` CI step fails until the committed copy is refreshed — the
// guide cannot drift from the implementation.
//
// Usage:
//   node scripts/generate-skill-md.mjs            # write SKILL.md
//   node scripts/generate-skill-md.mjs --check    # fail if SKILL.md is stale
//   AJISAI_BIN=/path/to/ajisai ...                # override CLI binary

import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { fatal, readJson, repoRoot, resolveAjisaiBin, spawnAgent, words, writeOrCheck } from './lib/common.mjs';

const fail = (message) => fatal('skill-md', message);

// ---------------------------------------------------------------------------
// CLI harness
// ---------------------------------------------------------------------------

const ajisaiBin = resolveAjisaiBin('skill-md');

// A Markdown code span around `text`. A message can itself quote code in
// backticks (a contract finding, a diagnosis's example), so the delimiter is
// one backtick longer than the longest run inside it, padded with a space
// when the text starts or ends with one.
function inlineCode(text) {
  const longest = Math.max(0, ...(text.match(/`+/g) ?? []).map((run) => run.length));
  const fence = '`'.repeat(longest + 1);
  const pad = longest > 0 ? ' ' : '';
  return `${fence}${pad}${text}${pad}${fence}`;
}

function runSnippet(code, command = 'compute') {
  // `check` reads no limit profile (a flag a command does not read is a usage
  // error), so it is spawned without the `--limits trusted` every compute gets.
  const proc = command === 'check'
    ? spawnSync(ajisaiBin, ['agent', 'check', '-'], { input: `${code}\n`, encoding: 'utf8' })
    : spawnAgent(ajisaiBin, code, { command });
  if (proc.error) fail(`failed to spawn ajisai CLI: ${proc.error.message}`);
  let json = null;
  try {
    json = JSON.parse(proc.stdout);
  } catch {
    fail(`CLI stdout for ${JSON.stringify(code)} is not valid JSON:\n${proc.stdout}`);
  }
  return { exit: proc.status, json };
}

function expectOk(code) {
  const { exit, json } = runSnippet(code);
  if (exit !== 0) fail(`snippet must succeed but failed (${json.message}): ${code}`);
  return json;
}

/** Run `ajisai agent check` on `code`; it must pass (exit 0). */
function expectCheckOk(code) {
  const { exit, json } = runSnippet(code, 'check');
  if (exit !== 0) fail(`check must pass but failed (${json.message}): ${code}`);
  return json;
}

function expectError(code) {
  const { exit, json } = runSnippet(code);
  if (exit !== 1) fail(`snippet must fail with exit 1 but exited ${exit}: ${code}`);
  if (!json.error || typeof json.error.category !== 'string') {
    fail(`error snippet missing error.category: ${code}`);
  }
  return json;
}

// ---------------------------------------------------------------------------
// Word inventory (§9) — surfaces from the manifest, contracts from words.json
// ---------------------------------------------------------------------------

// Vocabulary counts read from the manifest, so the sentence introducing the
// table cannot drift from the table itself.
function readVocabularyCounts() {
  const manifest = readJson('docs/word-manifest.json');
  const { canonicalWords, semanticKernelWords, standardWords } = manifest.counts;
  for (const [name, value] of Object.entries({ canonicalWords, semanticKernelWords, standardWords })) {
    if (typeof value !== 'number') fail(`docs/word-manifest.json counts.${name} is missing`);
  }
  if (semanticKernelWords + standardWords !== canonicalWords) {
    fail('docs/word-manifest.json kernel + standard counts do not sum to the canonical inventory');
  }
  return { canonicalWords, semanticKernelWords, standardWords };
}

/** Every Core Word in manifest order, with its registry contract. */
function coreWordsWithContracts() {
  const manifest = readJson('docs/word-manifest.json');
  const contracts = new Map(words().entries.map((entry) => [entry.name, entry]));
  return manifest.entries
    .filter((entry) => entry.kind === 'coreword')
    .map((entry) => {
      const contract = contracts.get(entry.canonical);
      if (!contract) fail(`no contract found for coreword ${entry.surface}`);
      return { surface: entry.surface, family: entry.family, contract };
    });
}

/** skill/words.md's rows: every Word's full summary and example. */
function buildWordTable() {
  return coreWordsWithContracts().map(({ surface, family, contract }) => {
    const syntax = contract.documentation.syntax ? ` — e.g. \`${contract.documentation.syntax}\`` : '';
    return `| \`${surface}\` | ${family} | ${contract.documentation.summary}${syntax} |`;
  });
}

/** SKILL.md's §9: one row per family, naming its Words in manifest order. */
function buildWordIndex() {
  const byFamily = new Map();
  for (const { surface, family } of coreWordsWithContracts()) {
    if (!byFamily.has(family)) byFamily.set(family, []);
    byFamily.get(family).push(`\`${surface}\``);
  }
  return [...byFamily].map(([family, names]) => `| ${family} | ${names.join(' ')} |`);
}

// ---------------------------------------------------------------------------
// Curated, execution-verified snippet data
// ---------------------------------------------------------------------------

const canonicalExamples = [
  { id: 'scalar', title: 'Push a number: a bare scalar', code: '42' },
  { id: 'div', title: 'Exact rational division — no floats, ever', code: '1 3 DIV' },
  { title: 'Elementwise vector arithmetic', code: '[ 1 2 3 ] [ 4 5 6 ] ADD' },
  { title: 'Scalar broadcast over a vector', code: '5 [ 1 2 3 ] MUL' },
  { title: 'Remainder: name the operands, then a - b * floor(a/b)', code: "10 'A' BIND 3 'B' BIND A A B DIV FLOOR B MUL SUB" },
  { title: 'Comparison pushes a boolean', code: '1 2 LT' },
  { title: 'Comparison lifts over vectors element-wise', code: '[ 1 2 ] [ 3 1 ] LT' },
  { title: 'Range: start end, both included', code: '0 5 RANGE' },
  { title: 'A stride is a multiplication of a range', code: '0 5 RANGE 2 MUL' },
  { title: 'Fill a shape with one number: [ shape ] value', code: '[ 2 2 ] 7 FILL' },
  { title: 'MAP with a [ ] code block', code: '0 4 RANGE [ 2 MUL ] MAP' },
  { title: 'FILTER keeps matching elements', code: '0 10 RANGE [ 5 GT ] FILTER' },
  { id: 'fold', title: 'FOLD needs an explicit initial value', code: '[ 1 2 3 ] 0 [ ADD ] FOLD' },
  {
    id: 'record-basic',
    title: 'A Record from a Vector of keys and a Vector of values',
    code: "[ 'x' 'y' ] [ 1 2 ] RECORD",
  },
  {
    id: 'def-basic',
    title: 'Define a user word: [ body ] then name, then DEF',
    code: "[ 1 2 ADD ] 'MY-SUM' DEF MY-SUM",
  },
  {
    id: 'select-basic',
    title: 'SELECT: the two candidates, then the truth that chooses between them',
    code: "'non-negative' 'negative' 4 0 LT NOT SELECT PRINT",
  },
  {
    id: 'select-lanes',
    title: 'SELECT chooses lane by lane, so a whole vector branches at once',
    code: '0 [ -3 5 -1 ] [ -3 5 -1 ] 0 LT SELECT',
  },
  {
    id: 'select-whole',
    title: 'To choose one whole value by one truth, collect both and GET',
    code: '[ 1 2 3 ] [ 9 ] 2 COLLECT 0 1 FALSE SELECT GET',
  },
  { title: 'Strings are bare \'...\' literals; CHARS/JOIN convert', code: "'hello' CHARS REVERSE JOIN" },
  { title: 'Cast a string to an exact number', code: "'42' NUM" },
  { title: 'PRINT pops and emits to output (not the stack)', code: '[ 1 2 3 ] PRINT' },
  { title: 'Sorting is a plain Core word', code: '[ 3 1 2 ] SORT' },
  { title: 'Exact square root takes a bare scalar', code: '2 SQRT' },
  { title: 'A value used twice is named with BIND', code: "5 'N' BIND N N 1 ADD" },
];

// `#:contract` declarations: the one feature README's "Why Ajisai" leads
// with that no reading surface showed the syntax of. Each is run through the
// real check, so the grammar written here is the grammar the checker parses.
const contractDeclarations = [
  {
    id: 'contract-verified',
    title: 'Declare what your Word does; check verifies it before anything runs',
    code: "#:contract DOUBLE inputs=1 outputs=1 field=closed\n[ 2 MUL ] 'DOUBLE' DEF 21 DOUBLE",
  },
];

const commonErrors = [
  {
    title: 'Typo / unknown word',
    code: '[ 1 ] ADDD',
    fix: 'The message names the Core Words within a typo of what you wrote (here: `ADD`); otherwise grep §9. Word names are upper-cased automatically.',
  },
  {
    title: 'A Word from another language that Ajisai writes as a phrase',
    code: '10 3 MOD',
    fix: 'The message carries one runnable line that does the same thing with Core Words — MOD, OR, ABS, DUP, SWAP, IF, SUM and the like each have one. Adapt it to your operands; DEF it as a User Word if you need it more than once.',
  },
  {
    title: 'Stack underflow: operands must be pushed first',
    code: 'ADD',
    fix: 'Push both operands before the operator: `1 2 ADD`. Ajisai is postfix; there is no infix form.',
  },
  {
    title: 'FOLD without an initial value',
    code: '[ 1 2 3 ] [ ADD ] FOLD',
    fix: 'FOLD is `vector init [ op ] FOLD`: `[ 1 2 3 ] 0 [ ADD ] FOLD`.',
  },
  {
    title: 'A block that leaves more than its one result',
    code: '[ 1 2 3 ] [ 2 MUL 7 ] MAP',
    fix: 'A MAP / FILTER / FOLD / SCAN block leaves exactly one value — its result. A surplus is not discarded, it is this error. Consume what you computed before the block ends, or name it with `BIND`, which leaves nothing: `[ 2 MUL ]`, or `[ \'X\' BIND X X MUL ]`. `check` reports this before anything runs when the block is written right before the Word.',
  },
  {
    title: 'A `#:contract` declaration the body contradicts',
    code: "#:contract DOUBLE inputs=2 outputs=1\n[ 2 MUL ] 'DOUBLE' DEF 5 DOUBLE",
    fix: 'The declaration is checked before anything runs, and a violated one stops the run (`contractDecls.findings` lists every finding). Fix the body or the line; `ajisai agent infer-contracts` answers a paste-ready `suggested` line for the Word as written.',
  },
  {
    title: 'SELECT takes three operands: both candidates, then the truth',
    code: "[ 'big' ] [ 5 ] [ 3 ] GT SELECT",
    fix: "SELECT is `whenTrue whenFalse truth SELECT` — push both candidates before the test that chooses between them: `[ 'big' ] [ 'small' ] [ 5 ] [ 3 ] GT SELECT`. It chooses between values, never running either one, so an effect goes after it: `... SELECT PRINT`.",
  },
  {
    title: 'SELECT needs a truth value, not a number',
    code: "[ 'y' ] [ 'n' ] 1 SELECT",
    fix: 'The third operand must be TRUE, FALSE or an absence — a scalar is not a truth value (§4). Write the test: `1 0 EQ NOT`.',
  },
  {
    title: 'Broadcast shape mismatch',
    code: '[ 1 2 ] [ 1 2 3 ] ADD',
    fix: 'Elementwise ops need equal or broadcastable shapes (a scalar `5`, or a one-element vector, broadcasts; `[2]` vs `[3]` does not).',
  },
  {
    title: 'NUM casts strings, not booleans',
    code: 'TRUE NUM',
    fix: "NUM accepts strings: `'42' NUM`. There is no boolean→number cast.",
  },
  {
    title: 'Old one-vector RANGE form',
    code: '[ 0 5 ] RANGE',
    fix: 'RANGE takes two bounds: `0 5 RANGE`. A stride is a multiplication: `0 5 RANGE 2 MUL`.',
  },
];

// Programs that succeed while meaning something other than they look like they
// mean. `commonErrors` cannot hold these: nothing raises, so a reader — and an
// AI in particular — has no signal that anything went wrong, which is exactly
// what makes them worth writing down. Both the wrong and the right form are
// executed, and the two stacks are printed side by side.
const silentMistakes = [
  {
    title: 'A one-element vector where a Word wants an element',
    wrong: '[ 1 2 3 ] [ 1 ] [ 9 ] PUT',
    right: '[ 1 2 3 ] 1 9 PUT',
    fix: 'PUT and GET take an *element*, not a one-element vector holding it: `[ 9 ]` is that vector, so it is stored as one. Write a scalar bare (§2) and this cannot happen; a one-element vector is a vector, everywhere, and no error says so.',
  },
];

const forbiddenPatterns = [
  {
    pattern: 'DUP / SWAP / DROP / OVER / ROT',
    code: 'DUP',
    why: 'Forth-style stack shufflers do not exist. Every Word consumes the operands it reads; name a value with `BIND` to use it more than once.',
  },
  {
    pattern: 'IF / ELSE / THEN / WHILE',
    code: '[ 1 ] IF',
    why: 'No structured keywords, and no loops. Branch with SELECT over two values; iterate with MAP / FILTER / FOLD / SCAN.',
  },
  {
    pattern: 'A word calling itself',
    code: "[ REC ] 'REC' DEF",
    why: 'The User dictionary is acyclic: `DEF` refuses a body that names the word being defined, directly or through other user words, so this fails at definition time rather than the call. Repetition is expressed only through MAP / FILTER / FOLD / SCAN over an already-finite vector.',
  },
  {
    pattern: 'Parentheses ( )',
    code: '( 1 2 )',
    why: 'Reserved; not valid in source. `[ ]` is the sole bracket, for vectors and code alike.',
  },
  {
    pattern: 'Double-quoted strings',
    code: '"hello" PRINT',
    why: "Strings use single quotes: 'hello'.",
  },
  {
    pattern: '// line comments',
    code: '// comment',
    why: 'Comments start with `#`.',
  },
];

// ---------------------------------------------------------------------------
// Section renderers
// ---------------------------------------------------------------------------

function renderResult(json) {
  const parts = [];
  if (json.output.length > 0) parts.push(`prints \`${json.output.join(' ⏎ ')}\``);
  if (json.stackDisplay.length > 0) parts.push(`stack: \`${json.stackDisplay.join('  ')}\``);
  if (parts.length === 0) parts.push('stack: (empty)');
  return parts.join('; ');
}

// §2/§3 used to restate a runnable-looking syntax shape (`[ body ] 'NAME'
// DEF`, `[ a ] [ b ] truth SELECT`) as its own hand-typed
// backtick span, independent of the identical, generator-executed example a
// few sections later in §6 — exactly the "same fact in two places, only one
// of them cross-checked" pattern this repo's Phase-2 alignment pass looks
// for. `{ body } 'NAME' DEF` drifted from real syntax there and went
// undetected because nothing ran it. Pulling the literal code from the §6
// entry by id closes that gap structurally: there is only one hand-typed
// copy of the fact, and it is the one the generator already executes.
function canonicalExampleCode(id) {
  const found = canonicalExamples.find((entry) => entry.id === id);
  if (!found) fail(`no canonicalExamples entry with id ${JSON.stringify(id)}`);
  return found.code;
}

function renderCanonicalExamples() {
  return canonicalExamples
    .map((example) => {
      const json = expectOk(example.code);
      return `- ${example.title}\n  \`${example.code}\` → ${renderResult(json)}`;
    })
    .join('\n');
}

/** The display of an executed §6 example, by id — the one hand-typed copy. */
function canonicalExampleDisplay(id) {
  return expectOk(canonicalExampleCode(id)).stackDisplay.join('  ');
}

function renderContractDeclarations() {
  return contractDeclarations
    .map((entry) => {
      const checked = expectCheckOk(entry.code);
      const decls = checked.contractDecls;
      if (!decls || decls.outcome !== 'value') {
        fail(`declaration must verify (contractDecls.outcome value), got ${JSON.stringify(decls)}: ${entry.code}`);
      }
      const run = expectOk(entry.code);
      if (run.contractDecls?.outcome !== 'value') fail(`compute must carry the verified declaration: ${entry.code}`);
      return [
        `- ${entry.title}`,
        '  ```ajisai',
        ...entry.code.split('\n').map((line) => `  ${line}`),
        '  ```',
        `  \`check\` → exit 0, \`contractDecls: { outcome: "value" }\`; \`compute\` → ${renderResult(run)}, with the same \`contractDecls\`.`,
      ].join('\n');
    })
    .join('\n');
}

function renderCommonErrors() {
  return commonErrors
    .map((entry) => {
      const json = expectError(entry.code);
      return [
        `- **${entry.title}** — \`${entry.code}\``,
        `  → exit 1, ${inlineCode(`message: ${JSON.stringify(json.message)}`)}, ${inlineCode(`error: ${JSON.stringify(json.error)}`)}.`,
        `  Fix: ${entry.fix}`,
      ].join('\n');
    })
    .join('\n');
}

function renderSilentMistakes() {
  return silentMistakes
    .map((entry) => {
      const wrong = expectOk(entry.wrong);
      const right = expectOk(entry.right);
      if (wrong.stackDisplay.join(' ') === right.stackDisplay.join(' ')) {
        fail(`silent mistake ${JSON.stringify(entry.wrong)} no longer differs from the correct form`);
      }
      return [
        `- **${entry.title}** — both of these succeed (exit 0):`,
        `  \`${entry.wrong}\` → stack \`${wrong.stackDisplay.join(' ')}\``,
        `  \`${entry.right}\` → stack \`${right.stackDisplay.join(' ')}\``,
        `  Fix: ${entry.fix}`,
      ].join('\n');
    })
    .join('\n');
}

function renderForbiddenPatterns() {
  return forbiddenPatterns
    .map((entry) => {
      expectError(entry.code); // verified: really rejected by the implementation
      return `- **${entry.pattern}** (\`${entry.code}\` fails) — ${entry.why}`;
    })
    .join('\n');
}

function verifiedNilSection() {
  // Verify the documented NIL behavior against the real CLI before writing it.
  const projected = expectOk('-1 SQRT');
  if (projected.stackDisplay.join(' ') !== 'NIL') fail('a negative radicand must project to NIL');
  const reason = projected.stack[0]?.semantics?.absence?.reason;
  if (reason !== 'domainMiss') fail('a negative radicand must carry the reason domainMiss');
  const fallback = expectOk("-1 SQRT 'S' BIND 99 S S NIL? SELECT");
  if (fallback.stackDisplay.join(' ') !== '99/1') fail('the fallback must replace NIL');
  // Lifted over a vector the same law projects lane by lane, so the top stays
  // a vector. Written with a scalar operand it would read as `NIL`, which
  // taught the collapse rather than the lane law.
  const lifted = expectOk('[ 4 -1 ] SQRT');
  if (lifted.stackDisplay.join(' ') !== '[ 2/1 NIL ]') {
    fail('a negative radicand must empty only its own lane');
  }
  // Division is not where NIL comes from: a quotient by zero is a number.
  const byZero = expectOk('100 0 DIV -5 0 DIV 0 0 DIV');
  if (byZero.stackDisplay.join(' ') !== '1/0 1/0 0/0') fail('a quotient by zero is a point over zero');
  return {
    reason,
    fallbackStack: fallback.stackDisplay[0],
    liftedStack: lifted.stackDisplay[0],
    byZeroStack: byZero.stackDisplay.join(' '),
  };
}

function verifiedExactnessSection() {
  // Comparison over the algebraic field is total: values built through
  // different histories compare equal when they denote the same real.
  const json = expectOk('8 SQRT 2 SQRT 2 SQRT ADD EQ');
  if (json.stackDisplay.join(' ') !== 'TRUE') fail('sqrt(8) must equal sqrt(2)+sqrt(2)');
  // POW answers inside the field and nowhere else: a cube root leaves it.
  const outside = expectOk('8 1/3 POW NIL-REASON');
  if (outside.stackDisplay.join(' ') !== "'domainMiss'") fail('8 1/3 POW must project domainMiss');
  return { decided: json.stackDisplay.join(' '), outside: outside.stackDisplay.join(' ') };
}

// ---------------------------------------------------------------------------
// Document assembly
// ---------------------------------------------------------------------------

function buildSkillMd() {
  const nil = verifiedNilSection();
  const exactness = verifiedExactnessSection();
  const wordRows = buildWordIndex();
  const vocabulary = readVocabularyCounts();

  return `<!-- GENERATED FILE — do not edit by hand.
     Regenerate: npm run generate:skill   (verified against the ajisai CLI)
     Source of truth for semantics: SPECIFICATION.html.
     Generator: scripts/generate-skill-md.mjs -->

# Ajisai — Agent Writing Protocol (SKILL.md)

How to *write working Ajisai on the first try*. Every code line below was
executed by the generator against the real interpreter; results shown are
actual outputs. **If a word is not in the §9 index, it does not exist — when
unsure, grep §9 before writing.** Each Word's full summary is in
\`skill/words.md\` (over MCP: \`word_contract\` or \`ajisai://contracts\`).

## 1. Run loop

\`\`\`sh
ajisai agent compute program.ajisai   # exit 0 = ok, 1 = language error, 2 = usage
ajisai agent check program.ajisai     # resolve names, verify contracts and blocks; no execution
\`\`\`

Read the JSON in this order (contract: docs/dev/agent-cli-output-contract.md):
1. \`status\` / exit code. On ok: \`stackDisplay\` (final stack, bottom→top) and \`output\` (PRINT lines).
2. On error: \`message\`, and \`error\` — \`category\` (the spec/outcomes.json error category), \`resource\` for a ceiling, \`word\` (the Word it is attributed to) and \`line\`/\`column\` locate the failure.
3. A NIL on the stack carries its reason in \`semantics.absence.reason\`.

## 2. Minimal syntax

- Postfix, stack-based. Operands first, word last: \`1 2 ADD\`.
- Numbers are **exact rationals** (\`1/3\`, \`3.14\` → 157/50). No floats. Display shows \`3/1\` for 3.
- A scalar is written bare: \`${canonicalExampleCode('scalar')}\`, \`${canonicalExampleCode('div')}\`. Data lives in vectors: \`[ 1 2 3 ]\`, and vectors nest for ragged and grouped data. \`[ 42 ]\` is a one-element *vector*, not another way to write 42 — arithmetic broadcasts it like a scalar, but a Word that takes an *element* (\`PUT\`, \`GET\`, \`RECONCILE\`) stores or reads the vector itself, and nothing errors (§7). Write scalars bare and the question never arises.
- Strings: \`'single quotes'\` (a value domain of its own, not a vector of codepoints). Booleans: \`TRUE\` / \`FALSE\`, with a NIL read as UNKNOWN — SQL's three-valued logic. Absence: \`NIL\`.
- Code blocks are quoted programs passed to MAP / FILTER / FOLD / DEF, written as an ordinary Vector (§6) — there is no separate block bracket. SELECT is not among them: it takes values, not code.
- Named data is a Record, built by \`RECORD\` from a Vector of keys and a Vector of values: \`${canonicalExampleCode('record-basic')}\`. It is not a Vector and is never code. It displays as \`${canonicalExampleDisplay('record-basic')}\` — the keys, the values and the Word that joins them, which is also valid source that rebuilds it.
- Define a user word with a body Vector, then a \`'NAME'\` string, then \`DEF\`, then call \`NAME\`: \`${canonicalExampleCode('def-basic')}\` (§6). Words are case-insensitive (canonicalized to upper case).
- Declare a user word's contract on a \`#:contract\` comment line — \`#:contract DOUBLE inputs=1 outputs=1 field=closed\` — and \`check\` verifies it against the body before anything runs; \`compute\` runs the same check first and refuses a program whose declaration is violated (§2a).
- Comments: \`#\` to end of line.
- Every Word consumes the operands it reads. To use a value more than once, name it with \`BIND\` and read the name: \`5 'N' BIND N N 1 ADD\` leaves \`5/1  6/1\`.
- One word does one thing to the stack; there are **no** DUP/SWAP-style shufflers (§8).

## 2a. Declaring a contract (\`#:contract\`)

One comment line per Word, written in the keys and values of a contract Record
(the same vocabulary \`CONTRACT\` answers in):

\`#:contract NAME [inputs=N] [outputs=N] [field=closed|leaving]\`

\`field=closed\` promises the Word never answers a point over zero (\`1/0\`
\`0/0\`) from operands holding none — only \`DIV\`, \`POW\`, \`NUM\` and a literal over
zero leave the field, and a \`DIV\` by a non-zero literal (\`2 DIV\`) or a \`POW\`
to a non-negative literal (\`2 POW\`) does not; a divisor the body computes is
not read. Inside \`closed\` code the field laws (distributivity, \`x x SUB\` = 0)
hold; a \`leaving\` report's \`fieldExits\` names where the body leaves.

\`inputs\`/\`outputs\` must equal what the body does; \`field=closed\` is a bound the
body must not exceed; a key left out is not checked. The check is conservative:
a body the inference cannot read is reported as *cannot verify* (a \`note\`),
never as a false violation. To write a
declaration without guessing, run \`infer_contracts\` first and paste its
\`suggested\` line.

${renderContractDeclarations()}

## 3. Control and iteration

- Branch: the two candidates, then the truth that chooses between them, then \`SELECT\`: \`${canonicalExampleCode('select-basic')}\` (§6). Both candidates are values the program already built, so neither is skipped and nothing is evaluated by SELECT itself. The choice is made lane by lane, so a Vector of truths branches a whole Vector at once: \`${canonicalExampleCode('select-lanes')}\`. An absent truth chooses neither and answers that same absence. For the same reason SELECT cannot choose between two whole Vectors: candidates of different lengths are \`shapeMismatch\`, and a one-element candidate is silently repeated (\`[ 1 2 3 ] [ 9 ] FALSE SELECT\` is \`[ 9 9 9 ]\`). To pick one whole value by one truth, collect both and read one: \`${canonicalExampleCode('select-whole')}\` is \`[ 9 ]\`.
- Iterate data, not counters: \`MAP\` / \`FILTER\` / \`FOLD\` with block operands (examples in §6). \`FOLD\` requires an explicit initial value: \`${canonicalExampleCode('fold')}\`. The block leaves **exactly one** value — the mapped element, the truth, the next accumulator; leaving none or a surplus is a \`blockContractViolation\` (§7).
- Budget: a block iteration costs one step per element for every Word the block runs (literals are free), so under the MCP profile's 100,000-step budget a one-Word block (\`[ ADD ] FOLD\`) walks 100,000 elements, a two-Word block (\`[ 3 MUL 1 ADD ] MAP\`) 50,000, a three-Word block 33,000 — tens of thousands of elements divided by the Words in the block, not more. Beyond that, write the operation on whole vectors — \`V 3 MUL 1 ADD\` over 100,000 lanes is two steps — and leave no large intermediate on the stack.
- Irrationals cost more than their class says: an axis marked \`valueDependent: "algebraicTerms"\` in \`word_contract\` (MUL, ADD, EQ, SORT and others) also grows with the number of terms in an algebraic value, and a product of distinct square roots doubles its terms with each factor. Run a small case first and read \`resourceUsage.algebraicTerms\`, the largest term count a result reached.
- No recursion: \`DEF\` refuses a word whose body names itself, directly or through other user words (a diagnosed error at definition time, not at the call). Repetition is expressed only through MAP / FILTER / FOLD / SCAN over an already-finite vector.
- No block runs itself: a block EXEC / MAP / FILTER / FOLD / SCAN runs is *written* (a \`[ ]\` literal of the code running it, or one nested inside) or *built from data* (taken apart and rebuilt, or bound in another frame). Only one block built from data may run at a time; a second is \`nestedDataBlock\`. So code that runs a block it is given cannot be given one that does the same — write such code as Words with \`DEF\`, which nest freely.
- JSON boundary: \`JSON-ENCODE\` writes a rational with no finite decimal, and \`1/0\` and \`0/0\`, as its lexeme in a string (\`"1/3"\`), so \`JSON-DECODE\` gives back a String and \`NUM\` recovers the number. For a receiver that needs a JSON or SQL number, choose the digits first: \`1 3 DIV 2 FORMAT NUM JSON-ENCODE\` is \`'0.33'\`.
- Naming a Word: \`DEF\`, \`BIND\`, \`DEL\` and \`CONTRACT\` take its name as a String (\`'INC' CONTRACT\`); \`CONTRACT\` also takes a Symbol, written \`[ INC ] 0 GET\`. A bare \`INC\` *calls* the Word, so \`INC CONTRACT\` runs INC first. \`DIGEST\` is the one exception: a String is text there, so \`'INC' DIGEST\` digests the text and \`[ INC ] 0 GET DIGEST\` is the Word's content identity.

## 4. NIL — absence is a value, not an exception

Failed partial operations *project to NIL*: \`-1 SQRT\` succeeds (exit 0) and
pushes \`NIL\` (reason: \`${nil.reason}\`). The NIL value itself carries
\`semantics.absence.reason\` on the stack.

Division by zero is **not** a projection: every number is a pair over a
non-negative denominator, taken up to a nonzero factor (the wheel of fractions),
and a quotient by zero is \`1/0\` for any nonzero dividend, whatever its sign,
and \`0/0\` for zero — \`100 0 DIV -5 0 DIV 0 0 DIV\` → stack \`${nil.byZeroStack}\`.
The two points \`1/0\` and \`0/0\` are numbers: they add, multiply, divide and
compare by \`EQ\`. \`1/0\` is unsigned and \`0/0\` absorbs everything it meets;
neither has a place in the order, so \`LT\` / \`GT\` / \`MIN\` / \`MAX\` / \`SORT\`
project \`domainMiss\` on them.

- Provide a fallback with \`BIND\`, \`NIL?\` and \`SELECT\`: \`-1 SQRT 'S' BIND 99 S S NIL? SELECT\` → stack \`${nil.fallbackStack}\`. \`NIL?\` consumes its subject like every Word and answers whether it was absent, which is exactly where \`SELECT\` wants the truth — so name the subject once and read it twice: the phrase reads "S, or the fallback if S is absent".
- Over a vector the projection is **per lane, not per value**: \`[ 4 -1 ] SQRT\` → stack \`${nil.liftedStack}\`. The lane that had no root is the only one emptied.
- That makes the top a vector, not a NIL, so \`NIL?\` — which asks about the whole value — answers FALSE and the fallback is not chosen. Recover a lifted result inside the vector, not around it.
- NIL flows through later operations (NIL projection rule); check for it where it matters instead of letting it propagate to the end.

## 5. Exactness — comparison decides over the algebraic field

Numbers are exact rationals, closed under \`SQRT\`. Arithmetic never rounds,
coefficients are arbitrary-precision, and **every comparison of two scalars
built from rationals and \`SQRT\` decides**: there is no budget, no refinement
limit, and no undecided outcome.

\`\`\`ajisai
8 SQRT 2 SQRT 2 SQRT ADD EQ   # √8 vs √2+√2
\`\`\`

→ stack \`${exactness.decided}\` (exit 0). Values built through different
histories are the same value when they denote the same real.

That field is the whole numeric domain. \`POW\` answers inside it — an integer
exponent, or \`p/2\` over a non-negative rational — and projects NIL for any
other exponent rather than leave it:

\`\`\`ajisai
8 1/3 POW NIL-REASON
\`\`\`

→ stack \`${exactness.outside}\` (exit 0). Truth has three values: \`TRUE\`,
\`FALSE\`, and the logical UNKNOWN, which is what a NIL operand reads as in a
truth position (§4) — no comparison produces it of its own. An operation that
cannot produce a value produces NIL (§4); a malformed one raises an error.

## 6. Canonical examples (all verified by the generator)

${renderCanonicalExamples()}

## 7. Common errors — actual CLI output, and the fix

${renderCommonErrors()}

These raise. The next one does not — it succeeds and answers something other
than it looks like it answers, which is the harder kind to notice:

${renderSilentMistakes()}

## 8. Forbidden patterns (each verified to fail)

${renderForbiddenPatterns()}

## 9. Word index

Generated from \`docs/word-manifest.json\` — the complete inventory:
${vocabulary.canonicalWords} canonical Words in one flat Core dictionary, of which
${vocabulary.semanticKernelWords} form the Semantic Kernel and ${vocabulary.standardWords} are Standard Words. Both are
ordinary Core Words called by their plain names; the split is a design
classification, not a namespace. A word absent here does not exist. There is
no module system and nothing to import. For a phrase you would otherwise
write out again and again — a remainder, a disjunction, a sum or mean, text
joined with a separator — \`docs/phrasebook/phrasebook.ajisai\` holds User Word
definitions whose contracts and examples CI verifies; copy the definitions
you need to the top of your program (over MCP, name them in \`words\`).

Every Word, by family. \`skill/words.md\` has each one's full summary — operand
order, NIL and error behaviour, and an example — and §6 and §7 above show the
common ones at work.

| family | Words |
|---|---|
${wordRows.join('\n')}
`;
}

/** skill/words.md: the full per-Word reference SKILL.md's §9 indexes. */
function buildSkillWords() {
  return `<!-- GENERATED FILE — do not edit by hand.
     Regenerate: npm run generate:skill
     Source of truth: spec/words.json. Generator: scripts/generate-skill-md.mjs -->

# Ajisai — every Word (companion to SKILL.md)

The full summary of every Core Word, from \`spec/words.json\`. \`SKILL.md\` §9
indexes these in one line each; read the row here when a Word's operand order,
NIL behaviour or errors matter. A word absent here does not exist.

| word | family | summary |
|---|---|---|
${buildWordTable().join('\n')}
`;
}

/**
 * SKILL.md is read whole by an agent before it writes a line, so its size is
 * a cost paid on every task. It was 63.5 KB, three quarters of it the full
 * Word summaries now in skill/words.md, and is about 20 KB without them. The
 * budget leaves room for a few more rules and keeps the file from growing
 * back; a section that needs more moves detail to skill/words.md.
 */
const SKILL_MD_BYTE_BUDGET = 22_528;

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

const content = buildSkillMd();
const skillBytes = Buffer.byteLength(content, 'utf8');
if (skillBytes > SKILL_MD_BYTE_BUDGET) {
  fail(`SKILL.md would be ${skillBytes} bytes, over its ${SKILL_MD_BYTE_BUDGET}-byte budget; move detail to skill/words.md`);
}

if (process.argv.includes('--stdout') && !process.argv.includes('--check')) {
  process.stdout.write(content);
} else {
  writeOrCheck(
    'skill-md',
    [{
      path: 'SKILL.md',
      content,
      missing: 'SKILL.md is missing; run `npm run generate:skill`',
      stale: 'SKILL.md is stale relative to the sources/CLI; run `npm run generate:skill` and commit the result',
    }, {
      path: 'skill/words.md',
      content: buildSkillWords(),
      missing: 'skill/words.md is missing; run `npm run generate:skill`',
      stale: 'skill/words.md is stale relative to spec/words.json; run `npm run generate:skill` and commit the result',
    }],
    { current: 'SKILL.md is up to date.', wrote: `wrote ${resolve(repoRoot, 'SKILL.md')}` },
  );
}
