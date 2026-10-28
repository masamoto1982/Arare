// H1–H5 (work order §3) over a whole run. Every number here is counted from
// graded files; nothing is estimated.

import { readFileSync } from 'node:fs';
import { classify, collectWords } from './evolve.mjs';
import { norm, referencedWords, tokenize } from './source.mjs';

const CORE = JSON.parse(readFileSync(new URL('../../mcp-server/assets/words.json', import.meta.url), 'utf8'));
const CORE_NAMES = new Set(CORE.entries.map((w) => w.name));

const coreWordOf = (token) => {
  const t = norm(token);
  return CORE_NAMES.has(t) ? t : null;
};

/** The model a subject ran on: its letter (the last character of its name) looked up in `models`. */
const modelOf = (models, agent) => models[agent.at(-1)] ?? null;

/**
 * One generation's results on the training tasks and on the held-out ones
 * (rotation.mjs), each as correct/attempted, compression and the share of
 * correct solutions reaching an inherited Word; null when nothing is held
 * out. Held-out reuse is the lexicon carrying over to tasks it was not
 * chosen on.
 */
export function heldOutSplit(gen) {
  const all = gen.graded.flatMap((x) => x.results);
  if (!all.some((r) => r.heldOut)) return null;
  const inheritedNames = new Set(gen.lexicon.entries.map((e) => norm(e.name)));
  const part = (results) => {
    const correct = results.filter((r) => r.correct);
    const sum = (k) => correct.reduce((n, r) => n + r[k], 0);
    const reaching = correct.filter((r) => r.uses.some((n) => inheritedNames.has(n)));
    return {
      correct: `${correct.length}/${results.length}`,
      compression: correct.length ? +(sum('tokens') / sum('expandedTokens')).toFixed(3) : null,
      reaching: inheritedNames.size ? `${reaching.length}/${correct.length}` : null,
      tasksReaching: inheritedNames.size ? new Set(reaching.map((r) => r.task)).size : null,
    };
  };
  return { train: part(all.filter((r) => !r.heldOut)), heldOut: part(all.filter((r) => r.heldOut)) };
}

/**
 * Per held-out family of one generation: correct solutions, those calling a
 * Word the same subject also reached from a correct training solution (own or
 * inherited), and those calling an inherited Word. Where subjects may define
 * Words, the first all but bounds the second: in Phase 4, 1 of 858 correct
 * held-out solutions with a lexicon reached an inherited Word its subject had
 * not also reached in training, so a family that shares little with the
 * training tasks even inside one subject (`series`) is out of reach of any
 * lexicon chosen from them. Subjects who may not define Words (Phase 5's
 * `probe`) call an inherited Word where it fits a held-out family alone, so
 * there the bound does not hold. Null when nothing is held out.
 */
export function heldOutFamilies(gen) {
  if (!gen.graded.some((x) => x.results.some((r) => r.heldOut))) return null;
  const inheritedNames = new Set(gen.lexicon.entries.map((e) => norm(e.name)));
  const byFamily = new Map();
  for (const x of gen.graded) {
    const fromTraining = new Set(x.results.filter((r) => !r.heldOut && r.correct).flatMap((r) => r.uses));
    for (const r of x.results.filter((r) => r.heldOut)) {
      const family = r.task.slice(0, r.task.indexOf('.'));
      const f = byFamily.get(family) ?? { family, correct: 0, attempted: 0, shared: 0, inherited: 0 };
      f.attempted += 1;
      if (r.correct) {
        f.correct += 1;
        if (r.uses.some((n) => fromTraining.has(n))) f.shared += 1;
        if (r.uses.some((n) => inheritedNames.has(n))) f.inherited += 1;
      }
      byFamily.set(family, f);
    }
  }
  return [...byFamily.values()].sort((a, b) => a.family.localeCompare(b.family));
}

export async function analyze(ajisai, gens, families, models = {}) {
  const graded = gens.flatMap((g) => g.graded.map((x) => ({ ...x, condition: g.condition })));
  const words = collectWords(graded, gens.map((g) => g.lexicon));
  const classes = await classify(ajisai, words);
  const known = new Set(words.keys());

  // Per generation: accuracy, solution length, compression against the expanded form, H5.
  const perGeneration = gens
    .map((g) => {
      const results = g.graded.flatMap((x) => x.results.filter((r) => r.submitted));
      const correct = results.filter((r) => r.correct);
      const sum = (rs, k) => rs.reduce((n, r) => n + r[k], 0);
      return {
        condition: g.condition,
        generation: g.generation,
        agents: g.graded.length,
        lexiconSize: g.lexicon.entries.length,
        // Over what this generation's subjects were given, which with rotation differs by generation.
        accuracy: `${correct.length}/${g.graded.reduce((n, x) => n + x.results.length, 0)}`,
        meanTokens: correct.length ? +(sum(correct, 'tokens') / correct.length).toFixed(1) : null,
        compression: correct.length ? +(sum(correct, 'tokens') / sum(correct, 'expandedTokens')).toFixed(3) : null,
        coreEquivalent: `${correct.filter((r) => r.coreEquivalent).length}/${correct.length}`,
        // Runs each subject made (cli route), on average: with runs allowed,
        // difficulty shows here rather than in accuracy (difficulty calibration).
        meanRuns: g.graded.some((x) => x.calls)
          ? +(g.graded.reduce((n, x) => n + (x.calls?.total ?? 0), 0) / g.graded.length).toFixed(1)
          : null,
        // Own Words defined per subject: with an inherited lexicon, the ones it saved writing.
        ownWords: g.graded.length ? +(g.graded.reduce((n, x) => n + x.definitions.length, 0) / g.graded.length).toFixed(1) : null,
        split: heldOutSplit(g),
        heldOutFamilies: heldOutFamilies(g),
        // Correct solutions per model in this generation.
        byModel: Object.fromEntries(
          g.graded.map((x) => [modelOf(models, x.agent) ?? x.agent, `${x.results.filter((r) => r.correct).length}/${x.results.length}`]),
        ),
      };
    })
    .sort((a, b) => a.condition.localeCompare(b.condition) || a.generation - b.generation);

  // H1: classes defined independently by two or more agents. An agent that
  // was handed a member of the class in its lexicon is not independent.
  const inherited = new Map(graded.map((g) => [g.agent, new Set(g.lexiconNames.map((n) => classes.get(n)?.classId))]));
  const byClass = new Map();
  for (const [name, c] of classes) {
    const w = words.get(name);
    const entry = byClass.get(c.classId) ?? { members: [], definers: new Set(), links: new Set() };
    entry.members.push({ name: w.name, body: w.body, note: w.note ?? '', definedBy: w.definedBy ?? null });
    if (w.definedBy && !inherited.get(w.definedBy)?.has(c.classId)) entry.definers.add(w.definedBy);
    if (c.link !== 'self') entry.links.add(c.link);
    byClass.set(c.classId, entry);
  }
  const convergent = [...byClass.entries()]
    .filter(([, e]) => e.definers.size >= 2)
    .map(([classId, e]) => ({ classId, definers: [...e.definers], links: [...e.links], members: e.members }))
    .sort((a, b) => b.definers.length - a.definers.length);

  // H2: how many lexicon entries are built from other lexicon entries.
  const compositional = gens
    .filter((g) => g.lexicon.entries.length > 0)
    .map((g) => {
      const names = new Set(g.lexicon.entries.map((e) => norm(e.name)));
      const built = g.lexicon.entries.filter((e) => referencedWords(e.body, names).size > 0).length;
      return { condition: g.condition, generation: g.generation, compositional: `${built}/${names.size}` };
    });

  // H4: Core usage across correct solutions, after expanding user Words away.
  const coreUse = new Map([...CORE_NAMES].map((n) => [n, 0]));
  for (const g of graded) {
    const defs = new Map(g.definitions.map((d) => [norm(d.name), d.body]));
    for (const r of g.results) {
      if (!r.correct) continue;
      const seen = new Set();
      const walk = (source) => {
        for (const t of tokenize(source)) {
          const core = coreWordOf(t);
          if (core) seen.add(core);
          else if (known.has(norm(t)) && !seen.has(`user:${norm(t)}`)) {
            seen.add(`user:${norm(t)}`);
            walk(defs.get(norm(t)) ?? words.get(norm(t)).body);
          }
        }
      };
      walk(r.solution);
      for (const n of seen) if (!n.startsWith('user:')) coreUse.set(n, coreUse.get(n) + 1);
    }
  }

  // Accuracy per model, over every subject that ran on it.
  const perModel = [...new Set(graded.map((g) => modelOf(models, g.agent)))]
    .filter(Boolean)
    .map((model) => {
      const mine = graded.filter((g) => modelOf(models, g.agent) === model);
      const correct = mine.reduce((n, g) => n + g.results.filter((r) => r.correct).length, 0);
      const attempted = mine.reduce((n, g) => n + g.results.length, 0);
      return { model, agents: mine.length, accuracy: `${correct}/${attempted}` };
    });

  // H1's baseline, split by model: for every two subjects that inherited
  // nothing, the share of the classes either defined that both defined
  // (Jaccard). Same-model pairs against cross-model pairs is the comparison
  // a single-model pilot could not make.
  const definedBy = new Map();
  for (const [name, c] of classes) {
    const w = words.get(name);
    if (!w.definedBy) continue;
    definedBy.set(w.definedBy, (definedBy.get(w.definedBy) ?? new Set()).add(c.classId));
  }
  const independent = graded.filter((g) => g.lexiconNames.length === 0).map((g) => g.agent);
  const pairs = [];
  for (let i = 0; i < independent.length; i += 1) {
    for (let j = i + 1; j < independent.length; j += 1) {
      const [a, b] = [independent[i], independent[j]];
      const sa = definedBy.get(a) ?? new Set();
      const sb = definedBy.get(b) ?? new Set();
      const both = [...sa].filter((x) => sb.has(x)).length;
      const either = new Set([...sa, ...sb]).size;
      const [ma, mb] = [modelOf(models, a), modelOf(models, b)];
      pairs.push({ a, b, models: [ma, mb], same: ma !== null && ma === mb, jaccard: either ? +(both / either).toFixed(3) : 0 });
    }
  }
  const meanOf = (xs) => (xs.length ? +(xs.reduce((n, x) => n + x, 0) / xs.length).toFixed(3) : null);
  const baseline = {
    sameModel: meanOf(pairs.filter((p) => p.same).map((p) => p.jaccard)),
    crossModel: meanOf(pairs.filter((p) => !p.same && p.models.every(Boolean)).map((p) => p.jaccard)),
    pairs,
  };

  // Reuse of inherited Words, per generation that inherited any: the share of
  // correct solutions reaching at least one, and how many inherited Words each
  // subject's correct solutions reached. C-shuffled against C-bottleneck says
  // whether reuse needs the names and notes.
  const reuse = gens
    .filter((g) => g.lexicon.entries.length > 0)
    .map((g) => {
      const inheritedNames = new Set(g.lexicon.entries.map((e) => norm(e.name)));
      const correct = g.graded.flatMap((x) => x.results.filter((r) => r.correct));
      const reaching = correct.filter((r) => r.uses.some((n) => inheritedNames.has(n))).length;
      const perAgent = g.graded.map((x) => ({
        agent: x.agent,
        inheritedUsed: new Set(x.results.filter((r) => r.correct).flatMap((r) => r.uses.filter((n) => inheritedNames.has(n)))).size,
      }));
      return { condition: g.condition, generation: g.generation, solutionsReaching: `${reaching}/${correct.length}`, perAgent };
    });

  // Correct solutions whose text is a task's reference solution, per subject:
  // the reference is in tasks/*.json, which subjects are told not to read. A
  // few short canonical answers match by themselves; a high count is a flag.
  const referenceOf = new Map(families.flatMap((f) => f.tasks.map((t) => [t.id, tokenize(t.reference).join(' ')])));
  const referenceMatches = graded.map((g) => ({
    agent: g.agent,
    matches: g.results.filter((r) => r.correct && tokenize(r.solution).join(' ') === referenceOf.get(r.task)).map((r) => r.task),
  }));

  // The cli route's call log, per subject.
  const calls = graded.filter((g) => g.calls).map((g) => ({ agent: g.agent, ...g.calls }));

  const coreFailures = graded.flatMap((g) =>
    g.results.filter((r) => r.correct && !r.coreEquivalent).map((r) => ({ agent: g.agent, task: r.task })),
  );

  for (const c of convergent) {
    c.models = [...new Set(c.definers.map((a) => modelOf(models, a)).filter(Boolean))];
  }

  return {
    perGeneration,
    perModel,
    baseline,
    reuse,
    calls,
    referenceMatches,
    convergent,
    classCount: byClass.size,
    wordCount: words.size,
    compositional,
    coreUse: [...coreUse.entries()].sort((a, b) => b[1] - a[1]),
    coreFailures,
  };
}

export function renderReport(report) {
  const out = [];
  out.push('# Lexicon-emergence run report', '');
  out.push('## Per generation', '');
  out.push('| condition | gen | agents | lexicon | own Words / subject | correct | mean tokens | tokens ÷ expanded | Core-equivalent (H5) |');
  out.push('| --- | --- | --- | --- | --- | --- | --- | --- | --- |');
  for (const g of report.perGeneration) {
    out.push(`| ${g.condition} | ${g.generation} | ${g.agents} | ${g.lexiconSize} | ${g.ownWords ?? '—'} | ${g.accuracy} | ${g.meanTokens} | ${g.compression} | ${g.coreEquivalent} |`);
  }
  if (report.perGeneration.some((g) => g.split)) {
    out.push('', '## Training and held-out tasks', '');
    out.push('Held-out results never rank a Word for the next lexicon. "reaching" is correct solutions that call an inherited Word; "tasks" is how many distinct tasks those are.', '');
    out.push('| condition | gen | train correct | train ÷ expanded | train reaching | held-out correct | held-out ÷ expanded | held-out reaching | held-out tasks |');
    out.push('| --- | --- | --- | --- | --- | --- | --- | --- | --- |');
    for (const g of report.perGeneration.filter((x) => x.split)) {
      const { train: t, heldOut: h } = g.split;
      out.push(`| ${g.condition} | ${g.generation} | ${t.correct} | ${t.compression} | ${t.reaching ?? '—'} | ${h.correct} | ${h.compression} | ${h.reaching ?? '—'} | ${h.tasksReaching ?? '—'} |`);
    }
  }
  if (report.perGeneration.some((g) => g.heldOutFamilies)) {
    out.push('', '## Held-out families: shared with training, and inherited', '');
    out.push('Of each family\'s correct held-out solutions: "shared" call a Word their own subject also reached from a correct training solution, "inherited" call an inherited Word. Where subjects may define Words, shared all but bounds inherited from above.', '');
    const familiesSeen = [...new Set(report.perGeneration.flatMap((g) => (g.heldOutFamilies ?? []).map((f) => f.family)))];
    out.push(`| condition | gen | ${familiesSeen.map((f) => `${f} correct · shared · inherited`).join(' | ')} |`);
    out.push(`| --- | --- | ${familiesSeen.map(() => '---').join(' | ')} |`);
    for (const g of report.perGeneration.filter((x) => x.heldOutFamilies)) {
      const cell = (name) => {
        const f = g.heldOutFamilies.find((x) => x.family === name);
        return f ? `${f.correct}/${f.attempted} · ${f.shared} · ${g.lexiconSize ? f.inherited : '—'}` : '—';
      };
      out.push(`| ${g.condition} | ${g.generation} | ${familiesSeen.map(cell).join(' | ')} |`);
    }
  }
  if (report.perGeneration.some((g) => g.byModel && Object.keys(g.byModel).length)) {
    out.push('', '## Correct solutions by generation and model', '');
    const modelsSeen = [...new Set(report.perGeneration.flatMap((g) => Object.keys(g.byModel ?? {})))];
    out.push(`| condition | gen | ${modelsSeen.join(' | ')} | mean runs |`, `| --- | --- | ${modelsSeen.map(() => '---').join(' | ')} | --- |`);
    for (const g of report.perGeneration) {
      out.push(`| ${g.condition} | ${g.generation} | ${modelsSeen.map((m) => g.byModel?.[m] ?? '—').join(' | ')} | ${g.meanRuns ?? '—'} |`);
    }
  }
  if (report.referenceMatches?.some((r) => r.matches.length)) {
    out.push('', '## Correct solutions identical to the reference', '');
    for (const r of report.referenceMatches.filter((x) => x.matches.length)) out.push(`- ${r.agent}: ${r.matches.length} (${r.matches.join(', ')})`);
  }
  if (report.perModel?.length) {
    out.push('', '## Accuracy per model', '');
    out.push('| model | agents | correct |', '| --- | --- | --- |');
    for (const m of report.perModel) out.push(`| ${m.model} | ${m.agents} | ${m.accuracy} |`);
  }
  const crossCount = report.convergent.filter((c) => (c.models?.length ?? 0) >= 2).length;
  out.push('', `## H1 — classes defined independently by two or more agents (${report.convergent.length} of ${report.classCount} classes, ${report.wordCount} Words; ${crossCount} reached from two or more models)`, '');
  if (report.baseline?.pairs.length) {
    out.push(`Baseline over subjects that inherited nothing — mean Jaccard of defined classes: same-model pairs ${report.baseline.sameModel}, cross-model pairs ${report.baseline.crossModel}.`, '');
  }
  for (const c of report.convergent) {
    out.push(`- **${c.definers.length} agents** (${c.definers.join(', ')}${c.models?.length ? `; models ${c.models.join(', ')}` : ''}), joined at ${c.links.join('/') || 'D0'}:`);
    for (const m of c.members) out.push(`  - \`${m.name}\` = \`${m.body}\`${m.note ? ` — ${m.note}` : ''}`);
  }
  if (report.reuse?.length) {
    out.push('', '## Reuse of inherited Words', '');
    out.push('| condition | gen | correct solutions reaching one | inherited Words reached, per subject |', '| --- | --- | --- | --- |');
    for (const r of report.reuse) {
      out.push(`| ${r.condition} | ${r.generation} | ${r.solutionsReaching} | ${r.perAgent.map((a) => `${a.agent} ${a.inheritedUsed}`).join(', ')} |`);
    }
  }
  if (report.calls?.length) {
    out.push('', '## What subjects ran (cli route)', '');
    out.push('| subject | compute calls | naming inherited Words | prelude not the lexicon |', '| --- | --- | --- | --- |');
    for (const c of report.calls) out.push(`| ${c.agent} | ${c.total} | ${c.withWords} | ${c.preludeMismatches} |`);
  }
  out.push('', '## H2 — lexicon entries built from other entries', '');
  for (const c of report.compositional) out.push(`- ${c.condition} gen${c.generation}: ${c.compositional}`);
  out.push('', '## H4 — Core Words by number of correct solutions reaching them', '');
  const used = report.coreUse.filter(([, n]) => n > 0);
  const unused = report.coreUse.filter(([, n]) => n === 0).map(([w]) => w);
  out.push(used.map(([w, n]) => `\`${w}\` ${n}`).join(' · '));
  out.push('', `Unused (${unused.length}): ${unused.map((w) => `\`${w}\``).join(' ')}`);
  out.push('', '## H5 — correct solutions whose Core-only expansion disagreed', '');
  out.push(report.coreFailures.length ? report.coreFailures.map((f) => `- ${f.agent} ${f.task}`).join('\n') : 'None.');
  return `${out.join('\n')}\n`;
}
