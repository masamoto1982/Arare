# Ajisai Authoring & Notation Style

The house style for **all authoritative writing about Ajisai** — the specification, the Reference, and any prose that refers to Ajisai or to the mathematics behind it. (Working name; this is the document previously sketched as 「仕様書の仕様」, broadened beyond the specification.)

## Authority

- **Non-canonical.** This document governs **how** Ajisai is written about, not **what** is true of it. It defines no language semantics.
- **Canonical source remains `SPECIFICATION.html`.** If this document ever appears to constrain meaning, the specification wins.
- **The authority model itself is specified.** LANG.AUTHORITY.SOURCES names the authoritative sources and what each is authoritative for, and LANG.AUTHORITY.PRESENT requires the three reading surfaces to describe the language as it currently is; this document supplies the notation rules that model requires. (The specification publishes stable `LANG.*` clause ids, so a citation survives reorganization — the numbered sections this document used to cite are gone.)
- **Scope:** the specification, the README (`README.md`), the Reference (`public/docs/`), and any other authoritative text that names Ajisai words, symbols, or formulas.
- Sibling conventions: `docs/dev/reference-writing-style.md` (the Reference site and `?`/LOOKUP text) and `docs/dev/three-layer-documentation-model.md` (user-facing guidance structure). This document is the shared notation discipline both of those, and the specification, adhere to.

## 1. Why this style exists

The essence of Ajisai is not source code in any one language. It is the **written-down mathematics, and the specification that contains it**: implement faithfully from the specification and the same programming experience follows. The notation is therefore part of the artifact, not decoration around it. If a formula or a word is presented inconsistently — or, worse, ambiguously — the specification stops being a reliable blueprint.

Two consistency problems dominate, and the rest of this document addresses them:

1. **Ajisai symbols look like punctuation.** Ajisai source is UTF-8 text, and every symbol token that Ajisai defines is a word.
2. **Mathematics and Ajisai code share glyphs.** `/`, `(`, `)` mean one thing in a formula and another in Ajisai source: `/` is part of a fraction literal there, and `(` `)` are ordinary name characters.

## 2. The symbol-as-word hazard

Every Word has one name, spelled in letters, and no symbol spelling resolves to it (LANG.SOURCE.NORMALIZE): `+`, `/` and `>=` are not Words — written in source they are names nothing defines, and the Unknown Word diagnosis says which Word to write instead. One registry holds every mark that does lexical work: `SURFACE_FORMS` in `rust/src/surface_forms.rs`, from which `docs/word-manifest.json` takes its `surface_forms` count. Everything in it is a mark that must carry the gray background of Section 3:

| Symbol | Concept | Kind |
|--------|---------|------|
| `[` `]` | `BEGIN-VECTOR` `END-VECTOR` | Structural delimiter, not a Word |
| `'` | `STRING-QUOTE` | Literal delimiter, not a Word |
| `#` | `COMMENT-LINE` | Source directive, not a Word |

Four lexical forms and nothing else: **a mark not in that table is ordinary punctuation in prose, and an ordinary name character in source.** The table once held ten symbol aliases of Words (`+` for `ADD`, `/` for `DIV`, `>=` for a comparison Word) and a Record delimiter pair; all of them are gone, and so are the target and consumption modifiers spelled `.` `,` `..` `,,`. `(` `)` and a bare `|` are ordinary name characters too (the per-character rejection rules were removed from `spec/grammar.json` in 2026-09; `check:grammar` holds the tokenizer to the grammar).

What remains is small and sharp. `[` `]` `'` `#` do structural work no Word does, and `/` still matters in one place: inside a number literal (`1/2`, `1/0`) it is part of the number, so a `/` set in the gray code channel reads as a fraction, never as a division bar. Those are the marks the channel rules protect.

## 3. Primary technique: the gray code background, reserved for Ajisai

The existing writing already does the right thing: whenever it names a symbol meaningful to Ajisai, it wraps that symbol in inline code, which renders with a **light gray background**. That background is the signal — it says "this mark is a word, not punctuation." It is an excellent, low-cost device, and it is **mandatory**:

> Every token meaningful to Ajisai — every word, symbol, modifier, literal, or snippet — is wrapped in `` `…` `` (inline) or a fenced block when it appears in prose. A bare `,` or `[ 1 2 3 ]` never sits in running text.

Crucially, the gray background is **reserved for Ajisai**. Mathematics does not borrow it (Section 4). Keeping the channel exclusive is what makes a gray `1/2` unambiguously an Ajisai number literal rather than a division to carry out.

## 4. Mathematics is a separate channel

Because a formula and an Ajisai snippet share glyphs (`/` is a division bar in a formula and part of a fraction literal in a snippet; `(` `)` are grouping in a formula and ordinary name characters in a snippet), the two must travel in **visibly different channels**, and the channel decides the reading:

| Channel | How it is set | A `/` in it means |
|---------|---------------|-------------------|
| Ajisai code | inline `` `…` `` (gray) or a fenced Ajisai block | part of a number literal (`1/2`); the Word that divides is `DIV` |
| Mathematics | LaTeX typeset by KaTeX on the HTML surfaces (Section 4.1); Unicode text math on Markdown surfaces (Section 4.2) | the division operator |

Rules for the mathematics channel:

1. **Do not put mathematics in the gray Ajisai code span.** That background belongs to Ajisai tokens; sharing it destroys the signal of Section 3.
2. **Use the delimiters of the surface.** The HTML surfaces typeset LaTeX through KaTeX using backslash delimiters — `\(…\)` inline, `\[…\]` display (Section 4.1); `\` is not an Ajisai token, so those delimiters collide with nothing. Markdown surfaces may use the `$…$` / `$$…$$` delimiters their renderers support (Section 4.2): `$` is not an Ajisai token either, so that pair is equally free.
3. **Set display formulas off from prose** in their own block, the way the specification writes the continued-fraction forms.
4. **A glyph shared by both channels is disambiguated by channel alone.** When the same expression is shown both as mathematics and as the Ajisai that realizes it, present them as two distinct things — for example a math display beside an Ajisai snippet, or two columns — never blended into one run of text.

The goal is that a reader (human or machine) can always tell, from presentation alone, whether a `/` is the operator or the word.

### 4.1 LaTeX via KaTeX on the HTML surfaces

The specification (`SPECIFICATION.html`) and the Reference (`public/docs/`) typeset their mathematics channel as **LaTeX rendered by KaTeX**. The essence of Ajisai is the written-down mathematics (Section 1); presenting continued fractions, intervals, and partial-quotient formulas as flat Unicode text undersells exactly the part of the artifact that matters most. Rendered LaTeX also *strengthens* the channel separation of this section: a typeset formula is unmistakably not a gray Ajisai code span and not prose. Use it actively — whenever a formula, a variable, an interval, or a set would otherwise be hand-built from Unicode glyphs and `<em>` italics, write it as LaTeX instead.

| Aspect | Rule |
|--------|------|
| Delimiters | inline `\(…\)` · display `\[…\]` — nothing else |
| Dollar delimiters | not configured on the HTML surfaces; one delimiter pair per surface keeps scanning deterministic |
| Library | KaTeX with its auto-render extension, **self-hosted** at `public/vendor/katex/` — no CDN, matching the repository's no-external-services stance |
| Refreshing the vendored copy | `npm run vendor:katex` (copies from the `katex` devDependency; woff2 fonts only) |
| Ajisai channel exclusion | auto-render is configured with `ignoredTags` covering `code` and `pre`, so the gray Ajisai channel is structurally unreachable by math rendering — the channel separation is enforced by tooling, not just by discipline |
| Irrational displays | the stack writes an irrational's normal form as one token, `1/1+sqrt(2)` (LANG.OBSERVATION.PROTOCOL): that is stack text, not mathematics, so it stays in the Ajisai channel and a formula about it (`\(1+\sqrt{2}\)`) goes in this one |
| Variables and names | single letters italic by default (`\(a_0\)`, `\(b_i\)`, `\(\varepsilon_i\)`); multi-letter names as `\mathit{…}` (`\(\mathit{num}/\mathit{den}\)`) |
| No-JS degradation | without JavaScript the raw `\(…\)` LaTeX source remains visible; this is acceptable — LaTeX source is itself a precise, AI-readable notation |

The LaTeX source inside the HTML is part of the artifact: it must state exactly the mathematics intended, because machine readers consume the source, not the rendering.

### 4.2 Markdown surfaces use GitHub math

The README and the `docs/dev/` working documents are Markdown and get no KaTeX runtime, but GitHub renders `$…$` (inline) and `$$…$$` (display) LaTeX natively. Use it: the dollar sign is not an Ajisai token, so there is nothing to collide with. Two cautions remain:

- **Code spans win.** An Ajisai snippet is always inside `` `…` ``, where Markdown math does not reach — the channel rule of Section 3 already guarantees this.
- **Unicode text math is still acceptable** for a stray `≤` or `√` in running prose where a full math span would be noise. Heavy mathematics belongs on the HTML surfaces anyway (three-layer model); if a Markdown document accumulates formulas, that is a sign the content belongs in the specification or the Reference.

## 5. Tables for enumerable structure

A table boundary is structural, not textual: nothing inside a cell has to be re-parsed as a separator. Reach for a table when the content is a set of rows sharing one shape:

| Content shape | Columns |
|---------------|---------|
| Word and its sugar and role | `Canonical` then `Sugar` then `Role` |
| Property across an axis | `Subject` then `Value` then `Notes` |
| Category membership | `Category` then `Members` |
| State transition | `From` then `Event` then `To` |
| Mapping / correspondence | `Source` then `Target` |
| Worked example | `Sample code` then `Expected value` then `Notes` |

Keep paragraphs for the definition of a single concept, the rationale behind a rule, and any reasoning that does not decompose into uniform rows.

## 6. Rules

1. **Mark every Ajisai-meaningful token as code** so it carries the gray background (Section 3). Non-negotiable baseline.
2. **Keep mathematics in its own channel** (Section 4); never give it the gray Ajisai background. On the HTML surfaces, typeset it as LaTeX via KaTeX with `\(…\)` / `\[…\]` (Section 4.1); on Markdown surfaces, GitHub's `$…$` math is available (Section 4.2).
3. **Never use a bare Ajisai token as prose punctuation.** A mark in Section 2's table (`+` `-` `*` `/` `%` `=` `<` `<=` `>` `>=` `[` `]` `{` `}` `'` `#`) appears only as marked-up code, never as the separator, bullet, or delimiter of running text. A mark outside that table is ordinary punctuation and needs no marking — which is a fact to check against the registries, not to recall.
4. **Promote an inline list of three or more code tokens to a table.**
5. **One concept axis per column.**
6. **Do not encode results with inline comment arrows.** `# → [ 1 ]` blurs code and result; use separate columns or blocks.
7. **A table or notation change preserves meaning exactly.** It is presentation only, never semantic.

## 7. Worked example

The hazard, then the fix.

**Before** — an inline list long enough that its separators do the structural work:

> The logic words are `AND` / `NOT` / `SELECT` / `RECONCILE`; the higher-order words are `EXEC` / `MAP` / `FILTER` / `FOLD` / `SCAN`.

Every separator there is a `/`, the glyph of a fraction literal, sitting unmarked between marked tokens — the one reading a gray background cannot help with, because the slash carries no background of its own. A comma would not have this problem (Section 2), and would still leave nine tokens for the reader to group by eye.

**After** — the same content as structure:

| Category | Members |
|----------|---------|
| Logic | `AND` and `NOT` and `SELECT` and `RECONCILE` |
| Higher-order | `EXEC` `MAP` `FILTER` `FOLD` `SCAN` |

The cell boundaries carry the separation, and nothing ambiguous is left adrift between two words.

## 8. In-cell separators

When a single cell must hold more than one token, separate them with something that is **not** Ajisai surface syntax:

- spelled-out **"and"** / **"or"** — preferred, because it always renders; or
- a middle dot `·`, which is not an Ajisai token; or
- a single space between adjacent code spans (`` `MAP` `FILTER` ``).

Never separate in-cell tokens with `/`: it is the glyph of a fraction literal, so it reads as part of the program. Never use `|` either — not because the language claims it (it does not; it is an ordinary name character) but because it is the Markdown table delimiter, and a cell separator that ends the cell is its own kind of wrong. `,` is no longer an Ajisai token, so it is no longer forbidden here; the preferences above still stand on their own, because a spelled-out "and" survives every renderer and a comma between two code spans still reads as a list of two things rather than one phrase.

## 9. Surfaces and required formats: the specification is HTML, the README is Markdown

**The specification is mandatorily authored in HTML, not Markdown.** The canonical specification is `SPECIFICATION.html`; authoring it as Markdown is a style violation. The reason for the HTML requirement is **not** visual decoration: HTML makes the structural tools this style depends on — tables for enumerable structure (Section 5), and embedded diagram sources such as PlantUML — directly usable, where Markdown keeps them second-class. The pages stay plain and unadorned, in the same sober styling the Reference uses. GitHub's repository view shows HTML files as source, so the **reading surface for HTML documents is the Pages site** (the build copies `SPECIFICATION.html` into the deployed site next to the Playground).

**The README is the one mandatory Markdown surface.** `README.md` is the repository's front door, and GitHub renders only Markdown there — an HTML README would greet visitors with source code. The README therefore stays GitHub-flavored Markdown and acts as the entry point that links to the rendered documents: the specification (`https://masamoto1982.github.io/Ajisai/SPECIFICATION.html`), the Reference, and the Playground.

Each surface applies this style with its own tooling:

- **Specification** (`SPECIFICATION.html`) — hand-authored HTML. The gray background is supplied by the page's `code` styling; enumerable structure lives in `ref-table` tables; mathematics is LaTeX rendered by the self-hosted KaTeX (Section 4.1). When editing a section for any reason, promote its inline token lists to tables and its Unicode text math to LaTeX; do not renumber sections or restructure headings solely to insert a table.
- **README** (`README.md`) — GitHub-flavored Markdown. Inline code gives the gray background; tables are native. Worked examples live in sample tables (sample code, expected value, notes), never as code comments with inline result arrows. Tech-stack badges are homemade, uniform SVGs kept in the repository (`docs/assets/badges/`) — no borrowed third-party badge images.
- **Reference** (`public/docs/`) — hand-authored HTML. The gray background is supplied by the page's `code` styling, and examples already live in tables (sample code, expected value, notes); mathematics is LaTeX rendered by the self-hosted KaTeX (Section 4.1). New pages follow the same channels.

Representative specification candidates for table promotion (illustrative, not exhaustive): the Core-word category lists, the Word enumerations in LANG.FAILURE.PASSTHROUGH, and any sentence that names four or more Words in a row.

Legacy references to `SPECIFICATION.md` in older commits and archived notes are obsolete; they denote the same document, now `SPECIFICATION.html`. A short-lived `README.html` existed during the 2026-06 migration and is likewise obsolete; the README is `README.md`.

## 10. Relationship to the other style documents

| Document | Layer it governs |
|----------|------------------|
| This document | The shared notation discipline for all writing about Ajisai |
| `reference-writing-style.md` | Reference site and `?`/LOOKUP help text |
| `three-layer-documentation-model.md` | Structure of all user-facing guidance |
| §11 of this document | The same discipline read backwards: which characters the lexicon may take, given the ones this prose relies on as separators |

All of them share one root principle: **Ajisai code, the mathematics behind it, and the prose about both must be visually and structurally distinct, so that a symbol is never mistaken for punctuation and an operator is never mistaken for a word.** The gray code span marks each Ajisai token; a separate math channel carries the formulas; tables separate them in bulk.

## 11. Which characters the lexicon may take

The discipline above read backwards. §2 and §8 say prose must avoid the
characters the language uses; this section says the language must avoid
the characters the prose uses as separators, or the loss goes unrecorded.
(Moved here from the 2026-09-22 character-allocation memo when that memo was
retired; the criterion is current.)

**Criterion.** Before a character is assigned a lexical role, check whether
it is already *working as a separator* on the writing side — the README, the
Reference, the Specification, and the generated surfaces (`SKILL.md`, the MCP
quickstart). If it is, the assignment trades the language's readability for
the documents'. Consuming a free character and taking a working one are
different decisions: the first spends a slot, the second is an exchange, and
an exchange has to say what it gives up before it is made.

The loss shows up in three shapes:

- **A gate needs an exception.** Ajisai's reading surfaces are executed
  (`check:reference-samples` runs every Reference sample; the SKILL.md
  generator runs every embedded fragment). When a fragment in prose cannot be
  told from source by a machine, the gate grows an exclusion — the Record
  section's `{ key value … }` schematics are already identified by their
  ellipsis and skipped. The more lexical and prose separators overlap, the
  more of these exclusions accrue.
- **`:` is already at work in Ajisai's own tooling.** `#:contract` (declared
  contracts) and `#@` (test directives) are both a mark after `#` that speaks
  to a tool. Lexically still comment text, but by convention the colon has a
  job.
- **`label: value` is a shape this style guide prescribes.**
  `structured-prose-style.md` names it as one of the translation-stable
  information shapes, and the Japanese Reference uses the full-width colon
  the same way.

Two cases fixed the criterion. `{` `}` had no writing-side use when the
per-character rejection rules were removed from the grammar, so they were
*taken*, for the Record display of the time (since replaced by the
`[ keys ] [ values ] RECORD` phrase, which is source and takes no mark of
its own, so `{` `}` are plain name characters again). `:` was at work (the two shapes above), so
it was *left alone* — the owner's decision, reinforced by the small gain on
offer: a quote closes only before whitespace, so `{ 'x': 1 }` could never be
written and the best available form was a floating `{ 'x' : 1 }`; the
documents' cost would have been paid without buying the shape it was paid
for.

What the criterion does not say: a commonly used mark is not permanently
untakeable — ask first whether the writing side can move that shape to
another information shape (a table, a sentence); if it can, move it first,
then assign. Writing-side use is not the only test either — a lexical rule
may refuse the form for its own reasons (for `:`, the string-closing rule),
so check both. And a taken character enters `SURFACE_FORMS`
(`rust/src/surface_forms.rs`), which `check:grammar` reconciles against the
word manifest, exactly as before; whole-lexeme rules are what the grammar
has, and reviving a per-character rule would be a separate discussion.
