#!/usr/bin/env node
// Lexicon-emergence experiment driver (design: docs/dev/lexicon-emergence-pilot-results-2026-09-23.md 付録 B).
//
//   node cli.mjs prompt  <run> <condition> <gen> <agent>   subject prompt for one agent
//   node cli.mjs grade   <run> <condition> <gen>           grade every submission of a generation
//   node cli.mjs evolve  <run> <condition> <gen> <K> [--rule uses|reinvention|breadth|families]
//                                                          choose generation gen+1's lexicon (K may be
//                                                          Infinity for C-open; --into <condition> writes
//                                                          it into another condition's gen+1)
//   node cli.mjs analyze <run>                             H1–H5 over every condition and generation
//
// The cli route (Phase 2: Claude Code subagents that run code through this file):
//   node cli.mjs prepare <run> <condition> <gen>                        write the generation's phrasebook
//   node cli.mjs compute <run> <condition> <gen> <agent> <file> [--words A,B] [--tool check]
//                                                                       run one file as a subject, logged
//   node cli.mjs shuffle <run> <from-condition> <from-gen> <condition> <gen>
//                                                                       C-shuffled: rename a lexicon's Words
//   node cli.mjs prompt  <run> <condition> <gen> <agent> --route cli    the cli route's subject prompt
//   node cli.mjs seal    <run> <condition> <gen>   move a graded generation's solutions out of the tree
//   node cli.mjs unseal  <run>                     bring every sealed generation back, for analysis
// Sealing keeps later subjects from reading earlier solutions: only the lexicon stays where they run.
// The stash is $LEXICON_STASH, or ~/.lexicon-stash, outside the repository.
// runs/<run>/design.json, when present, names the task families ("families") and which model
// each subject letter ran on ("models": { "A": "opus", … }), for grading and for the report,
// and may cap how many times each subject may run code ("callBudget": n, or per condition
// { "default": 0, "runs": null }, null meaning unlimited). The cap is stated in the cli route's
// prompt, and `compute` refuses a subject's run past it. "definitionBudget" (Phase 5), shaped the
// same, caps how many Words of its own a subject may define: the prompt states it and `grade`
// drops the definitions past it. "rotation" (Phase 4, lib/rotation.mjs)
// replaces "families": { "train": [[…], […]], "heldOut": […], "fixed": [<condition>, …] } —
// generation g trains on train[g mod n] (a fixed condition on train[0]) and also solves the
// held-out families, whose results never rank a Word for the next lexicon.
//
// Route B (the API harness, lib/harness.mjs) — needs ANTHROPIC_API_KEY and spends money:
//   node cli.mjs run   <run> <condition> <gen> <A,B,…> [options]   one generation's subjects
//   node cli.mjs pilot <run> [options]                             Phase 1's settings end to end
// Options: --model <id|large|medium|small> (default large), --effort <level> (default high),
//          --price <input>,<output> (USD per million tokens; required for a model with no listed price),
//          --budget <usd> (stop before any request once spent), --max-turns <n> (default 60),
//          and for pilot --gens <n> (3) --agents <n> (3) --k <n> (8).
//
// A run lives under runs/<run>/<condition>/gen<g>/ as lexicon.json, submissions/*.json, graded/*.json,
// for route B transcripts/*.json, and for the cli route calls/*.jsonl (every compute a subject ran)
// and work/<agent>/ (the files it ran).

import { appendFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { connect } from './lib/ajisai.mjs';
import { analyze, renderReport } from './lib/analyze.mjs';
import { classify, collectWords, nextLexicon } from './lib/evolve.mjs';
import { grade, loadFamilies } from './lib/grade.mjs';
import { allFamilyNames, familyNames } from './lib/rotation.mjs';
import { anthropicClient, Budget, MODELS, priceOf, runSubject, subjectSystem } from './lib/harness.mjs';
import { subjectPrompt } from './lib/prompt.mjs';
import { preludeMatches, writeLexiconPhrasebook } from './lib/lexicon-phrasebook.mjs';
import { shuffleLexicon } from './lib/shuffle.mjs';
import { norm } from './lib/source.mjs';

const ROOT = new URL('./', import.meta.url);
const TASKS = new URL('tasks/', ROOT);
const genDir = (run, condition, gen) => new URL(`runs/${run}/${condition}/gen${gen}/`, ROOT);

const readJson = (url) => JSON.parse(readFileSync(url, 'utf8'));
const writeJson = (url, value) => {
  mkdirSync(new URL('./', url), { recursive: true });
  writeFileSync(url, `${JSON.stringify(value, null, 2)}\n`);
};
const readAll = (dir) =>
  existsSync(dir) ? readdirSync(dir).filter((f) => f.endsWith('.json')).sort().map((f) => readJson(new URL(f, dir))) : [];
const lexiconOf = (dir) => (existsSync(new URL('lexicon.json', dir)) ? readJson(new URL('lexicon.json', dir)) : { entries: [] });

function* generations(run) {
  const base = new URL(`runs/${run}/`, ROOT);
  const conditions = readdirSync(base, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name);
  for (const condition of conditions) {
    const conditionDir = new URL(`${condition}/`, base);
    for (const g of readdirSync(conditionDir).filter((d) => /^gen\d+$/.test(d))) {
      const dir = new URL(`${g}/`, conditionDir);
      yield { condition, generation: Number(g.slice(3)), dir };
    }
  }
}

const argv = process.argv.slice(2);
const options = {};
const positional = [];
for (let i = 0; i < argv.length; i += 1) {
  if (argv[i].startsWith('--')) options[argv[i].slice(2)] = argv[(i += 1)];
  else positional.push(argv[i]);
}
const [command, run, condition, gen, arg] = positional;
const designOf = (name) => {
  const url = name ? new URL(`runs/${name}/design.json`, ROOT) : null;
  return url && existsSync(url) ? readJson(url) : {};
};
const design = designOf(run);
// "callBudget" and "definitionBudget" are each a number for every condition of
// a run, or an object keyed by condition with an optional "default"; null or
// absent means unlimited.
const perCondition = (key, conditionName) => {
  const b = design[key];
  if (b !== null && typeof b === 'object') return b[conditionName] !== undefined ? b[conditionName] : (b.default ?? null);
  return b ?? null;
};
const budgetFor = (conditionName) => perCondition('callBudget', conditionName);
const definitionBudgetFor = (conditionName) => perCondition('definitionBudget', conditionName);
// The families one condition's generation solves, and which of them are held out.
const familiesFor = (conditionName, generation) => {
  const names = familyNames(design, conditionName, generation);
  return { families: loadFamilies(TASKS, names.all), heldOut: names.heldOut };
};

async function gradeGeneration(ajisai, dir, conditionName, generation) {
  const lexicon = lexiconOf(dir);
  const { families, heldOut } = familiesFor(conditionName, generation);
  for (const submission of readAll(new URL('submissions/', dir))) {
    const graded = await grade(ajisai, submission, lexicon, families, heldOut, { definitionBudget: definitionBudgetFor(conditionName) });
    graded.calls = auditCalls(dir, submission.agent);
    writeJson(new URL(`graded/${submission.agent}.json`, dir), graded);
    const correct = graded.results.filter((r) => r.correct).length;
    const dropped = graded.definitionsDropped ? ` (${graded.definitionsDropped.length} definition(s) past the budget dropped)` : '';
    console.log(`${submission.agent}: ${correct}/${graded.results.length} correct${dropped}`);
  }
}

/**
 * The cli route's log of what a subject ran: how many calls, how many named
 * inherited Words, and whether each such call's `prelude` was the lexicon by
 * content (lexicon-digests.json). Null for a subject with no log.
 */
function auditCalls(dir, agent) {
  const log = new URL(`calls/${agent}.jsonl`, dir);
  if (!existsSync(log)) return null;
  const calls = readFileSync(log, 'utf8').split('\n').filter(Boolean).map((line) => JSON.parse(line));
  const digestsUrl = new URL('lexicon-digests.json', dir);
  const digests = new Map(existsSync(digestsUrl) ? Object.entries(readJson(digestsUrl)) : []);
  const withWords = calls.filter((c) => c.words?.length);
  return {
    total: calls.length,
    withWords: withWords.length,
    preludeMismatches: withWords.filter((c) => c.prelude && !preludeMatches(c.prelude, digests)).length,
  };
}

/** Write a generation's phrasebook and expected digests, once; the path, or null with no lexicon. */
async function prepareGeneration(ajisai, dir) {
  const lexicon = lexiconOf(dir);
  if (lexicon.entries.length === 0) return null;
  const phrasebook = fileURLToPath(new URL('phrasebook.ajisai', dir));
  if (!existsSync(phrasebook)) {
    const { digests } = await writeLexiconPhrasebook(ajisai, lexicon, phrasebook);
    writeJson(new URL('lexicon-digests.json', dir), Object.fromEntries(digests));
  }
  return phrasebook;
}

async function evolveGeneration(ajisai, runName, conditionName, generation, capacity) {
  const dir = genDir(runName, conditionName, generation);
  const lexicon = lexiconOf(dir);
  const graded = readAll(new URL('graded/', dir));
  const words = collectWords(graded, [lexicon]);
  const classes = await classify(ajisai, words);
  const next = nextLexicon(generation + 1, capacity, graded, words, classes, options.rule ?? design.selectionRule ?? 'uses');
  // Conditions that share a generation (Phase 3's gen0 feeds several
  // lexicon rules) write the next lexicon into another condition with --into.
  const into = options.into ?? conditionName;
  writeJson(new URL('lexicon.json', genDir(runName, into, generation + 1)), next);
  console.log(`${into} gen${generation + 1} lexicon (${next.rule}, K=${capacity}): ${next.entries.map((e) => e.name).join(' ')}`);
}

/** Route B: run every named subject of one generation in parallel through the API harness. */
async function runGeneration(ajisai, harness, runName, conditionName, generation, agents) {
  const dir = genDir(runName, conditionName, generation);
  const lexicon = lexiconOf(dir);
  // Subjects reach the inherited Words through the server's `words` argument,
  // from a phrasebook file written for this generation, and every result names
  // what it defined with its digest (lexicon-digests.json records the
  // expected ones). A generation with no lexicon runs on the plain server.
  let subjectAjisai = ajisai;
  if (lexicon.entries.length > 0) {
    const phrasebook = fileURLToPath(new URL('phrasebook.ajisai', dir));
    const { digests } = await writeLexiconPhrasebook(ajisai, lexicon, phrasebook);
    writeJson(new URL('lexicon-digests.json', dir), Object.fromEntries(digests));
    subjectAjisai = await connect({ phrasebook });
  }
  try {
    return await runSubjects(subjectAjisai, harness, dir, runName, conditionName, generation, agents, lexicon);
  } finally {
    if (subjectAjisai !== ajisai) await subjectAjisai.close();
  }
}

async function runSubjects(ajisai, harness, dir, runName, conditionName, generation, agents, lexicon) {
  const outcomes = await Promise.all(
    agents.map(async (agent) => {
      const meta = { run: runName, condition: conditionName, generation, agent };
      const { submission, record } = await runSubject({
        ...harness,
        ajisai,
        prompt: subjectPrompt(meta, lexicon, familiesFor(conditionName, generation).families, 'harness'),
        meta,
      });
      writeJson(new URL(`transcripts/${agent}.json`, dir), record);
      if (submission) writeJson(new URL(`submissions/${agent}.json`, dir), submission);
      console.log(`${agent}: ${record.ended} after ${record.turns} request(s), $${record.costUsd.toFixed(3)}`);
      return record;
    }),
  );
  return outcomes;
}

async function harnessSettings() {
  const model = MODELS[options.model ?? 'large'] ?? options.model;
  // Both are checked before any request is made: a model the harness cannot
  // price, or a budget that is not a number, would otherwise never stop a run.
  const price = options.price != null ? (([input, output]) => ({ input, output }))(options.price.split(',').map(Number)) : undefined;
  priceOf(model, price);
  const budget = new Budget(options.budget != null ? Number(options.budget) : null);
  return {
    client: await anthropicClient(),
    model,
    price,
    effort: options.effort ?? 'high',
    system: subjectSystem(new URL('../../SKILL.md', ROOT)),
    budget,
    maxTurns: Number(options['max-turns'] ?? 60),
  };
}

const letters = (n) => [...'ABCDEFGHIJ'].slice(0, n);

if (command === 'prompt') {
  process.stdout.write(
    subjectPrompt({ run, condition, generation: Number(gen), agent: arg }, lexiconOf(genDir(run, condition, gen)), familiesFor(condition, gen).families, options.route, {
      callBudget: budgetFor(condition),
      definitionBudget: definitionBudgetFor(condition),
    }),
  );
} else if (command === 'prepare') {
  const ajisai = await connect();
  try {
    const phrasebook = await prepareGeneration(ajisai, genDir(run, condition, gen));
    console.log(phrasebook ? `wrote ${phrasebook}` : 'no lexicon: subjects run on the plain server');
  } finally {
    await ajisai.close();
  }
} else if (command === 'compute') {
  // One run of a subject's file, through the server that serves this
  // generation's lexicon for `words`. The reply is the tool's text, exactly as
  // the MCP tool would give it; the call is logged for auditCalls.
  const dir = genDir(run, condition, gen);
  const file = positional[5];
  if (!arg || !file) {
    console.error('usage: node cli.mjs compute <run> <condition> <gen> <agent> <file> [--words A,B] [--tool check]');
    process.exit(2);
  }
  // With unlimited runs every subject tried until it was right, so accuracy
  // carried no information and only the number of runs differed (Haiku ran
  // 1,184 programs where Opus ran 116). A budget makes the runs count.
  const log = new URL(`calls/${arg}.jsonl`, dir);
  const spent = existsSync(log) ? readFileSync(log, 'utf8').split('\n').filter(Boolean).length : 0;
  const budget = budgetFor(condition);
  if (budget != null && spent >= budget) {
    process.stdout.write(`${JSON.stringify({ status: 'refused', message: `run budget spent: ${spent} of ${budget} runs used; write your submission from what you have` })}\n`);
    process.exit(3);
  }
  // The phrasebook is written by the first call of a generation (or by
  // `prepare`); later calls only read it.
  let phrasebook = null;
  if (lexiconOf(dir).entries.length > 0) {
    phrasebook = fileURLToPath(new URL('phrasebook.ajisai', dir));
    if (!existsSync(phrasebook)) {
      const plain = await connect();
      try {
        await prepareGeneration(plain, dir);
      } finally {
        await plain.close();
      }
    }
  }
  const ajisai = await connect(phrasebook ? { phrasebook } : {});
  try {
    const source = readFileSync(file, 'utf8');
    const words = options.words ? options.words.split(',').map((w) => w.trim()).filter(Boolean) : undefined;
    const tool = options.tool === 'check' ? 'check' : 'compute';
    const text = await ajisai.callText(tool, words ? { source, words } : { source });
    let parsed = null;
    try {
      parsed = JSON.parse(text);
    } catch {
      // A host error is text, not a result; it is logged as such.
    }
    mkdirSync(new URL('calls/', dir), { recursive: true });
    appendFileSync(
      new URL(`calls/${arg}.jsonl`, dir),
      `${JSON.stringify({ at: new Date().toISOString(), tool, words: words ?? [], source, status: parsed?.status ?? 'hostError', prelude: parsed?.prelude ?? null })}\n`,
    );
    process.stdout.write(`${text}\n`);
  } finally {
    await ajisai.close();
  }
} else if (command === 'shuffle') {
  // shuffle <run> <from-condition> <from-gen> <condition> <gen>
  const [, , fromCondition, fromGen, toCondition, toGen] = positional;
  const shuffled = shuffleLexicon(lexiconOf(genDir(run, fromCondition, fromGen)), `X${toGen}`);
  shuffled.generation = Number(toGen);
  writeJson(new URL('lexicon.json', genDir(run, toCondition, toGen)), shuffled);
  console.log(`${toCondition} gen${toGen} lexicon: ${shuffled.entries.map((e) => e.name).join(' ')}`);
} else if (command === 'seal' || command === 'unseal') {
  const stash = join(process.env.LEXICON_STASH ?? join(homedir(), '.lexicon-stash'), run);
  const moved = [];
  const move = (from, to) => {
    if (!existsSync(from)) return;
    cpSync(from, to, { recursive: true });
    rmSync(from, { recursive: true, force: true });
    moved.push(to);
  };
  if (command === 'seal') {
    const dir = fileURLToPath(genDir(run, condition, gen));
    for (const part of ['submissions', 'graded', 'calls', 'work']) {
      move(join(dir, part), join(stash, condition, `gen${gen}`, part));
    }
  } else if (existsSync(stash)) {
    for (const c of readdirSync(stash)) {
      for (const g of readdirSync(join(stash, c))) {
        for (const part of readdirSync(join(stash, c, g))) {
          move(join(stash, c, g, part), fileURLToPath(new URL(`runs/${run}/${c}/${g}/${part}`, ROOT)));
        }
      }
    }
    rmSync(stash, { recursive: true, force: true });
  }
  console.log(`${command}: ${moved.length} director${moved.length === 1 ? 'y' : 'ies'} moved`);
} else if (command === 'grade') {
  const ajisai = await connect();
  await gradeGeneration(ajisai, genDir(run, condition, gen), condition, gen);
  await ajisai.close();
} else if (command === 'evolve') {
  const ajisai = await connect();
  await evolveGeneration(ajisai, run, condition, Number(gen), Number(arg));
  await ajisai.close();
} else if (command === 'run') {
  const harness = await harnessSettings();
  const ajisai = await connect();
  try {
    await runGeneration(ajisai, harness, run, condition, Number(gen), arg.split(','));
  } finally {
    await ajisai.close();
  }
  console.log(`spent $${harness.budget.spentUsd.toFixed(3)}`);
} else if (command === 'pilot') {
  // Phase 1's design (work order §6): C-bottleneck over `gens` generations of
  // `agents` subjects with capacity K, and C-solo as one generation.
  const gens = Number(options.gens ?? 3);
  const agents = Number(options.agents ?? 3);
  const capacity = Number(options.k ?? 8);
  const harness = await harnessSettings();
  const ajisai = await connect();
  try {
    for (let g = 0; g < gens; g += 1) {
      await runGeneration(ajisai, harness, run, 'bottleneck', g, letters(agents).map((l) => `G${g}${l}`));
      await gradeGeneration(ajisai, genDir(run, 'bottleneck', g), 'bottleneck', g);
      if (g + 1 < gens) await evolveGeneration(ajisai, run, 'bottleneck', g, capacity);
    }
    await runGeneration(ajisai, harness, run, 'solo', 0, letters(agents).map((l) => `S0${l}`));
    await gradeGeneration(ajisai, genDir(run, 'solo', 0), 'solo', 0);
  } finally {
    await ajisai.close();
  }
  writeJson(new URL(`runs/${run}/harness.json`, ROOT), {
    model: harness.model,
    effort: harness.effort,
    maxTurns: harness.maxTurns,
    spentUsd: harness.budget.spentUsd,
    design: { gens, agents, capacity },
  });
  console.log(`spent $${harness.budget.spentUsd.toFixed(3)}; now: node cli.mjs analyze ${run}`);
} else if (command === 'analyze') {
  const gens = [...generations(run)].map(({ condition: c, generation, dir }) => ({
    condition: c,
    generation,
    lexicon: lexiconOf(dir),
    graded: readAll(new URL('graded/', dir)),
  }));
  const ajisai = await connect();
  const report = await analyze(ajisai, gens, loadFamilies(TASKS, allFamilyNames(design)), design.models ?? {});
  await ajisai.close();
  writeJson(new URL(`runs/${run}/report.json`, ROOT), report);
  writeFileSync(new URL(`runs/${run}/report.md`, ROOT), renderReport(report));
  console.log(renderReport(report));
} else {
  console.error('usage: node cli.mjs prompt|grade|evolve|analyze|run|pilot …  (see the header of cli.mjs)');
  process.exit(2);
}
