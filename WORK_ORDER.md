# Arare 作業指示書

Status: 発行版 2（2026-10-28）。Ajisai の内容から始めた直後の Arare リポジトリで作業する人間とエージェントに向けた文書。
置き場所：リポジトリ直下の `WORK_ORDER.md`。恒久ルールは別紙 `CLAUDE.md` に置く。
この文書はフェーズ 0〜1 の間に `docs/quality/` の計画書群へ分解され、分解し終えたら役目を終える（削除してよい）。

---

## 0. この文書の読み方

- **§2 決定事項**は確定済み。変更するには所有者の承認が要る。
- **§3 初期案**は Phase 1 で高位要求（HLR）として確定させる設計の出発点。根拠を示せば変更してよい。
- **§9 未決事項**は、指定されたフェーズの中で ADR（`docs/adr/`）を書いて決める。
- 各フェーズには**入口条件と出口条件**がある（§7）。出口条件を満たさないまま次のフェーズに進まない。

---

## 1. 目的

### 1.1 Arare とは何か

Arare（霰）は、**量子化を前提とし、失った精度を数字で管理する**スタック型・ベクトル指向の言語である。
姉妹言語 Ajisai（紫陽花）は丸めない厳密計算に全振りしている。Arare はその対極で、常に丸め、その代わりに「どれだけ不正確か」を型と契約で扱う。

### 1.2 開発動機と成功条件

**開発動機は Ajisai の遅さを解決することである。これを満たさなければ Arare を作る意味はない。**
したがって性能は機能要件と同じ重みの要件であり（§6）、性能要件を満たさない成果物は完成とみなさない。

参考値として、Ajisai リポジトリで 2026-10-28 に測った値を示す（Xeon 2.1GHz、ネイティブのリリースビルド、5回中の最良値）。

| 計算 | Ajisai | 固定幅のネイティブ | 差 |
|---|---:|---:|---:|
| 整数の内積（100万要素） | 3.3 ms | i64：0.36 ms ／ int8：0.09 ms | 9倍 ／ 37倍 |
| 小さい有理数の内積（100万要素） | 82 ms | f32：0.14 ms | 約580倍 |
| 調和級数 Σ1/k（k = 2万） | 57 ms | 0.028 ms | 約2,000倍 |
| ロジスティック写像（20ステップ） | 1,450 ms | 91 ns | 約1,600万倍 |
| トップレベルのスカラー演算 | 165〜416 ns／語 | 約1 ns | 100〜400倍 |

Ajisai の遅さの原因は4つある。(1) 値が膨らむこと（指数的に効き、最大の原因）、(2) 解釈実行のオーバーヘッド、(3) 値の幅、(4) WASM であること（1.7〜4倍）。
Arare はこの4つを、それぞれ固定幅、事前コンパイル、int8 の SIMD、ネイティブ実行と WASM SIMD で解決する。

### 1.3 Ajisai との関係

- Ajisai は **Arare の正解の基準（オラクル）**である。同じソースを両方で実行し、`|Arare の結果 − Ajisai の厳密値| ≤ Arare が推論した誤差予算` が成り立つことを、検証の柱の一つにする。
- Word の名前と意味は、Arare 固有の理由がない限り Ajisai に揃える。揃えておかないと、オラクルとの比較ができなくなる。

---

## 2. 決定事項（所有者承認済み）

| ID | 決定 |
|---|---|
| D-01 | **構文は Ajisai と同じスタック型**で、Word を並べて書く。Playground、ステップ実行、語彙パネル、Reference を引き継ぐための前提である |
| D-02 | **既定の数は int8／int16／int32 の固定小数点**とする。素数剰余（RNS）は既定にしない |
| D-03 | **素数は剰余が速く計算できるものに限る**（Mersenne31 = 2³¹−1、Goldilocks = 2⁶⁴−2³²+1、BabyBear = 2³¹−2²⁷+1）。用途は (a) 64bit を超える精度が要るときの広幅モード (b) 大きな畳み込みを NTT で計算すること (c) Halton 列によるディザ |
| D-04 | **実行方式は2つ持つ**。ステップ実行用の参照インタプリタ（遅いが中身を追える）と、事前コンパイル（ネイティブ版と WASM SIMD128 版）。性能要件は事前コンパイルの経路に課す |
| D-05 | **三値論理は Ajisai の strong Kleene をそのまま引き継ぐ**。UNKNOWN は「真偽の位置に現れた NIL」として扱い、`FILTER` は TRUE の要素だけを残す |
| D-06 | **真偽値と NIL のメモリ配置は Apache Arrow と同じにする**。有効ビット列と値ビット列を持ち、Arrow C Data Interface を通じてコピーせずに受け渡せるようにする |
| D-07 | **条件が UNKNOWN のときの分岐は SQL に合わせ、ELSE 側に進める**（`CASE WHEN` と同じ）。これは Ajisai の `SELECT`（NIL を返す）とは意図的に変える点である |
| D-08 | **集約は SQL に合わせ、NIL を飛ばして集計する**（`[ 1 NIL 2 ]` の和は 3）。全要素が NIL の集約は NIL とする |
| D-09 | **輪（wheel）の2つの点を採用する**。`⊥`（0/0）と `1/0` を持つ。`1/0` は「範囲外」、つまりオーバーフローを表す印として使う |
| D-10 | ⊥ と 1/0 は**数**であり、NIL は**欠損**である。両者を混同しない。⊥ と 1/0 の大小を問うと UNKNOWN になる |
| D-11 | **NIL の理由は参照インタプリタ（ステップ実行）でだけ保持する**。コンパイルした経路では有効ビットだけを持つ |
| D-12 | **契約キー `field=closed` を引き継ぐ**。Arare では「オーバーフローもゼロ除算も起きないことが静的に証明された」という意味になる |
| D-13 | **Playground の GUI は「エンジンの差し替え口を持つ共通の殻」にする**。Ajisai のエンジンを、オラクルとして Playground に同居させる |
| D-14 | **最初の市場は CPU とエッジでの平文計算**とする。ZK と FHE のバックエンドは Phase 7 以降に回す |
| D-15 | **形式検証と、DO-178C 風の検証を最初から入れる**（§5）。ただし認証の取得を主張するものではない |
| D-16 | **文書の言語は Ajisai の慣例に従う**。仕様（`spec/`）とコードのコメントは英語、開発メモ（`docs/dev/`）と作業指示は日本語（Q-04 で決定） |
| D-17 | **Ajisai の Git 履歴は引き継がない**。Arare の履歴は、Ajisai `4d5f693`（2026-10-23）の内容を取り込んだ1コミット `481dc6c` から始まる（Q-05 で決定。タグは付けない） |

---

## 3. 初期案（Phase 1 で HLR として確定させる）

### 3.1 値の符号化（N bit の場合）

| 符号 | 意味 | int8 での値 |
|---|---|---|
| `−2^(N−1)` | ⊥（0/0） | −128 |
| `2^(N−1) − 1` | 1/0 | 127 |
| `−(2^(N−1) − 1)` | 使わない（正規化で生成されない） | −127 |
| `[−(2^(N−1)−2), 2^(N−1)−2]` | 有限の値 | [−126, 126] |

有限の値の範囲を対称にするので、符号の反転は有限の値の中で閉じる。1/0 の符号反転は 1/0 自身とし、1/0 には符号を持たせない（Ajisai と同じ）。

### 3.2 スケール

- 実数値は `符号 × 2^s` で表す。既定のスケールは2の冪とし、rescale はシフトと丸めで済ませる。
- 2の冪以外のスケール（滑らかな数 `2ᵃ3ᵇ5ᶜ7ᵈ` など）は拡張案として扱い、Phase 1 では対象外とする。

### 3.3 拡幅と丸め

- **算術演算は結果が収まる幅まで自動的に広げる**。例えば int8 × int8 は int16 になり、そのとき幅と範囲は静的に決まる。
- **幅を狭めるのは `RESCALE` と `NARROW` を明示したときだけ**とし、丸めが起きるのもそこだけにする。丸めの既定は最近接偶数丸め（round half to even）。
- 狭めた結果が有限の値の範囲を超えたら 1/0 にする。上限に張り付かせる飽和はしない。
- ゼロでない値がゼロに丸められたら 0 にする。Arare の 0 は「ゼロに丸められた値」を含む。
- この方針により、オーバーフローが起きうる場所は明示的に狭める箇所と最大幅だけになり、静的解析で扱える（D-12）。

### 3.4 輪の規則

Ajisai の `spec/language-semantics.md` の LANG.VALUES.EXACT と同じ規則を、§3.1 の符号の上で定義する。

- `x/0 = 1/0`（x ≠ 0 のとき）、`0/0 = ⊥`、`0 · 1/0 = ⊥`、`1/0 + 1/0 = ⊥`、`1/0 + 有限の値 = 1/0`。
- ⊥ は、どの演算に対しても結果を ⊥ にする（吸収元）。
- 等しさは表示の同一性で決める。`⊥ EQ ⊥` は TRUE。
- 大小（`LT`、`GT`、`MIN`、`MAX`、`SORT`）を ⊥ や 1/0 に問うと UNKNOWN になる。

### 3.5 SQL への書き出し

1/0 と ⊥ は SQL の NULL として書き出し、どちらだったかは Arrow のフィールドのメタデータに残す。

---

## 4. 引き継ぎの棚卸し（Phase 0 で実施）

**方針**：Ajisai のエンジンのコードは Arare リポジトリから取り除く。オラクルとして必要な Ajisai は、Rust では Git の特定のリビジョンに固定した依存として、Playground ではビルド済みの WASM バンドルとして参照する。

| パス | 扱い | 作業内容 |
|---|---|---|
| `src/`（Playground の GUI、約8,700行） | **改修** | エンジンとの境界である `src/wasm-interpreter-types.ts` を「差し替え口のインターフェース」に一般化する。Ajisai 用と Arare 用の2つの実装を持つ |
| `src/wasm/generated/` | **改修** | Ajisai の WASM を `src/wasm/ajisai-oracle/` に移し、元の Ajisai のコミットの SHA を記録する。Arare の WASM は別の場所に生成する |
| `src-tauri/` | **流用** | 名前の変更だけ行う |
| `public/docs/`、`scripts/generate-word-reference.mjs` | **改修** | Phase 0 では Ajisai の生成済み Reference をオラクル用にそのまま残し、生成器は削除した（PR-0008）。Phase 5 で `UPSTREAM.md` のコミットから生成器を移植し、項目に「幅、スケール、誤差、field」を追加する |
| `spec/` | **改修** | Phase 0 では `host-protocol.schema.json`（Playground とエンジンの境界）だけを残し、Ajisai の仕様と生成器は削除した。生成の仕組み（`words.schema.json`、`generate-specification.mjs`）は Phase 1 で `UPSTREAM.md` のコミットから移植し、中身を Arare の HLR にする（PR-0008） |
| `rust/` | **置換** | 済み：`rust/src` などを削除し、§8 のクレート構成の空のワークスペース（`rust/crates/`）にした。wasm-bindgen の境界の作り方とエージェント向け CLI の型は、`UPSTREAM.md` のコミットを参考にする |
| `rust/src/**/*_proofs.rs`、`*_mcdc_tests.rs` | **型として流用** | 中身は削除するが、Kani のハーネスと MC/DC 表の書き方は Arare でも踏襲する |
| `docs/quality/` | **改修** | DO-178C 風の計画書群に作り直す（§5.2） |
| `docs/dev/`、`docs/showcase/`、`SPECIFICATION.html`、`SKILL.md`、`skill/` | **削除** | Ajisai の経緯資料と生成物なので持ち込まない。必要になれば Arare 用に再生成する |
| `docs/i18n/` | **保留** | 日本語の要約を出す仕組みとして Phase 5 で判断する |
| `tools/mcp-server/` | **保留** | 済み：Ajisai の MCP サーバーをエージェント用のオラクルとして残し、CI から外した。`publish-mcp.yml` は削除した（PR-0007） |
| `tools/lexicon-emergence/` | **削除** | |
| `scripts/check-*.mjs` | **選別** | 済み：traceability、coverage-ratchet、version-sync、file-size-budget、contrast を Arare 用に改修して残した。spec-citations、spec-version、reading-surfaces は読む対象（Ajisai の仕様）がなくなったので削除し、Arare の仕様ができたら移植する。Ajisai 固有のものは削除した |
| `scripts/bench/` | **改修** | Phase 0 では削除した（Ajisai のエンジンが前提だったため）。Phase 4 で、ケースの形式（`speed-bench-cases.json`）を `UPSTREAM.md` のコミットから移植し、§6 の性能要件のケースを入れる |
| `.github/workflows/` | **改修** | `test.yml` の quality-gate（全ステップがマージを止める）と formal-verification ジョブの構造を流用する |
| `.claude/hooks/session-start.sh` | **改修** | lexicon-emergence と MCP サーバーの手順を外し、Lean と Kani の準備を加える（§5.4） |
| `.claude/skills/spec-impl-alignment/` | **改修** | Phase 0 では削除した（Ajisai の仕様が前提だったため）。Arare の仕様ができたら（Phase 1 以降）、照合用に書き直す |
| `public/images/Ajisai_QR_*`、バッジ類 | **削除・差し替え** | |
| `LICENSE`（MIT） | **流用** | 著作権表示の年と名前を確認する。`THIRD-PARTY-LICENSES.md` は依存が変わるたびに更新する |
| `.mcp.json` | **改修** | 当面は Ajisai の MCP サーバーをオラクル用に残してよい。Arare の MCP サーバーは Phase 6 で追加する |

名前の変更：`ajisai` を含むファイルは約210ある（2026-10-28 時点）。ただし大半は §4 の表で削除するか置き換える対象なので、**名前の変更は削除が終わってから**行う。

---

## 5. 品質と検証の体制（最初から入れる）

### 5.1 原則

1. **要求が先、コードは後**。要求 ID のないコードはマージしない。
   Ajisai では、要求の文面の大半が後から復元されたものだった（`docs/quality/TRACEABILITY_MATRIX.md` の冒頭を参照）。Arare ではこれを繰り返さない。
2. **トレーサビリティは双方向で、機械で検査する**。要求 → 設計 → コード → 検証の各リンクを `scripts/check-traceability-matrix.mjs` の流れで検査し、切れたリンクがあれば CI を落とす。
3. **検証は独立させる**。実装したセッションとは別のセッション（または人間）が、要求に基づくテストをレビューする。
4. **証明は試験の代わりになりうる**（DO-333 の考え方）。ただし、何を証明して何を証明していないかを、`FORMAL_VERIFICATION.md` に必ず書く。
5. **認証の取得は主張しない**。「DO-178C 風の開発品質の枠組み」と表記する。

### 5.2 計画書と成果物（DO-178C の成果物との対応）

すべて `docs/quality/` に置く。Phase 0 では骨子だけを書き、各フェーズで中身を埋めていく。

| DO-178C の成果物 | Arare のファイル |
|---|---|
| PSAC（全体計画） | `PLAN_OVERVIEW.md` |
| SDP、SVP、SCMP、SQAP（開発、検証、構成管理、品質保証の各計画） | `DEVELOPMENT_PLAN.md`、`VERIFICATION_PLAN.md`、`CONFIGURATION_MANAGEMENT_PLAN.md`、`QUALITY_ASSURANCE_PLAN.md` |
| 要求、設計、コードの各標準 | `STANDARDS.md` |
| SRD（高位要求） | `spec/`。各要求に `AR-HLR-nnn` を振る |
| SDD（低位要求と設計） | `docs/design/`。各要求に `AR-LLR-nnn` を振る |
| SVCP（検証のケースと手順） | テストのコードと `VERIFICATION_CASES.md` |
| SVR（検証の結果） | CI が出力する成果物。リリースごとに `docs/quality/results/` に要約を残す |
| SCI、SECI（構成の索引、環境の索引） | `CONFIGURATION_INDEX.md`。固定したツールチェーンと版をすべて記載する |
| PR（問題報告） | GitHub の Issue テンプレート。報告から修正、検証までをリンクでつなぐ |
| SAS（達成のまとめ） | リリースごとに `RELEASE_SUMMARY.md` |

### 5.3 品質レベル（DAL に相当するもの）

| レベル | 対象 | 求める検証 |
|---|---|---|
| **QL-A** | 値の符号化と算術のカーネル（輪、丸め、rescale、Kleene の演算、Arrow のビット列）、コンパイラの下位変換とコード生成、誤差予算の検査器 | 要求に基づくテスト、MC/DC 表、形式証明（可能な範囲すべて）、独立したレビュー、カバレッジのラチェット（下がったら CI を落とす） |
| **QL-B** | 字句解析と構文解析、形状と幅の推論、参照インタプリタ、ホストプロトコル | 要求に基づくテスト、判定カバレッジ、ラチェット |
| **QL-C** | MCP サーバー、Arrow と SQL の入出力、Reference の生成器 | 要求に基づくテスト、文カバレッジ |
| **QL-D** | Playground の GUI、文書 | 要求に基づくテスト |

**派生要求**（上位の要求から直接は導けず、設計の都合で生まれた要求）には `derived: true` を付け、レビューで必ず理由を確認する。

### 5.4 形式検証の構成

| 層 | ツール（版を固定する） | 証明の対象 |
|---|---|---|
| 意味論のモデル | **Lean 4** と Mathlib（`formal/` に置く。`lean-toolchain` と `lake-manifest.json` で版を固定） | 符号集合の上で輪の公理が成り立つこと。Kleene の演算が束になること。**`field=closed` のコードの中では演算が ℤ と一致すること**。**rescale の誤差が 1/2 ulp 以下であること**。**誤差予算の推論が健全であること**（最重要の定理） |
| モデルと実装の橋渡し | Lean で書いたモデルを実行し、Rust の実行可能仕様と比べる差分試験 | int8 の二項演算のすべての組（65,536通り）について、Lean のモデルの結果と Rust の結果が一致すること |
| 実装 | **Kani**（Ajisai と同じく 0.68.0 から始める） | int16 と int32 のカーネルが、全範囲で実行可能仕様と一致すること。オーバーフロー、範囲外の添字、panic が到達不能であること |
| 全数試験 | 通常のテスト | int8 の二項演算は全組を CI で毎回試す（数秒で終わる） |
| 翻訳の検証 | 差分試験 | コンパイルした結果（ネイティブ版と WASM 版）が、int8 では全数、int16 と int32 では無作為と境界値で、参照インタプリタと一致すること |
| オラクル | Ajisai（固定したリビジョン） | ランダムに生成したプログラムで `|Arare − Ajisai| ≤ 推論した予算` が成り立つこと。これは誤差予算の健全性を経験的に確かめる試験であり、Lean の証明と二重に確かめることになる |

MC/DC は Ajisai と同じく、**テストの中に表として書く**方式を踏襲する（`fraction_mcdc_tests.rs` の書き方）。rustc の MC/DC 計測機能には頼らない。

### 5.5 ツールの適格性（DO-330 の考え方）

- Arare を開発するのに使うツール（rustc、Lean、Kani、カバレッジの計測器）は、版を固定し、`CONFIGURATION_INDEX.md` に記載する。rustc の出力は、ビルドした成果物に対してテストを実行することで確かめる。
- **Arare が利用者に提供するツール**のうち、コンパイラは「出力がソフトウェアの一部になるツール」、誤差予算の検査器は「検証の代わりになるツール」に当たる。どちらも将来、適格性を示す一式を提供できるよう、決定的であること（同じ入力に同じ出力）と、要求との対応関係を最初から保つ。

### 5.6 構成管理

- ツールチェーンはすべて版を固定する（Rust の nightly、Kani、Lean、Node、wasm-opt）。
- proptest のシードを固定する（Ajisai と同じく `PROPTEST_RNG_SEED`）。
- ベースラインはコミットで識別する。取り込み直後の状態が `481dc6c` で、元の Ajisai のコミットの SHA は `UPSTREAM.md` に記録してある。タグは所有者の判断で付けない。
- 生成物は手で編集せず、必ず生成スクリプトで作り直す。CI ではスクリプトの `--check` モードで差分がないことを確かめる。

---

## 6. 性能要件

AR-PERF 系の要求は、**同じマシン、同じ実行で測った基準ループとの比**で定義する。絶対時間で定義すると CI のマシンによって揺れるからである。ベンチマークは CI で毎回実行し、要件を割ったら CI を落とす。

| ID | 対象（ネイティブ版、事前コンパイルの経路） | 要件 |
|---|---|---|
| AR-PERF-001 | int8 の内積（100万要素） | 手で書いた Rust の int8 ループの **1.5倍以内**。かつ Ajisai のネイティブ版の **20倍以上速い** |
| AR-PERF-002 | 小さい有理数の内積に相当する固定小数点の計算 | Ajisai の **300倍以上速い** |
| AR-PERF-003 | ロジスティック写像（20ステップ） | **1 µs 以下**（オラクルとの誤差は予算の範囲内） |
| AR-PERF-004 | 調和級数（k = 2万） | Ajisai の **1,000倍以上速い** |
| AR-PERF-005 | トップレベルのスカラー演算 | **1語あたり 2 ns 以下** |
| AR-PERF-006 | 輪の印と NIL の伝播によるオーバーヘッド | 印を持たない場合の **1.1倍以内**（計算が律速になる処理でも測る） |
| AR-PERF-007 | WASM SIMD128 版 | ネイティブ版の **3倍以内**（目標値。この値は未測定なので Phase 4 で見直す） |

Ajisai の値は、オラクルとして固定したリビジョンで、同じ実行の中で測る。

---

## 7. フェーズ計画

| フェーズ | 内容 | 出口条件 |
|---|---|---|
| **0. 初期化** | §4 の棚卸しを実施する。計画書の骨子（§5.2）を書く。ツールチェーンを固定する。CI の骨組みを作る。Playground を差し替え口の構造にし、まず Ajisai のオラクルエンジンだけで動かす | CI が緑になる（Rust のコアは空でもよい）。Playground が Ajisai の WASM で起動する。計画書の骨子のレビューが終わっている |
| **1. 要求と形式モデル** | 値のモデル（§3）を HLR として書く。Lean のモデルを書き、§5.4 の定理（予算の健全性を除く）を証明する | HLR のレビューが終わっている。Lean の証明が CI で通る。§3 の初期案がすべて決定または ADR になっている |
| **2. 参照インタプリタ** | 最小限の Word を実装する：リテラル、`QUANTIZE`、`ADD`、`SUB`、`MUL`、`DIV`、`RESCALE`、`NARROW`、`EQ`、`LT`、`AND`、`NOT`、`FILTER`、`MAP`、`FOLD`、`SUM`、`SELECT`。全数試験、Kani、MC/DC 表、オラクルとの差分試験の仕組みを作る | QL-A と QL-B の検証がすべて通る。Lean のモデルと Rust の差分試験が全数で一致する |
| **3. 誤差予算の検査器** | 範囲と誤差を静的に解析し、`field=closed` を判定する。予算の健全性を Lean で証明する | 健全性の定理が証明されている。オラクル試験（ランダムなプログラム1万本以上）で違反がない |
| **4. 事前コンパイラ** | 中間表現を経て、WASM SIMD128 版とネイティブ版のコードを生成する（バックエンドは ADR で選ぶ）。翻訳の検証を行う | 翻訳の検証が通る。AR-PERF-001〜006 を満たす。AR-PERF-007 を測定して見直す |
| **5. Playground** | 共通の殻を完成させる。Arare のエンジンを接続する。Reference を生成する。Ajisai との比較欄と誤差予算のパネルを作る。ステップ実行と Run を切り替えられるようにする | 「Playgroundで開く」リンクが Arare の Reference で動く。同じソースを Ajisai と Arare で実行した差が表示される |
| **6. 外部との接続** | MCP サーバーを作る。Arrow の入出力（DuckDB と Polars でコピーせずに受け渡す）を作る。SQL への書き出しの対応表（§3.5）を実装する | Arrow の往復で値と NIL が保たれる |
| **7. 以降** | 広幅モード（RNS：M31、Goldilocks、BabyBear）、NTT、Halton 列によるディザ、ZK と FHE のバックエンド | 各機能について別途計画を立てる |

**どのフェーズにも共通する条件**：CI が緑であること。トレーサビリティの検査が通ること。カバレッジのラチェットが下がっていないこと。そのフェーズの成果が `CONFIGURATION_INDEX.md` に記録されていること。

---

## 8. 想定するクレート構成（Phase 0 で ADR にする）

```
rust/
  arare-spec/      # 実行可能仕様（Lean のモデルを写したもの。速度は求めない）     QL-A
  arare-kernel/    # 符号化、算術、Kleene、Arrow のビット列。#![no_std] にできる形   QL-A
  arare-front/     # 字句解析、構文解析、形状と幅の推論                             QL-B
  arare-budget/    # 誤差予算の検査器、field 契約                                    QL-A
  arare-interp/    # 参照インタプリタ（NIL の理由を保持する、ステップ実行用）        QL-B
  arare-codegen/   # 中間表現 → WASM SIMD128 ／ ネイティブ                           QL-A
  arare-wasm/      # Playground 用の wasm-bindgen の境界                             QL-B
  arare-cli/       # エージェント向けの CLI                                           QL-C
formal/            # Lean 4
oracle/            # Ajisai（固定したリビジョン）との差分試験
```

---

## 9. 未決事項

| ID | 内容 | 決めるフェーズ |
|---|---|---|
| Q-01 | 幅とスケールをどこに書くか。Ajisai には型を書く場所がない。案：データが入る場所（リテラルや `QUANTIZE`）でだけ宣言し、ほかはすべて推論する | 1 |
| Q-02 | コード生成のバックエンド。候補：WASM は `wasm-encoder` で直接生成し、ネイティブは Cranelift を使うか、Rust のコードを生成する | 4 |
| Q-03 | `SELECT` と `FOLD` を SQL の意味に変えたとき（D-07、D-08）、オラクルとの比較をどう揃えるか。Ajisai 側で NIL を飛ばす前処理を書くか、比較器の側で吸収するか | 2 |
| Q-04 | ~~仕様書の言語~~ → D-16 で決定 | — |
| Q-05 | ~~Git の履歴~~ → D-17 で決定 | — |
| Q-06 | int64 を持つか。持つ場合、最大の幅で起きるオーバーフローをどう扱うか | 1 |

---

## 10. 最初のセッションで行う作業（Phase 0 の着手）

進捗（2026-10-28）：1〜8 は済み（masamoto1982/Arare#1 でマージ）。残した課題は `docs/quality/PROBLEM_REPORTS.md` にある。

1. ~~`UPSTREAM.md` を作り、`arare-baseline-0` のタグを付ける~~（ベースラインのコミットで済んでいる。タグは所有者の判断で付けない）。
2. ~~§4 の「削除」に当たるものを削除する~~。
3. ~~`src/wasm/generated/` を `src/wasm/ajisai-oracle/` に移し、Playground が Ajisai のエンジンで起動することを確かめる~~。
4. ~~`rust/` を空のワークスペース（§8 の骨組み）にする~~。
5. ~~`formal/` に Lean 4 のプロジェクトを作り、ツールチェーンを固定する~~（Lean v4.34.0）。
6. ~~`docs/quality/` の計画書の骨子と、`CONFIGURATION_INDEX.md` の初版を書く~~。
7. ~~名前を変える~~。Playground 内部の識別子は Phase 5 で扱う（PR-0006）。
8. ~~以上をドラフトの PR にまとめ、CI が緑になるまで直す~~。

---

## 付録 A. Ajisai で読むべきファイル

| 目的 | パス（Ajisai リポジトリ） |
|---|---|
| 三値論理の定義 | `spec/language-semantics.md` の LANG.VALUES.TRUTH |
| 輪の規則と field 契約 | 同じファイルの LANG.VALUES.EXACT と LANG.CONTRACT.FIELD |
| Kani の使い方とハーネスの型 | `docs/quality/FORMAL_VERIFICATION.md`、`rust/src/interpreter/truth_ops_proofs.rs` |
| MC/DC 表の書き方 | `rust/src/types/fraction_mcdc_tests.rs` |
| トレーサビリティとカバレッジのラチェット | `docs/quality/TRACEABILITY_MATRIX.md`、`scripts/check-coverage-ratchet.mjs` |
| CI のマージ判定 | `.github/workflows/test.yml` の quality-gate ジョブと formal-verification ジョブ |
| Playground とエンジンの境界 | `src/wasm-interpreter-types.ts`、`spec/host-protocol.schema.json` |
| Reference の生成 | `scripts/generate-word-reference.mjs`、`spec/words.json` |
| 速度の計測 | `rust/examples/speed_bench.rs`、`scripts/bench/speed-bench-cases.json` |
| 厳密計算の速度比較 | `docs/dev/exact-arithmetic-benchmark-2026-10-21.md` |
| SQL との親和性に関する経緯 | `docs/dev/sql-affinity.md`、`docs/dev/three-valued-truth.md` |
