# 仕様・実装整合化の方法論 — 4フェーズ手順とスイート裁定規則

> Status: **Non-canonical / 設計メモ（`[設計根拠]`）.** 本書は Ajisai の意味論を
> 一切定義しない。正典は `spec/` 配下の各ソースと、そこから生成される
> `SPECIFICATION.html` のみ。本書は乖離を見つけ、どちらの側を直すかを決める
> *手続き* を定める運用文書である。
>
> `spec-impl-drift-tactic.md` の後継。前身は当時の `SPECIFICATION.html` 単一
> ファイル構成（§2.4/§2.5/§16.1 という節番号）を前提にしていたが、現行の
> `spec/` は複数ソース構成（`spec/README.md`、`LANG.AUTHORITY.SOURCES`）に移行して
> おり、前身の節参照はすべて失効していた。本書はその失効した参照を、現行の
> 安定アンカー（`LANG.*` クラウズ ID）に置き換えて引き継ぐ。方法論そのもの
> （スイート裁定規則）に変更はない。2026-09、3フェーズ手順の実地適用
> （host protocol V1/V2 の整理、PR #1604-#1608）を経て、フェーズ4（削除規律）
> を追加した。以降、本書を根拠として参照する運用チェックリスト
> `.claude/skills/spec-impl-alignment/SKILL.md` が併設されている（PR #1609
> で新設、PR #1610 が初回の実行）。PR #1611 の実地適用で得た二つの教訓
> ——コミット済みビルド生成物の陳腐化（Phase 2）と、発見間の適用範囲の
> 規律（Phase 4）——は本書とチェックリストの両方へ反映してある。同じ
> Phase 2 の枠組みに、2026-09、MCP サーバーへの実接続で見つかった第三の
> 盲点——AI エージェント向け生成プローズ（`SKILL.md`、`tools/mcp-server/`
> のツール説明文）の例示コードが実装から独立に陳腐化しうること——を追加した。

## 0. なぜタイムスタンプではなくスイートか

「どちらが新しい意図か」を手書きの日時で裁定する方式は採らない。理由は二つ:

1. `LANG.AUTHORITY.SOURCES` が権威順位をすでに恒久的に決着させている——本文
   （`spec/` の正典ソース）が常に勝つ。タイムスタンプが追加する情報はない。
2. 実際に観測される乖離の大半は「本文が書いていない」(仕様の穴) であり、
   存在しない項目に決定日時は付けられない。タイムスタンプは起きていない
   問題（本文 vs 実装の真正面衝突）のために台帳を増やし、支配的な問題
   （穴）を検出できない。

provenance（いつ・なぜ変わったか）が要る場面では、手書き日時ではなく
**git 履歴**を使う: `git log -L` / `git blame` を該当箇所に当てる。これは
不変・精密・改竄困難で、手書きタイムスタンプの上位互換である。

## 1. 4フェーズ手順

### Phase 1 — 仕様内部の整合性

`spec/` のソース（`language-semantics.md`、`words.json`＋`words.schema.json`、
`gui-semantics.md`、`host-protocol.schema.json` ほか。一覧は `spec/README.md`）を
実装を一切見ずに読み合わせる。既存の自動チェック（`specification:check`、
`semantic-kernel:check`、`word-schema:check` 等）は生成の往復一致は保証するが、
**プローズとスキーマの意味的な食い違い**までは見ない——これは人間（または
エージェント）が読んで見つけるべき領域である。

見つかった矛盾は、より広く/強く主張している側を、実際に定義・使用されている
範囲まで縮小して解消する。片方を「正」と決め打ちせず、両方を機械的に
突き合わせて判断する。

### Phase 2 — 実装内部の整合性

仕様を一切見ず、実装（`rust/src`、`src/`、`tools/mcp-server/`）だけを見て、
同じことを2箇所以上で別々に実装している箇所、あるいは「クロスチェックされて
いると信じられているが実際にはされていない」箇所を探す。典型的な兆候:

- 「〜と同じ規則」「mirrors 〜」といったコメントがありながら、実際に共有
  関数を呼んでいない（コピーが2つある）。
- そのコピーを比較するテストが存在しない、または存在すると主張されている
  テストファイルが実在しない。
- 一方が最近の変更に追随し、もう一方が追随していない（`git log` でどちらが
  最後に触られたかを確認する）。

見つかった食い違いは、実際に生きている（本番から呼ばれている）側を正とし、
死んでいる側は削除する。両方生きている場合は、実機で正しい方の挙動を
確認してから、誤っている方を実装で修正し、再発防止のクロスチェックテストを
追加する。

#### 検証の盲点 — コミット済みビルド生成物

この repo の生成物の大半は、生成スクリプト自身の `--check` モードで CI が
ハードにゲートしている（`rust/src/kernel/generated/` は `word-registry:check`、
ほかに `word:manifest:check`、`word:reference:check`、
`semantics:table:check`、`semantic-kernel:check`）。危険なのは
**鮮度チェックが advisory（best-effort）にとどまる、または存在しない生成物**
である——通常のテスト行列の中でそれを再生成する工程が一つも無い以上、
陳腐化は原理的に検出されない。緑の出力は反証にならない。

本 repo でこの分類に当たるのは、コミットされた wasm-pack バンドル2つ
——`src/wasm/generated/` と `tools/mcp-server/wasm/generated/`
（`npm run build:wasm` / `npm run build:mcp-wasm`）——だけである。
`.github/workflows/test.yml` の "Detect stale committed wasm bundle
(advisory)" は意図的に `continue-on-error: true` である。

したがって、Phase 2（および Phase 3・Phase 4）の変更を完了とする前に、
**diff に含まれるソースから決定論的に生成されるコミット済み成果物**
——コンパイル済みバイナリ、生成バンドル、lockfile、golden snapshot——を
列挙し、ゲートが advisory なものは自分で再生成して差分ごとコミットする。
既存の `*:check` 群とテスト行列を通しただけでは、構造上ここは見えない。

さらに、同じ事実の二重実装を直したときは、両者を**実際に走らせて
突き合わせる**スイート（`npm run test:mcp-backends` =
`tools/mcp-server/backend/parity-test.js`、native CLI 対 WASM ワーカー）
まで含めて確認する。陳腐化した二者は互いに一致するため、片方を直すまで
このスイートも緑のままである（§2 の PR #1611 を参照）。

#### 検証の盲点 その2 — MCP 向け生成プローズ

`tools/mcp-server/` は Phase 2 の対象実装（`rust/src`、`src/`、
`tools/mcp-server/`）に含まれるが、その中の「AI エージェント向け生成
ドキュメント」——`SKILL.md`、`mcp-quickstart.md` → `assets/quickstart.md`、
`index.js` のツール説明文——は、MCP クライアント（AI）がツール呼び出しの
*前に*読む一次資料であるにもかかわらず、中の例示コードが実装と一致するかを
機械的に確認する仕組みが、部分的にしか存在しなかった。

2026-09、実際に Ajisai の MCP サーバーへ接続して手動でツールを叩いた際に
発覚した実例（PR #1615）:

1. `SKILL.md` §2/§3 の地の文（`generate-skill-md.mjs` の手書きプローズ）が
   `{ body } 'NAME' DEF` という、CodeBlock/Vector unification で廃止済みの
   `{ }` 構文を教えていた。同ファイルの冒頭は「Every code line below was
   executed by the generator against the real interpreter」と自己申告して
   いたが、実際に検証されるのは §6 の `canonicalExamples` 配列（`runSnippet`
   経由で実行）だけで、§2/§3 の地の文は独立した手書き文字列としてこの網の
   外にあった。
2. 同じ `{ }` バグが `tools/mcp-server/index.js` の
   `sourceSchema.properties.source.description`——`compute`/`check`/
   `infer_contracts` 全ツールが共有する、MCP クライアントが最初に読む
   ツール説明文——にも独立に存在していた。この行の直前のコメントは
   「Each rule was executed against the engine before being written here」
   と主張していたが、これも一度も実行検証されていなかった。
3. 対照的に `assets/quickstart.md`（`ajisai://guide/quickstart` として配信）
   の ```` ```ajisai ```` フェンス付きブロックは `selftest.js` の
   `prefaceExamples` が実際にライブ実行して検証しており、無事だった。同じ
   「MCP 向け生成文書」という括りの中に検証済みの部分と未検証の部分が
   混在しており、後者は前者の存在によって「このファイルは検証済みだ」と
   いう誤った安心感を生んでいた。

これは Phase 2 が既に定義しているパターンそのもの——「クロスチェックされて
いると信じられているが実際にはされていない箇所」——の一例であり、対象が
Rust/TS の実装コードではなく AI エージェント向けの生成プローズだった、と
いう違いしかない。

対処（PR #1615 で実施したこと。今後この種の生成プローズを追加・変更する
際の指針でもある）:

- 具体的な構文の断片を地の文で裏付けなしに手書きしない。`generate-skill-md.mjs`
  は `canonicalExamples` の該当エントリに `id` を振り、§2/§3 の記述を
  `canonicalExampleCode(id)` 経由の参照に直した——手書きコピーは残らず、
  §2/§3 に出る具体例は必ず §6 で実行検証済みの文字列そのものになる。
- プレースホルダ（`body`/`guard`/`NAME` 等の疑似変数）でしか説明できない
  構文の「形」は、バッククォートで囲んだコード片としては書かない。
  バッククォートは「これは検証済みの Ajisai ソースである」という主張に
  なるため、プレースホルダをそこに置くと同じ主張を裏切る——地の文の平文で
  説明する。
- `tools/mcp-server/index.js` のツール説明文にも `tool-description.test.js`
  へ実行検証を追加した。説明文中のバッククォート片のうち「Ajisai ソース
  らしい形」（`[` か `'` を含む、または数字/大文字 Word/演算子だけの裸の式）
  をしたものを、サーバー自身が使う実バックエンド（`createBackend()`）の
  `check()` に実際に通し、`status: "ok"` を要求する。
- `{`/`}` は独立した3箇所（`SKILL.md`、`index.js`、そして偶然だが
  `assets/quickstart.md` はセーフだった）で再発した具体的な前科があるため、
  `generate-skill-md.mjs` に生成後アサーションを追加した: §2/§3 に
  `{`/`}` が現れたら、それが「廃止された」と明記する行の上でない限り
  生成を失敗させる。

### Phase 3 — 仕様と実装の突き合わせ

Phase 1・Phase 2 をそれぞれ個別に終えた後、初めて両者を比較する。

本フェーズは本来「`spec/` 対 実機」の2項比較だが、`SKILL.md`／
`assets/quickstart.md`（MCP が `ajisai://guide/quickstart` として配信する
ガイド）は `spec/` の正典ソースに含まれない非正典ドキュメントであり、この
比較の外側にいる。しかし実際に行動するのは `spec/` を読む人間ではなく
このガイドを読む AI であり、上の「検証の盲点 その2」が示す通り、ここに
`spec/` とは独立した第3の食い違いが生じうる。`spec/` との不一致は
Phase 1-3 の通常経路で扱うが、ガイド自身の記述と実機の一致は、Phase 2 の
「MCP 向け生成プローズ」節が定める実行検証（`generate-skill-md.mjs` の
`canonicalExamples` 参照、`selftest.js` の `prefaceExamples`、
`tool-description.test.js` の `check()` 呼び出し）が担う、別系統の保証
であると明示しておく。

**スイート裁定規則**（旧 `spec-impl-drift-tactic.md` §3.3 を引き継ぐ）:
観測される実装の挙動と本文が食い違ったとき、

- その挙動を **conformance suite (`tests/conformance/index.html`,
  `LANG.CONFORMANCE.CORPUS`) が pin している** → 意図的な設計判断とみなし、
  **spec → impl**（本文を加筆・修正して挙動を正典化する）。
- **スイートが沈黙している** → 実装バグの候補とみなし、`LANG.CONFORMANCE.FAMILIES`
  の law test に照らして本文への明文違反を確認できたら、**impl → spec**
  （実装を本文へ合わせて直す）。
- **本文内が複数箇所で食い違う**（Phase 1 の対象と重なる場合）→
  `LANG.AUTHORITY.SOURCES` の権威順位と git provenance を補助に、人間の承認を
  得て決定する。

どちらの方向であっても、変更は同一 PR 内で conformance ケースか law test を
追加・更新し、機械的に再発防止できる状態にしてから完了とする。

### Phase 4 — 死んだ側の全削除（改修履歴を含む）

Phase 3 の是正が完了し、仕様と実装が一致した時点で、**もう存在意義のない
ものを残さない**。

- **仕様だけがあり実装がないもの**（例: 2026-09 に見つかった
  `spec/host-protocol-v2.schema.json` — schema・golden fixture・
  対応する Rust/TS モジュールのどれも「no consumer reads it yet」だった）
  は、schema・実装・テスト・golden fixture を含めて全削除する。将来また
  必要になったら、そのときの実装から書き起こす——死んだ設計を仮死状態で
  持ち越さない。
- **実装だけがあり仕様にないもの**も同様に削除する。ただし削除前に、
  本当に「仕様に無い」のか（宣言が漏れているだけで実は正しい機能なのか）
  を Phase 3 の判断で確定させてから行う。
- **その是正に至る過程を記録した改修履歴**（`docs/dev/` の一過性の調査・
  監査レポート）も、Phase 1-3 が既に本文へ折り込んだ後は同様にノイズになる。
  ただし削除前に、**他の `docs/dev/` 文書や `tests/conformance/index.html`
  からその文書が実際に引用されていないか**を repo 全体で grep すること
  （`grep -rl <filename> .`）。引用がある場合は、削除前にその引用を
  自己完結する記述に書き換える（本書のように、失効した外部参照を安定
  アンカーへ差し替える）か、`docs/dev/INDEX.md` の行を合わせて削除する。
  引用を残したまま参照先だけ消すと、今回まさに直している「仕様にない
  実装／実装にない仕様」と同型の乖離を `docs/dev/` の中に作ってしまう。

#### 適用範囲の規律 — 一つの発見は芋づるの許可証ではない

死んだものを辿ると、必ず別の死んだものが見える。そのとき、二つ目の発見が
「同じ発見の別断面」なのか、**構造的に別物**なのか——別サブシステム、
別のファイル種別（CI がゲートするデータファイル、`spec/` ソース、定式化の
一節）、あるいは今の掃討範囲を超える独自の多ファイル走査を要するもの——を
先に判定する。構造的に別物であれば、**手を止め、触らず、PR 説明に後続課題
として明示的に名指しする**。Phase 3 が「本文内が食い違い、かつ conformance
の pin が無い」場合に人間の判断へ差し戻すのと同じ扱いであり、独立した
1ラウンドとして改めて着手する。

これは慎重さのためではなく、範囲の規律である。フェーズに分け、発見ごとに
PR を切ること自体が、小さく検証可能な単一目的の diff を得るための手段で
あって、書いている途中で膨らんだ削除は、宣言した範囲に照らしてレビュー
できない。「ついでに直した」部分は、後から読むと理由の書かれていない変更
としてしか残らない。

## 2. 実地適用の記録（2026-09、PR #1604-#1611, #1615）

この手順を実際に通しで走らせた記録（PR #1604-#1609 が初回セッション、
#1610 以降はチェックリスト経由の別ラウンド）:

- Phase 1: `spec/language-semantics.md` のプローズが `host-protocol.schema.json`
  の前身が定義していない範囲を主張していた等、5件の矛盾を修正（PR #1604）。
- Phase 2: GUI のソースフォーマッタが実際のトークナイザと矛盾する形で
  `^`/`|` を扱っていた実バグを発見・修正。MCP サーバーの単語提案アルゴリズムに
  クロスチェックテストを追加。デッドコードだった科学的記数法フォーマッタを
  バグごと削除（PR #1605, #1606）。
- Phase 3: `spec/host-protocol-v2.schema.json`（V2）が実装のどこからも
  呼ばれていない設計目標に過ぎず、実際に出荷されているのは別の、より
  リッチな形（`rust/src/types/value_protocol.rs`）だと判明。実機出力
  17パターンで検証した上で、実際に出荷されている形を正典として
  記述し直した（PR #1607）。
- Phase 4: V2 一式（schema・golden fixture・Rust モジュール・TS デコーダ・
  対応テスト）を削除（PR #1608）。次いで `docs/dev/` の一過性の調査・監査
  レポート11件、および本書の前身 `spec-impl-drift-tactic.md` を削除し、
  参照していた文書を本書へ向け直した（PR #1609、本書の新設を含む）。
- Phase 4（続き）: PR #1609 が header 引用だけ直して残した
  `ajisai-self-hosting-design.md` を、本文まで含めて監査。節番号・比較対象の
  ディレクトリ・語彙・自称成果物のすべてが失効していると確認して全文削除
  （PR #1610）。この回は本手順を `.claude/skills/spec-impl-alignment/SKILL.md`
  として起こしたうえで、スキル経由で初めて走らせた回でもある。
- Phase 2/4（PR #1611）: 到達不能だった `SafetyLevel` の C / Quarantined
  変種、CLI 側だけが出力し誰も読んでいなかった4フィールド
  （`semanticKind`/`shape`/`capabilities`/`origin`、
  `rust/src/agent/report.rs::semantics_json`。WASM 境界の
  `value_semantics_to_js` では既に削除済みで、コメント自身が
  "the retired HostProtocolV1" と書いていた）、`docs/dev/` に残っていた
  失効した §番号引用、および当時の66語のどこにも存在しない
  `SPAWN`/`AWAIT` 系を前提にした `rust/src/agent/contract_linearity.rs` を
  削除。この回から二つの教訓が出た:
  - コミット済み `.wasm` バンドルが**本 PR 以前から**自身のソースに対して
    陳腐化していた。ローカル検証（cargo fmt/clippy/test、npm check/lint/test、
    全 `*:check`）はすべて緑だったが、CLI 側を WASM ソースへ合わせた瞬間に
    `backend/parity-test.js` が CI（Quality Gate）で落ちた。陳腐化した二者が
    偶然一致していたため、それまで見えなかったのである。
    `npm run build:wasm` / `build:mcp-wasm` で再生成してコミットし解消
    （Phase 2「検証の盲点」の由来）。
  - `contract_linearity.rs` の調査中に、同じ `SPAWN`/`AWAIT` 面が
    当時の `ajisai-mathematical-formalization.md`（後に削除）§9-septies では実在しない
    `rust/tests/child_runtime_laws.rs` を根拠に `HOLDS` と記載される一方、
    CI がゲートする `docs/formalization-coverage.json` では正しく
    `"Exploratory"` に分類されている、という Phase 1 型の矛盾が判明した。
    承認済みの範囲を大きく超えるため着手せず、後続課題として PR 説明に
    明記するにとどめた（Phase 4「適用範囲の規律」の由来）。
    **後続対応（2026-09）**: 監査を再開したところ、同じ非ゲート散文が
    §9-septies 以外にも実在しないテスト（`record_laws.rs`・
    `effect_observation_laws.rs`）を根拠にした `HOLDS` 主張、実装から
    退役済みの値域（Record/CodeBlock/子ランタイムハンドル）を前提にした
    §1.1 の値空間定義、文書内でも食い違う conformance 件数（53 / 45、
    実数 293）など、広範囲に陳腐化していることが判明した。CI 非ゲートで
    検証の当てがなかったため、`ajisai-mathematical-formalization.md`
    を全文削除し、これを引用していた `rust/tests/*.rs` のコメント・
    `docs/formalization-coverage.json` の `formalization_sections` フィールド
    （CIの必須要件ごと）・関連 `docs/dev/` メモの参照を除去した。
- Phase 2（MCP 向け生成プローズ、2026-09、PR #1615）: Ajisai の MCP サーバーへ実際に
  接続してツールを手動で叩く中で、`SKILL.md` §2/§3 と
  `tools/mcp-server/index.js` のツール説明文それぞれに独立に残っていた、
  廃止済み `{ }` ブロック構文を発見・修正。`generate-skill-md.mjs` を
  `canonicalExamples` 参照方式に直し、`tool-description.test.js` に
  ライブバックエンドでの実行検証を追加し、生成後アサーションで `{`/`}`
  の再発を構造的に防いだ（本節「検証の盲点 その2」の由来）。
