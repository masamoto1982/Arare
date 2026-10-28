# Structured Prose Style

How to choose between a sentence, a label:value list, a table, and a diagram when writing about Ajisai — the specification, the README, and the Reference. The goal is content that survives translation without being rewritten.

## Authority

- **Non-canonical.** This document governs **how** Ajisai is written about, not **what** is true of it. It defines no language semantics.
- **Canonical source remains `SPECIFICATION.html`.**
- **Scope:** the same three surfaces `ajisai-authoring-style.md` governs — the specification, the README, and the Reference.
- **Relationship to `ajisai-authoring-style.md`:** that document governs notation — how an Ajisai token, a formula, and prose are visually kept apart. This document governs a layer above notation: how a piece of *information* is shaped, independent of which channel it is written in. Apply both; they do not conflict.

## 1. Why: translation is where flowing prose breaks

The Reference's Japanese edition is a rewrite of the English one, not a translation pass — the English original's prose could not be translated sentence-by-sentence and stay natural. That difficulty is the evidence this document acts on, not a hypothetical:

- A sentence that packs several facts about one subject forces a translator to decide a new sentence structure, not just new words — Japanese is SOV, English is SVO, and a sentence with three clauses reorders all three.
- Once restructured, the two languages' prose can no longer be diffed or kept in sync by a mechanical check; drift is caught only by a human rereading both in full.
- A structured unit — one label, one value; one row, one comparable fact — has no sentence-level word order to restructure. Only the labels and cell text translate. The shape stays identical across languages, which is what lets a script (or a person) verify that a translated page still has the same rows as its source.

**Prefer structure over flowing prose wherever the content is a set of facts, not an argument.**

## 2. The test: is this a list of facts, or a chain of reasoning?

Ask one question: **does sentence 2 depend on sentence 1, or could the two be read in any order?**

| If the content is… | Then it is… | Write it as |
|---|---|---|
| Independent facts about one subject | a list of facts | a label:value list |
| The same facts across several subjects | a list of facts | a table |
| A condition and what follows from each of its cases | a list of facts | a table (`Condition` / `Result` columns) |
| A structure, a flow, or a sequence of states | a list of facts (spatial, not verbal) | a diagram, or a numbered list if no diagram is warranted |
| A definition that holds *because* of something, an exception justified by a reason, a proof step | a chain of reasoning | a short paragraph |

A chain of reasoning does not mean permission to write long. It means the logical connective (*because*, *therefore*, *not X but Y*, *unless*) is itself part of the meaning and would be lost by flattening the sentence into rows. Keep such a paragraph as short as the reasoning allows, and do not let a second, independent fact ride along inside it — split that fact out into its own sentence or row even when the reasoning around it stays prose.

### 2.1 Worked example: the bad case, the good case, and the case that must stay prose

**Bad** — the shape the user identified: several independent facts compressed into one sentence.

> 消防車は火災の消火を目的とした緊急車両で、車体色は赤である。

**Good** — the same facts, decomposed:

| | |
|---|---|
| 消防車の目的 | 火災の消火 |
| 消防車の色 | 赤 |

Ajisai's specification once had real content in the bad shape. An early draft of `LANG.MACHINE.LIMITS` read:

> Two limits exist and they mean different things. The **execution-step limit** bounds total work; exhausting it raises its registered ERROR. The **materialization ceiling** bounds how large a single generated collection may become; a well-formed request that exceeds it raises the same ERROR under the ceiling's own name.

Each half of that sentence is an independent fact about one of two subjects — this is the table case (the clause now holds it as one, with one row per ceiling):

| Limit | Bounds | On exhaustion |
|---|---|---|
| Execution-step limit | Total work | ERROR, `executionLimitExceeded` |
| Materialization ceiling | Size of one generated collection | ERROR, `resourceLimitExceeded`, naming `materializedElements` |

By contrast, `LANG.VALUES.EXACT` must **stay prose**:

> Comparison is accordingly **total**: every comparison of two scalars yields TRUE or FALSE in finite time, not as a separate guarantee but as the half of the condition that fixes the domain.

"Not as a separate guarantee but as the half of the condition that fixes the domain" is the entire point of the sentence — totality is not an independent fact sitting beside the domain's definition, it is a logical consequence of it. Splitting this into a label:value pair ("Totality: guaranteed") would state the conclusion and silently discard the reasoning that makes it true. When in doubt, keep the connective and the sentence together.

## 3. Diagrams

No diagram renderer is wired into this repository (no Mermaid, no PlantUML pipeline, no CDN — consistent with the project's self-hosted-only stance, `ajisai-authoring-style.md` §4.1). Two hand-authored forms are available without adding a dependency:

| Surface | Diagram form | Why |
|---|---|---|
| Specification, Reference (HTML) | Inline hand-authored SVG | Renders with no JavaScript, no build step, matches the self-hosted-only stance |
| README, `docs/dev/` (Markdown) | ASCII art in a fenced block | Always renders on GitHub; a diagram's labels are still short enough to translate like a table cell |

Reach for a diagram when the content is genuinely spatial — a structure, a state machine, a stack/frame shape — not as decoration for content a table already covers. `LANG.SOURCE.FRAME`'s two frame rules (whole-stack vs. isolated) are a candidate: today they are two paragraphs of prose; a small diagram showing the stack before and after each rule, beside a table naming which Words use which rule, would carry the same information with less to mistranslate.

Do not force a diagram where a table already says everything. A diagram earns its place only when position or shape *is* the information.

## 4. What does not change

- **Worked examples, code, and expected values are unaffected.** They already live in tables under `ajisai-authoring-style.md` §5 / §7 and `reference-writing-style.md`.
- **A restructuring preserves meaning exactly.** Same rule as `ajisai-authoring-style.md` §6.7 — presentation only, never semantic. Converting `LANG.MACHINE.LIMITS` to a table above changed no normative content.
- **Section 2's table itself is not a license to fragment reasoning.** A paragraph that stays a paragraph under the test in §2 is not a violation to be found and fixed later; it is the correct shape for that content.

## 5. Relationship to the other style documents

| Document | Layer it governs |
|---|---|
| `ajisai-authoring-style.md` | Notation: keeping Ajisai tokens, mathematics, and prose in visually distinct channels |
| This document | Information shape: sentence vs. label:value vs. table vs. diagram |
| `reference-writing-style.md` | Reference site and `?`/LOOKUP text conventions |
| `three-layer-documentation-model.md` | Structure of user-facing guidance across Reference / LOOKUP / hover |
| `ajisai-authoring-style.md` §11 | Why the lexicon leaves `:` alone, so the label:value shape above stays writable |

Applied together: a table cell still wraps its Ajisai tokens in `` `…` `` (notation), and a paragraph that must stay prose under §2 above still promotes any inline list of three or more tokens to a table where one appears inside it (`ajisai-authoring-style.md` §6.4).
