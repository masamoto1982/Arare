#!/usr/bin/env node
import { execFile } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, renameSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { promisify } from "node:util";
import { pathToFileURL, fileURLToPath } from "node:url";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const execFileAsync = promisify(execFile);
const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const scratch = mkdtempSync(join(tmpdir(), "ajisai-mcp-pack-"));
let tarball;
try {
  const { stdout } = await execFileAsync("npm", ["pack", "--json"], {
    cwd: here,
    encoding: "utf8",
  });
  const packed = JSON.parse(stdout)[0];
  tarball = join(here, packed.filename);
  const paths = new Set(packed.files.map(({ path }) => path));
  for (const required of [
    "index.js", "doctor.js", "result.schema.json", "assets/metadata.json", "assets/words.json",
    "assets/word-manifest.json", "assets/quickstart.md", "backend/native-cli.js", "backend/wasm-worker.js",
    "backend/wasm-worker-entry.js", "wasm/generated/ajisai_core.js", "wasm/generated/ajisai_core_bg.wasm",
    "wasm/generated/package.json", "LICENSE", "README.md", "CHANGELOG.md",
  ]) {
    if (!paths.has(required)) throw new Error(`tarball is missing ${required}`);
  }
  // The evaluation harness, its corpora and the repository's own tests are
  // how this package is developed, not what it runs. Shipped, they were files
  // no installed copy could use: every harness reads corpora or traces that
  // live only in this checkout.
  const devOnly = [...paths].filter((path) =>
    /^(eval|golden)\//.test(path) ||
    /(^|\/)[^/]*(test|smoke)[^/]*\.js$/.test(path) ||
    ["eval-common.js", "benchmark.js", "number-baseline.js", "evaluation-contract.js", "validate-evaluation.js",
      "score-traces.js", "score-repairs.js", "capture-traces.js", "capture-repairs.js", "sync-assets.js",
      "selftest.js"].includes(path));
  if (devOnly.length) throw new Error(`tarball ships development-only files: ${devOnly.join(", ")}`);

  try {
    await execFileAsync(
      "npm",
      ["install", "--ignore-scripts", "--no-audit", "--no-fund", "--prefix", scratch, tarball],
      { encoding: "utf8" },
    );
  } catch (error) {
    if (!/E403|403 Forbidden|ENOTCACHED/.test(error.stderr ?? "")) throw error;
    // Restricted development sandboxes may forbid registry reads. Still test
    // the exact tarball contents, borrowing only the already lockfile-installed
    // dependencies. CI has registry access and takes the clean-install path.
    const unpack = join(scratch, "unpack");
    mkdirSync(unpack, { recursive: true });
    await execFileAsync("tar", ["-xzf", tarball, "-C", unpack]);
    const modules = join(scratch, "node_modules");
    mkdirSync(modules, { recursive: true });
    const installed = join(modules, "ajisai-mcp-server");
    renameSync(join(unpack, "package"), installed);
    symlinkSync(join(here, "node_modules"), join(installed, "node_modules"), "dir");
    console.warn("WARN registry unavailable; used lockfile-installed dependencies");
  }
  const installedEntry = join(scratch, "node_modules", "ajisai-mcp-server", "index.js");
  const installed = await import(pathToFileURL(installedEntry));

  async function withServer(run) {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const scenarioServer = installed.createServer();
    const scenarioClient = new Client({ name: "pack-smoke", version: "1" });
    await Promise.all([
      scenarioServer.connect(serverTransport),
      scenarioClient.connect(clientTransport),
    ]);
    try {
      await run(scenarioClient);
    } finally {
      await scenarioClient.close();
      await scenarioServer.close();
    }
  }

  // Scenario 1: neither AJISAI_REPO nor AJISAI_BIN is set — the true
  // zero-configuration default. The installed tarball must still expose its
  // packaged static resources (never read from this checkout: `here` inside
  // the installed copy resolves under `scratch`, nowhere near this repo) and
  // compute through its own packaged, self-contained WASM worker backend,
  // with no native binary and no checkout in reach.
  delete process.env.AJISAI_REPO;
  delete process.env.AJISAI_BIN;
  // A decoy where discovery used to look: `../../rust/target` from the
  // installed package is the installing project's own directory. An installed
  // copy must never run a binary it finds there.
  const decoy = join(scratch, "rust", "target", "debug", "ajisai");
  mkdirSync(dirname(decoy), { recursive: true });
  writeFileSync(join(scratch, "rust", "Cargo.toml"), "[package]\nname = \"not-ajisai\"\n");
  writeFileSync(decoy, "#!/bin/sh\nexit 99\n", { mode: 0o755 });
  await withServer(async (client) => {
    const tools = await client.listTools();
    if (tools.tools.length !== 4) throw new Error("installed package did not expose four tools");
    const guide = await client.readResource({ uri: "ajisai://guide/quickstart" });
    if (!guide.contents[0]?.text?.includes("Ajisai")) {
      throw new Error("installed package did not expose its packaged guide");
    }
    const contract = await client.callTool({ name: "word_contract", arguments: { word: "MAP" } });
    if (contract.structuredContent?.matches?.[0]?.name !== "MAP") {
      throw new Error("installed package did not expose its packaged Word registry");
    }
    const computed = await client.callTool({ name: "compute", arguments: { source: "1 3 DIV" } });
    if (computed.structuredContent?.stackDisplay?.[0] !== "1/3") {
      throw new Error("installed package could not compute with neither AJISAI_REPO nor AJISAI_BIN set (WASM backend)");
    }
    if (computed.structuredContent?.mcp?.backend?.kind !== "wasmWorker") {
      throw new Error(
        `installed package picked up a native binary outside itself (backend ${computed.structuredContent?.mcp?.backend?.kind})`,
      );
    }
  });
  console.log(`PASS clean-installed ${packed.filename} computes with no repository and no native binary (WASM backend)`);

  // Scenario 1b: launch the package the way every documented client entry
  // launches it — by its `bin` name, through `node_modules/.bin`.
  //
  // Everything above imports `createServer` from the installed file, and that
  // is a different code path from running the executable: the entry-point
  // guard compared `argv[1]` (the .bin *symlink*) with `import.meta.url` (its
  // target), so launching by name started a process that served nothing and
  // exited 0. A client saw a server with no tools and no error. Both npm-based
  // README recipes — `npx -y ajisai-mcp-server` and a bare
  // `"command": "ajisai-mcp-server"` — went through that symlink, so the only
  // invocation anyone had actually tested was the one naming `index.js`
  // directly. Spawning the real binary is what closes that gap.
  const binDirectory = join(scratch, "node_modules", ".bin");
  const binPath = join(binDirectory, "ajisai-mcp-server");
  if (!existsSync(binPath)) {
    // The offline fallback above unpacks the tarball by hand and never runs
    // npm's bin linking. Recreate the symlink it would have made: launching
    // through a symlink is the whole point of this scenario.
    mkdirSync(binDirectory, { recursive: true });
    symlinkSync(installedEntry, binPath);
  }
  const { StdioClientTransport } = await import("@modelcontextprotocol/sdk/client/stdio.js");
  const spawned = new Client({ name: "pack-smoke-bin", version: "1" });
  await spawned.connect(new StdioClientTransport({ command: process.execPath, args: [binPath] }));
  try {
    const tools = await spawned.listTools();
    if (tools.tools.length !== 4) {
      throw new Error("the installed bin entry served no tools when launched by name");
    }
    const computed = await spawned.callTool({ name: "compute", arguments: { source: "1 3 DIV" } });
    if (computed.structuredContent?.stackDisplay?.[0] !== "1/3") {
      throw new Error("the installed bin entry did not compute when launched by name");
    }
  } finally {
    await spawned.close();
  }
  console.log("PASS the installed bin entry serves MCP when launched by name through node_modules/.bin");

  // The same installed copy must be able to say whether it is healthy, without
  // a checkout and without an MCP client — that is the only thing an operator
  // whose client shows an empty tool list can run.
  const doctored = await execFileAsync(process.execPath, [binPath, "--doctor"], {
    encoding: "utf8",
  });
  if (!doctored.stdout.includes("checks passed") || doctored.stdout.includes("FAIL")) {
    throw new Error(`--doctor did not pass on the installed package:\n${doctored.stdout}`);
  }
  console.log("PASS the installed package self-diagnoses with --doctor and exits 0");

  // Scenario 2: AJISAI_BIN is an explicit override (the native/Docker
  // deployment story). The installed tarball must prefer and use the native
  // backend when pointed at one.
  //
  // This runs in a spawned process, and has to. The backend is resolved once
  // per *process* and cached, so setting `AJISAI_BIN` and calling
  // `installed.createServer()` again reused the WASM backend scenario 1 had
  // already fixed: the assertion passed on a machine with no native binary at
  // all, which is exactly the machine it exists to catch. It proved the WASM
  // backend twice and reported it as proof of the override.
  const nativeBin = join(repoRoot, "rust", "target", "debug", "ajisai");
  if (!existsSync(nativeBin)) {
    throw new Error(
      `no native binary at ${nativeBin}; build it first (npm run test:mcp-pack from the ` +
        "repository root does, or: cargo build --manifest-path rust/Cargo.toml --bin ajisai)",
    );
  }
  const native = new Client({ name: "pack-smoke-native", version: "1" });
  await native.connect(new StdioClientTransport({
    command: process.execPath,
    args: [binPath],
    env: { ...process.env, AJISAI_BIN: nativeBin },
  }));
  try {
    const computed = await native.callTool({ name: "compute", arguments: { source: "1 3 DIV" } });
    if (computed.structuredContent?.mcp?.backend?.kind !== "nativeCli") {
      throw new Error(
        `AJISAI_BIN did not select the native backend (got ${computed.structuredContent?.mcp?.backend?.kind})`,
      );
    }
    if (computed.structuredContent?.stackDisplay?.[0] !== "1/3") {
      throw new Error("installed package could not compute through the native AJISAI_BIN backend");
    }
  } finally {
    await native.close();
  }
  console.log(`PASS clean-installed ${packed.filename} computes through an explicit AJISAI_BIN (native backend, ${packed.files.length} files)`);
} finally {
  if (tarball) rmSync(tarball, { force: true });
  rmSync(scratch, { recursive: true, force: true });
}
