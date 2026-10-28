#!/usr/bin/env node
// Re-applies a license banner to the vendored KaTeX assets after they are
// copied from node_modules. KaTeX's dist min files carry the MIT license in a
// sibling `.LICENSE.txt` sidecar rather than an inline banner, so a plain copy
// would ship the code without a visible notice. This restores an inline
// `/*! ... MIT License ... */` banner (idempotent) so the required copyright
// and license notice travels with each redistributed file.
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { repoRoot } from './lib/common.mjs';

const vendorDir = resolve(repoRoot, 'public/vendor/katex');

// Derive the version from the vendored bundle so the banner never goes stale.
const mainJs = readFileSync(resolve(vendorDir, 'katex.min.js'), 'utf8');
const versionMatch = mainJs.match(/version:"(\d+\.\d+\.\d+)"/);
const version = versionMatch ? versionMatch[1] : 'unknown';

const banner =
  `/*! KaTeX v${version} | MIT License | ` +
  'Copyright (c) 2013-2020 Khan Academy and other contributors | ' +
  'Full text: ./LICENSE and /THIRD-PARTY-LICENSES.md */';

const files = ['katex.min.css', 'katex.min.js', 'contrib/auto-render.min.js'];

for (const rel of files) {
  const path = resolve(vendorDir, rel);
  const source = readFileSync(path, 'utf8');
  if (source.startsWith('/*! KaTeX')) continue; // already bannered
  writeFileSync(path, `${banner}\n${source}`);
  console.log(`[vendor-katex-banner] added banner to ${rel}`);
}
