// The operator-facing half of the same executable.
//
// A stdio MCP server is silent when it is healthy and silent when it is
// wedged, and from a terminal the two are indistinguishable: the operator sees
// a process that started and a client that offers no tools, with nothing
// saying whether the fault is the Node version, a corrupt packaged asset, a
// missing WASM bundle or the client's own configuration. `--doctor` is the
// difference. It runs the same asset validation, backend selection and real
// computation the server runs, against the same installed copy, and answers
// with an exit code.
//
// Everything here is reached only when `index.js` was given arguments, and
// never after a transport is open, so the stdout MCP frames travel over is
// never written to by a diagnostic.

import {
  createBackend,
  engineVersion,
  specificationVersion,
  LIMITS,
  packageEngines,
  registryDigest,
  serverVersion,
  validateAssets,
} from "./index.js";

const USAGE = `ajisai-mcp-server — bounded, diagnostic Ajisai computation over MCP (stdio)

  ajisai-mcp-server              serve MCP on stdin/stdout (what a client launches)
  ajisai-mcp-server --doctor     check this installation, exit 0 (healthy) or 1
  ajisai-mcp-server --version    print adapter, engine and registry provenance
  ajisai-mcp-server --help       print this message

With no arguments the process speaks MCP and writes nothing else to stdout.
The commands above are for a human at a terminal and never run in that mode.`;

/**
 * The provenance line, and the reason there is more than one version on it.
 *
 * The adapter and the language engine are separately versioned components, and
 * a bug report that names only one of them is a bug report missing half its
 * subject.
 */
function versionLine() {
  let registry;
  try {
    registry = registryDigest().slice(0, 16);
  } catch {
    // A corrupt registry is exactly what `--doctor` is for; `--version` still
    // answers what it can rather than failing to identify the build at all.
    registry = "unavailable";
  }
  return `ajisai-mcp-server ${serverVersion()} (engine ${engineVersion()}, specification ${specificationVersion()}, registry ${registry})`;
}

/** The declared Node floor, read from the package rather than restated here. */
function requiredNodeMajor() {
  const declared = packageEngines().node ?? "";
  const [match] = declared.match(/\d+/) ?? [];
  return match ? Number(match) : 0;
}

/**
 * One diagnostic step: a stable label, and either the observation that made it
 * pass or the reason it did not.
 *
 * A step reports *host* detail — paths, versions, spawn failures — on purpose.
 * That is the inverse of the rule the tool boundary follows, and for the same
 * reason: this output has an operator in front of it, and the MCP boundary
 * does not.
 */
async function step(label, run) {
  try {
    const detail = await run();
    return { label, ok: true, detail };
  } catch (error) {
    return { label, ok: false, detail: error?.detail ?? error?.message ?? String(error) };
  }
}

function requireThat(condition, reason, detail) {
  if (!condition) throw new Error(reason);
  return detail;
}

async function doctor(write) {
  const steps = [];
  const required = requiredNodeMajor();
  steps.push(await step("node runtime satisfies the declared engine range", () => {
    const major = Number(process.versions.node.split(".")[0]);
    return requireThat(
      major >= required,
      `running Node ${process.versions.node}, package requires ${packageEngines().node}`,
      `Node ${process.versions.node} (requires ${packageEngines().node})`,
    );
  }));

  steps.push(await step("packaged assets load and match their recorded digest", () => {
    validateAssets();
    return `registry ${registryDigest().slice(0, 16)}, engine ${engineVersion()}`;
  }));

  // Resolved once and reused: `--doctor` must exercise the very backend a
  // served session would use, not a second selection that could differ.
  let backend = null;
  steps.push(await step("an execution backend is available", () => {
    backend = createBackend();
    return requireThat(
      backend !== null,
      "no native binary found (build rust/ or set AJISAI_BIN) and the packaged WASM bundle is missing",
      `${backend?.kind} backend selected`,
    );
  }));

  // The two claims the product is actually about. A server that starts, loads
  // its assets and then rounds is a working server answering wrongly, and only
  // running something proves it does not.
  steps.push(await step("exact rational arithmetic stays exact", async () => {
    requireThat(backend !== null, "skipped: no execution backend");
    const result = await backend.compute("2 3 DIV 1 3 DIV ADD");
    const [display] = result.stackDisplay ?? [];
    return requireThat(
      result.status === "ok" && display === "1/1",
      `2 3 DIV 1 3 DIV ADD answered ${result.status} ${JSON.stringify(result.stackDisplay)}`,
      "2 3 DIV 1 3 DIV ADD = 1/1",
    );
  }));

  steps.push(await step("an algebraic value keeps its exact normal form", async () => {
    requireThat(backend !== null, "skipped: no execution backend");
    const result = await backend.compute("2 SQRT");
    const [term] = result.stack?.[0]?.semantics?.exactTerms ?? [];
    return requireThat(
      term?.numerator === "1" && term?.denominator === "1" && term?.radicand === "2",
      `2 SQRT produced no (1/1)·√2 exact term: ${JSON.stringify(term ?? null)}`,
      "2 SQRT = (1/1)·√2 in exactTerms",
    );
  }));

  write(versionLine());
  write("");
  for (const { label, ok, detail } of steps) {
    write(`${ok ? "ok  " : "FAIL"}  ${label}${detail ? ` — ${detail}` : ""}`);
  }
  write("");
  const failed = steps.filter(({ ok }) => !ok).length;
  if (failed === 0) {
    write(`doctor: ${steps.length} checks passed; wall-time limit ${LIMITS.wallTimeMs} ms`);
    return 0;
  }
  write(`doctor: ${failed} of ${steps.length} checks failed`);
  return 1;
}

/**
 * Run the terminal command line. Returns the process exit code.
 *
 * `write` is injected so the self-test can assert on what a command prints
 * without capturing the process's own streams.
 */
export async function runCli(argv, write = (line) => console.log(line)) {
  const [command, ...rest] = argv;
  if (rest.length > 0) {
    write(`ajisai-mcp-server: ${command} takes no further arguments`);
    write("");
    write(USAGE);
    return 2;
  }
  if (command === "--help" || command === "-h") {
    write(USAGE);
    return 0;
  }
  if (command === "--version" || command === "-v") {
    write(versionLine());
    return 0;
  }
  if (command === "--doctor") return doctor(write);
  write(`ajisai-mcp-server: unknown option ${command}`);
  write("");
  write(USAGE);
  return 2;
}
