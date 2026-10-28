// "Did you mean" for a Word name the packaged registry does not hold.
//
// A caller that mistypes a name is the overwhelmingly likely cause of an empty
// `word_contract` result, and the complete name list is sitting in the same
// process. The ranking is fixed so the same misspelling always produces the
// same list: a distance ceiling by name length, at most three suggestions,
// ties broken by name. rust/src/diagnostic_hint.rs ranks the same way for the
// Unknown Word diagnosis; word-candidates.cases.json pins both.

const MAX_CANDIDATES = 3;

function distanceCeiling(length) {
  if (length <= 3) return 1;
  if (length <= 7) return 2;
  return 3;
}

/** Levenshtein distance, two rows at a time. */
function editDistance(left, right) {
  const a = [...left];
  const b = [...right];
  if (a.length === 0) return b.length;
  if (b.length === 0) return a.length;

  let previous = Array.from({ length: b.length + 1 }, (_, i) => i);
  let current = new Array(b.length + 1);
  for (let i = 0; i < a.length; i += 1) {
    current[0] = i + 1;
    for (let j = 0; j < b.length; j += 1) {
      const substitution = previous[j] + (a[i] === b[j] ? 0 : 1);
      current[j + 1] = Math.min(substitution, previous[j + 1] + 1, current[j] + 1);
    }
    [previous, current] = [current, previous];
  }
  return previous[b.length];
}

/**
 * Full-width ASCII (U+FF01–U+FF5E, what a Japanese input method produces for
 * `ＡＤＤ`) folded to its ASCII form, so `ＬＥＮＧＨＴ` suggests `LENGTH`.
 */
function foldFullWidth(text) {
  return text.replace(/[\uFF01-\uFF5E]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xff01 + 0x21));
}

/**
 * Registry names within a plausible typo distance of `word`, best match first.
 * `entries` are `spec/words.json` entries, and a Word is considered under its
 * one name.
 */
export function suggestWords(word, entries) {
  const written = String(word ?? "").trim().toUpperCase();
  const needle = foldFullWidth(written);
  if (!needle) return [];
  const ceiling = distanceCeiling([...needle].length);

  const scored = [];
  const seen = new Set();
  for (const entry of entries ?? []) {
    const upper = String(entry.name).toUpperCase();
    if (upper === written || seen.has(upper)) continue;
    const distance = editDistance(needle, upper);
    if (distance > ceiling) continue;
    seen.add(upper);
    scored.push([distance, upper]);
  }
  // Code-unit order, not localeCompare: the tie-break must not depend on the
  // host's collation, and it is the order the Rust suggester sorts in.
  scored.sort((left, right) => left[0] - right[0] || (left[1] < right[1] ? -1 : left[1] > right[1] ? 1 : 0));
  return scored.slice(0, MAX_CANDIDATES).map(([, name]) => name);
}
