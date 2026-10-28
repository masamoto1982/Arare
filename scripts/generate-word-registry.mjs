import { spawnSync } from 'node:child_process';
import { readJson, words, writeOrCheck } from './lib/common.mjs';

// Project spec/words.json into a checked-in Rust Word registry — the single
// source of truth for the Word inventory, executor keys, static contract
// fields, and the documentation prose hover and LOOKUP read (migration plan
// Phase 3). Run with `--check` in CI to fail on
// drift; run without it to regenerate.

const outputPath = 'rust/src/kernel/generated/word_registry.rs';
const entries = words().entries;
const schema = readJson('spec/words.schema.json');
const specificationVersion = readJson('spec/version-lock.json').specificationVersion;

// The contract vocabularies are generated from the schema's own `enum` lists
// rather than hand-written on the Rust side. That is what makes a narrower
// implementation vocabulary impossible: a value the specification admits is a
// variant here by construction, so it can never be silently collapsed into a
// neighbouring one (audit finding — the hand-written `NilPolicy` carried 5 of
// the specification's 7 values, so `passthroughThenProject` and `inspectNil`
// were inexpressible and 11 Words were mislabelled as a result).
const schemaEnum = (field) => {
  const found = [];
  const walk = (node) => {
    if (Array.isArray(node)) {
      node.forEach(walk);
    } else if (node && typeof node === 'object') {
      for (const [key, value] of Object.entries(node)) {
        if (key === field && value && typeof value === 'object' && Array.isArray(value.enum)) {
          found.push(value.enum);
        } else {
          walk(value);
        }
      }
    }
  };
  walk(schema);
  if (found.length !== 1) {
    throw new Error(`spec/words.schema.json must declare exactly one enum for "${field}" (found ${found.length})`);
  }
  return found[0];
};

// `starts-with?` / `passthroughThenProject` -> `StartsWith` / `PassthroughThenProject`.
const pascal = (value) =>
  value
    .replace(/[^A-Za-z0-9]+/g, ' ')
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .split(' ')
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1).toLowerCase())
    .join('');

// `ordered` derives `PartialOrd`/`Ord` for an enum whose spec declaration order
// *is* a lattice order, so the runtime can compare variants directly instead of
// mapping them through a hand-written rank. `parsed` additionally emits
// `from_spec_str`, for the vocabularies a caller writes by hand (a `#:contract`
// declaration) and the runtime therefore has to read back.
const CONTRACT_ENUMS = [
  { rustName: 'Family', field: 'family', doc: 'Semantic family the Word selects its shared laws from.' },
  { rustName: 'NilPolicy', field: 'nilPolicy', doc: 'How the Word behaves when an operand is NIL (LANG.FAILURE.PASSTHROUGH).' },
  { rustName: 'Partiality', field: 'partiality', doc: 'Whether well-formed application is total, partial, or NIL-projecting.' },
  {
    rustName: 'FieldClosure',
    field: 'field',
    doc: 'Whether the Word keeps numbers inside the field or can answer a point over zero (LANG.CONTRACT.FIELD).',
    ordered: true,
    parsed: true,
    note:
      'The variants are declared in the schema\'s own order, and that order **is** the\n'
      + '/// order a contract widens along (`closed` < `leaving`): one `leaving` Word in a\n'
      + '/// body makes the body `leaving`, which is what the derived `Ord` means here.',
  },
  { rustName: 'Purity', field: 'purity', doc: 'Observational purity class (LANG.CONTRACT.REGISTRY).' },
  { rustName: 'Determinism', field: 'determinism', doc: 'What the Word\'s result may depend on beyond its operands.' },
  { rustName: 'VocabularyTier', field: 'vocabularyTier', doc: 'Where the Word sits in the public Core: the Semantic Kernel or the Standard vocabulary.' },
  {
    rustName: 'CostClass',
    field: 'class',
    doc: 'Growth class of a Word\'s charge on one metered resource, as a function of its input.',
  },
];

const enumBlocks = CONTRACT_ENUMS.map(({ rustName, field, doc, ordered, parsed, note }) => {
  const values = schemaEnum(field);
  const variants = values.map((value) => `    /// \`${value}\`\n    ${pascal(value)},`).join('\n');
  const arms = values
    .map((value) => `            ${rustName}::${pascal(value)} => ${JSON.stringify(value)},`)
    .join('\n');
  const parseArms = values
    .map((value) => `            ${JSON.stringify(value)} => Some(${rustName}::${pascal(value)}),`)
    .join('\n');
  const derives = `Clone, Copy, Debug, PartialEq, Eq, Hash${ordered ? ', PartialOrd, Ord' : ''}`;
  const fromSpecStr = parsed
    ? `

    /// The variant a canonical spec string names, or \`None\` when the string is
    /// not one the specification admits.
    pub fn from_spec_str(value: &str) -> Option<${rustName}> {
        match value {
${parseArms}
            _ => None,
        }
    }`
    : '';
  return `/// ${doc}
///
/// Generated from the \`${field}\` enum in spec/words.schema.json: every value the
/// specification admits is a variant, so the implementation vocabulary cannot be
/// narrower than the canonical one.${note ? `\n///\n/// ${note}` : ''}
#[derive(${derives})]
pub enum ${rustName} {
${variants}
}

impl ${rustName} {
    /// The canonical spec string for this variant.
    pub const fn as_spec_str(self) -> &'static str {
        match self {
${arms}
        }
    }${fromSpecStr}
}`;
}).join('\n\n');

const enumRef = (rustName, value) => `${rustName}::${pascal(value)}`;

// Stack arity: a decimal count, or a data-dependent marker the specification
// names explicitly. Keeping the markers as variants (rather than collapsing
// both into one "dynamic" case, as the hand-written MassContract did) is what
// lets the static mass validator stay engaged for every Word whose arity the
// specification pins exactly.
const arity = (value) =>
  typeof value === 'number' ? `Arity::Fixed(${value})` : `Arity::${pascal(String(value))}`;

// The executor key is a unique, valid PascalCase Rust identifier for every
// Word, so it doubles as the canonical WordId variant — the runtime dispatches
// on `WordId` itself rather than on a parallel hand-written key enum, which is
// why the key is not also emitted as a string field: one fact, one
// representation. Guard the uniqueness the enum relies on rather than emit a
// file that would not compile.
const ids = entries.map((word) => word.executorKey);
const duplicates = ids.filter((id, index) => ids.indexOf(id) !== index);
if (duplicates.length > 0) {
  throw new Error(`words.json has duplicate executorKey values: ${[...new Set(duplicates)].join(', ')}`);
}

// The three cost axes are projected in a fixed order rather than by iterating
// the entry's own key order, so a reordered `cost` object in spec/words.json
// cannot silently permute which resource a bound is attached to.
const COST_AXES = ['steps', 'numeric', 'collection'];
// Emitted in the shape rustfmt produces, since the generated file is checked in
// and CI runs `cargo fmt --check` over it: a one-line struct literal here would
// regenerate clean and then fail formatting.
const cost = (value) => {
  const axes = COST_AXES.map((axis) => {
    const declared = value[axis];
    return `            ${axis}: CostAxis {
                class: ${enumRef('CostClass', declared.class)},
                exact: ${declared.exact},
            },`;
  });
  return `WordCost {\n${axes.join('\n')}\n        }`;
};

// spec/words.json values are ASCII, so a JSON string literal is also a valid
// Rust string literal.
const rustStr = (value) => JSON.stringify(value);
// Always inline; the final `rustfmt` pass below (not a hand-rolled width
// guess) decides whether a slice literal wraps. An earlier version of this
// function tried to predict rustfmt's own `array_width` heuristic by hand —
// which depends on more than raw character count (rustfmt's short-element
// special-casing among other things) — and got it wrong the moment an
// `errorWhen` slice crossed a length it hadn't been tested against, silently
// committing a file `cargo fmt --check` and `word-registry:check` disagreed
// on. Piping through the real tool is the only way this stays correct as
// slice contents change.
const rustStrSlice = (values) => {
  if (values.length === 0) return '&[]';
  return `&[${values.map(rustStr).join(', ')}]`;
};

// A projection condition of "never" is the absence of one, so it is projected
// as an empty slice rather than as a string every reader would have to compare
// against. A Word may declare several conditions (`POW` projects for a zero
// base under a negative exponent and for an exponent that leaves the field),
// so the slice is the shape even where only one is declared.
const projection = (when) => {
  if (when === 'never') return '&[]';
  const conditions = Array.isArray(when) ? when : [when];
  return rustStrSlice(conditions);
};

// The NIL reasons the projection conditions answer with, each named once (two
// conditions may share one); the ids `NIL-REASON` reports.
const projectionReasons = (reason) => {
  if (reason === null || reason === undefined) return '&[]';
  const reasons = Array.isArray(reason) ? reason : [reason];
  return rustStrSlice(reasons);
};

const operandRoles = (roles) =>
  `&[${(roles ?? []).map((role) => `OperandRole::${pascal(role)}`).join(', ')}]`;

// One position's domains: a single domain or a list, always a slice in Rust.
const domainSets = (sets) =>
  `&[${(sets ?? [])
    .map((set) => `&[${[set].flat().map((domain) => `ValueDomain::${pascal(domain)}`).join(', ')}]`)
    .join(', ')}]`;

const variants = entries.map((word) => `    ${word.executorKey},`).join('\n');

const rows = entries
  .map(
    (word) => `    GeneratedWord {
        id: WordId::${word.executorKey},
        name: ${rustStr(word.name)},
        family: ${enumRef('Family', word.family)},
        stack_inputs: ${arity(word.stack.inputs)},
        stack_outputs: ${arity(word.stack.outputs)},
        operand_roles: ${operandRoles(word.stack.operands)},
        operand_domains: ${domainSets(word.stack.domains?.operands)},
        result_domains: ${domainSets(word.stack.domains?.results)},
        nil_policy: ${enumRef('NilPolicy', word.nilPolicy)},
        projection: ${projection(word.projection.when)},
        projection_reasons: ${projectionReasons(word.projection.reason)},
        partiality: ${enumRef('Partiality', word.partiality)},
        field: ${enumRef('FieldClosure', word.field)},
        purity: ${enumRef('Purity', word.purity)},
        determinism: ${enumRef('Determinism', word.determinism)},
        cost: ${cost(word.cost)},
        vocabulary_tier: ${enumRef('VocabularyTier', word.vocabularyTier)},
        standard_kind: ${word.standardKind ? `Some(${rustStr(word.standardKind)})` : 'None'},
        effects: ${rustStrSlice(word.effects)},
        error_when: ${rustStrSlice(word.errorWhen)},
        syntax: ${word.documentation.syntax ? `Some(${rustStr(word.documentation.syntax)})` : 'None'},
        summary: ${rustStr(word.documentation.summary)},
        stack_effect: ${rustStr(word.documentation.stackEffect)},
        hover_summary: ${rustStr(word.documentation.hover)},
        hover_syntax: ${rustStr(word.documentation.syntax)},
    },`,
  )
  .join('\n');

const output = `// @generated by scripts/generate-word-registry.mjs; do not edit.
//
// Canonical Word registry projected from spec/words.json — the single source of
// truth for the Word inventory, executor keys, and the static contract
// fields. Regenerate with \`npm run word-registry:generate\`; CI runs
// \`npm run word-registry:check\` to guard against drift.

/// The specification version this registry was generated from
/// (\`spec/version-lock.json\`), which every agent envelope names.
pub const SPECIFICATION_VERSION: &str = "${specificationVersion}";

/// Canonical identity of a Core Word, named by its executor key (unique across
/// the registry).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WordId {
${variants}
}

${enumBlocks}

/// What a Word does with one operand (LANG.FAILURE.PASSTHROUGH).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OperandRole {
    /// Read as one Scalar, String or Boolean: an absent operand makes the
    /// result that absence, and a Vector or Record lifts the Word over its
    /// elements (LANG.COLLECTIONS.LIFT).
    Leaf,
    /// Read whole: an absent operand makes the result that absence.
    Data,
    /// Carried without being read: a NIL is an ordinary value here.
    Element,
    /// Directs the Word rather than being data it transforms — a block, a
    /// name, or a message: a NIL here is malformed use.
    Control,
    /// Read in truth position, where a NIL is UNKNOWN (LANG.VALUES.TRUTH).
    Truth,
}

/// A value domain (LANG.VALUES.DISJOINT), as a Word's \`stack.domains\`
/// names one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueDomain {
    /// A number: a rational or an algebraic Scalar.
    Scalar,
    Boolean,
    String,
    /// A Vector, however it is held (a dense Tensor is one too).
    Vector,
    Record,
    Nil,
    Symbol,
    /// Any value: the position takes every domain.
    Any,
}

/// Stack arity as declared in spec/words.json: an exact count, or one of the
/// data-dependent markers the specification names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arity {
    /// An exact operand/result count.
    Fixed(u8),
    /// \`variable\` — data-dependent, not statically pinned.
    Variable,
}

/// A Word's declared charge on one metered resource.
///
/// \`class\` is a **sound upper bound**: the charge never grows faster than this
/// in the Word's input. \`exact\` records that some contribution provably
/// *attains* the class — the witness that lets a \`#:contract\` mismatch be
/// reported as an error rather than a note, since without it a looser class
/// might simply be an over-approximation the caller has every right to beat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CostAxis {
    pub class: CostClass,
    pub exact: bool,
}

/// A Word's declared charge on all three metered resources, projected from
/// spec/words.json.
///
/// The three axes are the runtime's own counters (\`executionSteps\` /
/// \`numericWork\` / \`collectionWork\`), so a declared bound and a measured
/// \`resourceUsage\` are answers about the same quantity. They join pointwise
/// under concatenation, which is what lets a phrase's bound be computed from
/// its Words' bounds without running it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WordCost {
    pub steps: CostAxis,
    pub numeric: CostAxis,
    pub collection: CostAxis,
}

/// A Word's spec-declared contract, projected from spec/words.json with each
/// field typed by the vocabulary spec/words.schema.json declares for it.
#[derive(Debug)]
pub struct GeneratedWord {
    pub id: WordId,
    pub name: &'static str,
    pub family: Family,
    pub stack_inputs: Arity,
    pub stack_outputs: Arity,
    /// What the Word does with each operand, first-pushed first; empty for a
    /// data-dependent arity. The declared NIL contract reads this position by
    /// position (\`declared_nil_contract\`), and \`nil_policy\` is its
    /// summary.
    pub operand_roles: &'static [OperandRole],
    /// The value domains of each operand and of each result, one slice of
    /// domains per position; both empty when either arity is data-dependent.
    /// An operand's domains are those of one lane for a \`leaf\` or \`truth\`
    /// operand and of the whole value otherwise; a result's are those of the
    /// answer when every operand is in its domains and nothing is lifted.
    /// Read by \`fusion_contract\`, which runs a Word inside a fused block
    /// only when every operand and result stays within scalars and Booleans.
    pub operand_domains: &'static [&'static [ValueDomain]],
    pub result_domains: &'static [&'static [ValueDomain]],
    pub nil_policy: NilPolicy,
    /// The conditions under which a *well-formed* operand yields a reasoned
    /// NIL; empty for the Words that declare \`never\`. Distinct from
    /// \`nil_policy\`, which is about NIL *operands*: a Word can pass a NIL
    /// through and still project one of its own (\`passthroughThenProject\`),
    /// or project without any NIL-operand rule engaging at all.
    ///
    /// A slice rather than a single condition because a Word can project for
    /// more than one reason: \`POW\` answers NIL both for a zero base under a
    /// negative exponent and for an exponent that leaves the exact field.
    pub projection: &'static [&'static str],
    /// The NIL reasons the conditions in \`projection\` answer with, each
    /// named once (two conditions may share one): the ids \`NIL-REASON\` reports, and what
    /// \`CONTRACT\` hands a program that asks what a Word can project.
    pub projection_reasons: &'static [&'static str],
    pub partiality: Partiality,
    /// Whether the Word can answer a point over zero from operands that hold
    /// none (LANG.CONTRACT.FIELD). Read by \`interpreter::word_contract\`,
    /// which joins it along a body.
    pub field: FieldClosure,
    pub purity: Purity,
    pub determinism: Determinism,
    /// What the Word charges on each metered resource.
    pub cost: WordCost,
    /// Which half of the public Core the Word belongs to. Both halves are
    /// ordinary sealed-Core Words reached by their plain names; the tier is a
    /// design classification the reading surfaces report, never a namespace.
    pub vocabulary_tier: VocabularyTier,
    /// Why a Standard Word is native rather than left to a user definition, or
    /// \`None\` for a Semantic Kernel Word.
    pub standard_kind: Option<&'static str>,
    /// The effects the Word declares, in the specification's own spelling.
    /// Empty for every \`pure\` Word; the vocabulary is open (the schema types
    /// it as free strings), so this is projected as declared rather than
    /// narrowed into a Rust enum.
    pub effects: &'static [&'static str],
    /// The conditions under which the Word *raises*, in the specification's
    /// own spelling — the ERROR counterpart of \`projection\`, which names the
    /// condition under which it answers NIL instead.
    ///
    /// Projected so a diagnosis can be derived from the declaration rather
    /// than from a hand-written table: a Word that raises for a condition it
    /// declares used to reach the caller as \`why: "unknown"\` with "read the
    /// message" as its only next check.
    pub error_when: &'static [&'static str],
    /// One line of the Word written correctly, as the specification writes it.
    ///
    /// Read by \`diagnostic_hint::usage_hint\` for a stack underflow or an
    /// operand of the wrong domain, where "how many operands and in what shape"
    /// is exactly the question and \`stack underflow\` alone answers none of it.
    pub syntax: Option<&'static str>,
    /// What the Word does, with its tested examples — the LOOKUP \`Summary\`
    /// section (\`documentation.summary\`).
    pub summary: &'static str,
    /// The stack effect in prose — the LOOKUP \`Stack Effect\` section
    /// (\`documentation.stackEffect\`).
    pub stack_effect: &'static str,
    /// The one-line hover title a host shows for the Word
    /// (\`documentation.hover\`), and its registered description.
    pub hover_summary: &'static str,
    /// The inline word-info preview (\`documentation.syntax\`): the same line
    /// as \`syntax\`, as the plain text a host renders.
    pub hover_syntax: &'static str,
}

pub const GENERATED_WORDS: &[GeneratedWord] = &[
${rows}
];
`;

// The committed file must satisfy `cargo fmt --check` as well as this
// script's own `--check`, so `rustfmt` — the tool that check actually runs —
// is the one that decides final formatting, not a hand-rolled approximation
// of its wrapping rules.
const formatted = (() => {
  const result = spawnSync('rustfmt', ['--edition', '2021'], {
    input: output,
    encoding: 'utf8',
  });
  if (result.error || result.status !== 0) {
    const detail = result.error ? result.error.message : result.stderr;
    throw new Error(`rustfmt failed to format the generated word registry: ${detail}`);
  }
  return result.stdout;
})();

writeOrCheck(
  'word-registry',
  [{
    path: outputPath,
    content: formatted,
    stale: `${outputPath} is stale. Run npm run word-registry:generate.`,
  }],
  {
    current: `${entries.length} Words are current.`,
    wrote: `wrote ${entries.length} Words to ${outputPath}.`,
  },
);
