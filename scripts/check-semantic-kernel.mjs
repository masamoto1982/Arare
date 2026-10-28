import { families, readText, reporter, words as wordsDocument } from './lib/common.mjs';

const language = readText('spec/language-semantics.md');
const words = wordsDocument();

const report = reporter('semantic-kernel');
const fail = report.fail;

// The kernel is a ceiling, not a floor: a shorter specification is always an
// improvement, a longer one is the regression this gate exists to catch.
//
// The budget is normally slack, and then one day it is not. Adding
// LANG.SOURCE.FRAME took the file to the ceiling exactly, so the next clause
// added here fails this check on arrival. That is the gate working: the answer
// is to shorten or merge an existing clause, which is the question "does the
// specification need this, or does it already say it elsewhere?" asked at the
// only moment anyone will actually ask it.
//
// Two answers that look like fixes are not. Raising the number right after
// hitting it retires the brake; and reflowing prose to put more words on fewer
// lines keeps the count down while the specification grows, which is the thing
// being measured. Raising it deliberately, when the language genuinely has more
// to say, is a different act and a fine one — it just wants to be a decision
// rather than a reflex.
//
// `headroom` below is reported on every green run, so how close the file is
// sitting is visible without reading this comment.
//
// Raised to 404 for LANG.DICTIONARY.ACYCLIC: the User dictionary's reference
// graph became acyclic by rule (no definition may name itself, directly or
// through a chain of other User words), retiring the guarded-tail-recursion
// feature and its own clause text in LANG.SOURCE.FRAME. Net new content, not
// reflowed old content — the language now has something it did not have
// before (every evaluation is structurally finite, not merely
// resource-bounded) — so this is the deliberate raise the comment above asks
// for, not a reflex.
//
// Raised to 408 for LANG.RECORDS.STRUCTURE: the vocabulary-100 work order
// (retired; its adoption rules are docs/dev/ajisai-minimal-core-identity.md 付録
// B) admitted a seventh value domain, the keyed correspondence, and a domain
// needs its own clause — its structure, its identity, its containment rule —
// where the earlier phases of that work order fit into existing clauses. Four
// lines: the heading, the paragraph, and their separators. Net new content
// again.
//
// Raised to 412 for LANG.CONTRACT.FIELD: total division moved the cost of a
// zero divisor from a run-time failure to the three points over zero, where
// the field laws stop, and the contract gained the one axis that says before
// anything runs where that can happen (`field`: `closed` < `leaving`). It is a
// new fact about every Word and every body, not a reflow of an old clause, so
// it takes the same four lines a new clause took above.
//
// Raised to 414 for LANG.VALUES.SHAPE: reading a number as whether its two
// halves are nonzero turned out to grade the whole arithmetic by FOUR — the
// product is the knowledge meet, the reciprocal is NOT, the numbers are a
// Clifford monoid over the four shapes — and it fixes which laws survive the
// three points: the additive wheel laws everywhere, the reciprocal's
// involution everywhere but -1/0, with the trilemma that forbids a signed zero.
// The correspondence paragraph moved out of LANG.VALUES.TRUTH into the new
// clause, so the net cost is the heading, its separators and two paragraphs of
// new fact, not a reflow.
// Raised to 418 for LANG.VALUES.ORDER: the SQL review (2026-10-13) found the
// order and the grouping key were each defined nowhere but in the Words that
// used them — Strings had no order, a NIL element stopped SORT, and UNIQUE /
// TALLY / GROUP split NULLs by reason where SQL's DISTINCT and GROUP BY make
// one group. One clause now fixes all three: the order of numbers and of
// Strings, where a NIL sorts, and the key a grouping Word bundles by. Net new
// content, at the four lines a clause costs.
const LINE_BUDGET = 418;
const lines = language.split('\n').length;
if (lines > LINE_BUDGET) {
  fail(
    `language-semantics.md has ${lines} lines (maximum ${LINE_BUDGET}). ` +
      'Shorten or merge a clause rather than raising the budget; see the note above this check.',
  );
}

const clauseIds = new Set([...language.matchAll(/id="[^"]+">(LANG\.[A-Z.]+)/g)].map((match) => match[1]));
if (clauseIds.size === 0) fail('no language clause IDs found');

// Families are derived from the Words (scripts/lib/common.mjs `families()`):
// one per `family` a Word names, sharing the clauses all its members cite. So
// a duplicate family, a Word naming a missing family and a family with no
// Words cannot occur, and a family's clauses are its Words', checked below —
// each was a check of its own here while the families were a hand-kept file.
// What stays is the ceiling on how many there are.
const familyIds = new Set(families().map((family) => family.id));
if (familyIds.size > 12) fail(`${familyIds.size} semantic families (maximum 12)`);

for (const word of words.entries) {
  for (const clause of word.clauses) {
    if (!clauseIds.has(clause)) fail(`Word ${word.name} references missing clause ${clause}`);
  }
}

// Vocabulary growth is the failure mode this project is guarding against, so the
// count is a budget rather than a fixed inventory: shrinking is free, growing is
// a deliberate specification change.
// Raised from 70 to 100 for the vocabulary-100 work order
// (docs/dev/ajisai-minimal-core-identity.md 付録 B): the owner's decision to
// remake the vocabulary as ten concepts and 100 Words, each admitted on one of
// two grounds the work order states (inexpressible in a total, non-recursive
// language, or the closure of a small symmetric family). The number is the
// work order's ceiling, not a target — its §1 forbids padding to reach it — so
// this is the deliberate raise the comment above asks for.
if (words.entries.length > 100) fail(`${words.entries.length} canonical Words (maximum 100)`);

const headroom = LINE_BUDGET - lines;
const budget = headroom === 0 ? 'at the line budget' : `${headroom} lines under budget`;
report.done(`${lines} lines (${budget}), ${clauseIds.size} clauses, ${familyIds.size} families, ${words.entries.length} Words.`);
