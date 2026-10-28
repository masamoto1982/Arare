// Materializes `limits.json` into runnable cases.
//
// The two oversized inputs are generated rather than committed: a 64 KiB blank
// program and a 4097-digit literal are not files anyone should have to read in
// a diff, and generating them keeps the source exactly one byte or one digit
// past the ceiling it is testing, by construction rather than by counting.

import { readFileSync } from "node:fs";

const spec = JSON.parse(readFileSync(new URL("./limits.json", import.meta.url), "utf8"));

function materialize(probe) {
  if (!probe) return null;
  if (probe.kind === "source") return probe.source;
  if (probe.kind === "generatedSource") {
    if (probe.generator === "spaces") {
      // Whitespace is a legal, no-op program, so nothing but the byte length
      // itself decides the outcome.
      return " ".repeat(probe.bytes);
    }
    if (probe.generator === "digits") {
      return `[ ${"9".repeat(probe.digits)} ]`;
    }
    if (probe.generator === "algebraicCascade") {
      // `(√p₁+√q₁)·(√p₂+√q₂)·…` — the term count doubles with every factor,
      // and multiplying surds is the only way to grow one. `repetitions`
      // applies the same cascade through a user word N times, which
      // accumulates work without growing any single value: the difference
      // between testing a size ceiling and testing the cumulative one.
      const pairs = [
        [2, 3], [5, 7], [11, 13], [17, 19], [23, 29], [31, 37],
        [41, 43], [47, 53], [59, 61], [67, 71], [73, 79], [83, 89],
      ];
      const cascade = pairs
        .slice(0, probe.factors)
        .map(([left, right], index) => `${left} SQRT ${right} SQRT ADD${index > 0 ? " MUL" : ""}`)
        .join(" ");
      if (!probe.repetitions) return cascade;
      return `[ ${cascade} ] 'C' DEF${" C".repeat(probe.repetitions)}`;
    }
    if (probe.generator === "wordChain") {
      // W1 pushes 1 and each Wk calls W(k-1), so calling the last Word nests
      // exactly `words` User Word frames: the call depth is the count, by
      // construction rather than by counting brackets.
      const lines = ["[ 1 ] 'W1' DEF"];
      for (let k = 2; k <= probe.words; k += 1) lines.push(`[ W${k - 1} ] 'W${k}' DEF`);
      lines.push(`W${probe.words}`);
      return lines.join("\n");
    }
    if (probe.generator === "widePowers") {
      // One 4096-digit literal, parsed once into a user word, multiplied in N
      // times. Each product widens the accumulator by the literal's width, so
      // the result's bit length is N times it and the boundary is a count of
      // multiplications rather than a committed 80,000-digit number.
      return `[ ${"9".repeat(probe.digits)} MUL ] 'M' DEF 1${" M".repeat(probe.multiplications)}`;
    }
  }
  throw new Error(`unsupported limit probe: ${JSON.stringify(probe)}`);
}

/** Every declared limit, with its probes materialized where it has them. */
export function limitCases() {
  return Object.entries(spec.limits).map(([name, entry]) => ({
    name,
    coverage: entry.coverage,
    declared: entry.declared,
    rustTest: entry.rustTest,
    note: entry.note,
    probes: ["under", "over"]
      .filter((edge) => entry[edge])
      .map((edge) => ({
        edge,
        source: materialize(entry[edge]),
        expect: entry[edge].expect,
      })),
  }));
}

/** The limit names the fixture claims to cover. */
export function coveredLimitNames() {
  return Object.keys(spec.limits).sort();
}
