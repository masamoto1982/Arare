# Total division and the three points over zero

Status: non-canonical. 方針記録。正典は `spec/language-semantics.md` の
LANG.VALUES.EXACT と、`spec/words.json`・`spec/outcomes.json`・`spec/grammar.json`
のみ。本書は採用した判断とその理由を記録し、何も定義しない。

## 判断

数は**非負の分母を持つ既約な整数の組**である。除算は逆数との積であり、逆数は
すべての組に対して定義される（符号は分子へ運ぶ）。したがって除算は全域的で、
ゼロ除算は不在ではなく数を答える。`gcd(n, 0) = |n|` なので、0 の上の組は既約化
により `1/0`・`-1/0`・`0/0` の三つに尽きる。`100 0 DIV` が `1/0` なのは `4/2` が
`2/1` なのと同じ理由であり、`1/0` は `1/2` と同じくリテラルである。

四則は分数の公式をすべての組に適用して既約化したものと定める。公式から読み取れる
帰結（`x 0 DIV` は `x` の符号を 0 の上に置いた点、`1/0 1/0 ADD` は `0/0`、
`0 1/0 MUL` は `0/0`、`1 1/0 DIV` は `0`、`0/0` はすべてを吸収）を個別に決めない。
三つの点でだけ、0 に言及する体の法則（`0·x = 0`、`x − x = 0`、`x ÷ x = 1`、
分配法則）が成り立たなくなる。実装はこれを特別扱いで取り戻してはならない。

順序は `-1/0 < 体 < 1/0` で、`0/0` だけが順序を持たない。順序を問うワード
（LT GT MIN MAX SORT ORDER BSEARCH）は `0/0` に対して `domainMiss` を投影し、
真偽の位置ではそれが UNKNOWN として読まれる。等価は表示意味論で決まるので
`0/0 0/0 EQ` は TRUE である。

## 捨てたもの

- NIL 理由 `divisionByZero`（`spec/outcomes.json`）。ゼロ除算は数を答えるので、
  投影すべき不在がない。NIL の不在理由は `-1 SQRT` の `domainMiss` が代表する。
- 文法の `zeroDenominator` 規則。`1/0` はソースでもテキストでも数である。
- 密テンソルの不在マップ。密レーンは数だけを保持し、NIL はレーンに入らない。
  「分母 0 = 不在」という内部表現は、分母 0 が数になった時点で成立しない。
- `ErrorCategory`／`NilReason`／`AbsenceOrigin` の `DivisionByZero` 変種。

## なぜ三値へ寄せないか

ゼロ除算を NIL にする設計（旧）は、不在が算術を素通りする法則と、不在が
分子を覚えているという内部表現の二つを抱えていた。後者は観測できない構築履歴
であり、LANG.VALUES.DENOTATION に反する。前者は `0 x MUL` が 0 でなくなる代償を
払いながら、その代償を「不在」という別の層に隠していた。組の公式に戻せば代償は
同じ場所（三つの点）で同じ大きさのまま表に出て、不在は本来の意味（部分関数の
答えがないこと）だけを担う。三値論理には `0/0` の順序という一本の橋だけが残る。

## 法則がどこで成り立つかを契約で読む

全域除算の代償（三点で体の法則が崩れること）は、実行時にエラーとして現れない。
`1/0` は数なので、合図なしに計算が進む。この代償を「特別扱いで法則を取り戻す」形で
払うことは上で禁じた。代わりに採ったのは、**法則が成り立つ範囲を実行前に読める
ようにする**ことである（LANG.CONTRACT.FIELD）。

- すべての契約に `field` を加えた。`closed` は「読む数がすべて体の中なら、答える数も
  すべて体の中」、`leaving` は「三点を含まない被演算子から三点を答えうる」。
- Core Word で `leaving` なのは `DIV`・`POW`・`NUM` の三つだけ。いずれも失敗では
  なく設計として体の外へ届く Word である。他はすべて `closed`。
- User Word とブロックは、partiality と同じ歩査で `closed` < `leaving` の順に結合
  する。三点を書いたリテラル（`[ 1/0 MIN ]`）も `leaving` にする。読めないコード
  被演算子は弱い主張である `leaving` に倒す。
- `#:contract W field=closed` は上界の宣言で、推論が `leaving` なら違反になる。
- 直前に書いた数リテラルが体を出る原因の被演算子を決めるとき、その呼び出しは
  体を出ない。`DIV` の除数が有限で非零のリテラル、`POW` の指数が有限で非負の
  リテラルの場合である。そのリテラルは呼び出し時にスタックの先頭にあることが
  確定しているので、評価せずに読める。`[ 2 DIV ]`（半分にする）や `[ 2 POW ]`
  （二乗）は `closed` になり、`[ LENGTH DIV ]` のように計算した除数は `leaving`
  のまま残る。平均 `MEAN` が `leaving` になるのはこの計算した除数のためで、
  空ベクトルを渡すと実際に `0/0` が返る。推論が指す場所と三点が生まれる場所が
  一致している。
  このリテラル規則は、零形のスタック模擬（`docs/dev/zero-shape-arithmetic.md`）が
  成り立つ本体では、その特別な場合として吸収された。除数がリテラルでなくても、
  形が零でありえないと決まれば（`[ 2 SQRT DIV ]`、`[ LENGTH 1 ADD DIV ]`）`closed`
  になる。リテラル規則は模擬が成り立たない本体での結合に残る。
- `leaving` な本体の報告は、体を出る場所（呼んでいる Word と三点のリテラル）を
  本体に書かれた順に `fieldExits` として示す。`field=closed` の違反メッセージも
  その場所を示す。

これで全域除算と体の推論が衝突しなくなる。`closed` なコードに有限の被演算子を
与えた範囲では、分配律も `x − x = 0` も成り立ち、人間・ツール・エージェントは
その範囲で式を書き換えてよい。`leaving` の境界から先では、どこでも成り立つ法則
（加法・乗法の結合律と交換律）だけが残る。三値論理への唯一の橋（`0/0` の順序が
UNKNOWN になること）も、`leaving` の Word かリテラルを経由しなければ渡れないので、
`closed` な本体が有限の入力に対して `0/0` の順序を問うことはない（算術が UNKNOWN を生む唯一の経路がふさがっている）と実行前に分かる。NIL が真偽位置で UNKNOWN と読まれる経路は別で、これは従来どおり残る。

ゼロ除算を NIL や ERROR に戻す案は採らない。三点は数のまま流し、どこから
流れ出しうるかだけを契約が名指す。それが「隠さず流す」という Ajisai の幹と
全域除算を一致させる形である。

実装: `rust/src/interpreter/word_contract_field.rs` の `pushes_point_over_zero`・
`literal_operand_keeps_field`・`AccumulatedContract::widen_with_call`（模擬が成り立たない本体での結合）、`rust/src/interpreter/word_contract_shape.rs` の零形のスタック模擬、`rust/src/agent/contract_decl.rs` の宣言
検査、`rust/src/field_closure_laws.rs` の法則テスト（`leaving` の各 Word の証人と、
`closed` な算術 Word が有限の被演算子から体を出ないことの掃引）。

## 実装の要点

- `rust/src/types/fraction_extended.rs`: `extended_add` / `extended_mul` /
  `reciprocal`、`positive_infinity` / `negative_infinity` / `nullity`、
  `is_finite` / `is_nullity` / `signum`。
- `rust/src/types/fraction_order.rs`: `Fraction::order` は `Option<Ordering>` を
  返し、`0/0` に対してのみ `None`。`PartialOrd` はこれを経由し、`Ord` は実装しない。
- `rust/src/interpreter/comparison.rs`: `unordered_projection()` が順序ワードの
  投影先。`three_way_compare` と `compare_scalar_pair` は `Option` を返す。
- 高速層（`fused_block*.rs`・`dense_kernels.rs`・`quickened.rs`）
  は有限レーン以外を辞退し、組の公式を仮定する経路に 0 分母を通さない。
