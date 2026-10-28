# Agent CLI output contract (`ajisai agent`)

Status: implemented host contract. This document is not an authority for
language semantics; `SPECIFICATION.html` remains that authority. It documents
only commands and fields emitted by the current native CLI.

## Commands

```text
ajisai run <file.ajisai> [--step-limit <N>]
ajisai check <file.ajisai>
ajisai contract <file.ajisai>
ajisai agent <compute|check|infer-contracts> <file.ajisai|-> [--limits <agent|trusted>] [--step-limit <N>] [--compact]
ajisai test <file-or-dir> [--json]
ajisai repl [--json]
ajisai version [--json]
```

No other command or option is part of the current CLI contract. In particular,
this document does not reserve planned commands.

`agent` accepts `-` in place of a path and reads the program from standard
input, byte for byte, with nothing appended. An embedding host that already
holds the source needs no temporary file, no writable temporary directory, and
leaves no program on disk for the duration of the call.

| exit | meaning |
|---:|---|
| 0 | success |
| 1 | Ajisai language, check, contract, or test failure |
| 2 | CLI usage or host file-reading failure; JSON is not guaranteed |

`run`, `check` and `contract` are the human-readable forms of
`agent compute --limits trusted`, `agent check` and `agent infer-contracts`,
and take no `--json`: the machine form of an operation is its `agent`
operation, and there is one. (`run` executes under the trusted ceilings;
`agent compute` applies them only when asked. Both forms of `check` verify
`#:contract` declarations.) A flag a command does not read is a usage error
(exit 2), never silently ignored: `agent` always writes JSON and takes no
`--json`. `agent` (and `test`/`repl`/`version` with `--json`) write one JSON
document to stdout.
Program `PRINT` effects are captured in the document rather than mixed into
stdout.

`--limits` chooses the resource ceilings `agent compute` runs under: `agent` (the default) is the tighter profile
for untrusted, generated programs (`agent::api::LOCAL_AGENT_RUNTIME_LIMITS`);
`trusted` is the interpreter default that `run` uses. `--step-limit` is a
positive integer and applies to `run` and `agent compute`;
the default is the profile's own budget: 100,000 under `agent`
(`agent::api::LOCAL_AGENT_EXECUTION_STEPS`, the budget the MCP server
applies) and the host's derived budget under `trusted`
(`interpreter::DEFAULT_MAX_EXECUTION_STEPS`, currently 12,180,000 — see
`docs/dev/mcp-host-profiles.md`, re-derived per-container and not a value to
hard-code elsewhere).

`agent` writes its document indented for a reader by default. `--compact`
(only with `agent`) writes the same document as compact JSON with no trailing
newline: byte for byte what the WASM entry points (`agent_compute` and its
siblings) return. A host that caps a response by its size, as the MCP server's
`responseBytes` does, passes `--compact` so that the native and WASM backends
measure the same bytes and reject at the same result.

## `agent compute` and `agent check`

Both operations emit schema version 5:

```json
{
  "schemaVersion": 5,
  "specificationVersion": "1.0.0-beta.3",
  "status": "ok",
  "stack": [],
  "stackDisplay": [],
  "output": [],
  "message": null,
  "error": null,
  "resourceUsage": {},
  "contractDecls": null,
  "stackElided": null
}
```

`specificationVersion` is the version of the specification the engine
implements, read from `spec/version-lock.json` when the Word registry was
generated; `spec/CHANGELOG.md` says what changed in each. During the beta a
program's answer can change when this field does, so a stored result that
names it can be explained later. `agent infer-contracts`, `test --json` and
`version --json` carry it too.

`status` is `ok` or `error`. Ajisai language errors use exit 1 and retain this
JSON envelope. `check` tokenizes, checks delimiter structure, performs
best-effort static Word resolution, and never executes the program.
`contractDecls` contains the conservative result of comparing `#:contract`
declarations with inferred contracts. `check` also reads the block written
immediately before MAP, FILTER, FOLD or SCAN, at top level or in a top-level
definition's body: when its inferred arity cannot leave exactly one value
from the one (MAP, FILTER) or two (FOLD, SCAN) values its Word gives it, the
answer is the error running it raises — `blockContractViolation`, or
`stackUnderflow` for a block that reads below its frame — with `word` the
higher-order Word and `line`/`column` where it is written. A block of
`variable` arity is not judged. `compute` does not refuse such a program:
over an empty or absent collection the block never runs.

An error carries `message` and one `error` object, the whole of what
LANG.OBSERVATION.DIAGNOSIS asks an ERROR to carry:

```json
{ "category": "resourceLimitExceeded", "resource": "numericWork", "word": "MUL", "line": 1, "column": 25 }
```

`category` is the `spec/outcomes.json` error category; `resource` names the
ceiling a `resourceLimitExceeded` crossed, a key of the host's limit profile;
`word` is the Word the failure is attributed to — the innermost Word that
raised it, or the User Word whose body did, since from the caller's side that
Word is what failed; `line` and `column` are the top-level token that reached
it. A key with nothing to say is absent. Schema version 3 carried `diagnosis`,
`aiDiagnostic` and `errorFlowTrace` here; they were removed in 2026-10, with
their next-check suggestions and spelling candidates. What survives of them
is text: a `message` may end in ` — ` and a one-line repair
(`rust/src/diagnostic_hint.rs`) — for an unknown word, a runnable phrase or
the Core names within a typo of it; for a stack underflow or an operand of
the wrong domain, the Word's own `documentation.syntax` example; for a known
misreading of a Word (two whole Vectors of different lengths given to
`SELECT`), what was meant. It is part
of the message, which may evolve, and nothing branches on it.

`compute` runs the same declaration check **before executing anything**
(LANG.CONTRACT.CHECK is a pre-execution check, and `compute` is where
execution happens): a source that carries a `#:contract` directive gets
`contractDecls` in its envelope whatever the run does, and a declaration
inference disproves stops the run — `status: error`, `outcome:
error:contractViolation`, nothing executed and nothing printed. A source with
no directive carries no `contractDecls` and is untouched. A violated
declaration is reported in the shape every other error has, by `check` and
`compute` alike: `message` ("Contract declaration violated: …") and `error`
with `category: contractViolation` and `word` naming the declared Word.

`compute` also carries a top-level `outcome` string: which of LANG.FAILURE's
three results the run produced, as a `spec/outcomes.json` id. `status` alone
folds a reasoned absence into `ok`; `outcome` keeps the three apart
(`Report::outcome_id` in `rust/src/agent/report.rs`):

| run | `status` | `outcome` |
|---|---|---|
| top of stack is not NIL (an empty stack included) | `ok` | `value` |
| top of stack is NIL | `ok` | `nil:<reason>`, the top node's `semantics.absence.reason` |
| a language error | `error` | `error:<category>`, `error.category` |

The field is absent when the run cannot be classified (a NIL with no reason,
an error naming no category) rather than carrying a guessed id, and `check`
never carries it: it does not run the program.

Native `run` obtains this document from the typed, source-only Rust
`agent::api::compute` boundary. Terminal formatting is a consumer of that
report and is not part of computation semantics.
JSON `check` likewise consumes `agent::api::check`; human-readable check
output remains a terminal-only projection.
`agent::api` (`rust/src/agent/`) has no filesystem or terminal I/O and
compiles for `wasm32` as well as native, so the WASM one-shot entry point
(`agent_compute` in `rust/src/wasm_interpreter_bindings/mod.rs`, consumed by the MCP
adapter's `worker_threads` backend) renders the identical envelope; the
native CLI (`rust/src/cli`) is a thin file/terminal adapter over the same
module.

### `contractDecls`

```json
{
  "findings": [ { "severity": "error", "message": "`#:contract W`: declared `inputs=2` but inferred `inputs=1`." } ],
  "outcome": "error",
  "declarations": [ { "word": "W", "outcome": "error" } ]
}
```

A directive declares only `inputs=N`, `outputs=N` and `field=closed|leaving`;
any other term makes it malformed, which is a violation. Each declaration's
`outcome` is the check-time trichotomy (LANG.CONTRACT.CHECK): `value` when
verified, `nil` when the inference cannot verify it (a `note` finding — a
`variable` arity, a `leaving` resting only on code the walk could not read, a
Word defined twice), `error` when violated. The top-level `outcome` is the
worst of them.

### What the run cost, and what the runtime did

Two objects, because they answer different questions.

`resourceUsage` is **what this run spent of the budgets that could have refused
it**. Every key names a key of the host's declared limit profile and carries the
same number the ceiling compared against — read from the counter the check
reads, not a parallel copy — so an agent can subtract one from the other and
know what it has left.

```json
{ "executionSteps": 22, "numericWork": 20 }
```

The accumulating ceilings always appear. A per-result ceiling has no running
total, so what it can report is the largest result it measured: `algebraicTerms`
is the largest term count any result of the run reached — the number that
ceiling compared — and is present only when some result was an irrational.
`bigintBits` is not reported. Nothing here is a number nothing measured, which
is the defect this object exists to fix. A Word whose cost grows with that
term count marks the axis `valueDependent: "algebraicTerms"` in its registry
entry, set by `scripts/check-value-dependent-cost.mjs` from measurement.

How the runtime went about the work — which cache answered, which fast path
fired — is not reported. Those optimizer counters (`runtimeMetrics`) were
removed in 2026-10: they described no fact about the program, and keeping every
fast path's count equal to the plain route's cost code and time on each one.


### A report that cannot afford its stack

An error report carries two different things. The **error** is the answer:
why the program stopped, and where. The **stack** is residual
state — whatever the program happened to be holding at the time. When the two
together exceed what a host will accept, sending the residue and losing the
answer is the wrong trade.

So on `status: "error"`, slots whose values do not fit a fixed 64 KiB budget
are replaced in place: `value` becomes `null`, `type` and `semantics` still say
what the value was, and an `elided` record says what was dropped.

```json
{
  "type": "vector",
  "value": null,
  "semantics": {},
  "elided": { "reason": "errorStackBudget", "approxBytes": 8977832, "elements": 100000 }
}
```

The envelope repeats it at the top level as `stackElided`, so one field answers
"was anything dropped":

```json
{
  "reason": "errorStackBudget",
  "budgetBytes": 65536,
  "slots": [ { "index": 0, "approxBytes": 8977832, "elements": 100000 } ]
}
```

A **successful** result under the agent profile is elided the same way, with
`reason: "valueStackBudget"` and the profile's own budget
(`agent::api::AGENT_STACK_BUDGET_BYTES`, 440 KiB — sized so the MCP response,
which carries the envelope twice, stays under its 1 MiB `responseBytes`). A
success *is* its stack, so the budget is generous and the trusted profile
(`run`, `agent compute --limits trusted`) sets none and sends every success
whole. It exists because the alternative was worse: an oversized success was
refused outright as `responseTooLarge`, which told the caller that its answer
was too big and nothing else — not that it had left a 7,000-element
intermediate on the stack, not that the two small values beside it were fine.
Elided, the values that fit arrive whole and the one that does not arrives as
the record above, which is what lets the caller fix the program.

`approxBytes` is the size the slot would have serialized to — `stack` node and
`stackDisplay` string together — estimated from the value without building the
text, and calibrated to within a few percent of the real rendering (the first
estimate charged a flat envelope per element and put `0 99999 RANGE` at "~33
MB" for a stack that serializes to 8.6 MB).

Four rules make this safe to rely on.

- **The reason names the budget.** `errorStackBudget` is the fixed error
  budget; `valueStackBudget` is the agent profile's success budget, and a host
  that sets none never sees it.
- **Values are dropped, never reasons.** `error`, `message` and `output` are
  never elided by the stack budget.
- **Slots keep their index.** A dropped slot is replaced, never removed, so
  `stack` and `stackDisplay` stay the same length as the real stack and a
  reader that points at stack depth still points at the same thing.
  `stackDisplay` carries a matching `<elided …>` marker, so a text-only reader
  learns the same facts.
- **An ordinary result is untouched.** The error budget is 64 KiB — one
  sixteenth of the MCP adapter's 1 MiB `responseBytes` ceiling; a 5,000-element vector of
  small integers (433 KB) still fits the success budget. For both,
  `stackElided` is absent and nothing changes byte for byte. It appears as `null` here and is omitted entirely from the MCP
  envelope, which drops null top-level fields.

Distinguishing an elided slot from a genuine `NIL`: a `NIL` has
`type: "nil"` and carries `semantics.absence`; an elided slot keeps the real
domain in `type` and carries `elided`.


### Stack value nodes

Stack nodes are produced by the same Rust value-protocol mapping as the WASM
playground boundary:

```json
{
  "type": "number",
  "value": { "numerator": "3", "denominator": "2" },
  "semantics": {}
}
```

Arbitrary-precision integers are decimal strings, never JSON floating-point
numbers. Vectors contain arrays of value nodes. NIL carries its absence in
`semantics.absence`: `reason`, and the `detail` text a `userDeclared` reason
carries — nothing else. UNKNOWN is a NIL and is serialized as one; a
Boolean carries `semantics.truthValue`. Every field of a node is derived from
the value itself, never from the Word that produced it.

An algebraic irrational retains the approximate rational compatibility view,
marks it with `approximate: true`, and carries its authoritative multiquadratic
normal form:

```json
{
  "type": "number",
  "value": { "numerator": "768398401", "denominator": "543339720" },
  "semantics": {
    "approximate": true,
    "exactTerms": [
      { "numerator": "1", "denominator": "1", "radicand": "2" }
    ]
  }
}
```

`exactTerms` encodes `Σ (numerator / denominator) √radicand`. When it is
present, the `value` rational is a display compatibility view and is not the
canonical value.

`stackDisplay` writes the same normal form as one token — `sqrt(2)`,
`2/1*sqrt(2)`, `1/1+sqrt(2)`, `sqrt(2)-sqrt(3)` — exact and never truncated. It
is a display: read it, compute with `exactTerms`. It renders the stored normal
form, and a number has one normal form (radicands are square-free), so two
values `EQ` calls equal are written alike: `8 SQRT` and `2 SQRT 2 SQRT ADD` both
display `2/1*sqrt(2)`. There is no separate short-rendering
field: an `exactDisplay` once carried this string when `stackDisplay` was a
truncated continued fraction, and was removed once the two became the same.


## `contract`

`agent infer-contracts` returns, under `contracts`, one entry per user Word
(`contract` prints the same entries for a person):

```json
{ "name": "MEAN", "inputs": 1, "outputs": 1, "field": "leaving", "fieldExits": ["DIV"], "suggested": "#:contract MEAN inputs=1 outputs=1 field=leaving" }
```

`inputs` and `outputs` are counts, or `"variable"` where the walk cannot state
one (a Word of variable arity, a name nothing defines); `field` is
`"closed"`/`"leaving"` (LANG.CONTRACT.FIELD) with `fieldExits` naming where a
`leaving` body leaves, in body order; `suggested` is a paste-ready declaration.
Inference registers definitions without executing their bodies. Source that
does not tokenize or balance its vector delimiters is answered exactly as
`agent check` answers it — `status: "error"`, exit 1, `error.category:
"malformedSource"` — rather than as a success with no contracts. The entries
are produced by `agent::api::infer_contracts`, so native and embedded hosts
(the WASM one-shot entry point included) share one inference.

## `test`

`test` evaluates files against `#@` directives for expected status, stack,
output, and error text. With `--json`, it emits a test-run report and exits 1 if
any case fails. Test directives are host tooling comments and do not change
Ajisai program semantics.

## `repl`

The REPL preserves stack and definitions across lines. In JSON mode, every
submitted program line produces one small JSON document of its own —
`{ "status", "stackDisplay", "output", "message" }`, not the `agent` envelope. REPL
meta-commands (`:help`, `:stack`, `:reset`, and `:quit`) are host commands, not
Ajisai Words.

## `version`

`version --json` emits:

```json
{ "schemaVersion": 5, "status": "ok", "version": "1.0.0-beta.3", "specificationVersion": "1.0.0-beta.3" }
```


## `agent`

`agent` is the one JSON host boundary, used by the MCP adapter and by the
repository's own scripts. Its `compute`, `check` and `infer-contracts`
operations call the typed Rust agent API and always return a schema-versioned
object.

## Compatibility

Additive fields do not change `schemaVersion`; consumers must ignore fields
they do not recognize. Removing or renaming fields, changing their types, or
changing exit-code meaning requires a schema-version increment. Version 5
removed `observationDigest` and `receipt`: no host verified either, and a
value's digest is the `DIGEST` Word's.

The MCP-specific envelope and its exact-value definitions are additionally
published as `tools/mcp-server/result.schema.json`. That schema adds MCP
provenance and applied limits to the native report without changing Ajisai
semantics.
