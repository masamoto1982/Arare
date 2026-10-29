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

- 既定の数は int8／int16／int32 の固定小数点（int64 は拡幅の上限）。算術演算は結果が収まる幅まで広げ、丸めの場所は `RESCALE`、`NARROW`、`DIV` だけ（ADR-0002）。
- 輪の2点：⊥（0/0）は吸収元、1/0 は範囲外（オーバーフロー）を表す。どちらも数であり NIL ではない。大小を問うと UNKNOWN になる。
- 三値論理は strong Kleene。UNKNOWN は真偽の位置に現れた NIL。`FILTER` は TRUE の要素だけを残す。分岐が UNKNOWN のときは ELSE 側に進み、集約は NIL を飛ばす（SQL と同じ）。
- 真偽値と NIL のメモリ配置は Apache Arrow と同じ（有効ビット列と値ビット列）。
- Word の名前と意味は、Arare 固有の理由がない限り Ajisai に揃える。Ajisai はオラクル（正解の基準）である。

## 検証で使うコマンド

- `npm run check:rust`：`rust/` で fmt、clippy（`-D warnings`）、テスト
- `npm run check:formal`：`formal/` で `lake build` のあと、全定理が標準の公理だけに依ることを確かめる（`lake build` だけでは `sorry` が通る）。elan が要る。入れ方は `docs/quality/FORMAL_VERIFICATION.md`
- `cargo kani`（`rust/` で）：Kani 0.68.0 が要る
- `npm run check:gates`：要求の登録簿、トレーサビリティ、版の一致、Rust のファイルサイズ、Playground のコントラスト
- `npm run check:oracle`、`npm run check:oracle-random`：Ajisai との比較。Arare の値が、検査器の推論した誤差予算の中に収まるか（コーパスと、生成した 1 万本のプログラム）
- `npm run requirements:generate`：`spec/requirements.json` を変えたら、`spec/REQUIREMENTS.md` と追跡表の生成部分を作り直す
- `npm run check`、`npm run lint`、`npm test`：TypeScript
- 性能（Phase 4 から）：AR-PERF の要件を割っていないか、ベンチマークで確かめる

## 置き場所

- 計画書と標準：`docs/quality/`（英語）。開発メモ：`docs/dev/`（日本語）
- Phase 0 で意図的に残した課題：`docs/quality/PROBLEM_REPORTS.md`
- Ajisai のオラクル：`src/wasm/ajisai-oracle/`（Playground）、`tools/mcp-server/`（エージェント用）。固定したコミットは `UPSTREAM.md`
