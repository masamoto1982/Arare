# MCP evaluation harness

Status: non-canonical, design rationale (`[設計根拠]`). This page is the record
of how the MCP server (`tools/mcp-server/`) is evaluated against real models
and against itself — the corpus, the scorers, the captured baselines and the
budgets — moved out of the server's README, which is for someone connecting
to and using the server. Nothing here defines language semantics; `spec/` and
`SPECIFICATION.html` remain the only canon.

The commands, run from `tools/mcp-server/`:

```sh
npm run eval:validate      # corpus and trace contracts
npm run eval:performance   # latency and response-size budgets (eval/performance.json)
npm run eval:number-baseline
npm run eval:traces        # score the corpus answering itself
npm run eval:repairs
npm run eval:capture       # drive a real model (needs Anthropic credentials)
npm run eval:capture-repairs
```

`eval/cases.json` is the agent-evaluation corpus: 81 cases (61 positive, 20
negative), each asked in English and Japanese, so 162 prompts. Three of the
positive cases (`phrasebook-*`, added 2026-10-15) expect a `words` argument
naming a phrasebook Word, so they measure whether a model reaches for the
phrasebook rather than rewriting the phrase. `npm run eval:traces` scores the
corpus answering itself — `score-traces.js --reference` — which executes every case's expected
tool call against the real backend. It measures backend semantic correctness only; model tool
selection and source generation require captured model traces and are not claimed by this score.

Every case is bilingual because Ajisai is a Japanese-authored language with an
English tool surface, so "does a Japanese prompt reach the same tool with the
same source as its English twin" is a product question rather than a
translation detail. Both halves of a pair name the same task and therefore
share one expected tool and one expected result, which is what makes the
difference between their scores attributable to the language and nothing else.
The contract rejects a pair whose two sides are the same string: a copied
prompt still scores twice, and would report a comparison it never made.

`score-traces.js` accepts captured model traces in the documented reference
shape and reports tool-selection accuracy, first-attempt generation rate,
end-to-end semantic success, missing traces and irrelevant-tool rate — overall,
per language, and as a `languageGap` between the two. Selection and generation
are separate numbers because they have different repairs: a model that reaches
for the wrong tool with correct source has a tool problem, and one that reaches
correctly and writes source computing the wrong thing has a language problem.
Generation is rated over the positive cases only, since a case whose correct
answer is no call has nothing to generate. `positiveSelectionAccuracy` is there
for the same reason from the other side: `toolSelectionAccuracy` mixes the two
classes, so growing the negative set moves it without any behaviour changing.
Each score carries a `composition` block naming how many cases of each class it
was computed over — rates over one class survive a corpus that grows, rates over
both only compare within one composition.

A turn may hold several tool calls, and all of them are recorded and scored. A
model that looks a Word up and then computes has made one attempt containing two
calls, not a wrong choice — 91 of 130 turns in the first baseline did exactly
that, so keeping only the first call scored the lookup as the model's decision
and reported 0.323 selection accuracy where reading the whole turn reports
0.469. `reachedExpectedToolFirstRate` reports the stricter reading beside it,
without making instinct a pass criterion.

The selection reference fixture is built from the corpus in memory
(`score-traces.js --reference`) rather than committed. A perfect fixture is the
corpus answering itself with its own reference arguments, so maintaining 130 of
them by hand only meant that adding a case failed `--require-perfect` for a
reason unrelated to the scorer it asserts, and a generated copy only moved that
drift into a check.

Every trace document declares what produced it. `provenance.source` is either
`referenceFixture` — a trace built to pass the scorer, whose
perfect result describes the scorer and nothing else — or `model`, a real
capture, which must additionally record the model id, prompt-template digest,
tool-choice setting, capture time, and the server, engine and registry versions
it ran against. A document without that block is rejected rather than scored,
because the same numbers mean "the harness works" or "the model performs this
well" depending on an answer the file was not carrying. The scorers print the
provenance alongside the metrics, so a score copied out of a log still says
which it is.

`--require-perfect` is only valid on a `referenceFixture`. It asserts that the
scorer runs end to end; pointing it at a model trace would turn the first clean
run into a committed claim that the model is perfect, which is the one thing
this corpus is least entitled to say. A model trace is scored and reported,
never asserted.

**Model baselines have been captured** and are committed under `eval/traces/`:
`claude-opus-5-full-corpus.json` and `claude-opus-5-repairs-full-corpus.json`,
the full-corpus pair. The intermediate captures taken while the tool
descriptions were being tuned (baseline, after-syntax-rules, after-negatives,
after-entry-surface, and their repair counterparts) were superseded by that
pair and removed; git history holds them. `npm run eval:capture` drives a
real model over the server's tools — one
call per corpus case per language, `tool_choice: auto` so the irrelevant-intent
cases can correctly produce no call — and writes a
`model` trace under `eval/traces/`, kept apart from the committed fixtures so no
directory listing presents the two as the same kind of artifact. It resolves
credentials the way the Anthropic SDK does (`ANTHROPIC_API_KEY`,
`ANTHROPIC_AUTH_TOKEN`, or an `ant auth login` profile) and, finding none,
exits non-zero having written nothing. `capture-traces.test.js` exercises the
harness against a scripted client; it tests prompt assembly and tool-call
extraction, not a model.
`npm run eval:capture-repairs` captures the other half: for each repair case it
asks, executes the model's call against the real server, hands the whole
structured result back as a `tool_result`, and records the second attempt. Both
attempts are recorded as *calls*, never as outcomes — the scorer replays them
itself, because a capture that recorded its own verdict would be grading the
model with the code that produced its answer. A turn that calls nothing, or a
model that gives up after reading the diagnosis, is recorded rather than
dropped: a harness that could only capture the runs that went well would report
a repair rate computed over those.

`score-repairs.js` replays a failed attempt and its model-produced revision,
requires the expected structured diagnosis before the revision can count, and
reports diagnosis-observation and diagnosis-driven repair rates, per language.
It replays whichever tool the model chose, not only `compute`: `1 2 AD` through
`check` returns the identical diagnosis, so replaying one tool scored a model
that checked before running as never having seen a diagnosis at all.
The cases cover unknown Words, stack shape, malformed source and the
`collectionWork` ceiling — the last of these exists to make a claim testable:
a ceiling named for collections should send a repair at the collection, and its
source contains no arithmetic, so a repaired attempt that succeeds can only
have shrunk the collection. Their reference trace is
also a scorer fixture, not evidence of model performance.
When a refusal comes from a ceiling charged as the operation proceeds — the
collection scans — `diagnosis.resourceLimit` carries a `progress`
`{ completed, total, unit }` alongside `observed`. It exists because `observed`
cannot serve there: such a meter stops the instant the budget is crossed, so it
reads a hair over the limit however far over the request was, and a reader
taking it proportionally under-corrects wildly. `completed` is the size that
fits. Measured against a real model, adding it moved the diagnosis-driven repair
rate from 0.750 to 1.000 on the same corpus.

`eval:validate` rejects duplicate or unknown case IDs, unknown tools, malformed
JSON pointers and incomplete committed reference traces before scores are
calculated. This prevents malformed or selectively omitted traces from
silently producing plausible metrics.
`eval:performance` measures five post-warmup rounds over seven representative
compute, check, inference and registry cases. It reports p50/p95/max latency by
tool and fails when the overall p95 exceeds the committed one-second local
stdio budget. It also reports median and maximum response size — for the whole
result and for its text block alone — and fails against
`medianResponseBytesBudget`; response bytes are deterministic for a fixed
corpus and engine, so unlike the latency figures that gate is exact and
reproducible. The budget is a ceiling against accidental growth, and a field
added on purpose raises it by what that field costs and no more: 1,100 became
1,140 bytes on 2026-10-15 for `specificationVersion`, which a result carries
once (an earlier draft carried it twice, in the envelope and in `mcp`, and the
budget refused that). The latency measurements describe this adapter and machine, not
remote service latency.
`eval:number-baseline` compares canonical results for five selected rational,
decimal and integer operations with JavaScript `Number`. It includes two
exactly representable controls as well as known precision-sensitive cases, and
labels its scope explicitly; it is not a general JavaScript or CAS benchmark.
