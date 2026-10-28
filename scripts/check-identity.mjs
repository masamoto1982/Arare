#!/usr/bin/env node
// Gate for spec/identity.json, the canonical identity law.
//
// The law's whole content is that a procedure which cannot decide identity says
// so, rather than answering `different`. The way that goes wrong on paper is a
// level quietly claiming to decide more than it does, so this gate checks the
// shape of each level's claim against itself:
//
//   - a level that decides `different` must be able to reach it, and a level
//     that cannot must not list it;
//   - a level that is not total must name how `unknown` arrives — a reason id
//     spec/outcomes.json declares, or an explicit soundness/incompleteness pair
//     saying which direction it can answer in;
//   - a level that claims totality must not also claim incompleteness.
//
// The behavioural half — that the invariances hold, that the incompleteness
// witness really does denote one function under two identities, and that
// definitions which observably disagree never share an identity — is in
// rust/src/identity_laws.rs, which runs these same witnesses against the engine.
//
// Usage:
//   node scripts/check-identity.mjs

import { readJson, readText, reporter } from './lib/common.mjs';

const report = reporter('identity', { countSummary: true });
const fail = report.fail;

const identity = readJson('spec/identity.json');
const outcomes = readJson('spec/outcomes.json');
const semantics = readText('spec/language-semantics.md');

const lawOutcomes = new Set(identity.law.outcomes.map((o) => o.id));
for (const required of ['same', 'different', 'unknown']) {
  if (!lawOutcomes.has(required)) {
    fail(`the law must name the outcome "${required}" — it is the trichotomy applied to identity`);
  }
}

for (const clause of identity.clauses) {
  if (!semantics.includes(clause)) {
    fail(`clauses names "${clause}", which spec/language-semantics.md does not define`);
  }
}

const nilReasons = new Set(outcomes.nilReasons.map((r) => r.id));
const levels = identity.levels;
if (!Array.isArray(levels) || levels.length === 0) fail('the law declares no levels');

for (const level of levels) {
  const id = level.id ?? '<unnamed>';
  const decides = new Set(level.decides ?? []);

  for (const outcome of decides) {
    if (!lawOutcomes.has(outcome)) {
      fail(`level "${id}" says it decides "${outcome}", which the law does not define`);
    }
  }
  if (!decides.has('same')) {
    fail(`level "${id}" decides no "same" — a procedure that can never answer yes decides nothing`);
  }

  if (level.total === true) {
    if (decides.has('unknown')) {
      fail(`level "${id}" claims totality but lists "unknown" among what it decides`);
    }
    if ('incompleteness' in level) {
      fail(`level "${id}" claims totality but also declares an incompleteness`);
    }
    if (!decides.has('different')) {
      fail(`level "${id}" claims totality but cannot answer "different"`);
    }
  } else {
    // A partial level must say how it falls short, one of the two honest ways:
    // it reaches `unknown` with a registered reason, or it is one-sided and
    // says which side it can answer.
    const reachesUnknown = decides.has('unknown');
    const oneSided = 'soundness' in level && 'incompleteness' in level;
    if (!reachesUnknown && !oneSided) {
      fail(
        `level "${id}" is not total but explains neither how "unknown" arrives nor which ` +
          'direction it can answer — a partial procedure has to say how it is partial',
      );
    }
    // `unknown` reaches a reader two ways, and which one it is decides whether
    // a registry reason applies. A comparison the language performs answers
    // with a NIL, so its reason must be a declared one; a rule for what a
    // reader of identities — a host, or a program comparing `DIGEST` texts —
    // may conclude is not a value at all, and demanding a NIL reason for it
    // would be asking the wrong question.
    if (reachesUnknown) {
      const arrival = level.unknownArrivesAs;
      if (!arrival || typeof arrival.kind !== 'string') {
        fail(`level "${id}" reaches "unknown" but does not say how it arrives`);
      } else if (arrival.kind === 'nilReason') {
        if (!nilReasons.has(arrival.reason)) {
          fail(
            `level "${id}" reaches "unknown" as NIL reason "${arrival.reason}", which ` +
              'spec/outcomes.json does not declare',
          );
        }
      } else if (arrival.kind === 'readingRule') {
        if (typeof arrival.note !== 'string' || arrival.note.trim() === '') {
          fail(
            `level "${id}" reaches "unknown" as a reading rule but does not say what a reader ` +
              'may conclude from it, which is the whole content of that claim',
          );
        }
      } else {
        fail(`level "${id}" names unknown arrival kind "${arrival.kind}", which is not one this gate knows`);
      }
    }
    if (oneSided && decides.has('different')) {
      fail(
        `level "${id}" declares an incompleteness — unequal does not mean different — yet lists ` +
          '"different" among what it decides',
      );
    }
    if (oneSided && !('whyNotDecidable' in level)) {
      fail(`level "${id}" is one-sided but does not say why the other direction is out of reach`);
    }
  }

  // Witness shape. A level that decides something must show it deciding.
  const pairGroups = ['invariantUnder', 'distinguishes'];
  for (const group of pairGroups) {
    for (const [i, entry] of (level[group] ?? []).entries()) {
      for (const key of ['id', 'left', 'leftWord', 'right', 'rightWord']) {
        if (typeof entry[key] !== 'string' || entry[key].trim() === '') {
          fail(`level "${id}" ${group}[${i}] is missing a non-empty ${key}`);
        }
      }
    }
  }
  if (decides.has('same') && 'invariantUnder' in level && level.invariantUnder.length === 0) {
    fail(`level "${id}" declares an empty invariantUnder`);
  }
  if ('incompleteness' in level && !('incompletenessWitness' in level)) {
    fail(
      `level "${id}" admits an incompleteness but exhibits no witness for it — an admission ` +
        'nothing demonstrates is indistinguishable from an excuse',
    );
  }
  if ('incompletenessWitness' in level) {
    const w = level.incompletenessWitness;
    for (const key of ['left', 'leftWord', 'right', 'rightWord']) {
      if (typeof w[key] !== 'string' || w[key].trim() === '') {
        fail(`level "${id}" incompletenessWitness is missing a non-empty ${key}`);
      }
    }
    if (!Array.isArray(w.agreeOn) || w.agreeOn.length === 0) {
      fail(
        `level "${id}" incompletenessWitness names no inputs to agree on, so nothing shows the ` +
          'two denote one thing',
      );
    }
  }
  for (const [i, w] of (level.witnesses ?? []).entries()) {
    for (const key of ['source', 'expect']) {
      if (typeof w[key] !== 'string' || w[key].trim() === '') {
        fail(`level "${id}" witnesses[${i}] is missing a non-empty ${key}`);
      }
    }
    if (typeof w.expect === 'string' && w.expect.startsWith('nil:')) {
      const reason = w.expect.slice(4);
      if (!nilReasons.has(reason)) {
        fail(`level "${id}" witnesses[${i}] expects nil:${reason}, which the registry does not declare`);
      }
    }
  }
}

// At least one level must be one-sided, or the law's `unknown` outcome is
// decoration: the whole reason it exists is that content identity cannot answer
// `different`.
if (!levels.some((l) => 'soundness' in l && 'incompleteness' in l)) {
  fail(
    'no level declares a one-sided procedure, so nothing in this file needs the "unknown" ' +
      'outcome the law introduces',
  );
}

const totals = levels.filter((l) => l.total === true).length;
report.done(
  `${levels.length} levels (${totals} total, ${levels.length - totals} partial), ` +
    `${lawOutcomes.size} outcomes, every partial level says how it falls short.`,
);
