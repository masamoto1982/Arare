// Reject citations to `SPEC §<number>` anywhere in the repository.
//
// That numbering belongs to `SPECIFICATION.md`, a Markdown specification of
// seventeen numbered sections deleted in 3f21400c ("Migrate SPECIFICATION and
// README from Markdown to HTML"). Every citation to it has been dangling ever
// since, and nothing noticed: 241 of them accumulated across 120 files, of
// which 34 named subsections (§8.7, §4.8, §12.3, §2.5, §7.1.1, §7.15, §14.4)
// that never existed in that document either.
//
// The current specification numbers nothing. It publishes stable `LANG.*`
// clause IDs precisely so a citation survives the document being reorganized —
// which is what LANG.CONFORMANCE.CHANGE asks of a derived surface. So a
// citation is either a `LANG.*` clause ID, or it is prose that stands on its
// own; a section number is neither.
//
// This gate is the part that makes the fix stick. An earlier pass (710e8168)
// repointed the stale §-citations it found in `docs/dev/` and added no gate,
// so the far larger population in `rust/src`, `rust/tests` and `src/` stayed
// invisible for another six weeks.

import { readFileSync } from 'node:fs';
import { relative } from 'node:path';
import { repoRoot, walk } from './lib/common.mjs';

const SKIP_DIRS = new Set(['node_modules', 'target', '.git', 'dist', 'build']);
const EXTENSIONS = /\.(rs|ts|js|mjs|cjs|css|html|json|md|sh|yml|yaml)$/;
const CITATION = /SPEC\s*§\s*[0-9]+(?:\.[0-9]+)*/g;

// The same citation with the section sign left off. This form was invisible to
// the gate for as long as the gate existed: both patterns above require a `§`,
// and the whole-file guard below skipped any file that contained no `§` at all,
// so a comment naming the retired document and a bare number passed clean. Four
// such citations were sitting in `rust/src/interpreter/` — two of them in the
// diagnosis tables, which is the surface an agent is told to read literally.
// `SPEC` immediately before a number is unambiguous enough to match on its own:
// the current specification numbers nothing, so there is no live document for
// such a reference to name.
const CITATION_NO_SIGN = /\bSPEC\s+[0-9]+(?:\.[0-9]+)*\b/g;

// Source files have no sections of their own, so a bare `§N.M` in one names
// some other document — and after the sweep above, the only document it can
// name is the retired specification. A `.md` under docs/dev/ is different: a
// memo numbers its own sections and cites its siblings' legitimately, so bare
// references are only rejected in source.
//
// Multi-part only. A bare single-digit `§7` in source is usually a real
// reference to a document that owns numbered sections — the formalization
// roadmap's `§1.2-(T)`, a work order's `§3`, or SKILL.md's own `§6`/`§9`,
// which `generate-skill-md.mjs` writes *into* the file it generates. Those
// are correct, so the pattern requires a dot and the allowance below carries
// the rest.
const SOURCE_EXTENSIONS = /\.(rs|ts|js|mjs|cjs|css|html)$/;
const BARE_CITATION = /§ ?[0-9]+\.[0-9]+(?:\.[0-9]+)*/g;

// A `§N.M` is fine when the sentence around it names the document that owns
// the numbering. Four lines of lookback, because a doc comment wraps and the
// name often sits a line or two above the reference.
const NAMES_ITS_DOCUMENT =
  /three-layer|documentation model|work order|work-order|roadmap|migration plan|handoff|proposal|methodology|Phase \d|SKILL|\.(md|json|html|mjs)\b/i;

// A passage *about* a stale citation is not itself one. Keyed by path so a new
// dangling citation in the same file is still caught: the passage must also
// name the retired numbering it is discussing. Matched against the same short
// context window the bare check uses, because the disclaimer and the sections
// it disclaims routinely land on different lines of one wrapped comment.
const DISCUSSES_RATHER_THAN_CITES = new Map([
  // This file's own header enumerates the phantom subsections as evidence.
  ['scripts/check-spec-citations.mjs', /never existed/],
]);

const findings = [];
for (const file of walk(repoRoot, EXTENSIONS, (name) => SKIP_DIRS.has(name))) {
  const rel = relative(repoRoot, file);
  const exempt = DISCUSSES_RATHER_THAN_CITES.get(rel);
  let text;
  try {
    text = readFileSync(file, 'utf8');
  } catch {
    continue;
  }
  // Cheap whole-file guard before the per-line work. It must admit a file that
  // carries a sign-less `SPEC <number>` and no section sign anywhere —
  // requiring `§` here is exactly what hid that form. Testing for 'SPEC' alone
  // would instead make the bare-`§N.M` pass vacuous, which a probe caught and a
  // green run did not, so the two conditions stay separate.
  if (!text.includes('§') && !CITATION_NO_SIGN.test(text)) continue;
  // `CITATION_NO_SIGN` is a global regex, so the test above advanced its
  // lastIndex. Reset it, or the per-line pass below starts mid-string.
  CITATION_NO_SIGN.lastIndex = 0;
  const isSource = SOURCE_EXTENSIONS.test(file);
  const lines = text.split('\n');
  lines.forEach((line, index) => {
    const context = lines.slice(Math.max(0, index - 3), index + 2).join(' ');
    if (exempt && exempt.test(context)) return;
    for (const match of line.match(CITATION) ?? []) {
      findings.push(`${rel}:${index + 1}: ${match}`);
    }
    for (const match of line.match(CITATION_NO_SIGN) ?? []) {
      findings.push(`${rel}:${index + 1}: ${match} (no section sign)`);
    }
    if (!isSource) return;
    if (NAMES_ITS_DOCUMENT.test(context)) return;
    for (const match of line.match(BARE_CITATION) ?? []) {
      // `SPEC §N.M` was already reported by the pass above.
      if (new RegExp(`SPEC\\s*${match.replace('§', '§')}`).test(line)) continue;
      findings.push(`${rel}:${index + 1}: ${match} (bare)`);
    }
  });
}

if (findings.length > 0) {
  console.error(
    `[spec-citations] ${findings.length} citation(s) to the retired numbered specification:`,
  );
  for (const finding of findings) console.error(`  ${finding}`);
  console.error(
    '\nSPECIFICATION.md was deleted in 3f21400c; its section numbers name nothing.',
  );
  console.error(
    'Cite the stable clause ID instead (LANG.VALUES.EXACT, LANG.FAILURE.ERROR, …),',
  );
  console.error(
    'or drop the reference and let the surrounding prose stand on its own.',
  );
  process.exitCode = 1;
} else {
  console.log('[spec-citations] no citations to the retired numbered specification.');
}
