# Test Spec — #290 Phase 10 example golden round-trip

## 不足テスト (plan 計画分)
plan.md のテスト計画 T01〜T05 はすべて `golden_examples.rs` の既存テストと新規 2 件で充足済。新規追加するテストはない。
- T01_determinism: `cargo test --test golden_examples` を 2 回連続実行で確認 (Rust テストランナー側で自動)
- T02_circle_arc: `golden_circle_arc` で実装済
- T03_ellipse_conic: `golden_ellipse_conic` で実装済
- T04_boundary_schema_migration: `golden_circle_arc` の golden 文字列が `schema_version: 2\n` で始まる (`schema_version: 1` の原ファイルから migration が走った証拠) — assertion で同時に確認される
- T05_boundary_existing_unchanged: `cargo test -p engawa-format --test golden_examples` で 17 件 (既存) + 2 件 (新規) = 19 件すべて通過することで確認

## 実装差分から追加すべきテスト
**なし**。本 Issue の実装差分は `crates/engawa-format/tests/golden_examples.rs` への新規 #[test] 関数 2 件追加のみで、新しい本番コードパスは存在しない。

## エッジケース・退化入力
**なし**。本 Issue は wire-format pin のみで、退化入力 (空 profile / 不正 schema_version 等) はそれぞれ別 Issue (Phase 10 sketch curves の本体実装側 #273 / #274 / format バリデータ側) が担当する。

## 数値境界
**なし**。本 Issue は format 層のテキスト golden 検証で、数値演算を一切伴わない (浮動小数の比較は `assert_eq!` の文字列比較で代替されている)。

## 決定性
- `Document::to_yaml()` は `golden_examples.rs` 全体で前提化されている (`golden_extruded_rect` 以下、既存 17 件で実証)。本 Issue は同前提に乗るのみ
- `cargo test --test golden_examples` を 2 回連続で実行して両方 0 failures になることで再確認できる
- 追加の決定性テストは plan.md T01 に書いた通り、テストランナーが既に保証している (`assert_eq!` の文字列比較は決定的)

## 結論
本 Issue は test-only foundation で、core 実装と test 実装が同一 (= golden_examples.rs への 2 件追加) のため、STEP 6.6 で GLM が追加実装する内容は存在しない。STEP 6.6 は no-op として通過する。
