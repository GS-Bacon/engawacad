# Cleanup Task: Debug Stub Deletion

## 目的
STEP 8 前処理として、使い捨てデバッグスタブ12本を削除する。

## 削除対象ファイル
以下の12ファイルはすべて空のスタブ（コメントのみ）でコミット不要・削除対象。
mycad-kernel/tests/ 配下:
- debug_trim.rs, debug_trim2.rs, debug_trim3.rs, debug_trim4.rs, debug_trim5.rs
- diag_box_cut.rs, diag_box_cut2.rs, diag_box_cut3.rs, diag_box_cut4.rs, diag_box_cut5.rs
- diag_intersect.rs, diagnose_trim.rs

## 手順
1. `rm` で上記12ファイルを削除する
2. `cargo build --workspace` を実行してエラーがないことを確認する

## 期待結果
- 12ファイルが存在しない
- `cargo build --workspace` が成功する
- CI: `cargo xtask ci` がグリーンである
