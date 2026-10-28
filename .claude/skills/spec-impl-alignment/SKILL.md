---
name: spec-impl-alignment
description: Run Ajisai's 4-phase spec/implementation alignment pass — check spec/ for internal contradictions, check the implementation for internal contradictions, reconcile any remaining spec-vs-implementation disagreement toward whichever side has the better synergy, then delete whatever turns out to be spec with no implementation or implementation with no spec (including the docs/dev/ history that led there). Use when asked to audit, reconcile, or clean up drift between Ajisai's specification and its implementation, or to continue/repeat that effort.
---

# Ajisai spec/implementation alignment

Full method: `docs/dev/spec-impl-alignment-methodology.md` (non-canonical,
but the citable rationale for every step below — read it before a first
run). This file is the operational checklist; that one is the reasoning.

Canonical spec lives in `spec/` (the sources `spec/README.md` lists, among
them `language-semantics.md`, `words.json`+`words.schema.json`,
`gui-semantics.md` and `host-protocol.schema.json`) and is regenerated into
`SPECIFICATION.html` via `npm run specification:generate`. Nothing else
defines Ajisai semantics — not `docs/dev/`.

Work one phase to a clean, tested, committed state before starting the
next. Each phase's fixes get their own PR (small, verifiable diffs beat
one large one — see PRs #1604-#1611 for the shape this takes in
practice). The same reasoning applies *inside* a phase, between one
finding and the next: see "Scope discipline" under Phase 4.

## Phase 1 — spec-internal consistency

Read the `spec/` sources (the eight LANG.AUTHORITY.SOURCES names) against **each other**, without looking at
`rust/` or `src/`. Existing gates (`npm run specification:check`,
`semantic-kernel:check`, `word-schema:check`, `word:manifest:check`,
`word-registry:check`, `word:reference:check`,
`check:formalization-coverage`, `check:minimal-core`,
`check:unreachable-contract`) verify generation round-trips and structural
cross-references, but not whether one source's *prose* claims something a
sibling source's *schema* doesn't actually define. That gap is what a human
(or an agent) reading pass finds.

When two sources disagree, resolve toward whichever is **more clearly the
data of record** for that fact — narrow prose to match a schema's actual
declared shape, rather than widening the schema to match aspirational
prose (widening a schema is how host-protocol-v2 happened: see Phase 4).

Verify: regenerate (`npm run specification:generate`), then run every
`npm run *:check` script above plus `cargo test --all-targets` — a
spec-only change can still break a Rust test that reads `spec/` at
compile/test time (e.g. `include_str!`-based conformance or golden tests).

## Phase 2 — implementation-internal consistency

Ignore `spec/` entirely. Look for the same fact implemented independently
in more than one place — `rust/src`, `src/`, `tools/mcp-server/` — where
nothing actually cross-checks the two copies against each other. Grep for
tells: a comment claiming "mirrors X" or "same as Y" where X/Y is a
different file; a claimed shared test file that doesn't exist; one copy
whose `git log` is stale relative to the other's.

For each real finding: confirm which copy is actually live (called from
production code, not just from its own tests) by tracing callers, not by
reading doc comments — comments claim parity more often than code
delivers it. Fix the bug in the live copy if the two disagree on behavior;
delete the dead copy if one turns out to be unreferenced (grep the whole
repo for its filename/export before deleting — `src/gui/value-formatter.ts`
was dead and buggy; word-candidates.js was live and merely unguarded).
Add the missing cross-check test so the same drift can't ship silently
again.

Verify: `npm run check` (tsc), `npm run lint`, `npm run deadcode`, full
`npm test` (vitest), `cargo fmt --check`, `cargo clippy --all-targets --
-D warnings`, `cargo test --all-targets` — and, whenever the finding was
"the same fact in two copies", the suite that actually runs both copies
and compares them (`npm run test:mcp-backends`, i.e.
`tools/mcp-server/backend/parity-test.js`, native CLI vs. WASM worker).
The rest of the matrix only proves each copy self-consistent.

### Regenerate every checked-in artifact your change is upstream of

Most generated artifacts here are hard-gated by a `--check` mode on their
own generator — `word-registry:check` covers `rust/src/kernel/generated/`,
and `word:manifest:check`, `word:reference:check`,
`semantics:table:check`, `semantic-kernel:check` cover the rest — so
running the `*:check` matrix is enough for those. The dangerous class is
any checked-in artifact whose freshness gate is **advisory or missing**:
nothing in the normal matrix rebuilds it, so its staleness is invisible
by construction, and no amount of green output rules it out.

In this repo that class is exactly the two committed wasm-pack bundles,
`src/wasm/generated/` and `tools/mcp-server/wasm/generated/` (built by
`npm run build:wasm` / `npm run build:mcp-wasm`). CI's two "Detect stale
committed wasm bundle (advisory)" steps in `.github/workflows/test.yml`
are `continue-on-error: true` on purpose.

So before calling a phase done: list every checked-in artifact that is a
deterministic build product of a file in your diff — compiled binary,
generated bundle, lockfile, golden snapshot — and if its gate is
advisory, regenerate it yourself and commit whatever the diff shows.

Two independently stale copies agree with each other, which is why the
suite stays green until one of them is fixed. PR #1611 trimmed four dead
JSON fields (`semanticKind`/`shape`/`capabilities`/`origin`) out of the
CLI's `rust/src/agent/report.rs::semantics_json` to match the WASM
boundary, whose own source comment already called them "the retired
HostProtocolV1... no reader ever consulted them". Every command above
passed locally. The committed `.wasm` binaries had been stale since
*before* that PR, and the parity test had been passing only because the
untrimmed CLI emitted the same dead fields as the untrimmed binary.
Fixing the live side is what exposed the binary's own drift — in CI, not
locally. `npm run build:wasm && npm run build:mcp-wasm`, commit the
regenerated bundles.

### Verification blind spot — MCP-facing generated prose

`SKILL.md`, `tools/mcp-server/mcp-quickstart.md` → `assets/quickstart.md`,
and the tool `description`/`inputSchema` strings in
`tools/mcp-server/index.js` are implementation (`tools/mcp-server/` is
explicitly in Phase 2's scope) that an MCP client reads *before* its first
tool call. Two of the three had example code that drifted from the
tokenizer independently of each other and went undetected, because only
`mcp-quickstart.md`'s fenced blocks (`selftest.js`'s `prefaceExamples`) and
`SKILL.md` §6's `canonicalExamples` array were actually executed — the
hand-typed connective prose around them (`SKILL.md` §2/§3,
`index.js`'s `source` description) was not, despite both files claiming it
was. `{ body } 'NAME' DEF` and `{ 2 MOD 0 = } FILTER` both shipped this
way; full incident and fix in
`docs/dev/spec-impl-alignment-methodology.md`'s "検証の盲点 その2".

Before closing Phase 2: any backtick-quoted span in these files that reads
as literal Ajisai source (contains `[`, `'`, or is a bare postfix
expression) must either come from an already-executed example (reference
`canonicalExamples` by `id` in `generate-skill-md.mjs`, or be one of
`mcp-quickstart.md`'s fenced ```ajisai blocks) or be run against the live
backend directly (`tool-description.test.js` does this for `index.js` via
`createBackend()`). A syntax *shape* that needs a placeholder (`body`,
`guard`, `NAME`) to explain is prose, not backtick code — backticks assert
"this is verified Ajisai," and a placeholder can't be. `{`/`}` specifically
have recurred three times independently; `generate-skill-md.mjs` now fails
generation if either appears in §2/§3 outside a line documenting them as
retired.

## Phase 3 — spec vs. implementation

Only now compare the two. For each observed disagreement between what
`spec/` says and what a real build actually does (confirm with a live
program run — `ajisai agent compute -`, which always emits JSON, or the WASM/GUI
equivalent — never trust a doc comment's claim about behavior),
apply the suite-arbitration rule:

(The MCP-served guide, `ajisai://guide/quickstart`, is not one of the
canonical `spec/` sources and so sits outside this comparison — but it's
what an AI caller actually reads and acts on, ahead of `spec/`. Its
agreement with the real backend is guaranteed separately, by the
executable-verification mechanisms named in the "Verification blind spot —
MCP-facing generated prose" note above, not by this phase.)

- The behavior is **pinned by `tests/conformance/index.html`**
  (`LANG.CONFORMANCE.CORPUS`) → intentional design. Amend `spec/` to
  document it (spec catches up to impl).
- **The suite is silent** on it, and it demonstrably violates spec
  prose (checked against `LANG.CONFORMANCE.FAMILIES` law tests) →
  implementation bug candidate. Fix the implementation to match spec
  (impl catches up to spec), and add a conformance case pinning the
  corrected behavior.
- **`spec/` disagrees with itself** on the point → that's actually a
  Phase 1 finding surfacing late; resolve by authority order
  (`LANG.AUTHORITY.SOURCES`) and `git blame` provenance, with a human
  decision where genuinely ambiguous.

Never resolve a Phase 3 finding by expanding a schema to describe
something no consumer reads yet (see host-protocol-v2's postmortem in
`docs/dev/spec-impl-alignment-methodology.md` §2) — verify liveness with
a real build first (`grep -rl` for the module's own name/type across the
whole repo; a serializer nothing calls is not "the protocol" no matter
how complete its schema is).

Verify: Phase 1's and Phase 2's matrices both, since a Phase 3 fix moves
one side toward the other and can break either — including Phase 2's
artifact regeneration, because a reconciliation that edits Rust source
leaves the committed `.wasm` bundles behind until they are rebuilt.

## Phase 4 — delete what turns out to be one-sided

Once Phase 3's reconciliation lands and every remaining gate is green:

1. **Spec with no implementation** (a schema, a declared field, a
   documented behavior nothing in `rust/`, `src/`, or `tools/mcp-server/`
   ever produces or reads) — delete the spec text/schema/fixture. Do not
   leave it as a "future target": an unimplemented spec is noise, and if
   the feature is wanted later it gets written from the implementation
   that actually exists then.
2. **Implementation with no spec** (real, reachable code implementing a
   behavior no `spec/` source declares, and Phase 3 confirmed it's not
   simply an undocumented-but-intended feature) — delete the code, its
   tests, and its fixtures.
3. **The `docs/dev/` history that led to the now-resolved finding** —
   once Phase 1-3 have folded a memo's conclusions into the canonical
   spec or the implementation, the memo describing the investigation is
   noise for anyone opening `docs/dev/` next. Before deleting a memo:
   - `grep -rl '<filename>' .` across the whole repo (including other
     `docs/dev/` files and `tests/conformance/index.html` — they cite
     each other and the corpus more than you'd expect).
   - For every hit, either rewrite the citing passage to be
     self-contained (point at a stable `LANG.*` clause ID or the current
     file that actually carries the fact now, not the retiring memo) or
     remove the row from `docs/dev/INDEX.md` alongside the file.
   - Re-run `npm run check:docs-dev-drift` after — it only catches a
     `[設計根拠]`/`[方針記録]` memo's stale Rust-identifier claims, not a
     dangling filename citation, so the grep sweep above is not optional
     even when this check stays green.
   - A memo tagged `[執筆規約]` (writing convention) or one whose
     conclusions are still actively used (referenced by a CI script, or
     cited from a live Rust/TS source comment as design rationale) is
     *not* history — most of `docs/dev/` turns out to be this even
     though it looks like a one-off report at a glance. Check references
     before assuming a memo is safe to delete.

### Scope discipline — a finding is not a license to keep pulling

Tracing one dead thing reliably surfaces another. When it does, judge
whether the second finding is the *same* finding seen from another angle
or a structurally different one — a different subsystem, a different file
class (a CI-gated data file, a `spec/` source, a formalization section),
or something that would need its own multi-file sweep beyond the one
you're in. If it's structurally different: **stop, leave it untouched,
and name it explicitly as a follow-up** in the PR description, for a
separate decision and its own round. That is the same escalation Phase 3
already prescribes for a spec-disagrees-with-itself finding with no
conformance pin — hand it to human judgment rather than settle it
mid-pass.

This is scope discipline, not caution. Small, single-purpose diffs are
the entire reason for working in phases and per-finding PRs; a deletion
that grows while you write it can no longer be reviewed against a stated
scope, and the "while I was in there" edits are the ones that read as
unexplained later.

PR #1611 hit this: while confirming `rust/src/agent/contract_linearity.rs`
was vestigial (keyed on `SPAWN`/`AWAIT`/`STATUS`/`KILL`/`MONITOR`/
`SUPERVISE`, none of which were in the then-current 66-word vocabulary), it
became clear the same surface is also the subject of an entire section
(§9-septies) of the since-deleted `ajisai-mathematical-formalization.md` memo, marked
`HOLDS` and citing `rust/tests/child_runtime_laws.rs` — a file that does
not exist — while the CI-gated `docs/formalization-coverage.json`
correctly classifies that surface as `"Exploratory"`. A real Phase 1
contradiction and a Phase 4 candidate, but a much larger and differently
shaped one. It was left untouched and named as a follow-up finding in the
PR description; the module deletion shipped without it.

**Follow-up (2026-09)**: a later pass found the rot wasn't limited to
§9-septies — the same ungated document also cited nonexistent test files
for records and effect observation, built on a value-space definition
(§1.1) that assumed domains (Record, a separate code-block domain, a
child-runtime handle) the implementation had since retired, and disagreed
with itself on the conformance-suite size (53 vs. 45 vs. the real 293).
Being outside CI's reach, none of it was self-correcting. The document was
deleted outright, along with the dangling citations to it in `rust/tests/*.rs`
comments, the `formalization_sections` field (and its CI requirement) in
`docs/formalization-coverage.json`, and related `docs/dev/` cross-references.

Verify the full matrix again after deleting: every `npm run *:check`
script, `cargo fmt`/`clippy`/`test --all-targets`, `npm run check`/`lint`,
full `npm test`, plus the artifact regeneration and cross-copy parity
suite described under Phase 2 if the deletion touched anything a
committed bundle is built from.
