// The Lean model in formal/, read as text: which theorems it declares, under
// which fully qualified names, in which file, and which requirement and
// verification IDs each file cites. scripts/check-formal-axioms.mjs and the
// requirements registry gates (scripts/check-requirements.mjs,
// scripts/generate-requirements.mjs) read the model through this one reader,
// so a theorem is named the same way everywhere.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { repoRoot, walk } from './common.mjs';

const ID_RE = /\bAR-(?:SYS|HLR|LLR|INH|PERF|VER)-\d{3}(?:-[A-Z]\d?)?\b/g;

/**
 * Every `theorem` in formal/Arare/, as
 * `{ name, file, ids }`: the fully qualified name (namespaces applied), the
 * repository-relative file, and the AR-* IDs that file cites.
 */
export function leanTheorems() {
  const files = walk(resolve(repoRoot, 'formal/Arare'), ['.lean']).sort();
  const theorems = [];
  for (const file of files) {
    const rel = file.slice(repoRoot.length + 1);
    const text = readFileSync(file, 'utf8');
    const ids = [...new Set(text.match(ID_RE) ?? [])];
    const stack = [];
    for (const line of text.split('\n')) {
      const ns = line.match(/^namespace\s+(\S+)/);
      if (ns) { stack.push(ns[1]); continue; }
      const end = line.match(/^end\s+(\S+)/);
      if (end && stack[stack.length - 1] === end[1]) { stack.pop(); continue; }
      const th = line.match(/^(?:@\[[^\]]*\]\s*)?(?:private\s+)?theorem\s+([^\s(:]+)/);
      if (th) theorems.push({ name: [...stack, th[1]].join('.'), file: rel, ids });
    }
  }
  return theorems;
}
