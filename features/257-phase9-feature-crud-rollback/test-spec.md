# Test Spec — #257 phase9-feature-crud-rollback (STEP 6.5)

## 不足テスト (plan 計画分)

該当なし。plan.md の T01/T02/T03/T_DEG_unknown_id/T_BOUNDARY_first_feature はすべて GLM が STEP 6 で実装済み。

| ID | 配置 | 状態 |
|----|------|------|
| T01 | `crates/engawa-build/tests/257_phase9_feature_crud_rollback_acceptance.rs::t01_determinism` | ✅ |
| T02 | 同上 `::t02_normal_rollback_middle` | ✅ |
| T03 (cli) | `crates/engawa-cli/tests/257_phase9_entry_rollback_cli.rs::t03_*` | ✅ |
| T_DEG_unknown_id | acceptance.rs `::t_deg_unknown_id` | ✅ (※命名要リネーム、下記参照) |
| T_BOUNDARY_first_feature | acceptance.rs `::t_boundary_first_feature` | ✅ (※命名要リネーム、下記参照) |

## 実装差分から追加すべきテスト

### t05_rollback_tail (追加)

- **追加理由**: T02 は middle feature (sphere_1) を rollback するケース。**末尾 feature の rollback** (= 何も削除されない、つまり末尾の feature_id を渡して truncate(idx) で features[..idx] = 末尾 1 個削除) のケースをカバーしていない。実装の `truncate(idx)` semantic を完全に守るためには末尾ケースも明示テストすべき。
- **テスト内容**:
  - 3 feature doc `[box_1, sphere_1, cyl_1]` を作成
  - `rollback("cyl_1")` を実行 (末尾 feature)
  - 結果: `features == [box_1, sphere_1]` (cyl_1 のみ削除)
- **配置**: `crates/engawa-build/tests/257_phase9_feature_crud_rollback_acceptance.rs` に追加

### t06_rollback_validation_propagation (省略可)

- **追加理由**: GLM 実装は `next.validate()?` で Document::validate() のエラーを伝播するが、その path がテストされていない。
- **省略判断**: rollback は features を削除する方向 (新規追加なし) なので、validate 失敗パスは通常発生しない (元 doc が valid なら truncated doc も valid)。意図的に invalid な状態を作るのが困難。**本テストは省略**。

## エッジケース・退化入力

- **fn 命名リネーム** (#269 cycle 45 で観測した extractor regex 問題):
  - `t_deg_unknown_id` の `t_` は extractor regex `(test_|t\d+_)` に match しない → coverage_hints の degenerate カウントから漏れる
  - `t_boundary_first_feature` も同様 (`t_` で match しない)
  - **リネーム推奨**:
    - `t_deg_unknown_id` → `t03_deg_unknown_id` (T03 cli と数字衝突する場合は `t04_deg_unknown_id`)
    - `t_boundary_first_feature` → `t04_boundary_first_feature` (上記と通し番号調整)
    - 追加で書く `t05_rollback_tail` と通し番号合わせる
  - GLM 判断で適切な番号付け (既存 t01/t02 の続き) を行うこと

## 数値境界

N/A (本 Issue は履歴 Document 純関数変換のみで数値判断を伴わない)

## 決定性

T01 でカバー済み。追加不要。

## 類似ケース (バグ修正ではないため不要)

本 Issue は新規 feature 実装 (feature label) のためバグ修正類似ケース調査は対象外。
