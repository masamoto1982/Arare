// The two sides of the oracle comparison (ADR-0007), shared by the corpus
// (scripts/check-oracle.mjs, AR-VER-209) and the generated programs
// (scripts/check-oracle-random.mjs, AR-VER-217).
//
// Ajisai runs from the Node build committed under tools/mcp-server/wasm/
// generated/ (the commit UPSTREAM.md records). A prelude gives it Arare's
// Words as their exact counterparts: QUANTIZE, RESCALE and NARROW keep the
// exact value, NEG is `-1 MUL` (SUM is Ajisai's own). Arare runs as
// `arare check --lines` (ARARE_BIN, or rust/target/debug/arare): one process
// for every program, each answered with its stack and the error budget the
// checker infers for every value (AR-HLR-027). The prelude and the
// comparison itself live in src/engine/oracle-compare.mjs, which the
// Playground's comparison panel uses too (AR-LLR-037).

import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { repoRoot } from './common.mjs';
import { PRELUDE, norm } from '../../src/engine/oracle-compare.mjs';

const require = createRequire(import.meta.url);
const ajisai = require(resolve(repoRoot, 'tools/mcp-server/wasm/generated/ajisai_core.js'));

export const arareBin = process.env.ARARE_BIN ?? resolve(repoRoot, 'rust/target/debug/arare');

export { rat, norm, show, showRat, ratLe, within } from '../../src/engine/oracle-compare.mjs';

/** Arare's answers to many programs, one `arare check --lines` run. */
export function runArare(sources) {
  if (!existsSync(arareBin)) {
    throw new Error(`${arareBin} does not exist; build it with cargo build -p arare-cli (in rust/) or set ARARE_BIN.`);
  }
  for (const s of sources) if (s.includes('\n')) throw new Error(`a program must be one line: ${s}`);
  const r = spawnSync(arareBin, ['check', '--lines', '-'], {
    input: sources.join('\n') + '\n',
    encoding: 'utf8',
    maxBuffer: 1 << 30,
  });
  if (r.status !== 0) throw new Error(`arare check failed: ${r.stderr}`);
  const lines = r.stdout.trimEnd().split('\n');
  if (lines.length !== sources.length) throw new Error(`arare answered ${lines.length} of ${sources.length} programs`);
  return lines.map((l) => {
    const out = JSON.parse(l);
    return out.ok ? { stack: out.stack.map(norm), field: out.field } : { error: out.error };
  });
}

/** Ajisai's answer to one program, or its refusal. */
export async function runAjisai(source) {
  const out = JSON.parse(await ajisai.agent_compute(`${PRELUDE}\n${source}`));
  if (out.error) return { error: out.message ?? JSON.stringify(out.error) };
  return { stack: out.stack.map(norm) };
}
