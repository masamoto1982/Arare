#!/usr/bin/env node
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const assetsDir = join(here, "assets");
const sources = [
  [join(repoRoot, "spec", "words.json"), join(assetsDir, "words.json")],
  [join(repoRoot, "docs", "word-manifest.json"), join(assetsDir, "word-manifest.json")],
  // What `words` defines and `ajisai://phrasebook` serves.
  [join(repoRoot, "docs", "phrasebook", "phrasebook.ajisai"), join(assetsDir, "phrasebook.ajisai")],
  // The package is published on its own, so it carries the repository's
  // licence text rather than only naming it in `package.json`: an MIT grant
  // is conditioned on the notice travelling with the software.
  [join(repoRoot, "LICENSE"), join(here, "LICENSE")],
];
const rootPackage = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8"));
const words = readFileSync(sources[0][0]);
const versionLock = JSON.parse(readFileSync(join(repoRoot, "spec", "version-lock.json"), "utf8"));
const metadata = `${JSON.stringify({
  schemaVersion: 1,
  engineVersion: rootPackage.version,
  specificationVersion: versionLock.specificationVersion,
  registryDigest: createHash("sha256").update(words).digest("hex"),
}, null, 2)}\n`;

/**
 * The served quickstart is an MCP preface followed by the generated writing
 * protocol, not a copy of `SKILL.md`.
 *
 * `SKILL.md` opens with a CLI run loop (`ajisai agent compute file`) — commands a
 * connected MCP client cannot issue and has no reason to read about — and says
 * nothing about which tool to call. A model that read it first
 * learned the language before learning the interface. The preface answers the
 * interface question in one screen and hands off; the generated half stays
 * verbatim, so its examples remain the ones the generator verified.
 *
 * `SKILL_BOUNDARY` is what the self-test slices on to run the preface's own
 * examples against the live backend, and what a reader sees as the seam.
 */
export const SKILL_BOUNDARY = "<!-- BEGIN GENERATED SKILL.md -->\n";
const quickstart = Buffer.concat([
  readFileSync(join(here, "mcp-quickstart.md")),
  Buffer.from(`${SKILL_BOUNDARY}\n`),
  readFileSync(join(repoRoot, "SKILL.md")),
]);

/**
 * The MCP result schema's value node is spec/host-protocol.schema.json's, not
 * a second description of it.
 *
 * `result.schema.json` used to describe the stack node by hand, loosely
 * (`additionalProperties: true`, an untyped `value`), beside a canonical
 * schema that nothing validated. The two drifted — the canonical one never
 * declared `elided` or `absence.detail`, both of which the server emitted —
 * and neither caught it. The canonical definitions are copied in here under
 * the names the result schema uses, so the self-test's validation of every
 * live result is also a validation of the protocol, and `--check` fails the
 * moment the two differ.
 */
const protocolDefNames = {
  observedValue: "protocolNode",
  semantics: "protocolSemantics",
  absence: "protocolAbsence",
  exactTerm: "exactTerm",
  elided: "protocolElided",
  truncated: "protocolTruncated",
  integerString: "integerString",
  positiveIntegerString: "positiveIntegerString",
  nonNegativeIntegerString: "nonNegativeIntegerString",
};
const resultSchemaPath = join(here, "result.schema.json");
function resultSchemaWithProtocol() {
  const protocol = JSON.parse(readFileSync(join(repoRoot, "spec", "host-protocol.schema.json"), "utf8"));
  const result = JSON.parse(readFileSync(resultSchemaPath, "utf8"));
  const renamed = JSON.parse(
    JSON.stringify(protocol.$defs).replace(/"#\/\$defs\/([A-Za-z]+)"/g, (whole, name) => {
      if (!(name in protocolDefNames)) throw new Error(`host protocol defines no ${name}`);
      return `"#/$defs/${protocolDefNames[name]}"`;
    }),
  );
  for (const [name, target] of Object.entries(protocolDefNames)) {
    result.$defs[target] = renamed[name];
  }
  return Buffer.from(`${JSON.stringify(result, null, 1)}\n`);
}

const outputs = [...sources.map(([source, target]) => [readFileSync(source), target]),
  [resultSchemaWithProtocol(), resultSchemaPath],
  [quickstart, join(assetsDir, "quickstart.md")],
  [Buffer.from(metadata), join(assetsDir, "metadata.json")]];
/**
 * `server.json` is the MCP Registry's entry for this package, and the registry
 * proves ownership by reading `mcpName` back out of the published
 * `package.json`. The two files state one release, so every field they share
 * must agree — a registry entry naming a version npm does not hold points
 * installers at nothing. Returns the disagreements, empty when there are none.
 */
function serverJsonMismatches() {
  const pkg = JSON.parse(readFileSync(join(here, "package.json"), "utf8"));
  const server = JSON.parse(readFileSync(join(here, "server.json"), "utf8"));
  const npm = (server.packages ?? []).filter((entry) => entry.registryType === "npm");
  const facts = [
    ["server.json name", server.name, "package.json mcpName", pkg.mcpName],
    ["server.json version", server.version, "package.json version", pkg.version],
    ["server.json npm packages", npm.length, "exactly", 1],
    ["server.json package identifier", npm[0]?.identifier, "package.json name", pkg.name],
    ["server.json package version", npm[0]?.version, "package.json version", pkg.version],
  ];
  return facts
    .filter(([, left, , right]) => left !== right)
    .map(([leftName, left, rightName, right]) => `${leftName} is ${JSON.stringify(left)}; ${rightName} is ${JSON.stringify(right)}`);
}

// Generating or checking is the entry-point behaviour; importing this module
// for `SKILL_BOUNDARY` — which the self-test does, so the seam it slices on is
// the one that was written — must not rewrite the working tree as a side
// effect of the import.
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.includes("--check")) {
    const stale = outputs.filter(([content, target]) =>
      !existsSync(target) || !content.equals(readFileSync(target)));
    if (stale.length) {
      console.error(`MCP packaged assets are stale: ${stale.map(([, path]) => path).join(", ")}`);
      process.exit(1);
    }
    const mismatches = serverJsonMismatches();
    if (mismatches.length) {
      console.error(`server.json disagrees with package.json: ${mismatches.join("; ")}`);
      process.exit(1);
    }
    // npm includes prepack stdout before `npm pack --json`, which would corrupt
    // machine-readable pack output consumed by the smoke test.
    if (process.env.npm_lifecycle_event !== "prepack") {
      console.log("MCP packaged assets are current");
    }
  } else {
    mkdirSync(assetsDir, { recursive: true });
    for (const [content, target] of outputs) writeFileSync(target, content);
    console.log("updated MCP packaged assets");
  }
}
