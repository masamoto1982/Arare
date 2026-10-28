#!/usr/bin/env node
// File-size budget guard for the Rust source surface (SPECIFICATION the file-size budget in docs/dev/specification-implementation-rules.md).
//
// docs/dev/specification-implementation-rules.md keeps new and substantially-rewritten Rust files at or under 500 lines.
// A large legacy surface predates that discipline, so a hard 500-line wall would
// fail on day one and get muted rather than obeyed. Instead this guard encodes
// the the file-size budget in docs/dev/specification-implementation-rules.md policy mechanically:
//   - a file NOT recorded in the baseline must stay at or under LIMIT lines
//     (new files, and files small enough to have never breached it, cannot cross
//     the wall unnoticed);
//   - a file recorded in the baseline is grandfathered at its recorded size but
//     may not grow by GROWTH_ALLOWANCE or more (the over-budget surface can only
//     shrink or hold, never bloat).
//
// The baseline is docs/quality/file-size-baseline.json, regenerated with
// `--update`. Its leading `_comment` records why the largest offenders are left
// unsplit for now (see that file).

import { readFileSync, writeFileSync, statSync } from 'node:fs';
import { resolve, relative, sep } from 'node:path';
import { repoRoot, walk } from './lib/common.mjs';

const rustSrcDir = resolve(repoRoot, 'rust/src');
const baselinePath = resolve(repoRoot, 'docs/quality/file-size-baseline.json');

const LIMIT = 500;
// A baseline file may drift a little without a re-baseline, but crossing +10%
// is treated as bloat and blocks. Growth ratio = (current - baseline) / baseline.
const GROWTH_ALLOWANCE = 0.1;

const BASELINE_COMMENT = [
  'Baseline of Rust source files that exceed the SPECIFICATION the file-size budget in docs/dev/specification-implementation-rules.md 500-line',
  'budget. Recorded so the mandatory rule can be enforced going forward without',
  'a mass file split: files listed here are grandfathered at their recorded line',
  'count and must not grow by 10% or more; files not listed here must stay at or',
  'under 500 lines. Regenerate with: node scripts/check-file-size-budget.mjs --update',
  '',
  'A baselined path that no longer exists is a hard error: delete the entry in',
  'the same change that removes the file (re-baseline with --update), so the',
  'baseline never references source that is gone.',
];

// Count lines the same way `wc -l` does (number of newline characters), so the
// recorded baseline matches a shell survey and stays stable across platforms.
function countLines(content) {
  let count = 0;
  for (let i = 0; i < content.length; i++) {
    if (content[i] === '\n') count++;
  }
  return count;
}

// A file is exempt when its first line carries the `// @generated` marker.
// The the file-size budget in docs/dev/specification-implementation-rules.md budget exists to keep *hand-written and hand-maintained* Rust
// reviewable; a generated file is a mechanical projection of a source that is
// itself reviewed and drift-checked (e.g. spec/words.json), so its line count
// is not a maintainability signal and cannot be reduced by hand-splitting.
function isGenerated(absPath) {
  const content = readFileSync(absPath, 'utf8');
  const firstNewline = content.indexOf('\n');
  const firstLine = firstNewline === -1 ? content : content.slice(0, firstNewline);
  return firstLine.includes('@generated');
}

function toRepoPosix(absPath) {
  return relative(repoRoot, absPath).split(sep).join('/');
}

function measure() {
  const files = walk(rustSrcDir, ['.rs']).filter((file) => !isGenerated(file)).sort();
  const sizes = new Map();
  for (const file of files) {
    const lines = countLines(readFileSync(file, 'utf8'));
    sizes.set(toRepoPosix(file), lines);
  }
  return sizes;
}

function loadBaseline() {
  try {
    statSync(baselinePath);
  } catch {
    console.error(
      `[file-size-budget] missing baseline ${toRepoPosix(baselinePath)}; run: node scripts/check-file-size-budget.mjs --update`,
    );
    process.exit(1);
  }
  const parsed = JSON.parse(readFileSync(baselinePath, 'utf8'));
  return parsed.files ?? {};
}

function updateBaseline() {
  const sizes = measure();
  const files = {};
  for (const [path, lines] of [...sizes].sort((a, b) => b[1] - a[1])) {
    if (lines > LIMIT) files[path] = lines;
  }
  const doc = {
    _comment: BASELINE_COMMENT,
    limit: LIMIT,
    growthAllowance: GROWTH_ALLOWANCE,
    files,
  };
  writeFileSync(baselinePath, `${JSON.stringify(doc, null, 2)}\n`);
  console.log(
    `[file-size-budget] wrote ${toRepoPosix(baselinePath)} with ${Object.keys(files).length} baselined file(s)`,
  );
}

function check() {
  const baseline = loadBaseline();
  const sizes = measure();
  const violations = [];

  for (const [path, lines] of sizes) {
    if (path in baseline) {
      const recorded = baseline[path];
      const ceiling = Math.floor(recorded * (1 + GROWTH_ALLOWANCE));
      if (lines > ceiling) {
        violations.push(
          `${path}: ${lines} lines exceeds baseline ${recorded} by >=${Math.round(GROWTH_ALLOWANCE * 100)}% (ceiling ${ceiling}); split it or re-baseline with --update after a justified change`,
        );
      }
    } else if (lines > LIMIT) {
      violations.push(
        `${path}: ${lines} lines exceeds the ${LIMIT}-line budget (the file-size budget in docs/dev/specification-implementation-rules.md); keep new/rewritten files at or under ${LIMIT} lines`,
      );
    }
  }

  // A baselined path that no longer exists is a violation: the baseline must not
  // reference source that has been removed or renamed. The fix is to drop the
  // stale entry in the same change that removes the file (re-baseline with
  // --update), which is why an intentional deletion + baseline update passes.
  for (const path of Object.keys(baseline)) {
    if (!sizes.has(path)) {
      violations.push(
        `${path}: baselined file no longer present; drop the stale entry (re-baseline with --update in the change that removed it)`,
      );
    }
  }

  if (violations.length > 0) {
    console.error(`[file-size-budget] ${violations.length} violation(s):`);
    for (const v of violations) console.error(`  - ${v}`);
    process.exit(1);
  }

  console.log(
    `[file-size-budget] ${sizes.size} Rust source file(s) checked; ${Object.keys(baseline).length} baselined, 0 violations`,
  );
}

if (process.argv.includes('--update')) {
  updateBaseline();
} else {
  check();
}
