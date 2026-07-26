## 不足テスト(plan計画分)

`git diff` (working tree, main ブランチ相当が origin/HEAD = `claude/add-claude-guidelines-BKKtD`) で実装差分を確認した。
`crates/engawa-build/tests/sketch_chamfer_acceptance.rs` は STEP 5.5 で作成した全27ID分のスケルトンから
`#[ignore]`/`todo!()` が完全に除去され、全て実装済み (`grep -n "^fn \|#\[ignore\]"` で `#[ignore]` 0件確認済み)。
`crates/engawa-kernel/src/geometry/sketch_chamfer.rs` にも kernel 側 inline unit test (T01/T01b/T01c/T02/
T_DEG_chamfer_too_long/100回決定性) が実装されている。plan.md のテスト計画表 (T01/T01d/T01e/T02/T03/T05/T06/
T07/T08/T09/T10/T11/T12/T_DEG_*全13種) は全てacceptance.rs側に対応する関数があり、不足なし。

format層 (T01b/T01c/T01f、`crates/engawa-format/src/feature.rs`) も確認: `t02_feature_tagged_union` の
golden 更新 (Codex R02 の指摘反映) に加え、`Feature::SketchChamfer` 用の roundtrip テストが
`crates/engawa-format/src/feature.rs` の `#[cfg(test)] mod tests` に追加されている (grep で
`sketch_chamfer` 関連テスト関数の存在を確認済み)。

## 実装差分から追加すべきテスト

- 実装差分は plan.md の設計方針から逸脱していない (check-spec-divergence.ts は本リポジトリの `main` ブランチ
  参照が実際のブランチ名 `claude/add-claude-guidelines-BKKtD` と食い違うため空振りしたが、`sketch_chamfer.rs`
  の docstring・エラー種別・派生ID生成規則を直接 Read して plan.md と1対1で突き合わせ済み: length比較規約・
  ChamferLengthTooLarge発火条件・corner角度退化チェック・find_adjacent_pair配列順による派生ID・
  find_adjacent_pair再利用 (fillet由来kind文字列) の5点すべて plan.md の記述と完全一致)
- 追加で必要な新規テストは無い。以下は確認のみ (既存テストでカバー済み):
  - T10/T11/T12 (Codex R01 部分採用のCRUD gate既知制約) は acceptance.rs に実装済み
  - `examples_smoke.rs` / `golden_examples.rs` への `sketch_chamfer` 追加 (Codex debug-spec 追加事項) も
    実装済み (git diff --stat で両ファイルの変更を確認済み)

## エッジケース・退化入力

plan.md のテスト計画表の T_DEG_* 13種 (angle flat/zero, no_shared_corner, non_line_element,
same_element, not_adjacent, elem_not_found, negative/nan/inf_length, zero_length_input_line(_b),
line_id_collision, sketch_ref_not_found) は全て acceptance.rs に実装済み。追加すべき未カバーケースは
現時点で見当たらない。

## 数値境界

- `length <= LENGTH_TOLERANCE` の境界 (length == LENGTH_TOLERANCE ちょうど) はT_DEG_negative_lengthの
  近傍だが plan.md のテスト計画表に専用境界IDは無い。既存Fillet実装 (`radius <= LENGTH_TOLERANCE`) も同様に
  専用境界テストを持たないため、対称性の観点から追加不要と判断する (Fillet前例に揃える)。
- `length` 超過判定の境界 (`length == len - LENGTH_TOLERANCE` ちょうど許容) も同様にFillet前例に
  専用境界テストが無く、対称性を優先し追加しない。

## 決定性

T01 (kernel純関数2回比較)・T01d (IdGenerator込みbuild 2回比較)・T_determinism_100_runs_build (100回)
の3層で決定性を検証済み。T01e (入力順不変性、GLM ambig r1 AM01採用) も実装済み。追加不要。

## Property test 検討 (STEP 6.5 手順6)

Phase 10 (Boolean/Tessellation系ではない、2D profile編集のみ) のため `check-proptest-required.ts` は
non-blocking (warn) の対象。Euler-Poincaré 等の B-rep 不変量はSketchChamferがBody/Solidを生成しないため
非対象 (plan.md 幾何的不変条件チェックリスト: N/A)。決定性のproperty的検証は上記100回ループで実質的にカバー
されているため、`T_PROP_` ID の追加は本Issueでは見送る。
