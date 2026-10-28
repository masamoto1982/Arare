# 四値から三値への移行（2026-10-12）

Status: non-canonical. 正典は `spec/language-semantics.md` の LANG.VALUES.TRUTH。
本書は判断の理由だけを記録する。

## 何を変えたか

真理値を Belnap の四値（TRUE・FALSE・BOTH・UNKNOWN）から、SQL と同じ強 Kleene の
三値（TRUE・FALSE・UNKNOWN）にした。UNKNOWN は以前から独立した値ではなく、真偽の
位置で読まれる NIL なので、実体は BOTH の廃止だけである。

- `BOTH` ワードを廃止し、`spec/retired-words.json` に登録した（未知の名前になる）。
- `TRUE FALSE RECONCILE` は BOTH ではなく、ほかの食い違いと同じ `NIL(conflict)`。
- `AND` と `NOT` の表は 3×3 と 1×3。四値の表を三値に制限したものと同じなので、
  BOTH を使わないプログラムの答えは変わらない。
- `SELECT` の BOTH 規則（両候補の RECONCILE）と、`FILTER` が BOTH を残す規則は消えた。
  `FILTER` は SQL の `WHERE` と同じく TRUE の要素だけを残す。
- ホストプロトコルの `semantics.truthValue` は `"true"` と `"false"` だけになった。

後方互換は取らない（作者の判断）。

## なぜ

- **失うものが小さい。** BOTH が表していた「矛盾した」は、NIL の理由 `conflict` で
  表せる。`NIL-REASON` を読めば「何も知らない」と「情報源が食い違った」を区別できる
  ので、区別は消えず、真理値表の外へ出るだけ。
- **境界がそろう。** SQL、JSON、多くのホスト言語に BOTH はない。BOTH は
  `JSON-ENCODE` で `domainMiss` になるなど、境界ごとに特別扱いが要った。
- **予想を外す規則が消える。** `BOTH NIL AND` が FALSE、その双対が TRUE という規則は、
  束としては正しくても、読む側（人間でも AI でも）の予想を外しやすかった。
- **規則とコードが減る。** `rust-10k-reduction-plan.md` の段 7。

## 変えなかったもの

- 零形（`0`、有限の非零、`1/0`、`0/0`）の四分類と、それに基づく契約推論
  （`field`・`shapes`）。零形は抽象領域であって真理値ではない。二ビットとして読めば
  Belnap の FOUR と同じ束になるが、仕様ではその対応を Informative の段落に下げた。
- `knowledgeOrder` と `truthOrder` という語の族の名前。前者は「値について何が
  分かっているか」の順序、後者は FALSE < UNKNOWN < TRUE で、三値でも意味を持つ。
- NIL の伝播。SQL の `SUM` は NULL を飛ばすが、Ajisai の集約は NIL を伝える。
  「黙って捨てない」は Ajisai の核であり、ここは意図して SQL と違う。

## 消したテスト

- `zero_shape_lattice_laws.rs`：零形を FOUR の真理値として読み、四演算の正方形
  （⊗・⊕・∧・∨）を検証していた。真理値が三値になり、言語上の意味を失った。
- `belnap_truth_conformance_tests.rs` は `reconcile_conformance_tests.rs` に改め、
  BOTH の表を除いて `RECONCILE` の半束の法則だけを残した。三値の表は
  `kleene_truth_conformance_tests.rs` が持つ。
