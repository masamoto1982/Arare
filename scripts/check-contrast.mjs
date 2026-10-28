#!/usr/bin/env node
// Gate the Playground's colour tokens on WCAG 2.x contrast.
//
// Every colour token the stylesheets use as text (`color: var(--…)`) must be
// listed below with the surfaces it is drawn on, and must reach 4.5:1
// (WCAG 1.4.3) on each of them. A token used as text that is not listed fails
// the check too, so a new text colour cannot arrive unmeasured. Focus rings,
// listed separately, must reach 3:1 (WCAG 1.4.11).
//
// Two colours are held below 4.5:1 on purpose, by the maintainer's eye rather
// than the formula: placeholder text, meant to be readable when looked at and
// otherwise to recede, and the User Word "dependency" amber, whose lightness is
// what sets it apart from Core's orange-red. They are listed in EXCEPTIONS
// with the reason and the ratio they have now, which becomes their floor — a
// deliberate choice stays one, and does not drift fainter unnoticed.
//
// This exists because contrast was never measured: printed output shipped at
// 1.97:1 and five bracket-depth colours between 1.6:1 and 3.7:1.
//
// Tokens are read from src/styles/playground.css and converted OKLCH → sRGB
// (gamut-clipped, as a browser renders them) → relative luminance.
//
//   node scripts/check-contrast.mjs

import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from './lib/common.mjs';

const stylesDir = join(repoRoot, 'src', 'styles');
const tokensCss = readFileSync(join(stylesDir, 'playground.css'), 'utf8');

const TEXT_MINIMUM = 4.5;
const NON_TEXT_MINIMUM = 3;

// Text token → the surfaces it is drawn on.
const PANEL = ['--color-white', '--color-light'];
const TEXT = {
  '--color-text': [...PANEL, '--color-symbol', '--color-stack-top'],
  '--color-text-light': [...PANEL, '--color-symbol', '--color-stack-top'],
  '--input-text': ['--color-symbol'],
  '--input-placeholder': ['--color-symbol'],
  '--color-primary': PANEL,
  '--color-white': ['--color-primary'],
  '--color-stack': ['--color-white', '--color-stack-top'],
  '--color-nil': ['--color-white', '--color-stack-top'],
  '--color-core': PANEL,
  '--color-dependency': PANEL,
  '--color-non-dependency': PANEL,
  '--color-output-debug': PANEL,
  '--color-output-program': PANEL,
  '--color-output-error': PANEL,
  '--color-output-info': PANEL,
  // Brackets are drawn in the Stack, and the top item sits on its fill.
  ...Object.fromEntries(
    [1, 2, 3, 4, 5, 6, 7, 8, 9].map((depth) => [
      `--bracket-depth-${depth}`,
      ['--color-white', '--color-stack-top'],
    ]),
  ),
};

// Non-text marks → the surfaces they must stand out from.
const NON_TEXT = {
  '--color-primary': [...PANEL, '--color-symbol'], // focus rings
};

// Text colours held below 4.5:1 by design: surface → the floor they may not
// fall below (their ratio when the decision was made).
const EXCEPTIONS = {
  '--input-placeholder': {
    reason: 'placeholder text recedes behind what is typed',
    floors: { '--color-symbol': 1.77 },
  },
  '--color-dependency': {
    reason: 'the light amber is what tells a depended-on User Word from a Core Word',
    floors: { '--color-white': 2.43, '--color-light': 2.3 },
  },
};

function readTokens(css) {
  const raw = new Map();
  for (const match of css.matchAll(/(--[\w-]+):\s*([^;]+);/g)) raw.set(match[1], match[2].trim());
  const resolveToken = (name, seen = new Set()) => {
    if (seen.has(name)) throw new Error(`token cycle at ${name}`);
    const value = raw.get(name);
    if (value === undefined) throw new Error(`playground.css declares no ${name}`);
    const alias = value.match(/^var\((--[\w-]+)\)$/);
    if (alias) return resolveToken(alias[1], new Set([...seen, name]));
    const oklch = value.match(/^oklch\(\s*([\d.]+)%\s+([\d.]+)\s+([\d.]+)\s*\)$/);
    if (!oklch) throw new Error(`${name} is not an opaque oklch() colour: ${value}`);
    return [Number(oklch[1]) / 100, Number(oklch[2]), Number(oklch[3])];
  };
  return resolveToken;
}

function linearSrgb([lightness, chroma, hue]) {
  const a = chroma * Math.cos((hue * Math.PI) / 180);
  const b = chroma * Math.sin((hue * Math.PI) / 180);
  const l = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ].map((channel) => Math.min(1, Math.max(0, channel)));
}

function luminance(colour) {
  const [r, g, b] = linearSrgb(colour);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a, b) {
  const [lighter, darker] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (lighter + 0.05) / (darker + 0.05);
}

const token = readTokens(tokensCss);
const failures = [];
let measured = 0;
for (const [pairs, minimum, kind] of [
  [TEXT, TEXT_MINIMUM, 'text'],
  [NON_TEXT, NON_TEXT_MINIMUM, 'non-text'],
]) {
  for (const [foreground, surfaces] of Object.entries(pairs)) {
    for (const surface of surfaces) {
      const ratio = contrast(token(foreground), token(surface));
      measured += 1;
      const floor = kind === 'text' ? EXCEPTIONS[foreground]?.floors[surface] : undefined;
      const required = floor ?? minimum;
      if (ratio + 0.005 < required) {
        failures.push(
          `${kind} ${foreground} on ${surface}: ${ratio.toFixed(2)}:1, needs ${required}:1` +
            (floor === undefined ? '' : ` (the floor of a design exception: ${EXCEPTIONS[foreground].reason})`),
        );
      }
    }
  }
}
for (const [name, { floors }] of Object.entries(EXCEPTIONS)) {
  for (const surface of Object.keys(floors)) {
    if (!TEXT[name]?.includes(surface)) failures.push(`exception ${name} on ${surface} is not a measured pair`);
  }
}

// Every token the stylesheets draw text in must be measured above.
const usedAsText = new Set();
for (const file of readdirSync(stylesDir).filter((name) => name.endsWith('.css'))) {
  const css = readFileSync(join(stylesDir, file), 'utf8');
  for (const match of css.matchAll(/(?:^|[\s;{])color:\s*var\((--[\w-]+)\)/g)) usedAsText.add(match[1]);
}
for (const name of usedAsText) {
  if (!(name in TEXT)) failures.push(`${name} is used as text colour but not measured in check-contrast.mjs`);
}

if (failures.length) {
  console.error('[contrast] below the WCAG minimum:');
  for (const failure of failures) console.error(`  ${failure}`);
  process.exit(1);
}
console.log(
  `[contrast] ${measured} colour pairs meet WCAG contrast (text ${TEXT_MINIMUM}:1, non-text ${NON_TEXT_MINIMUM}:1) ` +
    `or their recorded design-exception floor (${Object.keys(EXCEPTIONS).join(', ')}); ` +
    `all ${usedAsText.size} text-colour tokens are measured.`,
);
