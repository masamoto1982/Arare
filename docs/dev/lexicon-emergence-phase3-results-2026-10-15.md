# 語彙創発実験 Phase 3 の結果（2026-10-15）

Status: 非正典・`[観察ノート]`。本書は Ajisai の意味論も互換性方針も定義しない。
正典は `spec/` 配下の各ソースと、そこから生成される `SPECIFICATION.html` のみ。

前提文書: `lexicon-emergence-phase2-results-2026-10-15.md`（以下「Phase 2」）、
`lexicon-emergence-difficulty-calibration-2026-10-15.md`（以下「較正」）。
道具: `tools/lexicon-emergence/`。生データ（`design.json`、各条件・各世代の `lexicon.json`・
`submissions/`・`graded/`・`calls/`・`work/`、`report.md`・`report.json`、被験体ごとの使用量
`usage.tsv`）は `tools/lexicon-emergence/runs/phase3-2026-10-15/` にコミットしたうえで削除した。
git 履歴にある。

Phase 3 の設計書（`lexicon-emergence-phase3-design-2026-10-15.md`）は、完了の定義どおり要点を
付録 B に移して削除した。

---

## 0. 要約

- 68 体（Opus・Sonnet・Haiku・Fable 各 17 体）が 6 族 33 問を解いた。世代 0 の 12 体で設計の
  天井の停止条件（無実行で 262/264）に当たったが、所有者の判断で世代 1〜3 も走らせた。
- **Q1（継承した辞書は無実行の初手の正答率を上げるか）は測れなかった。** 辞書なしの世代 0 が
  262/264（99.2%）、辞書を継承した無実行の 48 体が 1,576/1,584（99.5%）。上げる余地がない。
- 正答率に差がないので、辞書の効果は**圧縮**に出た。解答の長さを Core のみへの展開の長さで割った
  値は、容量なし（C-open）で 0.221 → 0.096 → 0.052 と縮み、世代 3 では解答がほぼ Word 1 つの
  呼び出しになった。容量 8 の条件は世代 3 で 0.29〜0.32 で、辞書を継承しない solo の 0.304 と
  同じ程度にとどまった。
- **Q4: 容量を外すと、辞書は約 40 語で固定点に近づいた**（27 → 42 → 38 語、世代 2 → 3 で
  38 語中 37 語が残った）。ただし入ったのは課題 1 問を丸ごと解く語が多い。容量 8 の使用回数順は
  Phase 2 と同じく早く固まった（8 語中 6 語、次に 7 語が残った）。
- **Q3: 再発明順は、使用回数順が一度も入れなかった語（`R1D.CAPITALIZE` など）を辞書に入れたが、
  辞書を揺らした。** 8 語中 2 語、次に 1 語しか残らなかった。多く使われる語ほど継承した体が
  定義し直さないので、再発明の数が減って辞書から落ち、次の世代で再び作られて戻る、という振動が
  起きた（単語分割の `G0D.WORDS` が世代 2 で落ち、世代 3 に `R2A.WORDS` として戻った）。
- **Q5: 名前と注記のない辞書でも、初手の再利用は起きた**が、体によって 0 か全部かに割れた。
  世代 1 は 132 問中 81 問が継承語に届いた（uses は 85/130）。世代 2 では 4 体中 2 体が継承語を
  1 つも使わなかった（35/131）。正答率は保たれた（132・131・131）。
- **Q6: `SELECT` のレーン選択による `shapeMismatch` は、世代 0 で 0 件、世代 1〜3 で 2 件**
  （どちらも Haiku、どちらも `orders.tag_counts`）。SKILL.md に足した一行で大半は防げたが、
  消えてはいない。
- 誤答は全 68 体で 10 問（世代 0 の 2 問を含む）。どれも被験体自身が書いた式の、引数の順・
  スタックの消費漏れ・`SELECT` の形の誤りで、継承語から広がった誤りはなかった。
- 実行を許した `runs` 条件は 3 世代とも 132/132 で、1 体あたりの実行回数の平均は 221 → 272 → 175。
  辞書が実行回数を減らしたとは言えない。

## 1. 正答

### 1.1 条件・世代・モデル別

| 条件 | 世代 | 実行 | Opus | Sonnet | Haiku | Fable | 計 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| solo | 0 | 不可 | 33 | 33 | 32 | 32 | 130/132 |
| bottleneck | 0 | 不可 | 33 | 33 | 33 | 33 | 132/132 |
| uses | 1 | 不可 | 31 | 33 | 33 | 33 | 130/132 |
| uses | 2 | 不可 | 33 | 33 | 33 | 33 | 132/132 |
| uses | 3 | 不可 | 33 | 33 | 33 | 33 | 132/132 |
| reinvention | 1 | 不可 | 33 | 33 | 32 | 33 | 131/132 |
| reinvention | 2 | 不可 | 33 | 33 | 32 | 32 | 130/132 |
| reinvention | 3 | 不可 | 33 | 33 | 33 | 33 | 132/132 |
| open | 1 | 不可 | 33 | 33 | 33 | 32 | 131/132 |
| open | 2 | 不可 | 33 | 33 | 33 | 33 | 132/132 |
| open | 3 | 不可 | 33 | 33 | 33 | 33 | 132/132 |
| shuffled | 1 | 不可 | 33 | 33 | 33 | 33 | 132/132 |
| shuffled | 2 | 不可 | 33 | 33 | 33 | 32 | 131/132 |
| shuffled | 3 | 不可 | 33 | 33 | 32 | 33 | 131/132 |
| runs | 0 | 無制限 | 33 | 33 | 33 | 33 | 132/132 |
| runs | 1 | 無制限 | 33 | 33 | 33 | 33 | 132/132 |
| runs | 2 | 無制限 | 33 | 33 | 33 | 33 | 132/132 |

モデル別の計（17 体ずつ）: Opus 559/561、Sonnet 561/561、Haiku 557/561、Fable 557/561。

### 1.2 圧縮

`report.md` の「tokens ÷ expanded」（解答のトークン数を、継承語・自前の語を Core のみに展開した
トークン数で割った値。小さいほど語に任せている）と、解答 1 つの平均トークン数:

| 条件 | 世代 0 | 世代 1 | 世代 2 | 世代 3 |
| --- | --- | --- | --- | --- |
| solo（辞書なし） | 0.304（10.9） | | | |
| bottleneck（辞書なし） | 0.541（19.4） | | | |
| uses | | 0.418（14.6） | 0.339（12.0） | 0.310（10.4） |
| reinvention | | 0.435（15.0） | 0.217（8.3） | 0.322（10.7） |
| open | | 0.221（8.2） | 0.096（3.7） | 0.052（2.0） |
| shuffled | | 0.484（15.7） | 0.404（13.4） | 0.293（9.7） |
| runs（実行あり） | 0.383（13.7） | 0.405（13.7） | 0.286（9.4） | |

容量 8 の 3 条件は、世代を追って縮んだが、世代 3 でも辞書なしの solo とほぼ同じである。
solo の 4 体は 1 体平均 22 語を自分で定義し、bottleneck の 4 体は 10.3 語だった。8 語の辞書は、よく定義する体が自分で作る分と同じくらいしか
圧縮しない。

## 2. 辞書の動き

### 2.1 各世代の辞書

括弧内は前の世代での使用回数。

| 条件 | 世代 1 | 世代 2 | 世代 3 |
| --- | --- | --- | --- |
| uses | `G0D.WORDS` `G0C.ORDERS` `G0A.UNWORDS` `G0D.SHIFT` `G0D.DIVISORS` `G0D.RUNS` `G0A.BSTEP` `G0B.REDUCE` | `G0C.ORDERS` `G0D.WORDS` `G0D.SHIFT` `G0D.RUNS` `G0A.UNWORDS` `U1D.ARGMAX` `U1D.TOPKEY` `G0D.DIVISORS` | `G0C.ORDERS` `G0D.WORDS` `G0D.SHIFT` `G0A.UNWORDS` `G0D.RUNS` `U1D.ARGMAX` `U1D.TOPKEY` `U2B.BRED` |
| reinvention | `G0D.WORDS` `G0C.ORDERS` `G0A.UNWORDS` `G0D.DIVISORS` `G0A.BSTEP` `G0A.BSCAN` `G0D.SHIFT` `G0D.RUNS` | `G0C.ORDERS` `G0A.BSTEP` `R1C.REVENUE` `R1D.USTEP` `R1D.USTACKS` `R1A.GAPMASK` `R1A.ARGMAX` `R1D.CAPITALIZE` | `R2A.WORDS` `R1C.REVENUE` `R2B.SPACEJOIN` `R2D.PRESENT` `R2D.STREAKS` `R2B.BSIGN` `R2B.BDEPTH` `R2D.LONGEST-GAP` |
| open | 27 語 | 42 語 | 38 語 |

| 条件 | 世代 1 → 2 で残った語 | 世代 2 → 3 で残った語 | 被験体 1 体が自分で定義した語の平均（世代 1・2・3） |
| --- | --- | --- | --- |
| uses | 6/8 | 7/8 | 7.5・9.8・12.8 |
| reinvention | 2/8 | 1/8 | 7.5・19.0・10.8 |
| open | 27 語中 — （42 語へ） | 38 語中 37 語 | 11.0・2.8・7.0 |
| shuffled | 毎世代改名（中身は uses と同じ規則） | 同左 | 4.5・12.0・17.0 |

世代 0 の辞書なしの体は、solo が 1 体平均 22 語、bottleneck が 10.3 語を定義した。

### 2.2 使用回数順（uses）: Phase 2 と同じ固定

世代 2 で `U1D.ARGMAX`・`U1D.TOPKEY` が入り、世代 3 で `U2B.BRED` が 1 語入れ替わっただけで、
単語分割（`G0D.WORDS`）・注文 JSON の読み出し（`G0C.ORDERS`）・空白での連結（`G0A.UNWORDS`）・
`G0D.SHIFT`・`G0D.RUNS` は 3 世代とも残った。継承した語は使われるので使用回数が高く、上位に残る。
Phase 2 の「1 世代で固定点」は容量 8 でも、6 族 33 問でも再現した。

### 2.3 再発明順（reinvention）: 入れ替わりと振動

再発明順は「その語の類を自分で定義して正答に使った体の数」で並べる。継承した体は継承語を
定義し直さずに使うので、よく使われる語ほど再発明の数が 0 に近づく。世代 1 で全員が使った
`G0D.WORDS`・`G0A.UNWORDS` は世代 2 の辞書から落ち、世代 2 の体はそれを作り直した。作り直した
体が多いので、世代 3 で `R2A.WORDS` として戻った。世代 2 の 1 体あたりの自前の定義（19.0 語）が
全条件で最も多いのは、この作り直しである。

一方、Phase 2 で使用回数順が取りこぼした種類の語（Phase 2 の CAP にあたる語頭の大文字化）は、
`R1D.CAPITALIZE` として世代 2 の辞書に入った。uses には一度も入っていない。Q3 の前半（取りこぼした
語を入れるか）は「入れる」、後半（次の世代の作り直しを減らすか）は「減らさない。むしろ増やす」。
継承した体が語を使ったかどうかを数えない規則は、継承そのものを罰する。

### 2.4 容量なし（C-open）: 課題を丸ごと解く語で固定点へ

世代 1 の辞書は bottleneck 世代 0 の全語（27 語）、世代 2 は 42 語、世代 3 は 38 語。世代 2 → 3 で
38 語中 37 語が残った。世代 3 の辞書には `O1A.RLE`・`O1A.COLLATZ`・`O1D.DIGITAL-ROOT`・
`O1D.FIB-INDEX`・`G0C.PRIMES`・`G0A.BINARY`・`O2A.LONGEST-GAP` のように課題名そのものの語が多く、
世代 3 の解答は平均 2.0 トークン（ほぼ Word 1 つの呼び出し）になった。

他の語から組み立てた語（H2）の割合は 7/27 → 14/42 → 15/38 と増えた。容量 8 の条件では 1〜2/8 である。
Phase 2 の固定点は容量の結果でもあった（Q4）。容量がなければ辞書は課題集合を覆うまで育ち、
覆ったところで止まる。止まった辞書は「言語の語彙」よりも「この 33 問の解答集」に近い。

### 2.5 名前と注記のない辞書（C-shuffled）

| 世代 | 継承語に届いた正答 | 体ごとの、届いた継承語の数 |
| --- | --- | --- |
| 1 | 81/132 | Z1A 7、Z1B 7、Z1C 6、Z1D 7 |
| 2 | 35/131 | Z2A 0、Z2B 0、Z2C 4、Z2D 8 |
| 3 | 69/131 | Z3A 8、Z3B 8、Z3C 3、Z3D 8 |

同じ世代の uses は 85/130・74/132・72/132 で、どの体も 6〜8 語を使った。shuffled の世代 1 は
uses とほぼ同じで、名前なしでも本体を読んで再利用は起きる（Q5 の前半）。世代 2 の Z2A（Opus）と
Z2B（Sonnet）は継承語を 1 つも使わず、すべて自分で書いた。正答率は落ちなかった（Q5 の後半）。
名前のない辞書は、使うかどうかが体の方針で 0 か全部かに割れる。世代 3 の辞書は世代 2 で使われた
語から選ぶので、Z2C・Z2D の 2 体だけの選択で決まった。

### 2.6 継承語への到達（全条件）

| 条件 | 世代 1 | 世代 2 | 世代 3 |
| --- | --- | --- | --- |
| uses | 85/130 | 74/132 | 72/132 |
| reinvention | 74/131 | 56/130 | 62/132 |
| open | 97/131 | 128/132 | 128/132 |
| shuffled | 81/132 | 35/131 | 69/131 |
| runs | 83/132 | 75/132 | — |

## 3. 誤答

| 体 | モデル | 課題 | 原因 |
| --- | --- | --- | --- |
| S0C | Haiku | `orders.top_sku` | 冒頭の `G KEYS` が消費されずに残った |
| S0D | Fable | `number.binary` | `5 0 RANGE 2 POW` を 2 の累乗のつもりで書いた（実際は各値の 2 乗）。最後の 0 で除算 |
| U1A | Opus | `runs.upruns`・`sensor.scaled` | 自前の `U1A.MAXOF`（`'MV' BIND MV 0 GET MV [ MAX ] FOLD`）で `FOLD` の引数の順を取り違えた。この語を使う 2 問を落とした |
| O1D | Fable | `number.collatz` | `SCAN` の引数の順の取り違え |
| R1C | Haiku | `orders.tag_counts` | `SELECT` で Vector を丸ごと選ぼうとして `shapeMismatch`（Q6） |
| R2C | Haiku | `runs.rle` | 値がスタックに 1 つ余った |
| R2D | Fable | `number.primes` | 自前の `R2D.PRIMES`（`1 'R2D.PN' BIND R2D.PN RANGE …`）で `1` を `BIND` の前に置いたため、n ではなく 1 が束縛され、範囲が n から 1 への降順になった |
| Z2D | Fable | `sensor.scaled` | スタックの不足（`stack underflow`） |
| Z3C | Haiku | `orders.tag_counts` | R1C と同じ `SELECT` の `shapeMismatch`（Q6） |

世代 1〜3 の 8 件はすべて被験体自身の式にあり、継承語の中の誤りではない。採点器の Core のみへの
展開と結果が違った正答（H5）は 0 件で、辞書を通じて誤りが広がった例もなかった。種類は世代 0 と
同じく、引数の順・スタックの消費・`SELECT` の形で、実行できれば最初の 1 回で気づく誤りである。

## 4. 問いへの答え

| 問い | 答え |
| --- | --- |
| Q1（H6）継承した辞書が初手の正答率を上げるか | 測れなかった。辞書なし 262/264（99.2%）、辞書あり 1,576/1,584（99.5%）。上げる余地がない |
| Q2（H1）4 モデルの類の重なり | 辞書を継承しない世代 0 の 12 体で、類の Jaccard は同一モデル対 0.188、異モデル対 0.158。全 68 体では 189 類のうち 81 類が 2 体以上で独立に定義され、66 類は 2 モデル以上にまたがった |
| Q3 再発明順と使用回数順 | 再発明順は使用回数順が入れない語（`R1D.CAPITALIZE`）を入れたが、作り直しは減らなかった。よく使われる語ほど落ちて作り直され、辞書が振動した（§2.3） |
| Q4 C-open | 約 40 語で固定点に近づいた（世代 2 → 3 で 37/38 が残った）。課題を丸ごと解く語が多く、世代 3 の解答は平均 2.0 トークン。Phase 2 の固定点は容量の結果でもある（§2.4） |
| Q5 C-shuffled で初手の再利用 | 起きた（世代 1 は 81/132、uses は 85/130）。ただし世代 2 では 4 体中 2 体が継承語を 1 つも使わず、体ごとに 0 か全部かに割れた。正答率は保たれた（§2.5） |
| Q6 `SELECT` の一行の効果 | 全 68 体で、レーン選択の `shapeMismatch` は 2 件（世代 1〜3 の Haiku 2 体、同じ `orders.tag_counts`）。世代 0 は 0 件。減ったが、消えてはいない |
| 補 実行回数 | runs 条件の平均は世代 0・1・2 で 221.3・272・174.8 回。世代 1 は E1C（Haiku）の 642 回が押し上げ、他の 3 体は 117〜211 回。辞書が実行回数を減らすとは言えない |

## 5. 汚染の統制

- 各世代の採点と選定の直後に `seal` し、作業木に残るのは辞書と `design.json` だけにした。
  `analyze` の前に `unseal` で戻した。
- 68 体すべてで、被験体が受け取った最初のメッセージが `cli.mjs prompt … --route cli` の出力と
  一致することを、記録から機械的に確かめた。
- 参照解と字句まで同じ正答は 21 体で計 29 件で、すべて短い定型の式（`sensor.ratios`・
  `sensor.present_mean`・`sensor.filled`・`orders.unpriced`・`number.divisors`・`sensor.longest_gap`）
  だった。
- 世代 3 の途中で API の利用上限（HTTP 429）に当たり、11 体が止まった。うち 5 体は解答を書き終えて
  いたのでそのまま採点し、6 体には「一時的な API の上限で中断した。続けてほしい」とだけ送って
  再開させた。再開した体には新しい情報を渡していない。
- E1C は条件を `seal` した後に完了したが、`seal` の後に書かれたファイルはなく、`unseal` 後の
  再採点で結果は変わらなかった。
- Phase 2・較正・Phase 3 世代 0 の生データは git 履歴にある。被験体が git を読むことは禁止したが、
  技術的には防いでいない（設計 §7 と同じ限界）。

## 6. 使用量

記録できた 63 体で計 850 万トークン（1 体 10.5 万〜20.3 万）。世代 0 が 171 万、世代 1・2 が各
269 万・267 万、世代 3 が記録できた 11 体で 142 万。モデル別の 1 体平均は Haiku 18.0 万、Sonnet
12.7 万、Fable 11.9 万、Opus 11.4 万。所要時間は 1 体 2.4〜11.5 分。中断した世代 3 の 5 体（O3A・O3B・
R3B・Z3B・Z3C）は完了通知が届かず、使用量がわからない。1 体 12 万前後とすれば全体で約 910 万で、
設計の見込み（1,000 万〜1,700 万）の下限を下回った。

## 7. 次に進むなら（観察から。決定ではない）

1. この課題集合では、無実行でも正答率は天井にある。辞書の効果を正答率で見るには、無実行で
   誤りが出る課題が要る。残った誤りは引数の順とスタックの消費という注意の誤りで、課題を長くすれば
   増やせるが、測るものが「言語」から「注意の持続」に寄る。
2. 選定規則は、どれも欠点が見えた。使用回数順は 1 世代で固まり、再発明順は継承を罰して振動し、
   容量なしは解答集になる。「継承した体が使った」と「継承しなかった体が作り直した」を足し合わせる
   規則なら、両方の欠点を打ち消せるかもしれない。
3. 容量なしの辞書が課題名の語で埋まることから、辞書の質を正答率や圧縮だけで測ると、課題集合への
   過適合を「良い言語」と読み違える。課題集合を世代ごとに入れ替える（訓練と評価を分ける）設計が
   要る。
4. Q6 の `SELECT` は Haiku に 2 件残った。SKILL.md の一行で足りないなら、`SELECT` が Vector を
   丸ごと選べないときのエラーに `2 COLLECT … GET` の書き方を添えるのが次の手である。
5. どれに進むかは所有者が決める。

## 付録 A. `report.md` の H4（Core 語別、正答の解答が届いた数）

上位: `BIND` 1,765、`GET` 1,267、`FILTER` 1,254、`MAP` 1,143、`ADD` 960、`EQ` 959、`NOT` 839、
`LENGTH` 806、`FOLD` 804、`SELECT` 766、`COLLECT` 714、`MUL` 683。

使われなかった語（19）: `RECONCILE` `ROUND` `SQRT` `GCD` `SHAPE` `RESHAPE` `DEPTH` `BSEARCH`
`WITHOUT` `JSON-ENCODE` `EXEC` `CONTRACT` `FAIL` `NIL-REASON` `ABSENT` `DEF` `DEL` `DIGEST` `PRINT`。

H5（Core のみへの展開と結果が違った正答）: 0 件。

## 付録 B. 設計の要点（削除した設計書から）

### B.1 問い

| 番号 | 問い | 主に見る比較 |
| --- | --- | --- |
| Q1（H6） | 継承した辞書は、無実行の初手の正答率を上げるか | 世代 0（辞書なし）と、uses の世代 1〜3 |
| Q2（H1） | 4 モデルで、辞書を継承しない体の類はどこまで重なるか | 世代 0 の類の Jaccard |
| Q3 | 再発明順の選定は、使用回数順が取りこぼした語を辞書に入れ、次の世代の作り直しを減らすか | uses と reinvention |
| Q4 | 容量を外すと辞書は固定点に達するか。Phase 2 の固定点は容量の結果か | uses と open |
| Q5 | 名前と注記のない辞書でも、無実行の初手で再利用が起き、正答率が保たれるか | uses と shuffled |
| Q6 | SKILL.md に足した `SELECT` の一行で、較正の誤り（レーン選択による `shapeMismatch`）は消えるか | 全条件の誤答の原因 |
| 補 | 実行できる条件では、辞書は正答までの実行回数を減らすか | runs の世代 0〜2 |

H1〜H5 の定義はパイロットの結果の付録 B にある。H6 は Phase 3 で置いた。

### B.2 固定した設定と条件

- cli 経路（Claude Code の subagent が `cli.mjs compute` で走らせる。無実行の条件では `compute` が拒む）。
- 被験体名の末尾の文字でモデルが決まる: A Opus・B Sonnet・C Haiku・D Fable。
- 課題族: `runs`・`number`・`brackets`・`sensor`・`orders`・`messy`（33 問）。
- SKILL.md は Phase 3 の設計を入れた変更の時点のもの（21,197 バイト）。同じ変更で §3 に、`SELECT` で
  2 つの Vector を丸ごと選べないことと `2 COLLECT … GET` の書き方を足した。
- 容量 K = 8（open は無制限）。実行の上限は既定 0、`runs` 条件だけ無制限。

```json
{
  "families": ["runs", "number", "brackets", "sensor", "orders", "messy"],
  "models": { "A": "opus", "B": "sonnet", "C": "haiku", "D": "fable" },
  "callBudget": { "default": 0, "runs": null }
}
```

| 条件 | 世代 | 被験体 | 辞書 | 実行 |
| --- | --- | --- | --- | --- |
| `solo` | 0 | S0A〜S0D | なし | 不可 |
| `bottleneck` | 0 | G0A〜G0D | なし（世代 1 の 4 条件の辞書の出所） | 不可 |
| `uses` | 1〜3 | U*g*A〜D | 使用回数順、K = 8 | 不可 |
| `reinvention` | 1〜3 | R*g*A〜D | 再発明順、K = 8 | 不可 |
| `open` | 1〜3 | O*g*A〜D | 容量なし | 不可 |
| `shuffled` | 1〜3 | Z*g*A〜D | 使用回数順、K = 8、各世代で全語を改名（`X<g>.`） | 不可 |
| `runs` | 0〜2 | E*g*A〜D | 世代 *g* の `uses` と同じ辞書（世代 0 はなし） | 無制限 |

4 条件の世代 1 は、同じ bottleneck 世代 0 から `evolve --into` で作った。条件間の差は、選定規則と
容量と名前だけである。再発明順（`--rule reinvention`）は、その語の類を自分で定義して正答に使った
体の数で先に並べ、同数なら使用回数で並べる。

### B.3 手順

各段の後に `grade` → `evolve`（・`shuffle`）→ `seal` の順で進めた。

```sh
# 世代 0 の後
node cli.mjs evolve <run> bottleneck 0 8 --into uses
node cli.mjs evolve <run> bottleneck 0 8 --rule reinvention --into reinvention
node cli.mjs evolve <run> bottleneck 0 Infinity --into open
node cli.mjs shuffle <run> uses 1 shuffled 1
cp runs/<run>/uses/gen1/lexicon.json runs/<run>/runs/gen1/
# 世代 g = 1, 2 の後
node cli.mjs evolve <run> uses <g> 8
node cli.mjs evolve <run> reinvention <g> 8 --rule reinvention
node cli.mjs evolve <run> open <g> Infinity
node cli.mjs evolve <run> shuffled <g> 8 && node cli.mjs shuffle <run> shuffled <g+1> shuffled <g+1>
cp runs/<run>/uses/gen2/lexicon.json runs/<run>/runs/gen2/   # g = 1 のときだけ
node cli.mjs seal <run> <cond> <g>
# 世代 3 の後に seal、最後に
node cli.mjs unseal <run> && node cli.mjs analyze <run>
```

### B.4 停止条件

世代 0 の無実行 8 体の正答が 95% 以上（251 問以上）なら天井、30% 未満（80 問未満）なら床として
止める。世代 1 以降で、ある条件の 4 体のうち 2 体以上が解答を書けなかったときは、その条件を止める。
今回は天井に当たり（262/264）、所有者の判断で世代 1〜3 を走らせた。世代 1 以降に解答を書けなかった
体はない。

### B.5 Phase 3 で入れた道具

| 変更 | 場所 |
| --- | --- |
| SKILL.md §3 の `SELECT` の一行（実行して確かめる canonical example `select-whole`） | `scripts/generate-skill-md.mjs` |
| 選定規則 `uses`・`reinvention`（`evolve --rule`、`design.json` の `selectionRule`） | `lib/evolve.mjs`・`cli.mjs` |
| 別の条件の次の世代へ辞書を書く `evolve --into`。K に `Infinity` | `cli.mjs` |
| 条件ごとの実行の上限（`callBudget` にオブジェクト、`null` は無制限） | `cli.mjs` |
| `seal`・`unseal` | `cli.mjs` |
| 世代・モデル別の正答と平均実行回数の表、参照解と字句まで同じ正答の表 | `lib/analyze.mjs` |
| 上の道具のテスト | `test/phase3-tooling.test.mjs` |
