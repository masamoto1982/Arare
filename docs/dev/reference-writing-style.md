# Ajisai Reference 執筆規約

Ajisaiのドキュメント（`?` (LOOKUP) で表示するビルトイン説明、およびヘッダーの Reference ボタンから開くページ群）における執筆規約。目的は **Ajisaiプログラムと自然言語の説明を視覚的・形式的に区別する** こと。

## 位置づけ

- **非正典。** 本書は書き方の規約であり、言語意味論を定義しない。言語意味論の正準文書は `SPECIFICATION.html`（Specification Authority 節）のみ。
- Reference・LOOKUP・hover は正準仕様から派生する文書面であり、仕様と矛盾する場合は仕様が優先する。
- 表記全般の共通規律は `docs/dev/ajisai-authoring-style.md` に従う。

## 規約

1. ドキュメント本文は Markdown で記述する。
2. Ajisaiコードは fenced code block `` ```ajisai `` … `` ``` `` の中だけに書く。
3. 実行結果やスタック状態など、Ajisaiソースではない出力例は `` ```text `` で囲む。
4. 本文中で記号やWord名を示すときはインラインコード（`` `+` ``, `` `NIL` ``）を使う。
5. 本文の地の文に `[ 1 2 3 ] +` のような裸のAjisaiコードを書かない。書くなら必ずコードブロックに入れる。
6. `# →` のような行内コメントで結果を併記する古い書式は使わない。コードと結果はブロックを分ける。
7. Referenceは現在のAjisaiだけを、Ajisai自身の言葉で述べる。「以前のAjisaiではこうだったが、今はこうだ」という経緯や、「他の言語ではこうだが、Ajisaiではこうだ」という対比で説明しない。ないものを挙げて説明するのではなく、あるものを述べる。経緯は `docs/dev/` に置く。

## 補足

- Bubble Rule の説明では、「できなかった → `NIL` / そもそも使い方が違う → エラー」という言い方を採用する。「泡」という訳語は使わない(2026-09-14 に Reference から水のメタファーを撤去したため)。ただし LOOKUP 本文は UTF-8 English plain text を維持し、`Bubble/NIL` と書く。
- `:` `「」` `（）` `、` `。` などの記号は Markdown 本文の通常文字として自由に使ってよい。Ajisai側で予約・温存する必要はない（Ajisai構文は本規約の影響を受けない）。
- ユーザー定義Wordに対する `?` は Ajisaiソースコードをそのまま返す（編集用途）。これは文書ではないので本規約の対象外。

## 推奨テンプレート（ビルトインWord）

```
# <記号> - <名称>

## 機能
<散文による説明>

## 使用法
​```ajisai
<シグネチャ的な使用形>
​```

## 使用例
​```ajisai
<入力コード>
​```
Result:
​```text
<期待出力>
​```

## 注意
- 箇条書きの注意事項
```

## 適用先

| 表示面 | 保管場所 |
|---|---|
| `?` (LOOKUP) のビルトイン説明 | `rust/src/builtins/detail-lookup-*.rs` の raw string literal |
| Reference ボタンから開くページ | `public/docs/en/index.html`・`public/docs/ja/index.html`（英日で内容を対応させる。共通の見た目と振る舞いは `public/docs/reference.css`・`reference.js`、構成は分割の木（プログラム＝要素の並び→区切りと評価→要素の4種類→名前＝束縛かワード→ワードの族→個々のワード）。分けて語れなくなったところで終わる。「ワード」頁の組み込みワード一覧と各族の頁末尾の契約は `scripts/generate-word-reference.mjs` が生成し、日本語の説明は `docs/i18n/word-summaries.ja.json`） |

ビルトイン説明テキストは段階的に本規約へ移行する。基準実装は `ADD`（`rust/src/builtins/detail-lookup-arithmetic-logic.rs`）。
