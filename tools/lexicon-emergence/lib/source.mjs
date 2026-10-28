// Source-level operations the experiment needs and the language does not
// answer for it: splitting text into tokens, finding which user Words a
// piece of source names, renaming bound variables canonically, and
// expanding user Words away so a solution can be checked in Core alone.
//
// This is a tokenizer for the experiment's own bookkeeping, not a second
// implementation of the grammar: every result it feeds into is run through
// the real engine before anything is concluded from it.

/** Split source into whitespace-delimited tokens, keeping a quoted string whole. */
export function tokenize(source) {
  const tokens = [];
  let i = 0;
  while (i < source.length) {
    if (/\s/.test(source[i])) {
      i += 1;
      continue;
    }
    if (source[i] === "'") {
      let j = i + 1;
      // A string closes at a quote followed by whitespace or end of input.
      while (j < source.length && !(source[j] === "'" && (j + 1 === source.length || /\s/.test(source[j + 1])))) {
        j += 1;
      }
      tokens.push(source.slice(i, j + 1));
      i = j + 1;
      continue;
    }
    let j = i;
    while (j < source.length && !/\s/.test(source[j])) j += 1;
    tokens.push(source.slice(i, j));
    i = j;
  }
  return tokens;
}

const isString = (token) => token.startsWith("'");

/** Canonical (upper-case) form of a name token. */
export const norm = (token) => token.toUpperCase();

/** The user-Word names, out of `known`, that `source` names. */
export function referencedWords(source, known) {
  const names = new Set();
  for (const token of tokenize(source)) {
    if (!isString(token) && known.has(norm(token))) names.add(norm(token));
  }
  return names;
}

/**
 * Rename every name a body binds with BIND to _B0, _B1, … in order of first
 * binding. Two bodies that differ only in what they call their locals come
 * out textually identical, which DIGEST alone does not see.
 */
export function alphaNormalize(source) {
  const tokens = tokenize(source);
  const renames = new Map();
  for (let i = 1; i < tokens.length; i += 1) {
    if (norm(tokens[i]) === 'BIND' && isString(tokens[i - 1])) {
      const bound = norm(tokens[i - 1].slice(1, -1));
      if (!renames.has(bound)) renames.set(bound, `_B${renames.size}`);
    }
  }
  return tokens
    .map((token) => {
      if (isString(token)) {
        const inner = norm(token.slice(1, -1));
        return renames.has(inner) ? `'${renames.get(inner)}'` : token;
      }
      return renames.get(norm(token)) ?? token;
    })
    .join(' ');
}

/**
 * Replace each user Word named in `source` by `[ body ] EXEC`, recursively,
 * so the result names Core Words only. `definitions` maps a normalized name
 * to its body text.
 */
export function expand(source, definitions) {
  return tokenize(source)
    .map((token) => {
      if (isString(token) || !definitions.has(norm(token))) return token;
      return `[ ${expand(definitions.get(norm(token)), definitions)} ] EXEC`;
    })
    .join(' ');
}

/** The DEF lines that make `definitions` (a list of { name, body }) exist, in order. */
export function prelude(definitions) {
  return definitions.map(({ name, body }) => `[ ${body} ] '${name}' DEF`).join('\n');
}

/**
 * Order definitions so every Word comes after the Words it names, and keep
 * only those reachable from `roots`. Unknown names are left to the engine.
 */
export function closure(roots, byName) {
  const known = new Set(byName.keys());
  const ordered = [];
  const seen = new Set();
  const visit = (name) => {
    if (seen.has(name) || !byName.has(name)) return;
    seen.add(name);
    for (const dependency of referencedWords(byName.get(name).body, known)) visit(dependency);
    ordered.push(byName.get(name));
  };
  for (const root of roots) visit(norm(root));
  return ordered;
}
