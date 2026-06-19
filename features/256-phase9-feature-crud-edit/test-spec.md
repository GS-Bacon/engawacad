# Test Spec — #256 phase9-feature-crud-edit (STEP 6.5)

## 不足テスト (plan 計画分)

該当なし。plan.md のテスト計画 ID (T01/T02/T03/T_DEG_unknown_id/T_DEG_invalid_spec) はすべて GLM が STEP 6 で実装済み。

| ID | 配置 | 状態 |
|----|------|------|
| T01 | `crates/engawa-build/tests/256_phase9_feature_crud_edit_acceptance.rs::t01_edit_determinism` | ✅ 実装済み |
| T02 | 同上 `::t02_edit_creates_box_params_updated` | ✅ |
| T03 (cli) | `crates/engawa-cli/tests/256_phase9_entry_edit_cli.rs::t03_cli_edit_updates_feature` + 派生 `t03_cli_edit_dry_run` | ✅ |
| T_DEG_unknown_id | acceptance.rs `::t_deg_unknown_id` | ✅ (※命名は要リネーム、下記参照) |
| T_DEG_invalid_spec | acceptance.rs `::t_deg_invalid_spec_id_mismatch` | ✅ (※命名は要リネーム、下記参照) |

## 実装差分から追加すべきテスト

GLM 実装 (`FeatureCrud::edit`) を確認すると、plan では明示していなかったが実装で守られている挙動が 2 件あり、追加テストでカバーすべき:

### t04_edit_preserves_idx_in_multi_feature_doc (追加)

- **追加理由**: GLM 実装は `features[idx] = new_feature` で in-place 置換 → idx 保持と他 feature 順序非変更 を実装しているが、現状テストは 1 feature doc (`features.len() == 1`) のみカバーしている。multi-feature doc での idx 不変・他 feature 順序保存の明示テストがない。
- **テスト内容**:
  - 3 feature doc `[CreateBox{box_1}, CreateSphere{sphere_1}, CreateCylinder{cyl_1}]` を作成
  - `FeatureCrud::edit(&doc, "sphere_1", new_sphere)` を実行
  - 結果: `features.len() == 3` / `features[0].id() == "box_1"` / `features[1] == new_sphere` (id=sphere_1 + new params) / `features[2].id() == "cyl_1"` を assert
- **配置**: `crates/engawa-build/tests/256_phase9_feature_crud_edit_acceptance.rs` に追加

### t05_edit_propagates_validation_error (追加)

- **追加理由**: GLM 実装の `let mut next = doc.clone(); next.root_component.features[idx] = new_feature; next.validate()?;` で `Document::validate()` のエラーが `FeatureCrudError::Validation` に `#[from]` 経由で伝播するが、その path のテストがない。empty `id` のような schema violation を起こす new_feature を渡した場合に Validation error が返ることを assert すべき。
- **テスト内容**:
  - 1 feature doc を作成
  - `new_feature = Feature::CreateBox { id: String::new(), width: 10.0, height: 20.0, depth: 30.0 }` (= empty id だが本 Issue 制約 `new_feature.id() == feature_id` を満たすため `feature_id = ""` を渡す)
  - 但し空文字列の feature_id を持つ既存 feature を doc に置く必要がある → ここは難しい (Issue の本来意図と外れる)。
  - **簡略アプローチ**: validate() 失敗の代表として、existing が valid な doc + edit で `new_feature.id()` を正しく合わせつつ Variable schema violation を起こすパスを試みる。これが難しければ「Validation エラー伝播は型レベル `?` 演算子で保証されている (`#[from] source: FormatError`)」として **本テストは省略可** とする。
- **配置**: 同上、ただし schema violation を起こせない場合は省略 (= medium 優先度)。GLM 判断で実施可能なら追加、困難なら skip。

## エッジケース・退化入力

- GLM 実装の退化テスト `t_deg_unknown_id` / `t_deg_invalid_spec_id_mismatch` の **fn 命名問題** (#269 cycle 45 で観測):
  - `t_deg_unknown_id` の `t_` は extractor regex `(test_|t\d+_)` (= `t` + 1 文字以上の数字 + `_`) に match しない → test-summary coverage_hints の `degenerate` kind カウントから漏れる
  - **リネーム推奨**: `t_deg_unknown_id` → `t04_deg_unknown_id` (もしくは `test_t_deg_unknown_id`)。`t_deg_invalid_spec_id_mismatch` → `t05_deg_invalid_spec_id_mismatch` (上記 t04 追加分と通し番号衝突する場合は適宜調整)
  - GLM 判断で適切な番号付け (例: 既存 t01/t02 の続きで t03/t04 の prefix 化) を行うこと

## 数値境界

N/A (本 Issue は履歴 Document 純関数変換のみで数値判断を伴わない)

## 決定性

T01 でカバー済み (同一入力 2 回 edit → `to_yaml()` byte-equal)。追加不要。

## 類似ケース (バグ修正ではないため不要)

本 Issue は新規 feature 実装 (feature label) のためバグ修正類似ケース調査は対象外。
