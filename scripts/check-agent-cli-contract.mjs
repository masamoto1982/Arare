#!/usr/bin/env node
import { readText } from "./lib/common.mjs";

const cliSource = readText("rust/src/cli/mod.rs");
const contract = readText("docs/dev/agent-cli-output-contract.md");

const usage = cliSource.match(/const USAGE: &str = "([\s\S]*?)";/)?.[1];
if (!usage) throw new Error("could not locate the native CLI USAGE string");

const implemented = [...usage.matchAll(/^  ([a-z][a-z-]*)[ <\[]/gm)]
  .map((match) => match[1])
  .sort();
const documentedBlock = contract.match(/```text\n([\s\S]*?)\n```/)?.[1];
if (!documentedBlock) throw new Error("could not locate the documented command block");
const documented = [...documentedBlock.matchAll(/^ajisai ([a-z][a-z-]*)/gm)]
  .map((match) => match[1])
  .sort();

if (new Set(implemented).size !== implemented.length) {
  throw new Error(`duplicate CLI command in USAGE: ${implemented.join(", ")}`);
}
if (new Set(documented).size !== documented.length) {
  throw new Error(`duplicate documented CLI command: ${documented.join(", ")}`);
}
if (JSON.stringify(implemented) !== JSON.stringify(documented)) {
  throw new Error(
    `agent CLI contract drift\nimplemented: ${implemented.join(", ")}\n` +
      `documented: ${documented.join(", ")}`,
  );
}

console.log(`[agent-cli-contract] ${implemented.length} implemented commands are documented exactly.`);
