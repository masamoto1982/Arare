#!/usr/bin/env node
// Generate docs/semantics-table.json: the exhaustive (Word x input-domain
// tuple) -> outcome-category table. Domains were first chosen by type, then
// re-chosen by the outcome each one reaches (below).
//
// Every Word sits in one flat dictionary with no imports, so the language's
// whole input/outcome surface is finite. Excluding the Words of data-dependent
// arity (today COLLECT and EXEC) leaves the rest with a
// fixed integer arity; every (Word, domain tuple) pair is run through the
// real `ajisai` CLI and its outcome recorded as a stable id — never the
// human-readable `message`, which can be reworded without changing meaning.
//
// Domain representatives are chosen by *outcome*, not by type (Phase 3): each
// one exists to reach a specific declared condition, recorded as its
// `motivatedBy`. A domain with no motivation is not added — see the pitfall
// notes below for the ones considered and rejected.
//
// Usage:
//   node scripts/generate-semantics-table.mjs            # write docs/semantics-table.json
//   node scripts/generate-semantics-table.mjs --check    # fail if it is stale
//   AJISAI_BIN=/path/to/ajisai ...                        # override CLI binary

import { cpus, totalmem } from 'node:os';
import { classifyOutcome, fatal, resolveAjisaiBin, spawnAgentAsync, words, writeOrCheck } from './lib/common.mjs';

const fail = (message) => fatal('semantics-table', message);

// ---------------------------------------------------------------------------
// Domain representatives (work order Phase 3, §3.4 as corrected against the
// real CLI — every claim below was checked with a live probe, not assumed).
//
// `motivatedBy` names the outcome id(s) this domain exists to reach. A domain
// with none is a *carried-over type representative*, standing in for one of
// the original six (scalar/boolean/string/vector/nil/codeblock); Phase 3
// does not add a new domain motivated by nothing (pitfall A).
//
// Order and source text are fixed by this file and must not change casually:
// the order fixes the lexicographic order of domain tuples, which the
// committed table's cell order depends on.
//
// Domains considered and rejected, with the reason (kept here so the
// rejection is not silently rediscovered):
//
//   - `scalarFraction` ('1 2 DIV'), meant to reach `invalidInteger`: the table
//     already observes it through the integer-taking Words' existing domains,
//     so the whole table need not carry another.
//   - `vectorRagged` ('[ [ 1 ] [ 2 3 ] ]'), meant to reach `shapeMismatch`:
//     a ragged vector broadcasts element-wise against a same-length flat
//     vector instead of raising (confirmed: `[ [ 1 ] [ 2 3 ] ] [ 1 2 ] ADD`
//     answers a value). `shapeMismatch` is reached far more directly by two
//     *flat* vectors of different lengths, which is what `vectorTriple`
//     below is for.
//   - `codeBlockFails` ('{ 1 0 DIV }'), meant to reach the retired
//     `NilReason::ExecutionFailure`: that reason no longer exists (Phase 1
//     deleted it as unreachable), and separately `{ }` is no longer valid
//     source syntax at all (confirmed: `{ 1 0 DIV }` is a MalformedSource
//     parse error, not a CodeBlock value) — code and data share `[ ]` since
//     the CodeBlock/Vector unification. Both premises this domain was
//     designed around are gone.
//   - `textEmpty`, `textNumeric`, `vectorEmpty`, `vectorWithNil`: none names
//     a declared condition beyond what `textShort`/`vectorPair` already
//     reach — they would answer `value` everywhere, which is worth knowing
//     but is not a `motivatedBy` claim this file can make honestly.
//
// The one correction to an existing domain: `codeblock` used `'{ 1 }'`,
// which is the same dead syntax above — every cell that domain touched in
// the committed (pre-Phase-3) table was actually testing a parse error, not
// a CodeBlock value. Replaced with `'[ 1 ]'`.
// ---------------------------------------------------------------------------

const DOMAINS = [
  { id: 'scalarOne', source: '1', motivatedBy: [] },
  { id: 'scalarZero', source: '0', motivatedBy: [] },
  { id: 'scalarNegative', source: '-1', motivatedBy: ['domainMiss'] },
  // `0/0` is a number with no place in the order (LANG.VALUES.EXACT): the
  // one scalar the order Words — LT GT MIN MAX SORT ORDER BSEARCH — project
  // on, and the one FORMAT has no digits for.
  { id: 'scalarNullity', source: '0/0', motivatedBy: ['domainMiss'] },
  { id: 'scalarLarge', source: '999', motivatedBy: ['indexOutOfBounds'] },
  { id: 'booleanTrue', source: 'TRUE', motivatedBy: [] },
  { id: 'textShort', source: "'a'", motivatedBy: [] },
  { id: 'vectorPair', source: '[ 1 2 ]', motivatedBy: [] },
  { id: 'vectorTriple', source: '[ 1 2 3 ]', motivatedBy: ['shapeMismatch'] },
  { id: 'vectorHuge', source: '[ 0 1000001 ]', motivatedBy: ['resourceLimitExceeded'] },
  { id: 'nilLiteral', source: 'NIL', motivatedBy: ['literal'] },
  { id: 'codeBlock', source: '[ 1 ]', motivatedBy: [] },
];

function* domainTuples(arity) {
  if (arity === 0) {
    yield [];
    return;
  }
  for (const head of DOMAINS) {
    for (const rest of domainTuples(arity - 1)) {
      yield [head, ...rest];
    }
  }
}

// ---------------------------------------------------------------------------
// Word selection (Step 2.2, pitfalls A/B). `stack.inputs` is a plain integer
// for every Word except those of data-dependent arity (a JSON string such as
// "variable" in spec/words.json — today COLLECT and EXEC).
// This is not a hardcoded list (Phase 3 pitfall E): whichever Words currently
// have non-numeric `stack.inputs` are excluded, whatever their names are.
// ---------------------------------------------------------------------------

function loadWords() {
  const parsed = words();
  if (!Array.isArray(parsed.entries) || parsed.entries.length === 0) {
    fail('spec/words.json has no entries');
  }
  return parsed.entries;
}

function arityExclusionReason(word) {
  const inputs = word.stack.inputs;
  if (inputs === 'variable') return 'variableArity';
  if (inputs === 'control') return 'controlArity';
  fail(`${word.name}: unexpected non-numeric stack.inputs ${JSON.stringify(inputs)}`);
}

function selectWords(words) {
  const excluded = [];
  const domainWords = [];
  for (const word of words) {
    if (typeof word.stack.inputs !== 'number') {
      excluded.push({ word: word.name, reason: arityExclusionReason(word) });
      continue;
    }
    domainWords.push(word);
  }
  return { excluded, domainWords };
}

// ---------------------------------------------------------------------------
// Outcome classification (Step 2.2, pitfall D) is scripts/lib/common.mjs's
// classifyOutcome: the gates that read this table classify their own runs with
// the same rule, so a cell's outcome is spelled the way they compare it.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// One cell, spawned asynchronously so a worker pool (below) can run several
// at once. `spawn` (not `spawnSync`) is what makes that possible: the CLI
// process runs while Node's event loop keeps dispatching the next one.
// ---------------------------------------------------------------------------

function runCellAsync(ajisaiBin, program) {
  return new Promise((resolveCell) => {
    const proc = spawnAgentAsync(ajisaiBin, program);
    let stdout = '';
    let stderr = '';
    proc.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    proc.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    proc.on('error', (err) => fail(`failed to spawn ajisai CLI: ${err.message}`));
    proc.on('close', (code, signal) => {
      // A child killed mid-write leaves a truncated stdout, which used to be
      // reported only as invalid JSON — and a cell like `[ 0 1000001 ] 999
      // RANGE` peaks at ~3 GB, so under memory pressure the kernel's OOM
      // killer is the likeliest cause. Name the signal instead of the symptom.
      if (signal !== null) {
        fail(
          `CLI for ${JSON.stringify(program)} was killed by ${signal}`
            + (signal === 'SIGKILL' ? ' (likely out of memory; see WORKER_MEMORY_BYTES)' : '')
            + ` after ${stdout.length} bytes of stdout`
        );
      }
      // The CLI exits 0 (OK) or 1 (a language ERROR); anything else is a
      // failure of the CLI itself, whose stdout is not a report.
      if (code !== 0 && code !== 1) {
        fail(`CLI for ${JSON.stringify(program)} exited ${code}: ${stderr.slice(0, 2000)}`);
      }
      let json;
      try {
        json = JSON.parse(stdout);
      } catch (err) {
        fail(
          `CLI stdout for ${JSON.stringify(program)} is not valid JSON (${err.message}); `
            + `${stdout.length} bytes, starting:\n${stdout.slice(0, 2000)}`
        );
      }
      try {
        resolveCell(classifyOutcome(json));
      } catch (err) {
        fail(err.message);
      }
    });
  });
}

// One CLI per CPU, but never more than memory holds. A single cell can be
// large: `[ 0 1000001 ] 999 RANGE` answers a million-element stack, and the CLI
// peaks near 3.2 GB rendering it while this process holds its ~200 MB report.
// Four of those at once exceed a 16 GB machine, and the kernel's OOM killer
// then takes one child mid-write — the check's intermittent failure. Budgeting
// 4 GiB per worker keeps the worst case inside memory on any machine.
const WORKER_MEMORY_BYTES = 4 * 1024 ** 3;
function poolSize() {
  return Math.max(1, Math.min(cpus().length, Math.floor(totalmem() / WORKER_MEMORY_BYTES)));
}

// A fixed-size pool of workers pulling from a shared index, each awaiting its
// own cell before taking the next — never more than `concurrency` CLI
// processes alive at once. `results[i]` is written by whichever worker
// happens to process job `i`, so the array comes out in job order regardless
// of which worker finished which job when (Phase 3 pitfall B): the table's
// cell order is a property of `jobs`, not of completion timing.
async function runPool(jobs, concurrency) {
  const results = new Array(jobs.length);
  let next = 0;
  async function worker() {
    while (next < jobs.length) {
      const i = next++;
      results[i] = await jobs[i]();
    }
  }
  await Promise.all(Array.from({ length: Math.min(concurrency, jobs.length) }, worker));
  return results;
}

// ---------------------------------------------------------------------------
// Table assembly. Cell order: Word appearance order in spec/words.json, then
// domain-tuple lexicographic order — never `Object.keys`/`Map` insertion
// order, or worker completion order, either of which the `--check` mode's
// string comparison would silently depend on otherwise.
// ---------------------------------------------------------------------------

async function buildTable(ajisaiBin) {
  const { excluded, domainWords } = selectWords(loadWords());
  const specs = [];
  for (const word of domainWords) {
    for (const tuple of domainTuples(word.stack.inputs)) {
      const operands = tuple.map((domain) => domain.source);
      const program = [...operands, word.name].join(' ');
      specs.push({ word: word.name, inputs: tuple.map((domain) => domain.id), program });
    }
  }

  const jobs = specs.map((spec) => () => runCellAsync(ajisaiBin, spec.program));
  const outcomes = await runPool(jobs, poolSize());
  const cells = specs.map((spec, i) => ({ word: spec.word, inputs: spec.inputs, outcome: outcomes[i] }));

  return {
    schemaVersion: 2,
    generator: 'scripts/generate-semantics-table.mjs',
    // The runtime ceilings this table assumes. A cell whose outcome depends
    // on a ceiling (`vectorHuge` reaching `resourceLimitExceeded`) is only reliably
    // reproducible under the same profile — see MCP_README's "the playground
    // applies a different, looser profile" (Phase 3 pitfall C). This is the
    // native CLI's own built-in default (`AJISAI_BIN`/`AJISAI_REPO` unset),
    // which is what generates the committed table.
    profile: {
      source: 'nativeCliDefault',
      maxMaterializedElements: 1_000_000,
    },
    domains: DOMAINS,
    excluded,
    cells,
  };
}

const table = await buildTable(resolveAjisaiBin('semantics-table'));
const json = `${JSON.stringify(table, null, 2)}\n`;

writeOrCheck(
  'semantics-table',
  [{
    path: 'docs/semantics-table.json',
    content: json,
    stale: 'docs/semantics-table.json is stale. Run `npm run semantics:table` and commit the result.',
  }],
  {
    current: `docs/semantics-table.json is up to date (${table.cells.length} cells).`,
    wrote: `wrote ${table.cells.length} cells to docs/semantics-table.json`,
  },
);
