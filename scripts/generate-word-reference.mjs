#!/usr/bin/env node
// The Reference and the Playground's word catalog, from spec/words.json
// (AR-HLR-036, AR-LLR-038; CLAUDE.md, rule 5).
//
// Ported in shape from Ajisai's generator of the same name (at the commit
// UPSTREAM.md records): one registry of Words, generated surfaces that are
// never edited by hand, and `--check`, which fails when one is stale. What it
// writes is Arare's own:
//
//   - public/docs/en/index.html and public/docs/ja/index.html: every Word with
//     its syntax, what it does to a number's width, scale, error budget and
//     the field contract, and its samples, each with the result and budget
//     the Playground shows and an "Open in Playground" link that loads it
//     into the editor (`../../index.html#code=…`, src/gui/gui-application.ts);
//   - src/engine/words.generated.json: the Dictionary's Core Words and the
//     text a lookup shows, for the Playground's Arare engine.
//
//   node scripts/generate-word-reference.mjs [--check]

import { readJson, writeOrCheck, fatal } from './lib/common.mjs';

const TAG = 'word-reference';
const { words } = readJson('spec/words.json');

const LANGS = ['en', 'ja'];
const FACETS = ['width', 'scale', 'error', 'field'];

for (const w of words) {
  for (const key of ['name', 'category', 'syntax']) {
    if (typeof w[key] !== 'string' || w[key] === '') fatal(TAG, `${w.name ?? '?'}: no ${key}`);
  }
  for (const key of ['summary', ...FACETS]) {
    for (const lang of LANGS) {
      if (typeof w[key]?.[lang] !== 'string' || w[key][lang] === '') fatal(TAG, `${w.name}: no ${key}.${lang}`);
    }
  }
  if (!Array.isArray(w.examples) || w.examples.length === 0) fatal(TAG, `${w.name}: no example`);
  for (const e of w.examples) {
    if (!Array.isArray(e.result) || !Array.isArray(e.budget) || e.result.length !== e.budget.length) {
      fatal(TAG, `${w.name}: example ${JSON.stringify(e.source)} needs a result and a budget per value`);
    }
  }
}

const TEXT = {
  en: {
    title: 'Arare Reference',
    description: 'Arare Reference: every Word with what it does to width, scale, error budget and the field contract, and samples that open in the Playground',
    tagline: 'Every Word of Arare, generated from spec/words.json.',
    intro: [
      'Arare quantizes by design and accounts for the precision it gives up. A number has a width (8, 16, 32 or 64 bits) and a scale exponent e: its value is its code times 2^e. Arithmetic widens to a width that holds the exact result; rounding happens only at QUANTIZE, RESCALE and DIV.',
      'For every value a program leaves, the error-budget checker infers, without running it, how far Arare\'s value may be from the exact value Ajisai computes. A program is field=closed when the checker bounds every numeric result; otherwise it is field=open, and the checker says which Word and why.',
      'Each sample opens in the Playground, which runs it in Arare and in Ajisai side by side and shows the difference against the budget.',
    ],
    words: 'Words',
    syntax: 'Syntax',
    facets: { width: 'Width', scale: 'Scale', error: 'Error', field: 'field' },
    samples: 'Samples',
    source: 'Source',
    result: 'Result',
    budget: 'Budget',
    noBudget: 'none',
    open: 'Open in Playground',
    playground: 'Back to the Playground',
    language: 'Language',
    categories: {
      numbers: 'Numbers and rounding',
      arithmetic: 'Arithmetic',
      comparison: 'Comparison',
      truth: 'Truth',
      blocks: 'Blocks and aggregates',
      literals: 'Literals',
      names: 'Names',
    },
  },
  ja: {
    title: 'Arare リファレンス',
    description: 'Arare リファレンス：各ワードが幅・スケール・誤差予算・field 契約に何をするか。サンプルは Playground で開ける',
    tagline: 'Arare の全ワード。spec/words.json から生成しています。',
    intro: [
      'Arare は量子化を前提とし、失った精度を数字で管理する言語です。数は幅（8、16、32、64 ビット）とスケール指数 e を持ち、値は符号 × 2^e です。算術は厳密な結果が収まる幅まで広げ、丸めは QUANTIZE、RESCALE、DIV の3か所でだけ起きます。',
      '誤差予算の検査器は、プログラムを実行せずに、残る値ごとに Ajisai が計算する厳密値からどれだけ離れうるかを推論します。すべての数値結果を抑えられたプログラムは field=closed、そうでなければ field=open で、どのワードがなぜ抑えられないかを示します。',
      'サンプルは Playground で開けます。Playground は同じソースを Arare と Ajisai で実行し、差を誤差予算と並べて表示します。',
    ],
    words: 'ワード',
    syntax: '構文',
    facets: { width: '幅', scale: 'スケール', error: '誤差', field: 'field' },
    samples: 'サンプル',
    source: 'ソース',
    result: '結果',
    budget: '誤差予算',
    noBudget: 'なし',
    open: 'Playgroundで開く',
    playground: 'Playground に戻る',
    language: '言語',
    categories: {
      numbers: '数と丸め',
      arithmetic: '算術',
      comparison: '比較',
      truth: '真偽',
      blocks: 'ブロックと集約',
      literals: 'リテラル',
      names: '名前',
    },
  },
};

const escape = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

/** The Playground URL that loads `source` into the editor. */
export const playgroundHref = (source) => `../../index.html#code=${encodeURIComponent(source)}`;

const anchor = (name) => `word-${name.toLowerCase()}`;

function page(lang) {
  const t = TEXT[lang];
  const categories = [...new Set(words.map((w) => w.category))];
  for (const c of categories) if (!t.categories[c]) fatal(TAG, `no ${lang} title for the category ${c}`);
  const nav = categories.map((c) => {
    const links = words.filter((w) => w.category === c)
      .map((w) => `<a href="#${anchor(w.name)}"><code>${escape(w.name)}</code></a>`).join(' ');
    return `        <li><span class="ref-nav-category">${escape(t.categories[c])}</span> ${links}</li>`;
  }).join('\n');
  const entries = words.map((w) => {
    const facets = FACETS.map((f) => `            <dt>${escape(t.facets[f])}</dt><dd>${escape(w[f][lang])}</dd>`).join('\n');
    const samples = w.examples.map((e) => {
      const budgets = e.budget.map((b) => (b === null ? t.noBudget : `± ${b}`)).join('  ');
      return [
        '              <tr>',
        `                <td><code>${escape(e.source)}</code></td>`,
        `                <td><code>${escape(e.result.join('  '))}</code></td>`,
        `                <td>${escape(budgets)}</td>`,
        `                <td><a class="open-playground" href="${escape(playgroundHref(e.source))}">${escape(t.open)}</a></td>`,
        '              </tr>',
      ].join('\n');
    }).join('\n');
    return [
      `      <section class="word-entry" id="${anchor(w.name)}">`,
      `        <h3><code>${escape(w.name)}</code></h3>`,
      `        <p>${escape(w.summary[lang])}</p>`,
      `        <p class="word-syntax">${escape(t.syntax)}: <code>${escape(w.syntax)}</code> <span class="word-reqs">(${w.requirements.map(escape).join(', ')})</span></p>`,
      '        <dl class="word-facets">',
      facets,
      '        </dl>',
      '        <div class="ref-table-wrap">',
      '          <table class="ref-table">',
      `            <caption>${escape(t.samples)}</caption>`,
      `            <thead><tr><th>${escape(t.source)}</th><th>${escape(t.result)}</th><th>${escape(t.budget)}</th><th></th></tr></thead>`,
      '            <tbody>',
      samples,
      '            </tbody>',
      '          </table>',
      '        </div>',
      '      </section>',
    ].join('\n');
  }).join('\n');
  const other = lang === 'en' ? 'ja' : 'en';
  return `<!DOCTYPE html>
<!-- Generated by scripts/generate-word-reference.mjs from spec/words.json; do not edit. -->
<html lang="${lang}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="description" content="${escape(t.description)}">
    <title>${escape(t.title)}</title>
    <link rel="stylesheet" href="../reference.css">
</head>
<body>
  <header class="ref-header">
    <a href="../../index.html" class="ref-brand"><img src="../../images/QR_341216.png" alt="" class="logo"> Arare</a>
    <p class="ref-tagline">${escape(t.tagline)}</p>
    <nav class="ref-lang" aria-label="${escape(t.language)}"><a href="../${other}/index.html" hreflang="${other}">${other === 'ja' ? '日本語' : 'English'}</a></nav>
  </header>
  <main class="ref-main">
    <h1>${escape(t.title)}</h1>
${t.intro.map((p) => `    <p>${escape(p)}</p>`).join('\n')}
    <nav class="ref-nav" aria-label="${escape(t.words)}">
      <ul>
${nav}
      </ul>
    </nav>
    <h2>${escape(t.words)}</h2>
${entries}
  </main>
  <footer class="ref-footer"><a href="../../index.html">${escape(t.playground)}</a></footer>
</body>
</html>
`;
}

/** The Playground's catalog: each Word's Dictionary entry and lookup text. */
function catalog() {
  const entries = words.map((w) => ({
    name: w.name,
    summary: w.summary.en,
    syntax: w.syntax,
    documentation: [
      `${w.name} — ${w.summary.en}`,
      `Syntax: ${w.syntax}`,
      ...FACETS.map((f) => `${TEXT.en.facets[f]}: ${w[f].en}`),
      `Example: ${w.examples[0].source}  →  ${w.examples[0].result.join('  ')}`,
    ].join('\n'),
  }));
  return `${JSON.stringify({ _generated: 'scripts/generate-word-reference.mjs from spec/words.json; do not edit', words: entries }, null, 2)}\n`;
}

const stale = (path) => `${path} is stale; run node scripts/generate-word-reference.mjs`;
writeOrCheck(TAG, [
  ...LANGS.map((lang) => ({ path: `public/docs/${lang}/index.html`, content: page(lang), stale: stale(`public/docs/${lang}/index.html`), missing: stale(`public/docs/${lang}/index.html`) })),
  { path: 'src/engine/words.generated.json', content: catalog(), stale: stale('src/engine/words.generated.json'), missing: stale('src/engine/words.generated.json') },
], { current: 'the Reference and the word catalog are current.', wrote: 'wrote the Reference and the word catalog.' });
