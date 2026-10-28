#!/usr/bin/env node
// The canonical lexical grammar, executed.
//
// Everything this module does is read out of `spec/grammar.json`. It hardcodes
// no character, no token spelling and no rule order — only the closed set of
// action and matcher names the grammar is allowed to use. That is the whole
// point: a grammar file nothing executes is one more ledger of assertions, and
// Ajisai already has those. Held against the Rust tokenizer by
// `rust/../tests` and by `scripts/check-grammar.mjs`, this turns
// `spec/grammar.json` into a truth condition instead of a description.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const ACTIONS = new Set([
  'skip',
  'consumeToLineTerminator',
  'scanStringLiteral',
  'scanToWhitespace',
]);

const MATCHERS = new Set([
  'literal',
  'literalAsciiCaseInsensitive',
  'containsAny',
  'pattern',
  'charClass',
  'otherwise',
]);

export function loadGrammar(repoRoot) {
  return JSON.parse(
    readFileSync(resolve(repoRoot, 'spec/grammar.json'), 'utf8'),
  );
}

function codepointPredicate(spec) {
  const ranges = (spec?.codepoints ?? []).map((entry) => {
    const [lo, hi] = entry.split('-');
    return [Number.parseInt(lo, 16), Number.parseInt(hi ?? lo, 16)];
  });
  return (ch) => {
    const cp = ch.codePointAt(0);
    return ranges.some(([lo, hi]) => cp >= lo && cp <= hi);
  };
}

function asciiCaseFold(s) {
  // Rust's eq_ignore_ascii_case folds ASCII letters only. String#toUpperCase
  // is Unicode-aware and would additionally fold e.g. 'ı' onto 'I', which
  // would make this lexer accept names the implementation does not.
  let out = '';
  for (const ch of s) {
    const cp = ch.codePointAt(0);
    out += cp >= 0x61 && cp <= 0x7a ? String.fromCodePoint(cp - 32) : ch;
  }
  return out;
}

/**
 * Build a lexer from the grammar data. Returns `lex(input)` yielding either
 * `{ tokens }` or `{ condition }` naming one entry of `grammar.sourceErrors`.
 */
export function makeLexer(grammar) {
  const isWhitespace = codepointPredicate(grammar.characterClasses.whitespace);
  const isLineTerminator = codepointPredicate(
    grammar.characterClasses.lineTerminator,
  );
  const numericPattern = new RegExp(grammar.numericGrammar.pattern, 'u');
  const namedPatterns = new Map([
    ['numeric', numericPattern],
  ]);

  const scanPhase = grammar.phases.find((p) => p.id === 'scan');
  const positionRules = scanPhase.positionRules;

  const rejectedChar = new Map();
  for (const group of scanPhase.rejectedCharacters ?? []) {
    for (const ch of group.chars) rejectedChar.set(ch, group.condition);
  }

  for (const rule of positionRules) {
    if (!ACTIONS.has(rule.action)) {
      throw new Error(
        `[reference-lexer] grammar names unknown action "${rule.action}" in position rule "${rule.id}"`,
      );
    }
  }

  const matcherName = (matcher) => {
    const keys = Object.keys(matcher);
    if (keys.length !== 1 || !MATCHERS.has(keys[0])) {
      throw new Error(
        `[reference-lexer] grammar uses unknown matcher ${JSON.stringify(matcher)}`,
      );
    }
    return keys[0];
  };

  const classPredicates = new Map();
  const predicateFor = (name) => {
    let predicate = classPredicates.get(name);
    if (!predicate) {
      const cls = grammar.characterClasses[name];
      if (!cls) {
        throw new Error(
          `[reference-lexer] grammar references unknown character class "${name}"`,
        );
      }
      predicate = codepointPredicate(cls);
      classPredicates.set(name, predicate);
    }
    return predicate;
  };

  const guardHolds = (matcher, ch) => {
    switch (matcherName(matcher)) {
      case 'charClass':
        return predicateFor(matcher.charClass)(ch);
      case 'literal':
        return ch === matcher.literal;
      case 'otherwise':
        return true;
      default:
        throw new Error(
          `[reference-lexer] matcher ${matcherName(matcher)} is not valid as a position guard`,
        );
    }
  };

  const classifyLexeme = (lexeme) => {
    for (const rule of grammar.lexemeRules) {
      let hit = false;
      switch (matcherName(rule.match)) {
        case 'literal':
          hit = lexeme === rule.match.literal;
          break;
        case 'literalAsciiCaseInsensitive':
          hit =
            asciiCaseFold(lexeme) ===
            asciiCaseFold(rule.match.literalAsciiCaseInsensitive);
          break;
        case 'containsAny':
          hit = rule.match.containsAny.some((c) => lexeme.includes(c));
          break;
        case 'pattern': {
          const pattern = namedPatterns.get(rule.match.pattern);
          if (!pattern) {
            throw new Error(
              `[reference-lexer] grammar references unknown named pattern "${rule.match.pattern}"`,
            );
          }
          hit = pattern.test(lexeme);
          break;
        }
        case 'otherwise':
          hit = true;
          break;
        default:
          throw new Error(
            `[reference-lexer] matcher ${matcherName(rule.match)} is not valid as a lexeme matcher`,
          );
      }
      if (!hit) continue;
      if (rule.condition) return { condition: rule.condition };
      return { token: { id: rule.emits, value: lexeme } };
    }
    throw new Error(
      `[reference-lexer] lexeme rules are not total: nothing classified ${JSON.stringify(lexeme)}`,
    );
  };

  const pairs = grammar.delimiterPairs ?? [];
  const openTokenOf = new Map(pairs.map((pair) => [pair.openToken, pair]));
  const closeTokenOf = new Map(pairs.map((pair) => [pair.closeToken, pair]));

  const structuralValidation = (tokens) => {
    const delimiters = [];
    for (const token of tokens) {
      if (openTokenOf.has(token.id)) delimiters.push(openTokenOf.get(token.id));
      else if (closeTokenOf.has(token.id)) {
        const pair = closeTokenOf.get(token.id);
        if (delimiters.pop() !== pair) return pair.conditions.unexpectedClose;
      }
    }
    const unclosed = delimiters[delimiters.length - 1];
    return unclosed ? unclosed.conditions.unclosed : null;
  };

  return function lex(input) {
    const chars = Array.from(input);
    const tokens = [];
    let i = 0;

    while (i < chars.length) {
      const rule = positionRules.find((r) => guardHolds(r.guard, chars[i]));

      if (rule.action === 'skip') {
        i += 1;
        continue;
      }

      if (rule.action === 'consumeToLineTerminator') {
        while (i < chars.length && !isLineTerminator(chars[i])) i += 1;
        continue;
      }

      if (rule.action === 'scanStringLiteral') {
        const close = grammar.stringLiteral.close;
        let j = i + 1;
        let contents = '';
        let closed = false;
        while (j < chars.length) {
          if (chars[j] === close) {
            if (j + 1 >= chars.length || isWhitespace(chars[j + 1])) {
              closed = true;
              j += 1;
              break;
            }
          }
          contents += chars[j];
          j += 1;
        }
        if (!closed) return { condition: grammar.stringLiteral.condition };
        tokens.push({ id: 'String', value: contents });
        i = j;
        continue;
      }

      // scanToWhitespace
      const start = i;
      while (i < chars.length && !isWhitespace(chars[i])) {
        const condition = rejectedChar.get(chars[i]);
        if (condition) return { condition };
        i += 1;
      }
      const classified = classifyLexeme(chars.slice(start, i).join(''));
      if (classified.condition) return { condition: classified.condition };
      tokens.push(classified.token);
    }

    const structural = structuralValidation(tokens);
    if (structural) return { condition: structural };

    return { tokens };
  };
}
