// The three-and-a-half levels of "same Word" the work order counts
// convergence at (§2.2), plus D0α, which the pilot added after measuring
// that DIGEST distinguishes bodies differing only in their BIND names.
//
//   D0   DIGEST of the Word as defined                       (sound; one-directional)
//   D0α  DIGEST after renaming bound variables canonically    (sound; coarser than D0)
//   D1   the engine's answers on a fixed probe battery        (depends on the battery)
//
// D2 (CONTRACT shape) is not computed: CONTRACT reads a bound name as an
// unresolved Word and answers `inputs: variable` for any body using BIND,
// which is most of what agents write (measured in the pilot).

import { createHash } from 'node:crypto';
import { alphaNormalize, closure, norm, prelude } from './source.mjs';

// Scalars, one- and many-element Vectors, nesting, NIL, text and a Record:
// §2.2 requires scalars and one-element Vectors both, since `[ 2 ] MUL` and
// `2 MUL` agree on everything else.
const PROBES = [
  '0', '1', '-2', '7/3',
  '[ 2 ]', '[ 3 1 4 1 5 ]', '[ 10 -2 7/2 ]', '[ [ 1 2 ] [ 3 4 ] ]',
  'NIL', "'hello world'", "'a rose is a rose'", "[ 'a' 'b' ] [ 1 2 ] RECORD",
  // Irregular text: without these, a word-splitting title-caser and a
  // character-scanning one answered alike on every probe (pilot, 2026-09-23).
  "''", "'x  y'", "' lead'",
];
const PAIR_PROBES = ['1', '-2', '[ 2 ]', '[ 3 1 4 ]', "'ab cd'"];

function probeTuples() {
  const tuples = [[]];
  for (const p of PROBES) tuples.push([p]);
  for (const a of PAIR_PROBES) for (const b of PAIR_PROBES) tuples.push([a, b]);
  return tuples;
}

const digestOf = (result) => (result.status === 'ok' ? result.stackDisplay?.at(-1) ?? null : null);

export async function identify(ajisai, name, byName) {
  const defs = closure([name], byName);
  const setup = prelude(defs);
  const alphaSetup = prelude(defs.map((d) => ({ name: d.name, body: alphaNormalize(d.body) })));
  const d0 = digestOf(await ajisai.compute(`${setup}\n[ ${name} ] 0 GET DIGEST`));
  const d0a = digestOf(await ajisai.compute(`${alphaSetup}\n[ ${name} ] 0 GET DIGEST`));

  const answers = [];
  for (const tuple of probeTuples()) {
    const run = await ajisai.compute(`${setup}\n${tuple.join(' ')} ${name}`);
    // An error answers with its category: since the server stopped sending a
    // diagnosis (ajisai-mcp-server 0.9.0) every error read as one answer, so
    // two Words failing differently on a probe agreed there.
    answers.push(run.status === 'ok' ? run.stackDisplay : `error:${run.error?.category ?? run.status}`);
  }
  const d1 = createHash('sha256').update(JSON.stringify(answers)).digest('hex').slice(0, 16);
  // A Word that errors on every probe says nothing about what it computes.
  const informative = answers.some((a) => Array.isArray(a));
  return { name: norm(name), d0, d0a, d1: informative ? d1 : null };
}
