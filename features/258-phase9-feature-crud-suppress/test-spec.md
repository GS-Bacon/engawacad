# Test Spec — #258 phase9-feature-crud-suppress (STEP 6.5)

## 不足テスト (plan 計画分)

| ID | 配置 | 状態 |
|----|------|------|
| T01 | acceptance.rs::t01_suppress_deterministic | ✅ |
| T02 | acceptance.rs::t02_suppress_normal | ✅ |
| T03 | acceptance.rs::t03_restore_normal | ✅ |
| T04 (cli suppress) | `crates/engawa-cli/tests/258_phase9_entry_suppress_cli.rs::t04_*` (GLM 実装で配置確認要) | 要確認 |
| T05 (cli restore) | 同上 | 要確認 |
| T_DEG_referenced | acceptance.rs::t_deg_referenced | ✅ (※命名要リネーム、下記参照) |
| T_DEG_unknown_id | acceptance.rs::t_deg_unknown_id | ✅ (※命名要リネーム) |

## fn 命名リネーム (extractor regex 適合)

- `t_deg_referenced` / `t_deg_unknown_id` の `t_` は extractor regex `(test_|t\d+_)` に match しない
- リネーム推奨:
  - `t_deg_referenced` → `t04_deg_referenced` (cli テストと番号が衝突する場合は GLM 判断で調整)
  - `t_deg_unknown_id` → `t05_deg_unknown_id`
- cli テストの番号も合わせて整理

## 実装差分から追加すべきテスト

### t06_suppress_skipped_in_simulate (追加)

- **追加理由**: GLM は `simulate_history` で suppressed=true を inert 扱いするよう実装したはず。これを直接 assert するテストがない (T02 は suppressed=true flag 立ちだけ確認)。
- **テスト内容**:
  - 2 feature doc `[box_1 (suppressed=true), sphere_1]` を作成 (suppressed=true を直接 push or suppress 経由)
  - `simulate_history` (private なので直接呼べない場合は、build_assembly 経由で確認) で suppressed feature が executed_at に入らないこと
- **省略可**: simulate_history が private function なら直接テスト困難。代わりに `build_assembly` で 1 件削減されることを assert する形に書き換えても良い

### t07_cli_dry_run (追加: cli)

- **追加理由**: cli `--dry-run` 分岐の test がない可能性。
- **テスト内容**: `engawa entry suppress <input> box_1 --dry-run` で stdout に suppressed=true を含む YAML が出力され、input file は変更されない

## 数値境界 / 決定性

- 数値境界: N/A
- 決定性: T01 でカバー済み

## 類似ケース

本 Issue は新規 feature 実装のためバグ修正類似ケース調査は対象外。
