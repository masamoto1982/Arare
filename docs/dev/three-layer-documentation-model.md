# Ajisai Three-Layer Documentation Model

Note: this model describes the word-help surface hierarchy (Reference / LOOKUP / hover) and is orthogonal to the document-role axis (Specification / Reference / README).

Status: proposal (revision of the草案 dated 2026-05).

> Note (2026-10-15): the hover half of this proposal shipped — every Core Word's `hover_summary` and `hover_syntax` are projected from `documentation.hover` and `documentation.syntax` in `spec/words.json` (`rust/src/kernel/generated/word_registry.rs`), and `rust/src/builtins/builtin_word_details_tests.rs` runs every `hover_syntax` and requires it to call its Word. The vocabulary this proposal was written against has since changed: no Word has a symbol spelling any more (LANG.SOURCE.NORMALIZE), so there is no sugar to prefer, and Words such as `MOD`, `LTE`, `KEEP`, `LOOKUP`, `OR-NIL` and `IMPORT` are not Words (`spec/retired-words.json`; Lookup is a host operation, `spec/gui-semantics.md`). Section references to `SPECIFICATION.html` (§2.4–§2.5, §6.5, §7.14) are to its former numbering. Where this document and the registry disagree, the registry is current.
Authority: non-canonical. `SPECIFICATION.html` remains the canonical source for language semantics; its Specification Authority section (§2.4–§2.5) defines Reference, LOOKUP, and hover as derived documentation under the specification's authority. This document defines the **structure and policy of user-facing guidance**, not language behavior.

Related existing documents:

- `docs/dev/reference-writing-style.md` — current writing convention. Will be amended to align with this model (see §7).
- `SPECIFICATION.html` §7.14 — Coreword contract metadata (`partiality`, `nil_policy`, `safety_level`). Stability/contract claims in any guidance layer must agree with §7.14.

---

## 1. Goal

Ajisai exposes word-level help in three places. They should not all carry the same content rendered at different lengths. Each layer has a distinct role, audience, and timing.

| Layer | Audience moment | Density | Source of truth |
|-------|-----------------|---------|-----------------|
| Reference | "I want to learn / look up later" | Full | Concept pages + per-word entries |
| LOOKUP (`?`) | "I have this word in front of me — what does it do?" | Medium | Per-word entry, English plain text, editor-safe |
| Hover | "I'm typing — remind me which one this is" | Minimal | One-line role + shortest syntax |

Every implementation decision below follows from this separation.

---

## 2. Reference (Layer 1)

### 2.1 Scope

Reference is the **entire body of Ajisai-the-language documentation**. It is delivered as a static site rooted at `public/docs/index.html` (currently a stub linking to GitHub; to be expanded).

Reference covers:

1. **Getting started** — install, the editor surface, first program.
2. **Concept guide** — value model, stack, modifiers, fractional dataflow, error model. Mirrors SPECIFICATION.html sections in tutorial form.
3. **Word catalog** — one page per built-in word. Generated from the same data that drives LOOKUP, plus a `concept` field that LOOKUP does not display.
4. **Module reference** — per-module pages.
5. **Developer notes** — debugging model, runtime invariants, version/stability notes.

Items 1, 2, 4, 5 are hand-authored Markdown (or HTML). Item 3 is **derived from the generated registry entry** (`GeneratedWord`, the struct §5 proposed as `BuiltinSpec`; the separate copy has since been removed) plus optional concept text. Sharing the data source between Reference word pages and LOOKUP is mandatory: a divergence between the two is a documentation bug.

### 2.2 Tone

Reference is documentation, not tooltips. Markdown is the authoring format, full prose is allowed, and translations may exist in parallel directories. The English text remains canonical when translations disagree.

### 2.3 Per-word entry fields

A word entry on the Reference site is rendered from these fields:

```
Name
Sugar (if any)
Category
Concept            ← Reference-only, LOOKUP omits
Summary
Syntax (canonical and shorthand)
Stack Effect
Behavior
Examples
Failure / NIL Behavior
Side Effects
Modifier Interaction (when non-standard)
Related Words
Stability          ← must equal SPECIFICATION.html §7.14 contract
Implementation Notes (optional)
```

`Concept` answers *why this word exists in the language*, e.g.

> `CONTRACT` is an introspection word. It lets a program read the contract of a Word or a block from inside the language, without running it, which is what lets a program check what it is about to call.

Such language-design context belongs in Reference and may be repeated in concept-guide pages. It does **not** appear in LOOKUP.

Reference must include a concept page for the Bubble Rule. The page explains: well-formed operations that cannot produce a value return Bubble/NIL with a reason, while malformed usage raises an error. Per-word Reference entries for `GET`, `SQRT`, `NUM`, and `POW` must describe their Bubble/NIL cases separately from contract-violation errors.

(2026-09-14: the water metaphor was removed from `README.md` and `public/docs/ja/index.html` as reader-facing noise. The concept page still exists under `#nil` and still explains this rule — it now just says `NIL` throughout instead of the borrowed word "泡"/"Bubble". `Bubble/NIL` unchanged in LOOKUP text below, which is Rust source, out of scope for that cleanup.

What was removed is the mapping — 器=Stack, 水=値, 流れ=dataflow, 波紋=`PRINT`, 泡=`NIL`, 決壊=error — used as an explanatory system across both documents. Do not reintroduce it. (2026-10-14: at the owner's request the Japanese Reference also dropped its 「なぜAjisaiを作ったのか」 section, and its closing water-vessel image and hydrangea etymology moved into a hover tooltip on the first "Ajisai" of the introduction, which now leads with what Ajisai can do. 2026-10-15: the English Reference was regenerated from the Japanese and follows it. README's "Why Ajisai" is unchanged for now.)

---

## 3. LOOKUP (Layer 2)

### 3.1 Role

LOOKUP answers, for one specific word the user already chose:

- What is it?
- What role does it play?
- How do I write it?
- What does it do to the stack?
- What happens when it fails?
- What related words should I know?

LOOKUP does not teach the language and does not include design history.
For Bubble Rule words, LOOKUP may include concise UTF-8 English failure text such as "Produces a Bubble/NIL when the index is out of range; raises StructureError when the target is not indexable." It must not use Japanese prose such as "泡".

### 3.2 Built-in vs user word — preserve current behavior

LOOKUP's two branches in `rust/src/interpreter/execute-lookup.rs::op_lookup` remain unchanged:

- **Built-in word** → `lookup_builtin_detail` returns formatted documentation; this string is loaded into the editor via `interp.definition_to_load`.
- **User word** → the original defining program (`def.original_source`) is loaded into the editor unchanged.

Only the **content** that `lookup_builtin_detail` produces changes. The function signature and the user-word path stay the same.

### 3.3 Output format

LOOKUP output is loaded into the code editor textarea, so it must be plain text usable beside Ajisai source:

- UTF-8 English plain text.
- Plain text (no Markdown rendering — the editor displays it verbatim).
- Lines ≤ 80 columns recommended.
- No control characters, no trailing whitespace.
- Two-space indentation for nested blocks.

The English-only rule is a **policy change** from the current `reference-writing-style.md`, which permits Japanese Markdown. Rationale: LOOKUP output sits inside an editor used to write code, where mixed-script content visually competes with the program. Reference (Layer 1) is where translated and richly formatted help lives.

### 3.4 Template

```
# LOOKUP

Sugar:
  ? = LOOKUP

Category:
  introspection

Summary:
  Shows the documentation for a word.

Role:
  Provides word-level guidance from inside Ajisai.

Syntax:
  Canonical:
    'ADD' LOOKUP
  Shorthand:
    'ADD' ?

Stack Effect:
  [word] -> []

Behavior:
  Reads a word name from the stack and loads its documentation
  into the editor.

Examples:
  Canonical:
    'ADD' LOOKUP
  Shorthand:
    'ADD' ?

  Result:
    Loads the documentation for ADD into the editor.

Failure:
  Unknown words raise UnknownWord.

Side Effects:
  Modifies the editor text area.

Related:
  DEF, DEL

Stability:
  stable
```

### 3.5 Role vs Behavior — disambiguation rule

`Role` and `Behavior` are easy to blur. Apply this rule:

- **Role** = the word's *position in the language*. Why it exists. Single sentence, often nominalised ("Provides...", "Defines...", "Acts as...").
- **Behavior** = the *mechanical effect on inputs and runtime state*. Concrete, imperative, reads top-to-bottom like a small spec.

Example for `ADD`:

```
Role:
  Numeric addition; one of the four arithmetic primitives.

Behavior:
  Pops two numeric values, pushes their sum. Operand types follow
  the numeric coercion rules in Section 4.
```

### 3.6 Sugar handling

For words that have sugar listed in SPECIFICATION.html §6.5, the `Syntax:` and `Examples:` blocks always show **both** the canonical and shorthand forms. For words without sugar, only `Canonical:` appears (no empty `Shorthand:` heading).

LOOKUP's example for the word **must reflect runtime semantics**. `LOOKUP` itself pops the word name from the stack, so the canonical example is `'ADD' LOOKUP`, not `ADD LOOKUP`.

---

## 4. Hover (Layer 3)

### 4.1 The two hover surfaces

The current GUI has two distinct surfaces; both already exist in `src/gui/dictionary-element-builders.ts` and `src/gui/vocabulary-state-controller.ts`:

| Surface | DOM element | Driven by |
|---------|------------|-----------|
| Native tooltip on the word button | `button.title` | `BuiltinSpec.short_description` (today) → `hover_summary` (proposed) |
| Inline syntax preview alongside the dictionary | `elements.builtInWordInfo` | `BuiltinSpec.syntax` (today) → `hover_syntax` (proposed) |

These two stay separate. `title` shows on mouse-hover only and is not visible to keyboard navigation; the inline preview is visible on focus and hover both.

### 4.2 Hover content rules

- **One line each.** No multi-line content in either hover surface.
- **No** failure modes, side effects, modifier interaction, related words, stability, or prose.
- `hover_summary` form: `WORD — short verb phrase`. Example: `ADD — add values`.
- `hover_syntax` form: the **shortest useful** invocation, sugar preferred when shorter.

### 4.3 Sugar preference for `hover_syntax`

No Word has a symbol spelling, so there is no shorter form to prefer: `hover_syntax` is the Word's `documentation.syntax` in `spec/words.json`, one real invocation with its operands shown — `1 2 ADD`, `[ 1 2 3 ] [ 2 MUL ] MAP`, `'42' NUM`. The registry is the table; this document does not copy it.

`hover_syntax` is **not optional**: every built-in must carry one example string.

---

## 5. Data model

The codebase already has `BuiltinSpec` in `rust/src/builtins/builtin-word-definitions.rs`. The proposal **extends** this struct rather than introducing a parallel `BuiltinDoc`.

### 5.1 New / renamed fields

> Design note: map / form / fold の分類は UI 境界には公開しない。Rust内部では `WordShape` として型付きで保持する。この分類は内部メタデータであり、通常UIのワードボタン表示には反映しない。

```rust
pub struct BuiltinSpec {
    // — existing identity —
    pub name: &'static str,
    pub category: &'static str,
    pub word_shape: WordShape,
    pub detail_group: BuiltinDetailGroup,
    pub executor_key: Option<BuiltinExecutorKey>,


    // — Layer 3 (hover) —
    pub hover_summary: &'static str,    // replaces today's `short_description` for the title attribute
    pub hover_syntax:  &'static str,    // replaces today's `syntax` for the inline preview

    // — Layer 2 (LOOKUP) —
    pub summary:        &'static str,
    pub role:           Option<&'static str>,
    pub syntax_forms:   &'static [BuiltinSyntaxDoc],
    pub stack_effect:   &'static str,
    pub behavior:       &'static str,
    pub examples:       &'static [BuiltinExampleDoc],
    pub failure:        Option<&'static str>,
    pub side_effects:   &'static [&'static str],
    pub modifier_interaction: Option<&'static str>,
    pub related:        &'static [&'static str],
    pub stability:      &'static str,   // must agree with §7.14 contract

    // — Layer 1 (Reference only) —
    pub concept:        Option<&'static str>,
}

pub struct BuiltinSyntaxDoc {
    pub canonical: &'static str,
    pub shorthand: Option<&'static str>,
    pub description: Option<&'static str>,
}

pub struct BuiltinExampleDoc {
    pub canonical: &'static str,
    pub shorthand: Option<&'static str>,
    pub result:    Option<&'static str>,
}
```

### 5.2 Why one struct, not two

The current codebase already passes `BuiltinSpec` through the WASM boundary (see `vocabulary-state-controller.ts:254-271`). Splitting Reference data into a second `BuiltinDoc` struct duplicates the registry and creates two sources of truth that can drift. A single struct, with `concept` reserved for the Reference renderer, keeps the data canonical and lets each renderer pick the fields it cares about.

### 5.3 Single source of truth: stability

`stability` must mirror the contract entry from SPECIFICATION.html §7.14 (`partiality`, `nil_policy`, `safety_level`). The recommended display rule:

- `stable` ⇔ §7.14 says `safety_level: A` or `B` and the word is not deprecated.
- `experimental` ⇔ `safety_level: D`.
- `deprecated` ⇔ explicit deprecation flag.

A consistency test in `rust/src/interpreter/` should fail if `BuiltinSpec.stability` and the registry contract disagree.

---

## 6. Renderer responsibilities

| Renderer | Reads | Writes |
|----------|-------|--------|
| `lookup_builtin_detail` (Rust) | all LOOKUP-tier fields | plain-text string into the editor |
| Reference site word page (build-time) | all fields including `concept` | static HTML under `public/docs/words/` |
| `createWordButtonElement` title (TS) | `hover_summary` | `button.title` |
| `renderWordInfo` for builtins (TS) | `hover_syntax` | `elements.builtInWordInfo` |

User-word LOOKUP path is untouched; user-word hover continues to read `wordInfo.description` and the live source from `interpreter.lookup_word_definition`.

---

## 7. Reconciliation with `reference-writing-style.md`

The existing rule that LOOKUP output is **Markdown with Japanese permitted** is superseded for built-in words. After this model is adopted, that document should be amended:

- LOOKUP built-in output → UTF-8 English plain text (this document, §3.3).
- Reference site → Markdown, translations allowed.
- User-word LOOKUP → unchanged (still inserts the original source).

The phrase "`rust/src/builtins/detail-lookup-*.rs` の raw string literal" in the current writing-style doc is aspirational; those files do not exist yet. With this model the per-word LOOKUP text is constructed by `lookup_builtin_detail` from the generated registry entry's fields, not from per-file raw literals. The amendment should reflect that.

---

## 8. Implementation phases

Phase 1 — wiring (small, mechanical):

1. Rename `BuiltinSpec.short_description` → `hover_summary`; rename `BuiltinSpec.syntax` → `hover_syntax`. Update WASM serialization and TS callers.
2. Update `createWordButtonElement` and `renderWordInfo` callsites to consume the new field names.
3. Author `hover_summary` / `hover_syntax` strings per §4.3 for every existing built-in.

Phase 2 — LOOKUP body:

4. Add the LOOKUP-tier fields to `BuiltinSpec` (`role`, `syntax_forms`, `stack_effect`, `behavior`, `examples`, `failure`, `side_effects`, `modifier_interaction`, `related`, `stability`).
5. Replace the placeholder body of `lookup_builtin_detail` with a renderer that emits the §3.4 template from those fields.
6. Add the registry-vs-§7.14 consistency test (see §5.3).

> **Phase 2 implementation status (2026-07):** landed with one deliberate
> deviation from item 4. The file-size budget (§14.1) rules out growing
> `builtin_word_definitions.rs` by the authored content, so the authored
> fields (`behavior` / `examples` / failure note / `related`) live in a
> keyed side table, `rust/src/builtins/builtin_word_lookup_docs.rs`,
> mirroring the `module_word_docs.rs` precedent; an orphan-check test keeps
> it drift-free against `BuiltinSpec`. `failure` and `side_effects` are not
> authored at all — they are **derived at render time** from the §7.14
> contract metadata (`partiality` / `nil_policy` / `effects`), so every
> word renders the full template (Examples fall back to `hover_syntax`)
> and those sections can never contradict the registry. Authored entries
> exist for a first batch of core words (vector access, arithmetic,
> comparison, casts/text, control/HOF, dictionary words, and the §2.3
> Bubble-Rule set GET / DIV / NUM / CHR); remaining words render the
> derived template until authored. `syntax_forms` and
> `modifier_interaction` are still open.

Phase 3 — Reference site:

7. Add `concept: Option<&'static str>` to `BuiltinSpec`.
8. Build `public/docs/words/<NAME>.html` from `BuiltinSpec`.
9. Author the concept guide and developer-notes pages and link them from `public/docs/index.html`.
10. Amend `docs/dev/reference-writing-style.md` per §7.

Phases are independent enough to land in separate PRs. Phase 1 is safe to ship without Phase 2.

---

## 9. Non-goals

- This document does not redesign the editor, the dictionary panel, or the WASM API surface beyond field renames.
- It does not change user-word behavior in any layer.
- It does not require Reference to be complete before LOOKUP is improved; the placeholder body in `lookup_builtin_detail` is a separate problem solved by Phase 2 alone.
