# Ajisai Minimal Core — 幹を定める、亜種を生む前に

> Status: **Non-canonical / 設計メモ.** 本書は言語意味論を一切定義しない。
> 正典は `SPECIFICATION.html` のみ。本書は「Ajisai の *同一性* をどの語・どの法則が
> 担うのか」を切り分け、`Ajisai Minimal Core` を宣言するための手続き文書である。
> 関連正典: `spec/language-semantics.md`（正典順位は LANG.AUTHORITY.SOURCES）。
> Core Words の契約定義・辞書語彙階層（core/module/alias/surface）は
> `SPECIFICATION.html` と `docs/word-manifest.json` に委ねる。
> 関連データ: `docs/formalization-coverage.json`・`docs/word-manifest.json`。
> 本書はセルフホスティングを「実装言語の選択が一つ増えるだけ」と位置づけ、
> capability-gated kernel profile（公式ビルドだけが実行できる語という特権階層）を
> 採らない立場に立つ（§7）。
>
> 注記（2026-10-08）: 本書が前提とする連分数エンジン（`continued_fraction.rs`、`>CF`）は
> 現在のツリーには無い。無理数は多重二次体の正規形（`rust/src/types/exact/algebraic.rs`）
> で保持され、連分数は有理近似を導出するときだけ使い捨てで計算される。本文中の
> 連分数への言及は当時の実装を指す。
>
> 注記（2026-10-15）: §1〜§8 の語数・エントリ数（表層語彙 215、core 97、module 88 など）と
> 語名（`OR-NIL` `KEEP` `IMPORT` `MOD` など）は本書を書いた 2026-09 時点のもので、その後の
> 最小 Core への削減でモジュール・別名とともに消えた。現在の語彙は 77 語（Kernel 49・
> Standard 28）で、数の記録は `docs/word-manifest.json` の `counts`。付録 B は現在の
> 事実に合わせて改めた。

## 0. なぜ幹を定めるのか

紫陽花（植物）は品種改良が盛んで、実に多様な園芸品種がある。それらが「紫陽花」で
あり続けるのは、花序・装飾花・土壌 pH に応じた発色といった**変わらない幹**を共有する
からである。プログラミング言語 Ajisai も同じ道を歩むには、多様な拡張を許しつつ
「これが変われば Ajisai ではない」という幹＝**Minimal Core** を明示する必要がある。

本書は二つの実務的懸念を**同時に**解く。

1. **中心概念が多く、価値が分散している。** 表層語彙は 215（core 97 / module 88 /
   alias 20 / surface 10, `docs/word-manifest.json`）。初見の利用者・AI に「Ajisai は
   結局、何を最も簡単にする言語か」が伝わりにくい。
2. **信頼すべき実装（trusted core）が大きい。** Rust はテスト込みで約 73k 行、
   最大は `continued_fraction.rs`（約 200KB）。

この二つは別問題に見えて、根は一つ——**「幹（Core）と派生（Derived）の境界が
宣言されていない」**という一点である。境界を宣言すれば懸念 1 が、境界の内側を縮めて
外側を Ajisai 自身で書き直せば（セルフホスト）懸念 2 が解ける。

## 1. 出発点 — Ajisai の本質的価値

Ajisai の最大の成果は、継続分数やベクトル演算**そのものではない**。

> 「値がない」「まだ分からない」「プログラムが間違っている」を分離し、それらを
> 隠さず流す計算モデルを、仕様・実装・検証・AI 向け契約まで一貫して構築していること。

したがって Minimal Core は、この本質的価値を担う語と法則から定義しなければならない。
実装の都合（30 代数プリミティブの並び）から定義してはならない。**幹は同一性であり、
同一性はこの三分離とその伝播規律にある。**

## 2. 30 代数プリミティブは同一性の観点で等価ではない

`docs/formalization-coverage.json` の `algebra_primitives` を `algebraic_family` で
束ねると、本質的価値に照らして三層に自然分離する。

### 2.1 同一性の層（identity）— 「在る／無い／未決／不正」を分けて隠さず流す

| family / primitive | 担う同一性 | 語（代表） | 形式化 |
|---|---|---|---|
| `bubble.domain/passthrough/handler`（#10-12） | **値がない**を理由付きで透過 | `NIL` `OR-NIL` | §5 Bubble モナド |
| `k3.domain/meet/join/involution`（#1-4） | **まだ分からない**を論理で GLB/LUB 透過 | `TRUE` `FALSE` `AND` `OR` `NOT` | §4 Kleene 3 値代数 K3 |
| `exact-real.order`（#6） | 厳密体上の比較は**常に決まる**（TRUE か FALSE） | `EQ` `LT` `GT` | §3.3 厳密順序 |
| `observation.structured-diagnostic`（#27）+ `capability.check`（#29） | **プログラムが間違っている**を、ホスト例外を漏らさず構造化診断として観測 | 構造化診断・capability 不足 | §8 観測関数と同一性 |

値空間 V が**直和（direct sum）**であり各成分が互いに素（disjoint）であること——
#1 は「numbers と素」、#10 は「FALSE・数値ゼロ・論理 UNKNOWN と素」と明記——が、
この「分離」の実体である。null / NaN / false / 例外を混ぜないという設計は、比喩では
なくデータ構造レベルの非交差性として保証されている（§1.1 値空間 V）。

### 2.2 流れの層（flow）— 値を運ぶ機構

| family / primitive | 役割 | 語（代表） | 形式化 |
|---|---|---|---|
| `state-transformer.combinator/composition/identity`（#21-23） | Σ = Stack × Dict × Eff 上の合成モノイドと恒等 | `COND` `EXEC` `EVAL` `FLOW` `IDLE` | §2 状態変換子モノイド |
| `modifier.consumption/region`（#17-20） | 消費・領域選択の直交軸 | `KEEP` `EAT` `STAK` `TOP` `FORC` | §6 修飾子コンビネータ |
| `dictionary.lookup/finite-partial-map`（#24-25） | 決定的名前解決・有限部分写像 | `DEF` `IMPORT` `LOOKUP` `record` | §9-quinquies/F 辞書 |
| `eff.append`（#26） | 自由モノイド効果ログ Eff への追記機構 | （効果の土台） | §9-sexies/G 効果代数 |

### 2.3 素材の層（material）— 強力だが同一性ではない → 派生ライブラリ／セルフホスト対象

| family / primitive | 内容 | 語（代表） | 形式化 |
|---|---|---|---|
| `exact-arithmetic`（#5,7,8） | bihomographic・continued-fraction・gosper | `ADD` `SUB` `MUL` `DIV` `MOD` `FLOOR` `>CF` | §3.1-3.2 数＝行列積 |
| `exact-scalar`（#9） | codepoint 列としての文字列 | `CHARS` `JOIN` `TRIM` `TOKENIZE` | §9-octies |
| `structure-lift`（#13-16） | 添字函手・zip・reshape 群（ベクトル/テンソル） | `MAP` `FILTER` `FOLD` `UNFOLD` `SCAN` `RESHAPE` | §7 ベクトル/テンソル |
| `observation.digest`（#28） | 正準直列化上の決定的ダイジェスト | `CRYPTO@HASH` | §8 観測代数 |
| `handle.domain`（#30, 既に Exploratory） | 子ランタイム/監督ハンドル | `SPAWN` `AWAIT` `KILL` `MONITOR` | §9-septies |
| hosted effects（#29 経由） | 能力ゲート付き副作用 | `NOW` `RANDOM` `PRINT` `IO@*` `MUSIC@*` `JSON@*` `TIME@*` `MATH@*` | §9-sexies/G |

**ChatGPT の本質的価値と完全に一致する:** 継続分数もベクトルも素材の層に落ちる。
同一性は 2.1 の第一層だけである。

## 3. 境界の決定 — Minimal Core = 同一性 ＋ 流れ

Minimal Core は **2.1（同一性）＋ 2.2（流れ）** とする。素材の層（2.3）は Core の外、
派生ライブラリ（後方互換保証の対象外・セルフホスト対象）とする。

- **同一性のみ（最小）は採らない。** 純粋だが、算術もベクトルも失うため、Core 単体で
  意味のある計算例を書けない。「使える最小核」として弱い。
- **既存 30 をそのまま Core とはしない。** 実装形状に忠実だが、同一性と素材が混在し、
  「Ajisai の中心価値」を語彙として絞り込む効果が出ない。
- 採る境界（同一性＋流れ）は、Bubble / UNKNOWN / 診断が**実際に「流れる」**ための
  最小限の運搬機構（修飾子・COND・辞書解決・EVAL）まで含む、自己完結した使える核。

## 4. Minimal Core の定義（語のリストではなく、値モデル＋規律＋流れ）

Minimal Core は次の三部で定義される。**語の集合ではなく、この構造が Ajisai の同一性**である。

1. **値空間 V の分離（§1.1）.** V は互いに素な直和成分を持つ:
   数・真偽（K3, U を含む）・文字・Bubble（NIL）。この非交差性そのものが
   「値がない／まだ分からない／不正」の分離である。
2. **三つの伝播規律.**
   - Bubble パススルー: 理由付き NIL は派生語を**変えずに透過**する（#11）。
   - Kleene 強 meet/join/involution: U は論理を GLB/LUB で透過する（#2-4）。
     `FALSE AND U → FALSE`, `TRUE OR U → TRUE`。
   - 構造化診断: error はホスト例外の漏洩ではなく、観測可能な構造化値（#27）。
3. **流れ（§2）.** 状態変換子モノイドの合成。値がその上を流れる基盤（COND・修飾子・
   辞書解決・EVAL）。

**幹と枝の関係（本書の要）:** Core は算術やベクトルを**所有しない**。しかし
**素材の層のあらゆる語は Core の伝播規律に拘束される**。`ADD` は派生語だが、
NIL を受ければ Core の Bubble パススルーに従って NIL を透過し、比較 `LT` は素材の
厳密順序を用いつつ Core の order 契約（常に決まる）に従う。これが
「単なる小さな標準ライブラリ」ではなく**カーネル**である理由——Core は演算アルゴリズムを
持たず、**欠落・未決・不正が演算をどう貫流するかの法則**を持つ。

## 5. 一つの微妙な決定 — 数の「領域」は Core、数の「演算」はライブラリ

直和 V は成分として数を**含まねばならない**（数が在るからこそ、数を真偽・NIL から
分離できる）。しかし数の上の**演算**（`ADD` `MUL`、継続分数の gosper 正規化）は
素材の層に置き、派生ライブラリとする。

- Core が持つ: 「数は V の一成分である」という領域宣言と、比較 `LT/EQ/…` の
  **契約**（結果は真偽であり、未決なら U）。
- Core が持たない: `ADD/MUL` の**実装**、連分数エンジン（§3.1-3.2）。

この分離は境界の選択（同一性＋流れ）の直接の帰結だが、比較語は「結果は同一性・
オペランドは素材」という**境界語**である点に注意が必要。比較語の *契約* は Core、
*実装* は素材。この点は仕様提案時に §7 の Core Word contract で明示的に裁定する
（未確定事項として記録）。

## 6. 縮小効果と、正直な限界

- **概念の分散（懸念 1）:** 「Ajisai とは何か」の答えが *215 語の看板* から
  *値の四分離＋三伝播規律＋流れ* に収束する。学ぶべき中心が固定される。
- **trusted core（懸念 2）:** 素材の層の語を Rust ビルトインから Ajisai prelude へ
  セルフホスト移送すれば、その分だけ「壊れると言語同一性が崩れる領域」が縮む。
- **正直な限界:** セルフホストで縮むのは**語彙の実装**であって**数値エンジンではない**。
  `continued_fraction.rs`（200KB）は素材だが Ajisai で書き直せず trusted core に残る。
  数値エンジンの規模と性能モデルは、本書とは別課題（利用者向けコストモデルの明文化）
  として扱う。Minimal Core の縮小効果を過大に主張しないこと。

## 7. 非目標との整合

**capability-gated kernel profile /「公式ビルドだけが実行できる語」**という
特権階層は本書でも採らない。Minimal Core は
**文書上の同一性宣言**であって、実行特権の層ではない。移植性目標（§2.4）と
`PORTABILITY.md` 原則 2（実装は参照実装の一つにすぎない）を保つ。Core の後方互換
保証は「仕様が定義する同一性を破らない」という規律であり、特定ビルドの特権ではない。

## 8. 次の手順（段階的・後戻り可能）

1. **境界の命名（コード変更ゼロ）— ✅ 実装済み.** `formalization-coverage.json` の
   30 プリミティブと 213 エントリに `core_tier: identity | flow | material | sugar` を
   付与した（同ファイル `core_tier_summary` に決定規則を記録）。規則は優先順位付きの
   決定的導出——(1) Sugar→sugar、(2) Exploratory→material、(3) moduleword→material
   （Hosted Modules §9.3）、(4) 残りは home `algebraic_family`（observation は digest/handle
   由来なら material、他は identity）。`derived_from` は「語が従う法則の全列挙」であって
   「語が何であるか」ではないため、第一信号には使わない。`core_tier` は
   `check-formalization-coverage.mjs` で必須化し、`word-manifest.json` へも伝播させた。
   結果: **Minimal Core（identity+flow）= 47 語**、material = 138、sugar = 28。
2. **仕様提案 — ✅ 実装済み.** `SPECIFICATION.html` に §2.6 "Ajisai Minimal Core" を
   規範的に追記した。Minimal Core = `core_tier` の `identity`+`flow`（機械可読な
   `formalization-coverage.json` を tier 帰属の典拠とする）。**後方互換保証(規範)**——
   Minimal Core 語の可観測契約(スタック効果・NIL パススルー・UNKNOWN・構造化診断下の
   挙動)は版を超えて安定であり、それを変えることは Ajisai の同一性への破壊的変更。
   material/sugar 層は保証対象外(園芸品種が育つ層)だが Minimal Core の伝播規律には拘束
   される。比較境界語(`EQ`〜`GTE`)は「契約は Minimal Core・実装(連分数順序)は material」
   と明記——契約は既存 §7.4 で決着済みのため裁定は追認にとどめた。既存の三つの "Core"
   語義(§9.3 Core Words 階層・Core Profile・§7.14 Coreword)との区別、および「新しい
   権威層・実行特権を作らない」ことも明記(self-hosting memo の非目標を引き継ぐ)。
3. **導出可能性の実証 — ✅ 実装済み（当初案は §8.2 により再構成）.**
   当初の「Ajisai prelude でビルトインを再定義して差し替える」案は **§8.2「ビルトイン語は
   再定義できない」により不可**。真のセルフホストは独立した別インタプリタ（Python 移植と
   同格）を意味し、1 語差し替えでは実現しない。代わりに **導出可能性の witness** を採った：
   素材語 `MATH@SIGN` を Minimal Core の語だけ（`NIL?` `LT` `GT` `COND` `IDLE` とリテラル）で
   User 語 `SIGN2` として再実装し、有理数域と NIL で両者が一致することを法則テスト
   （`rust/tests/minimal_core_derivation.rs`, proptest 128 例＋スポット＋NIL）で示した。
   これは trusted Rust core を縮めるのではなく、**Minimal Core の導出力を裏付ける証拠**。
   加えて witness は**オラクルとしての発見**を surface し、それを**是正済み**である：当初
   `SIGN2` は遅延無理数 `2 SQRT` を正しく `1` と符号付けする一方、ビルトイン `MATH@SIGN` は
   `apply_unary`（有理数限定）のため同入力を `SIGN: expected a number` で拒否していた。これは
   Python 移植が SPEC_GAPS を炙り出したのと同じ機序（`MATH@MIN`/`MAX` は全数域を扱うのに
   `MATH@SIGN` だけが有理数限定、という §7.4/§7.4.3 との不整合）。**バグとして修正**し、
   `op_sign` を `MATH@MIN`/`MAX` と同じ予算付き比較（対 `0`）に置き換えて全数域対応にし、
   未決時は U を返す挙動を **§7.4.3 に規範追記**（SIGN を comparison-dependent words に追加、
   §7.14 の Projecting/Passthrough 分類にも追加）。これにより witness の等価性は有理数だけで
   なく**遅延無理数まで含む全 admitted domain で成立**するようになった（同テストの
   `minimal_core_sign_matches_builtin_on_lazy_irrationals` が防護）。導出可能性の witness が
   ビルトインの欠陥を炙り出し、その修正によって Minimal Core と material 層の一致がむしろ
   強まった——「素材語は Core の規律に拘束される」という §2.6 の枠組みが実地で機能した例。
   さらに同じ `apply_unary`（有理数限定）欠陥を共有していた**隣接語 `MATH@NEG`/`MATH@ABS`
   も是正**：`NEG` は `ExactReal::neg` で連分数表現に直接作用する純粋算術として全数域 total 化
   （比較を含まず U を生じない）、`ABS` は対 `0` の予算付き比較で符号を決め負なら否定する
   比較依存語として全数域対応（未決時 U）。仕様は `ABS` を §7.4.3 の comparison-dependent
   words と §7.14 の Projecting/Passthrough に追加、`NEG` は Total/Passthrough と明記した。
4. **移行の計測.** `formalization-coverage` / `word-manifest` から「Core だけで書ける素材語」を
   静的判定し、trusted Rust core 行数をファイルサイズ予算と同じ発想で予算化する。

## 付録 B — 語彙の採用規則と退役候補の列

語彙 100 語への再設計指示書（`vocabulary-100-work-order-2026-09.md`、全 Phase 完了に伴い
2026-09-30 に `docs/dev/` から削除。本文は履歴に残る）の §7.3・§7.4 をここへ移した。
B.1 は語を足すときの判断規則、B.2 はその規則の条件 2 が指す入れ替え相手の列である。

**現在の事実（2026-10-15 時点。数と列はここではなくスクリプトが持つ）。**

- 語彙は 77 語（Kernel 49・Standard 28）。数の記録は `docs/word-manifest.json` の
  `counts`、検査は `scripts/check-minimal-core.mjs`（77・49・28 の一致）。
- 語数の上限 100 を検査するのは `scripts/check-semantic-kernel.mjs` であって
  `check-minimal-core.mjs` ではない。
- 退役候補の列は `scripts/check-minimal-core.mjs` の `RETIREMENT_QUEUE` が正本で、
  不変条件（重複なし・全員が現役の導出可能 Standard・`ROUND` を除く導出可能 Standard を
  すべて含む）も同スクリプトが検査する。列の順とその理由もそこのコメントにある。
- 導出可能 Standard の Kernel だけによる書き方は `rust/tests/standard_derivation_laws.rs`
  が実行して検証している。本書は句を写さない（写しは古くなる。B.2 の旧表がそうなった）。
- 正典の採用規則は LANG.AUTHORITY.IDENTITY（ユーザー定義で書ける語は Core に入れない。
  ただし定義で書くと漸近的に高くつく場合を除く）。B.1 の規則はそれと矛盾しない運用規則で
  あり、正典ではない。

### B.1 alpha 期の採用規則と、その最初の適用（旧 §7.3）

§7.2 の入れ替えは場当たりではない。以後の追加はこの規則で判断する。

> **規則。** alpha の間に語を足すときは、次のどちらかを満たすこと。
>
> 1. **言語内で書けない語であること。** 再帰がなく全域を要求するこの言語では、
>    `SORT` も `GCD` も `EXP` も定義としては書けない。書けない能力は語としてしか
>    持てないので、これは語数と引き換えにできない。
> 2. **さもなければ、同等以下の価値を持つ導出可能な語を一つ退役させること。**
>    語数は動かさない。

`UPPER` `LOWER` は条件 2 で入った（`NEQ` と `PROBE` を退役させた）。

**語数そのものを alpha の制約にしない理由。** 固定された語数は互換性の約束であり、
約束は 1.0 のものである。alpha とは語の集合をまだ動かせる期間のことなので、そこで
数を凍結すると、動かしてよいものを縛り、縛るべき採用規則の方が明文のないままになる。
`scripts/check-minimal-core.mjs` が 100 を検査するのは、追加を必ず意識させるための
引き金であって法則ではない。1.0 で凍結するのは、この規則と、その時点の語の集合の
両方である。そのとき初めて語数が約束になる。

**最初の適用: τ（2π）を検討し、入れないと決めた。** （以下は計算可能実数の層と
`PI` `EXP` があった時点の記録である。その層は `PI` `EXP` `LN` などとともに退役した
（`spec/retired-words.json`）ので、下の表は今は再現できない。判断の理由は残る。） 条件 1 を満たさない
（`PI 2 MUL` で書ける）。条件 2 は持ち出すまでもない（退役させるべき語がない以上、
入れ替えではなく純増になる）。補強となる事実が三つある。

1. `NEQ` を退役させた理由、すなわち等コストの略記であることが、そのまま当てはまる。
2. e は既にワードではない。自然対数の底は `1 EXP` で得る。π と並ぶ頻度を持つ定数に
   名前を与えていない言語が、2π に名前を与える理由はない。
3. 数値的な利点がない。実測は次のとおり。

   | 式 | 結果 |
   | --- | --- |
   | `PI 2 MUL 10 FORMAT` | `'6.2831853072'` |
   | `PI 10 FORMAT` | `'3.1415926536'` |
   | `PI PI EQ` | `NIL` |
   | `PI 2 MUL 2 DIV PI EQ` | `NIL` |

   `PI 2 MUL` は π の 512 ビット基底包囲をちょうど 2 倍に写すだけで、精度も予算も
   失わない。下二行の `NIL` は導出の産物ではない。π を π 自身と比べてさえ予算内では
   決まらないのだから、τ を固有の語にしても比較が良くなることはない。これは Tier 2
   の比較の性質であって、綴りの問題ではない。

満枠であることがこの答えを出させた点を記録しておく。上限に余裕があれば、τ は
「何を退役させるか」を問われないまま入っていた。空き枠を残して alpha を進める案
（語数を 90 程度に抑える）は、この理由で採らない。加えてその案の費用は具体的で、
便益は仮定でしかない。能力を落とさずに削れるのは導出可能な（当時）17 語だけなので、10 語
削るとはその句を毎回書くということであり、得られるのは現行規則が既に処理できている
判断を一度先送りすることだけである。

（§7.2 は語彙 100 語の確定後に行った事後調整——`PROBE` の `CONTRACT` への統合、`NEQ` の
退役、`UPPER` `LOWER` の追加、丸め規則の統一——を指す。）

### B.2 伸びしろ: 退役候補の列（旧 §7.4）

§7.3 の規則の条件 2 は「導出可能な語を一つ退役させる」と言うだけで、どれを、どの
順で、とは言っていない。所有者の判断でその順を先に決めた。語を足す必要が出たら、
列の先頭と入れ替える。

列そのものは `scripts/check-minimal-core.mjs` の `RETIREMENT_QUEUE` にある。2026-10-15
時点では `SUB` `REVERSE` `TRIM` `TOKENIZE` `MAX` `MIN` `TAKE` `DROP` の 8 語で、
`SUB` を先頭に置いたのは零形算術の読み（LANG.VALUES.SHAPE。車輪の演算に減法はなく、
`SUB` は否定した加法と同じ形表を持つ）による所有者の判断、残りは語彙創発パイロットでの
使用数の少ない順である。各語の Kernel だけによる書き方は
`rust/tests/standard_derivation_laws.rs` にある。

次の二つの判断は旧列から変わっていない。

- `ROUND` は導出可能だが列に入れない。その Kernel の句だけがオペランドの符号で分岐する
  ので、手で書いたときに最も間違えやすい。
- operational の Standard（2026-10-15 時点で 19 語）は列に入らない。言語内で書けない
  以上、退役は綴りではなく能力の削除になるからである。

列を入れ替えるときは、`RETIREMENT_QUEUE` からその語を外し、本書ではなくスクリプトの
コメントに理由を書くこと。

**旧列（2026-09）について。** 当初の列は `LTE` `GTE` `SUB` `CEIL` `OR` `QUANTIZE`
`ABS` `MIN` `MAX` `MOD` の 10 語で、語数は 100 で固定するとしていた。その後の最小 Core
への削減で `LTE` `GTE` `CEIL` `OR` `QUANTIZE` `ABS` `MOD` は列の順を待たずに退役し
（`spec/retired-words.json`）、それぞれの句は今は Unknown Word の診断と
`docs/phrasebook/phrasebook.ajisai` が示す。旧表の句は退役した `NEG` を使っていたので、
ここには残さない。

**この形を選んだ理由。** 所有者は当初、alpha を 90 語に抑えて 10 枠を空けておく案を
検討した。同じ伸びしろを、利用者が今日から費用を払う形で先取りするか、入れ替えが
実際に起きた日に払う形にするかの違いである。後者を採った。空き枠と待機列は、入れ替え
可能な候補があるという点で機能的に等しく、待機列の方は候補が現れるまで誰も
その句を書かずに済む。
