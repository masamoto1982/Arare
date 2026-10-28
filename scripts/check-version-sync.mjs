#!/usr/bin/env node
// The beta ships one version number. `package.json` is the source of truth and
// every other manifest must repeat it exactly: the npm package, the
// `ajisai-core` crate (which `ajisai version` prints through
// `CARGO_PKG_VERSION`), the Tauri crate, and the Tauri application config.
//
// Distribution metadata that drifts is worse than no metadata: a bug report
// naming a version has to identify one build. This gate keeps them equal
// without a generator, so a release bump is one edit plus this check.
import { readJson, readText, reporter } from './lib/common.mjs';

const readVersion = (path, pattern) => readText(path).match(pattern)?.[1] ?? null;

const expected = readJson('package.json').version;
// Cargo and npm both take SemVer, so a prerelease reads the same in both.
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

const sources = [
  ['package.json', expected],
  ['rust/Cargo.toml', readVersion('rust/Cargo.toml', /^version = "([^"]+)"/m)],
  ['src-tauri/Cargo.toml', readVersion('src-tauri/Cargo.toml', /^version = "([^"]+)"/m)],
  ['src-tauri/tauri.conf.json', readJson('src-tauri/tauri.conf.json').version],
];

const report = reporter('version-sync');
if (!SEMVER.test(expected)) report.fail(`package.json version ${expected} is not SemVer`);
for (const [path, version] of sources) {
  if (version === null) report.fail(`${path}: no version found`);
  else if (version !== expected) report.fail(`${path}: version ${version}; expected ${expected}`);
}
report.done(`${sources.length}/${sources.length} manifests declare ${expected}.`);
