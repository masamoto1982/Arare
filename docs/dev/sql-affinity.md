# SQL との親和性（2026-10-13）

Status: non-canonical. 正典は `spec/language-semantics.md` の LANG.VALUES.ORDER と、
`spec/words.json` の各ワードの契約。本書は判断の理由と、SQL の書き方を Ajisai の書き方へ
移すときの対応表を記録する。

## 経緯

三値論理を SQL の強 Kleene 論理に揃えた（`three-valued-truth.md`）あと、SQL の代表的な
NULL の挙動を約 90 パターン、Ajisai で書き直して照合した。論理の層（`AND`・`NOT` の表、
NULL の伝わり方、`FILTER` と WHERE、NOT IN の罠）は SQL と一致していた。ずれは論理の層の
周りに集まっていた。

- `UNIQUE`・`TALLY`・`GROUP` が、理由の違う NIL を別々のキーにしていた。JSON の `null`
  （理由 `literal`）とキーのない行（理由 `notFound`）が別グループになり、件数が黙って変わる。
- `SORT`・`ORDER`・`JOIN` が、NIL の要素で ERROR になっていた。
- 文字列に順序がなく、名前での並べ替えや最小値が書けなかった。
- SQL の型へ書き出すときの数の対応が決まっていなかった。

## 何を変えたか

新しい節 LANG.VALUES.ORDER に、順序とグループ化のキーをまとめて定めた。語は増やしていない（77 語のまま）。

1. **グループ化のキー。** `UNIQUE`・`TALLY`・`GROUP` は、すべての NIL を理由によらず、
   どの深さでも 1 つのキーとみなす。SQL の DISTINCT と GROUP BY が、すべての NULL を
   1 つのグループにするのと同じである。グループのキーは最初に出会った NIL で、その理由が
   残る。`EQ` と `RECONCILE` はこれまでどおり理由で区別する（LANG.VALUES.NIL）。
   Record のキーも同じ規則で 1 つのキーになる（下の「追補」）。
   実装は `rust/src/types/group_key.rs`。NIL を含まないキーは、NIL を含むかどうかの
   判定（スカラーと文字列なら 1 回の `match`）のほかは `Value` のハッシュと等価性を
   そのまま使うので、よくあるキーの費用は変わらない。
2. **NIL は最後に並ぶ。** `SORT` と `ORDER` は、NIL の要素を比較せずに脇へ置き、
   存在する要素のすべての後ろに、元の順序のまま置く。SQL の ORDER BY … NULLS LAST に
   当たり、`SORT REVERSE` は NULLS FIRST の降順になる。PostgreSQL の既定
   （ASC は NULLS LAST、DESC は NULLS FIRST）と一致する。`BSEARCH` はこの順序の
   ベクトルを読み、NIL の要素はどのキーにも見つからない。NIL のキーのレーンは、それ自体が答えになる。
3. **`JOIN` は NIL を伝える。** NIL の要素があれば、最初の NIL が理由を保ったまま答えになる。
   SQL の `'a' || NULL` が NULL になるのと同じである。数などテキストでない要素は、NIL が
   あっても先に ERROR になる（不在が誤用を隠さない）。
4. **文字列の順序。** 2 つの文字列は Unicode スカラー値で 1 文字ずつ比べ、真の接頭辞が先に来る。
   SQL のバイナリ照合（PostgreSQL の `COLLATE "C"`）と同じで、ロケール・大文字小文字の
   畳み込み・正規化は入らないので、どのホストでも同じ順序になる。UTF-8 のバイト順は
   この順序と一致するので、比較はバイト列の比較で済む。`LT`・`GT`・`MIN`・`MAX`・
   `SORT`・`ORDER`・`BSEARCH` が文字列を受け付ける。数と文字列の組は `nonNumeric`。

### 先の提案から変えた点

最初のレビューでは、`SORT` に NIL があれば結果全体を NIL にする（伝える）案を出した。
実装の段階で NULLS LAST に変えた。理由は 3 つある。

- 結果全体を NIL にすると、NULL を含む列で ORDER BY を書くたびに、先に NIL を取り除く
  必要が残る。`ORDER` の置換で表の他の列を並べ替える使い方もできなくなる。
- NULLS LAST は何も捨てないので、「黙って捨てない」という Ajisai の核に反しない。
- SQL の既定の動作と一致するので、ORDER BY がそのまま移せる。

`0/0` と `1/0` を含むベクトルは、これまでどおり `NIL(domainMiss)` を投影する。
それらは不在ではなく、順序を持たない数だからである（LANG.VALUES.EXACT）。

### 追補：検証で見つかった点（2026-10-14）

別のモデルによる検証で、次の 3 点を直した。

- **誤用が順序を持たない数の陰に隠れていた。** `SORT`・`ORDER`・`BSEARCH` は、
  `0/0` や `1/0` に出会った時点で `domainMiss` を返し、その後ろの要素の種類を
  見ていなかった。そのため `[ 0/0 'b' ] SORT` は NIL、`[ 'b' 0/0 ] SORT` は ERROR と、
  要素の並びで答えが変わり、誤用が NIL に変わっていた（LANG.FAILURE.TRICHOTOMY に反する）。
  今はすべての要素とキーの種類を先に確かめ、そのあとで順序を持たない数を投影する。
  この挙動は今回の変更より前からあった。
- **Record のキーとグループ化のキーが食い違っていた。** `RECORD` は理由の違う 2 つの
  NIL を別のキーとして受け入れたので、`KEYS UNIQUE` が `KEYS` より短い Record が作れた。
  一貫性を取るため、Record のキーの同一性をグループ化と同じにした。理由の違う 2 つの
  NIL のキーは `duplicateKey` になり、`MERGE` は NIL のキーを理由によらず同じキーとして
  上書きし（最初の綴りを保つ）、Record どうしの持ち上げもキーをこの規則で揃える。
  `EQ` で Record を比べるときは、ベクトルと同じく理由まで読む。
  Record の索引は、すべての NIL を 1 つの NIL に置き換えた正準形で引く。NIL を含まない
  キー（ほぼすべて）は置き換えずにそのまま使うので、引く費用は変わらない。
- **対応表の誤り。** `'a' || x` の行に、ベクトルのリテラルの中に変数を書く形
  （`[ 'a' X ] JOIN`）を載せていた。リテラルの中の名前はシンボルなので、これは
  ERROR になる。`'a' X 2 COLLECT JOIN` に直した。

## 変えなかったもの

- **集計は NIL を伝える。** SQL の SUM は NULL を飛ばすが、Ajisai の `FOLD` は NIL を伝える。
  「黙って捨てない」は核なので、ここは意図して SQL と違えたままにする。SQL の集計は、
  下の表のとおり `[ NIL? NOT ] FILTER` を前に置けば書ける。
- **`SELECT` は UNKNOWN で NIL を答える。** CASE WHEN が UNKNOWN を ELSE に倒す動作は、
  IS TRUE に当たる言い回しで書ける（下の表）。
- **`EQ` は値全体を比べ、要素ごとには持ち上がらない。** 列に対する `=` は `[ v EQ ] MAP`
  と書く。ベクトルの中の 2 つの NIL は、理由が同じときに限り等しい。
- **OR・IN・不等号などの語は足さない。** 言い回しで書け、三値の答えも SQL と一致する。
  語彙の上限（100 語）には余裕があるが、使われた記録のない導出可能な語は入れない、
  という採用規則（`ajisai-minimal-core-identity.md` 付録 B）に従う。

## SQL から Ajisai への対応表

列はベクトル、表は列の並びまたは Record のベクトルとして扱う。どの例も実行して確かめた。

| SQL | Ajisai | 結果 |
| --- | --- | --- |
| `WHERE x > 1` | `[ 1 NIL 3 ] [ 1 GT ] FILTER` | `[ 3 ]` |
| `x = 2`（列） | `[ 1 NIL 3 ] [ 2 EQ ] MAP` | `[ FALSE NIL FALSE ]` |
| `x <> 2` | `[ 1 NIL 3 ] [ 2 EQ NOT ] MAP` | `[ TRUE NIL TRUE ]` |
| `x <= 2` | `[ 1 NIL 3 ] 2 GT NOT` | `[ TRUE NIL FALSE ]` |
| `x >= 2` | `[ 1 NIL 3 ] 2 LT NOT` | `[ FALSE NIL TRUE ]` |
| `a OR b` | `A NOT B NOT AND NOT` | 三値の OR |
| `p IS TRUE` | `'X' BIND X NIL? NOT X AND` | UNKNOWN が FALSE になる |
| `p IS NOT FALSE` | `'X' BIND X NOT NIL? NOT X NOT AND NOT` | UNKNOWN が TRUE になる |
| `CASE WHEN x > 2 THEN 'big' ELSE 'small' END` | `'big' 'small' X 2 GT [ 'P' BIND P NIL? NOT P AND ] MAP SELECT` | NULL の行は `'small'` |
| `COALESCE(x, 0)` | `0 X X [ NIL? ] MAP SELECT` | `[ 1 NIL 3 ]` が `[ 1 0 3 ]` |
| `1 IN (2, NULL)` | `[ 2 NIL ] [ 1 EQ ] MAP FALSE [ 'B' BIND 'A' BIND A NOT B NOT AND NOT ] FOLD` | UNKNOWN |
| `1 NOT IN (2, NULL)` | `[ 2 NIL ] [ 1 EQ NOT ] MAP TRUE [ AND ] FOLD` | UNKNOWN |
| `SUM(x)` | `[ NIL? NOT ] FILTER 0 [ ADD ] FOLD` | NULL を飛ばした和 |
| `COUNT(x)` / `COUNT(*)` | `[ NIL? NOT ] FILTER LENGTH` / `LENGTH` | |
| `AVG(x)` | `[ NIL? NOT ] FILTER 'V' BIND V 0 [ ADD ] FOLD V LENGTH DIV` | 空なら `0/0` |
| `MAX(x)` | `[ NIL? NOT ] FILTER SORT -1 GET` | 空なら `NIL(indexOutOfBounds)` |
| `ORDER BY x` | `X SORT`、他の列は `C X ORDER GET` | NULLS LAST |
| `ORDER BY x DESC` | `X SORT REVERSE` | NULLS FIRST |
| `ORDER BY name`、`MIN(name)` | `NAMES SORT`、`'pear' 'apple' MIN` | バイナリ照合 |
| `SELECT DISTINCT x` | `X UNIQUE` | NULL は 1 つ |
| `COUNT(DISTINCT x)` | `X UNIQUE [ NIL? NOT ] FILTER LENGTH` | |
| `GROUP BY k` で `SUM(v)` | `K V GROUP VALUES [ [ NIL? NOT ] FILTER 0 [ ADD ] FOLD ] MAP` | NULL のキーは 1 グループ |
| `x LIKE '%or%'` | `[ 'or' SEARCH NIL? NOT ] FILTER` | |
| `'a' \|\| x` | `'a' X 2 COLLECT JOIN` | `X` が NULL なら NULL |
| `ROUND(x, 2)` で NUMERIC へ | `2 FORMAT NUM` | 下の境界を参照 |

`1 IN (…)` の FOLD の初期値は `FALSE`（OR の単位元）、`NOT IN` は `TRUE`（AND の単位元）である。

## SQL との境界

Ajisai の数は厳密な有理数（と代数的数）で、SQL の型とは 1 対 1 に対応しない。
`JSON-ENCODE` は精度を黙って落とさないことを優先して、次のように書き出す（LANG.RECORDS.STRUCTURE）。

| Ajisai の値 | `JSON-ENCODE` | SQL 側で起きること |
| --- | --- | --- |
| 有限小数で書ける有理数（`1/4`） | `0.25`（数） | NUMERIC にそのまま入る |
| それ以外の有理数（`1/3`） | `"1/3"`（文字列） | NUMERIC 列では型エラー |
| `1/0`、`0/0` | `"1/0"`、`"0/0"`（文字列） | 同上 |
| 無理数（`2 SQRT`） | `NIL(domainMiss)` | 書き出せない |
| NIL | `null` | NULL。理由は失われる |

NUMERIC(p, s) の列へ書き出すときは、スケール `s` を指定して `s FORMAT NUM` を通す。
`FORMAT` は四捨五入（0 から遠い方へ）で `ROUND` と同じ規則を使う。0 の上の点に
対しては `domainMiss` を投影するので、`1/0` と `0/0` は NULL になる。無理数も
指定した桁で書ける。

- `1 3 DIV 2 FORMAT NUM JSON-ENCODE` は `'0.33'`
- `[ 1 0 2 ] 0 DIV 2 FORMAT NUM JSON-ENCODE` は `'[null,null,null]'`
- `2 SQRT 4 FORMAT NUM JSON-ENCODE` は `'1.4142'`

この変換は、どこで丸めるかをプログラムに書かせるためのもので、エンコーダーに既定の
丸めは入れない。ゼロ除算の行を SQL のようにエラーとして扱いたいときは、
`[ 0 FORMAT NIL? ] FILTER` で `1/0`・`0/0`・NIL の行を拾える
（`[ 1 1/0 0/0 NIL ] [ 0 FORMAT NIL? ] FILTER` は `[ 1/0 0/0 NIL ]`）。
