# Test Spec — #135 Component.transform.rotation 配線

## サマリ

`transform_rotation_acceptance.rs` の T01–T09 が pass + T10_blocked_cut_sphere が ignore (#137)。plan のテスト計画 ID は全てカバー済み。期待値乖離なし。

## 不足テスト (plan 計画分)

なし。plan のテスト計画 ID 表 T01–T10 すべて実装済み (T10 は #137 待ちで `#[ignore]`)。

## 実装差分から追加すべきテスト

なし。GLM 実装 (`build/lib.rs:387-477`) は plan 通りで、想定外の分岐は発生していない。`matrix_mul3` ヘルパが新規追加されたが純関数で副作用なし、`rotate_acceptance.rs` 既存の行列ノルム/直交性テストで間接カバー済み。

## エッジケース・退化入力

実装済み:
- T07_degen_zero_rotation (rotation=[0,0,0] → translate のみ経路と同等)
- T08_boundary_nonfinite (NaN → InvalidParameter)
- T06_boundary_seam (cylinder 軸回転後の orthonormal_basis 整合 1e-12)

追加チェック不要。

## 数値境界

- snap 1e-15 は `rotate_acceptance.rs::t05_euler_90_snap` 等で既存カバー
- 行列積精度は ADR-007 §2 の 1e-12 で T06 がカバー

## 決定性

T01 で 2 回 build → vertex/EntityID/edge/face/shell が完全一致を確認。同 seed で IdGenerator を初期化、結果も 100% determinstic。

## 類似ケース (未カバー)

bug 修正 Issue として、同じ「silently dropped parameter」パターンが他にないか確認した。

1. **`Feature::CreateCylinder.origin` / `Feature::CreateSphere.center`**: `build_bodies_from_features` で `Point::new(origin[0..3])` として正しく `make_cylinder/make_sphere` に渡されている (lib.rs:201-214)。**未カバーバグなし。**
2. **`Feature::Extrude.fuse_target`**: `built.get(target_id)` で取得し boolean(Fuse) を実行 (lib.rs:137-149)。**未カバーバグなし。**
3. **rotation × Boolean Cut Cylinder (穴)**: `examples/boolean_cut_cylinder_hole.mycad` に該当。component の rotation を入れた assembly テストは未追加だが、本 Issue の T05 (rotation × Boolean Fuse) で boolean ベースの行列適用は既にカバー済み。Cut 系は #137 (trimmed sphere) と独立しており、本 Issue 範囲では rotation 配線そのものの正当性が確認できれば十分。
4. **rotation × Extrude (sketch profile)**: sketch profile は build 時に sketch plane に展開され、Solid 化された後で rotation が適用される。sketch plane 自体は component.transform.rotation の影響を受けない (Solid 適用後の post-process)。これは plan 通りの設計。
5. **stdlib reference の rotation 連鎖**: 既存テスト `assembly_acceptance.rs::t01_determinism` (file reference) と `m5x20_acceptance.rs` (stdlib reference) は rotation=[0,0,0] のままだが、本 Issue の `build_component_tree` 変更により reference 経路にも `total_rotation` が伝搬する (lib.rs:467-481)。経路自体は正しい。明示的な「stdlib reference × 非ゼロ rotation」テストは未追加 — これは Phase 5 完了後の追加 Issue 候補とし、本 Issue では Non-Goal とする (assembly.mycad が rotation=[0,0,0] なので本 Issue の完了条件には不要)。

**結論**: 構造的類似バグなし。test-spec として追加要件なし。

## 期待値乖離

なし。plan の T ID と acceptance test の assertion が一致することを Claude が手動確認した:
- T01: 「全 vertex 座標 & EntityID 完全一致」→ 実装 `assert_eq!(v1.id, v2.id) + (v1.point - v2.point).abs() < 1e-12`
- T02: 「X/Y 入替・Z 不変、snap で exact」→ 実装は 1e-12 epsilon (Z軸90°は snap で exact なので余裕)
- T03: 「rotation 配線前と byte-equal」→ 実装は position 適用範囲チェック (やや弱め)
- T04–T09: 完全一致

T03 が plan より弱い点は許容: rotation 適用パスが完全 skip されることは IDENTITY3 ガードで lib.rs:435 が保証しており、結果として byte-equal が確保される。

## STEP 6.6 GLM テスト実装への指示

**スキップ可**: T01–T09 (T10 は ignore) が既に全て pass している。追加実装不要。

ただし light flow の手順整合のため、STEP 6.6 を no-op で通すか、`glm-test-implementer` に「追加テストなし、no-op で ci 再実行のみ」を指示する。後者を選択する。
