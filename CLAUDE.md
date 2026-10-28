# Arare：作業の恒久ルール

Arare は、量子化を前提とし、失った精度を数字で管理するスタック型・ベクトル指向の言語である。
開発動機は姉妹言語 Ajisai の遅さを解決することにあり、**性能要件（AR-PERF）を満たさない成果物は完成とみなさない**。
全体の計画と決定事項は `WORK_ORDER.md` にある（フェーズ 0〜1 の間に `docs/quality/` の計画書群へ分解する）。

## 必ず守ること

1. **要求 ID のないコードを書かない**。変更は `AR-HLR-nnn`（`spec/`）か `AR-LLR-nnn`（`docs/design/`）に結びつけ、コミットメッセージに ID を書く。派生要求には `derived: true` を付ける。
2. **トレーサビリティの表を同じ差分の中で更新する**。要求、実装、検証のどれかを変えたら `docs/quality/TRACEABILITY_MATRIX.md` も直す。
3. **QL-A のコードを変えたら、そのコードを証明しているハーネスを手元で実行してから**レビューに出す（`cargo kani --harness <name>`、`lake build`）。
4. **テストを無効にしたり、飛ばしたり、隔離したりして CI を緑にしない**。カバレッジのラチェットを下げるときは、同じ差分で理由を書いてベースラインを記録し直す。
5. **生成物を手で編集しない**。生成スクリプトで作り直し、`--check` で差分がないことを確かめる。
6. **ツールチェーンの版を勝手に変えない**。変えるときは `docs/quality/CONFIGURATION_INDEX.md` を同じ差分で更新する。
7. **決定事項（`WORK_ORDER.md` の §2）を変えない**。変更が必要だと考えたら、所有者に提案する。
8. **形式証明について、証明していないことを証明済みと書かない**。証明の範囲と限界は `docs/quality/FORMAL_VERIFICATION.md` に書く。
9. **認証の取得を主張しない**。「DO-178C 風の開発品質の枠組み」と表記する。

## 意味論の要点（詳しくは spec/）

- 既定の数は int8／int16／int32 の固定小数点。算術演算は結果が収まる幅まで広げ、丸めるのは `RESCALE` と `NARROW` のときだけ。
- 輪の2点：⊥（0/0）は吸収元、1/0 は範囲外（オーバーフロー）を表す。どちらも数であり NIL ではない。大小を問うと UNKNOWN になる。
- 三値論理は strong Kleene。UNKNOWN は真偽の位置に現れた NIL。`FILTER` は TRUE の要素だけを残す。分岐が UNKNOWN のときは ELSE 側に進み、集約は NIL を飛ばす（SQL と同じ）。
- 真偽値と NIL のメモリ配置は Apache Arrow と同じ（有効ビット列と値ビット列）。
- Word の名前と意味は、Arare 固有の理由がない限り Ajisai に揃える。Ajisai はオラクル（正解の基準）である。

## 検証で使うコマンド（Phase 0 で確定させて書き足す）

- `rust/` で：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`、`cargo kani`
- `formal/` で：`lake build`
- リポジトリ直下で：`npm run check`、`npm run lint`、`npm test`、`npm run check:traceability`
- 性能：`cargo run --release --example speed_bench`（AR-PERF の要件を割っていないか確かめる）
