# Ajisai Portability Policy

Ajisai の移植性とは、Rust/WASM/Tauri/Web に依存せず、Ajisai Core の意味論を
別の実装言語・別の実行環境・別の UI 上で再現できる性質である。Ajisai の同一性は
特定実装ではなく conformance suite が定義する入出力の対応関係によって与えられる。

## Principles
1. Ajisai の正典は特定実装ではなく、仕様と conformance suite である。
2. Rust/WASM 実装は参照実装のひとつであり、唯一の正統実装ではない。
3. Ajisai Core はホスト非依存でなければならない。
4. 外部効果は Host Capability として扱い、Host Effect として構造化する。
5. Host Capability の不足は仕様化された方法で失敗する。
6. Core 語彙は決定的でなければならない。
7. Hosted 語彙は要求する Capability を明示しなければならない。
8. 新機能は Core か Hosted かを明示して追加する。
9. Platform 固有 API を Core へ直接持ち込まない。
10. ある実装が Ajisai であることは conformance suite を通すことで証明される。
11. conformance が固定する対応が現象であり、それ以外は実装の裁量である。
12. 正規化は最小限とし、緩める方向にのみ明示的に変更する。

## AI-readable Semantic Index

Ajisai の移植性メタデータは `docs/word-manifest.json` と
`docs/formalization-coverage.json` の二層で管理する。`word-manifest` は生成物であり、
各 surface word に `canonical` と `semantic_role` を付与した機械可読索引である。
`formalization-coverage` は各 canonical entry の数式化状況、`derived_from`、Primitive registry、
HostedEffect capability、Exploratory debt metadata を保持する。

Primitive registry の各 primitive は `admission_reason`、`introduced_by`、`can_derive`、
`status` を持たなければならない。Derived entry は登録済み primitive/schema だけを
`derived_from` で参照し、Sugar は `desugars_to`、HostedEffect は `capability` と
`effect_schema`、Exploratory は `reason`、`exit_options`、`review_gate` を持つ。
これらの欠落は `npm run check:formalization-coverage` で検出する。

## Lineage
Ajisai は FORTH から辞書システムを継承した遠縁である。ただし FORTH が
スタックベースなのに対し Ajisai は Vector ベースであり、実行モデルの根幹が
異なる。FORTH の移植性（小さなスタック VM の再実装）をそのまま継承せず、
Ajisai の移植性は conformance suite が定義する現象の再現そのものである。

## Portability Layers
| Layer | Description | Examples |
| --- | --- | --- |
| Core | ホスト非依存の意味論的核 | vector evaluation, arithmetic, map/fold/filter, NIL/UNKNOWN |
| Hosted | 外部能力を要求する語彙 | PRINT |
| Platform | 具体的な宿主 | Web, WASM, Tauri, CLI, WASI, Native |

## Conformance
Ajisai の現象は tests/conformance/ 配下の HTML スイートが定義する。各ケースは
Ajisai ソースと、期待される結果および Host Effect の列を構造化して持つ。
実装はこのスイートを通すことで適合性を示す。

判定規則・正規化規則の詳細は `tests/conformance/index.html` の冒頭に記述され、
参照実装側のランナーは `rust/src/conformance_tests.rs` にある。効果の観測対象は
構造化された Host Effect の列であり、人間可読な出力文字列ではない。
