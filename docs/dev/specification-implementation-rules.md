# AI-first Implementation Rules

How this repository is written. Engineering discipline for contributors —
not part of the language.

## Authority

- **Non-canonical.** LANG.AUTHORITY.SOURCES places `docs/dev/` outside the
  normative set. Nothing here defines Ajisai semantics or constrains a
  conforming implementation.
- Until #1652 this content was inlined into `SPECIFICATION.html` as a
  numbered section 12, where it rendered identically to the normative
  clauses two paragraphs after the one saying `docs/dev/` defines nothing.
  The specification's own table of contents never listed it. It now lives
  only here, which is where the authority clause already said it belonged.
- The first Mandatory rule below is machine-enforced by
  `scripts/check-file-size-budget.mjs` against
  `docs/quality/file-size-baseline.json`; the rest are read, not gated.

## Mandatory

- Prefer explicit, structurally searchable function and module names.
- Keep newly created Rust source files, and files that are substantially
  rewritten, at or under 500 lines. Existing over-budget files are recorded
  as a baseline (`docs/quality/file-size-baseline.json`) and, as a goal,
  must not be changed in ways that further increase their line count.
- Keep control flow shallow and phase-separated.
- Separate semantic changes from structural cleanup in change management.
- Maintain single canonical implementations; do not allow dual-mode drift.
  What has to be single is the *knowledge* — one rule, one number, one output
  format — not every block of text that resembles another. See **The DRY
  criterion** below for how the two are told apart.
- Source code comments are allowed when they clarify intent, invariants,
  traceability, or non-obvious behavior. When source code is changed, nearby
  comments must be reviewed and updated so they remain accurate. Comments
  that merely restate obvious code should be avoided.

## Advisory

- Extract a helper when two places encode the same decision. Duplicated
  control scaffolding is a reason to look, never a reason on its own to
  unify: this line used to read "prefer small helper extraction for
  duplicated control scaffolding", which measures shape, and shape is not
  what DRY is about.
- Prefer deterministic, low-ambiguity error classification.
- Prefer mechanically enforceable tests over narrative documentation.

## The DRY criterion

The principle's own statement is about knowledge: *every piece of knowledge
must have a single, unambiguous, authoritative representation within a
system*. It is not a rule against text that repeats. Two blocks that read
alike while encoding two independent decisions are not a violation, and
merging them couples the decisions — the next change to one arrives as a
parameter threaded through the other.

Before unifying two places, or before leaving a second copy standing, ask:

1. **If one changes, must the other change in the same commit, for the same
   reason?** Yes — one piece of knowledge, and it needs one representation.
   No — two pieces of knowledge that happen to agree today; leave them apart.
2. **Can a reader tell which copy is authoritative?** If not, the copies are
   already drifting, whether or not they currently disagree.
3. **What is the second copy for?** A projection generated from the source
   (`spec/*.json` → registry, docs, Specification) is not a second
   representation. A hand-kept restatement is.

Both directions have a cost and neither is free:

- A restated *number, rule or output format* drifts silently. It is why the
  host-side doc comments went on saying the default step budget was 100,000
  after the constant was rederived. Name the constant; do not restate its
  value.
- A shared helper over two unrelated decisions **couples** them. The MCP
  server's `executionSteps: 100_000` is the same literal as the interpreter's
  retired default and must stay its own: it is that host's policy, not a copy
  of the language's.

Where a second representation is genuinely unavoidable, it is allowed with a
mechanical gate that fails on disagreement, and a comment saying why it
exists. `scripts/lib/reference-lexer.mjs` executes `spec/grammar.json` as a
second lexer beside the Rust tokenizer, because a grammar file nothing runs is
only a description, and `scripts/check-grammar.mjs` fails when the two lexers
disagree. Deliberate non-sharing is recorded the same way:
`rust/src/interpreter/word_outcome_vocabulary.rs` opens by saying it is
deliberately independent of `word_contract.rs` rather than leaving a reader to
assume the overlap was missed.

This section came out of a 2026-09 review that measured duplication by
knowledge rather than by shape and found three knowledge duplications (the
default step budget restated, the diagnostic format doubled, the persisted
record shape restated), all fixed then. The review's memo was retired once
its findings were folded in here; the version history holds it.
