import { readJson, readText, reporter, words as wordsDocument } from './lib/common.mjs';

const words = wordsDocument();
const schema = readJson('spec/words.schema.json');
const manifest = readJson('docs/word-manifest.json');
const dispatchSource = readText('rust/src/interpreter/execute_builtin.rs');
const language = readText('spec/language-semantics.md');
const outcomes = readJson('spec/outcomes.json');

const report = reporter('word-schema');
const fail = report.fail;
const required = schema.$defs.word.required;
// The closed list of families is the schema's enum (the registry generator
// projects it into the Rust `Family`); what a family *means* — the laws its
// Words share — is derived from the Words themselves (scripts/lib/common.mjs
// `families()`), so it cannot disagree with them.
const familyIds = new Set(schema.$defs.word.properties.family.enum);
const manifestNames = new Set(manifest.entries.map((entry) => entry.canonical));
const names = new Set();

// `nilPolicy` is a summary of the operand roles (LANG.FAILURE.PASSTHROUGH),
// never a choice of its own: two Words that treat their operands alike treat
// a NIL alike. The summary names the strongest role present, in the order a
// NIL meets them at dispatch.
function derivedNilPolicy(roles, projecting) {
  if (roles.length === 0) return 'preserveReason';
  if (roles.includes('truth')) return 'kleeneAbsorbing';
  if (roles.includes('data') || roles.includes('leaf')) return projecting ? 'passthroughThenProject' : 'passthrough';
  if (!roles.includes('element')) return projecting ? 'createsNil' : 'rejectNil';
  return 'consumeNil';
}

// `partiality` summarizes the outcomes the Word declares, never a choice of
// its own: a Word that can project is `projecting`; one that can raise even on
// operands of the right kind — it runs a block, or declares a condition
// repaired in the program (spec/outcomes.json `repair`) — is `partial`; any
// other Word raises only on an operand of the wrong kind, and is `total`.
const programConditions = new Set(
  outcomes.errorCategories.filter((category) => category.repair === 'program').map((category) => category.id),
);
function derivedPartiality(word) {
  if (word.projection.when !== 'never') return 'projecting';
  if (word.purity === 'conditional' || (word.errorWhen ?? []).some((c) => programConditions.has(c))) return 'partial';
  return 'total';
}

for (const word of words.entries) {
  if (word.partiality !== derivedPartiality(word)) {
    fail(`${word.name} declares partiality ${word.partiality}, but its outcomes derive ${derivedPartiality(word)}`);
  }
  names.add(word.name);
  for (const field of required) if (!(field in word)) fail(`${word.name} lacks required field ${field}`);
  if (!familyIds.has(word.family)) fail(`${word.name} references unknown family ${word.family}`);
  if (word.vocabularyTier === 'standard' && !['shorthand', 'namedPattern', 'algorithm', 'operational'].includes(word.standardKind)) {
    fail(`${word.name} lacks a valid standardKind`);
  }
  if (word.vocabularyTier === 'kernel' && 'standardKind' in word) fail(`${word.name} is Kernel but declares standardKind`);
  if (!manifestNames.has(word.name)) fail(`${word.name} is absent from the frozen manifest`);
  // A Word that tests its operands and answers a truth value is named with a
  // trailing `?` (NIL?, HAS?), and a `?` name always answers one.
  // Relations and connectives (EQ LT GT, AND NOT SELECT) are operators, named
  // for the operation, and exempt: the truth-order family.
  const answersTruth = /-> \[ (TRUE \| FALSE|truths?) \]/.test(word.documentation.stackEffect);
  const operator = word.family === 'truthOrder';
  if (!operator && answersTruth !== word.name.endsWith('?')) {
    fail(
      answersTruth
        ? `${word.name} answers a truth value, so its name ends in ?`
        : `${word.name} ends in ? but does not answer a truth value`,
    );
  }
  // One notation for every stack effect: every operand and result in its own
  // `[ … ]` group with inner spaces, named rather than quoted, in one
  // vocabulary — `text` for a String, `TRUE | FALSE` for a truth value, `...`
  // for "and more", alternatives spaced around `|`.
  const effect = word.documentation.stackEffect;
  const [inputs = ''] = effect.split('->');
  if (
    /\[\]|'[A-Za-z.]+'/.test(effect) ||
    /\b(str|bool)\b/.test(effect) ||
    /(?<!\.)\.\.(?!\.)/.test(effect) ||
    /\S\||\|\S/.test(effect) ||
    // A projection is declared in `projection`; the stack effect names the
    // value answered, so `| NIL` alternatives are not written there.
    /\| NIL\b/.test(effect) ||
    !/^(\[ .* \] )?$/.test(inputs)
  ) {
    fail(`${word.name} stack effect \`${effect}\` departs from the one notation (bracketed, unquoted, text, TRUE | FALSE, ...)`);
  }
  // Two lifted operands are aligned lane by lane (LANG.COLLECTIONS.LIFT), and
  // two containers that do not align are `shapeMismatch`; a Word with fewer
  // has nothing to align and does not declare it for lifting's sake.
  const liftedCount = (word.stack.operands ?? []).filter((role) => role === 'leaf' || role === 'truth').length;
  if (liftedCount >= 2 && !(word.errorWhen ?? []).includes('shapeMismatch')) {
    fail(`${word.name} lifts ${liftedCount} operands but does not declare shapeMismatch`);
  }
  // A lifted operand is only meaningful for a Word with one result: the
  // lift assembles one result per element.
  if ((word.stack.operands ?? []).some((role) => role === 'leaf' || role === 'truth') && word.stack.outputs !== 1) {
    fail(`${word.name} lifts over an operand but does not answer exactly one value`);
  }
  const operands = word.stack.operands;
  if (typeof word.stack.inputs === 'number') {
    if (!Array.isArray(operands) || operands.length !== word.stack.inputs) {
      fail(`${word.name} declares ${word.stack.inputs} input(s) but ${operands?.length ?? 'no'} operand role(s)`);
    } else if (word.nilPolicy !== derivedNilPolicy(operands, word.projection.when !== 'never')) {
      fail(
        `${word.name} declares nilPolicy ${word.nilPolicy}, but its operand roles [${operands.join(', ')}] ` +
          `derive ${derivedNilPolicy(operands, word.projection.when !== 'never')}`,
      );
    }
  } else if (operands !== undefined) {
    fail(`${word.name} has data-dependent arity and must not declare operand roles`);
  }
  // Value domains (LANG.VALUES.DISJOINT) sit beside the roles and must agree
  // with them: a truth operand is read as a Boolean, a lifted lane is never a
  // container or an absence, and a control operand is a block, a name or a
  // message. Which values the Words actually answer is checked by running
  // them (`rust/src/interpreter/word_domains_tests.rs`).
  const domains = word.stack.domains;
  const fixedArity = typeof word.stack.inputs === 'number' && typeof word.stack.outputs === 'number';
  if (fixedArity !== (domains !== undefined)) {
    fail(`${word.name} ${fixedArity ? 'must' : 'must not'} declare stack.domains: they are declared exactly when inputs and outputs are fixed`);
  } else if (domains !== undefined) {
    if (domains.operands.length !== word.stack.inputs || domains.results.length !== word.stack.outputs) {
      fail(`${word.name} declares ${domains.operands.length} operand and ${domains.results.length} result domain(s) for ${word.stack.inputs} input(s) and ${word.stack.outputs} output(s)`);
    }
    const allowed = { truth: ['boolean'], leaf: ['scalar', 'boolean', 'string', 'symbol'], control: ['vector', 'string', 'symbol'] };
    (operands ?? []).forEach((role, i) => {
      const declared = [domains.operands[i]].flat();
      const permitted = allowed[role];
      if (permitted && declared.some((domain) => !permitted.includes(domain))) {
        fail(`${word.name} operand ${i + 1} is a ${role} operand but declares domain ${declared.join(' | ')}`);
      }
    });
  }
  for (const clause of word.clauses) if (!language.includes(`${clause} —`)) fail(`${word.name} references missing clause ${clause}`);
  // Two clauses restate a fact the declaration already records, so the
  // citation is derived from it rather than chosen: a Word is subject to the
  // lifting law exactly when it has a lifted operand, and to the projection
  // law exactly when it declares a projection.
  const derivedClauses = [
    ['LANG.COLLECTIONS.LIFT', (word.stack.operands ?? []).some((role) => role === 'leaf' || role === 'truth')],
    ['LANG.FAILURE.PROJECT', word.projection.when !== 'never'],
  ];
  for (const [clause, applies] of derivedClauses) {
    if (applies !== word.clauses.includes(clause)) {
      fail(`${word.name} ${applies ? 'must' : 'must not'} cite ${clause}: its declaration ${applies ? 'has' : 'has no'} the property the clause governs`);
    }
  }

  // The executor key used to be written a second time on the Rust spec entry,
  // and this check reconciled the two copies. It is written once now — the
  // generated `WordId` *is* the executor key — so what is checked instead is
  // that the key reaches a runtime arm. The compiler already requires the
  // dispatch match to be total over `WordId`; this catches the case a total
  // match cannot, a Word folded into a neighbour's arm by mistake.
  if (!dispatchSource.includes(`WordId::${word.executorKey} =>`)) {
    fail(`${word.name} has no dispatch arm for WordId::${word.executorKey}`);
  }
}

const canonicalManifestEntries = manifest.entries.filter((entry) => entry.kind === 'coreword');
const expected = new Set(canonicalManifestEntries.map((entry) => entry.canonical));
for (const name of expected) if (!names.has(name)) fail(`contract scope omits ${name}`);
for (const name of names) if (!expected.has(name)) fail(`contract scope contains unexpected Word ${name}`);
if (names.size !== expected.size) fail(`contract scope has ${names.size} entries; expected ${expected.size}`);

report.done(`all ${canonicalManifestEntries.length} Core contracts cover the ${manifest.entries.length}-surface manifest and current executors.`);
