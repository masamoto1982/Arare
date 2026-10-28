# Lexicon-emergence experiment

Tooling for the lexicon-emergence experiment: Core is held fixed, subject
agents solve shared task families through the Ajisai MCP server, and the
shared User dictionary passes from generation to generation through a
capacity bottleneck. This directory grades, classifies and reports; the
subject agents run as Claude Code subagents that paste inherited definitions
(route A, the pilot), as Claude Code subagents that run code through
`cli.mjs compute` (the cli route, Phase 2), or through the Claude API harness
in `lib/harness.mjs` (route B).

Nothing here defines Ajisai. Results are observation notes: the pilot's are
`docs/dev/lexicon-emergence-pilot-results-2026-09-23.md`, Phase 2's
`docs/dev/lexicon-emergence-phase2-results-2026-10-15.md`; each holds the
numbers from its run's `report.md`; the difficulty calibration's is
`docs/dev/lexicon-emergence-difficulty-calibration-2026-10-15.md`; Phase 3's
(68 subjects over generations 0–3, four selection conditions and a control
that may run code) is `docs/dev/lexicon-emergence-phase3-results-2026-10-15.md`,
whose appendix keeps the design it ran under. Phase 4's (52 subjects; two
training sets rotating by generation and three held-out families never
selected from) is `docs/dev/lexicon-emergence-phase4-results-2026-10-16.md`,
whose appendix likewise keeps its design. Phase 5's (32 new subjects; a
lexicon chosen by the number of task families reaching each Word, and the
same lexicon given to subjects who may not define Words of their own) is
`docs/dev/lexicon-emergence-phase5-results-2026-10-16.md`, with its design in
the appendix.
No run is kept under `runs/` (each run's
raw data was committed and then removed, and lives in git history);
the experiment's original work order and the Phase 3, Phase 4 and Phase 5 designs were removed from
`docs/dev/` the same way, and the hypotheses H1–H5 the work order named are
restated in the pilot results.

## Layout

| Path | What |
| --- | --- |
| `tasks/<family>.json` | Task families: prompts, inputs (the subject sees only the first), and a reference solution per task. `stats` and `text` are the pilot's; `series`, `prose` and `ledger` are Phase 2's, whose tasks share sub-computations and whose unseen inputs carry the edge cases; `sensor`, `orders` and `messy` (Ajisai's own traps) and `runs`, `number` and `brackets` (algorithms a total language must bound) come from the difficulty calibration |
| `lib/ajisai.mjs` | MCP client over `tools/mcp-server` — the grader uses the same `compute` tool as the subjects |
| `lib/source.mjs` | Tokenizing, bound-variable renaming, and expanding user Words into Core-only source |
| `lib/grade.mjs` | A solution is correct when every input leaves the reference's final stack; also runs the Core-only expansion (H5) |
| `lib/identity.mjs` | D0 (DIGEST), D0α (DIGEST after renaming bound variables), D1 (probe-battery fingerprint) |
| `lib/evolve.mjs` | Equivalence classes and the next generation's top-K lexicon, ranked by uses (`uses`, Phase 2's rule), first by how many subjects wrote the Word's class themselves (`reinvention`), first by how many distinct tasks reached it (`breadth`), or first by how many task families did (`families`); held-out results never count, and no rule gives a class two places |
| `lib/rotation.mjs` | Phase 4: the families a condition's generation solves — training families that rotate by generation, and held-out ones graded but never selected from |
| `lib/analyze.mjs` | H1, H2, H4, H5 over a run; correct solutions per generation and model with the mean number of runs; correct solutions token-identical to the reference; with held-out families, training and held-out results side by side, and per held-out family the correct solutions sharing a Word with their subject's own training solutions beside those reaching an inherited Word |
| `lib/prompt.mjs` | The single message a subject agent receives (route A: read SKILL.md, write a file, paste the inherited definitions; cli route: read SKILL.md, run files through `cli.mjs compute`, name inherited Words with `--words`, write a file; route B: the harness supplies SKILL.md and a `submit` tool, and the inherited Words come through `words`) |
| `lib/shuffle.mjs` | C-shuffled: a lexicon renamed to nonsense names, notes dropped, references between its Words rewritten |
| `lib/lexicon-phrasebook.mjs` | Route B: a generation's lexicon written as a phrasebook file with each Word's `#:digest`, served to that generation's subjects through the MCP server's `words` argument (`AJISAI_PHRASEBOOK`), and the check that a result's `prelude` matches it by content |
| `lib/harness.mjs` | Route B: one subject through the Claude API. Offers the MCP server's own `compute` / `check` / `word_contract` definitions plus `submit`, relays calls unchanged, caps turns and spending, records every request's served model, usage and cost |
| `test/` | The harness against a scripted model (no API call) with the real server and grader: `npm test` |
| `runs/<run>/design.json` | The families a run uses (or, in their place, a `"rotation"`: `{ "train": [[…], […]], "heldOut": […], "fixed": [<condition>, …] }`), the model behind each subject letter (`"models": { "A": "opus", … }`), optionally the selection rule (`"selectionRule"`) and how many times each subject may run code (`"callBudget"`: a number, or per condition `{ "default": 0, "runs": null }`; 0 forbids running at all, and the prompt says so; null is unlimited), and how many Words each subject may define (`"definitionBudget"`, shaped the same; the grader drops definitions past it and names them in `definitionsDropped`) |
| `runs/<run>/<condition>/gen<g>/` | `lexicon.json`, `submissions/`, `graded/`; with a lexicon, `phrasebook.ajisai` and `lexicon-digests.json`; for route B `transcripts/`; for the cli route `calls/<agent>.jsonl` (every run, its `--words` and its `prelude`) and `work/<agent>/` |

## Use

Requires `npm install` in `tools/mcp-server` and here (the SessionStart hook does both).

```sh
node cli.mjs prompt  <run> <condition> <gen> <agent>   # message for one subject
node cli.mjs grade   <run> <condition> <gen>           # after submissions are written
node cli.mjs evolve  <run> <condition> <gen> <K>       # writes gen+1/lexicon.json
node cli.mjs analyze <run>                             # runs/<run>/report.{md,json}

# The cli route (Phase 2) — the subjects are Claude Code subagents.
node cli.mjs prepare <run> <condition> <gen>                     # phrasebook for the generation's lexicon
node cli.mjs prompt  <run> <condition> <gen> <agent> --route cli # message for one subagent
node cli.mjs compute <run> <condition> <gen> <agent> <file> [--words A,B]   # what the subagent runs
node cli.mjs shuffle <run> bottleneck 1 shuffled 1               # C-shuffled from bottleneck's lexicon

# Phase 3 — several lexicon rules fed from one generation, earlier solutions sealed away.
node cli.mjs evolve  <run> bottleneck 0 8 --rule reinvention --into reinvention
node cli.mjs evolve  <run> bottleneck 0 Infinity --into open     # C-open: no capacity
node cli.mjs seal    <run> <condition> <gen>   # after grade and evolve: solutions leave the tree
node cli.mjs unseal  <run>                     # before analyze: everything comes back

# Phase 4 — design.json's "rotation" picks each generation's families; held-out results are not selected from.
node cli.mjs evolve  <run> base 0 8 --rule breadth --into breadth   # rank by distinct tasks reached

# Phase 5 — rank by task families reached; a condition with "definitionBudget": 0 gets the same lexicon.
node cli.mjs evolve  <run> families 1 8 --rule families --into probe

# Route B — needs ANTHROPIC_API_KEY in the environment and spends money.
node cli.mjs run   <run> <condition> <gen> G0A,G0B --model large --budget 10
node cli.mjs pilot <run> --model large --effort high --budget 40   # Phase 1's design end to end
```

`--model` takes a model id or `large` / `medium` / `small` (`claude-opus-5`,
`claude-sonnet-5`, `claude-haiku-4-5`); a model id with no rates in
`lib/harness.mjs` needs `--price <input>,<output>` in USD per million tokens,
and is refused without it. `--budget` is a positive ceiling in USD checked
before every request, counting the estimated cost of requests other agents
have in flight; a run that reaches it stops with an error and keeps what it
wrote. Server-side refusal fallbacks
are deliberately not enabled: a fallback would change which model answered,
which is the variable §5.3 controls. A refusal is recorded as the run's end.

## What the instrument can and cannot say

- **D0** equal means the same Word. It ignores Word names, even through
  dependencies. But it tells apart bodies that differ only in their `BIND`
  names.
- **D0α** renames bound variables to `_B0`, `_B1`, … before hashing, and
  merges those bodies.
- **D1** only means the engine gave the same answers on the probe battery in
  `lib/identity.mjs`. The battery includes scalars and one-element Vectors,
  because `[ 2 ] MUL` and `2 MUL` differ only on a scalar input, and irregular
  text (`''`, `'x  y'`, `' lead'`), because without it a word-splitting and
  a character-scanning title-caser answered alike (pilot, 2026-09-23). A Word that
  errors on every probe gets no D1.
- CONTRACT is not used as a level. For any body that uses `BIND`, it reads the
  bound names as unresolved Words and answers `inputs: variable`.

## Phase 2: what changed

The pilot's H5 counterexample spread because subjects pasted inherited
definitions into their source, and the Core-only expansion the grader audited
was not the program that ran. Since then each generation's lexicon is written
to `phrasebook.ajisai` beside it and served for `words`, and every result's
`prelude` names the Words it defined with the digest `lexicon-digests.json`
expects (`preludeMatches`). Route B does this through the API harness; the
cli route does it for Claude Code subagents, whose every run is logged so the
grader can count, per subject, the runs whose prelude was not the lexicon.
The `KEEP` Word behind the original counterexample has since been retired.

Phase 2 ran on the cli route on 2026-10-15 (Opus, Sonnet and Haiku subjects,
C-bottleneck, C-solo and C-shuffled). What it still could not do — subjects
from a model family other than Claude, which needs route B's harness taught
another provider and an API key for each — and what it found are in its
results note.
