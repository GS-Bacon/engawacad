# Test Spec for #264

GLM core dispatch が plan.md の T01〜T_boundary_* 10 件を全て実装済み (`crates/engawa-build/tests/feature_crud_plane_ref_acceptance.rs`)。`cargo test -p engawa-build --test feature_crud_plane_ref_acceptance` で 10/10 passed を確認 (2026-06-19)。

以下は **追加すべきテスト** の指示書 (STEP 6.6 で GLM test-implementer に任せる差分のみ列挙)。

## 不足テスト（plan 計画分）

なし — plan T01〜T07 + T_degen_no_plane_ref / T_degen_legacy_plane_string / T_boundary_no_downstream_sketch をすべて実装済み。

## 実装差分から追加すべきテスト

GLM 実装の `check_no_downstream_break` 拡張 (line 383-388) で、direct ref と implicit ref を両方持つ consumer の OR 判定が入った。これにより:

- **DIFF01 (downstream Extrude.fuse_target の implicit ref 重複)**: 同一 body を direct (Extrude.fuse_target) と implicit (CreateSketch.plane_ref) の両方が指す downstream を持つ history で、その body を consume する Cut を insert → `InsertBeforeConsumer` が **どちらか早い方の consumer** で返ることを確認 (現実装は features.iter().skip(at) 順、つまり最初に match した consumer の id が `displaced_feature_id` に入る)。重複 match で同じ `re_registered` フラグが 2 回評価されても結果が一致することの回帰。
- **DIFF02 (Derived chain で複数 Named feature_id を抽出)**: `EntityRef::Derived { from: [Named(box_1), Named(box_2)] }` のように provenance に 2 つの Named を持つ sketch を insert したとき、両方 (box_1 と box_2) の lifetime が解決されることを確認。`collect_named_feature_ids` の DFS で両方 push される + どちらか欠けると BodyNotFound、両方 live なら Ok。

## エッジケース・退化入力

- **EDGE01 (Derived from が空でないが Named を含まない深い chain)**: `Derived { from: [Derived { from: [Named(box_1)] }] }` のような 2 段ネストで Named leaf 1 個を抽出。3 段ネストでも同じ。
- **EDGE02 (同じ feature_id を複数回参照する Derived)**: `Derived { from: [Named(box_1), Named(box_1)] }` で `feature_implicit_body_refs` が `[box_1, box_1]` を返す (dedup しない設計; check_refs_resolve_before は重複 entry で同じ判定を 2 回するだけなので意味論上問題なし、`Ok`/`Err` 同一)。dedup は今は不要だが回帰用に明示。

## 数値境界

該当なし (本 Issue は ref tracking。tolerance 値を扱わない)。

## 決定性

- T01 で同一入力 → 同一 YAML を assert 済み。
- 追加 (`T01b_determinism_with_derived_chain`): `EntityRef::Derived` を含む sketch でも 2 回 insert で同一 YAML / 同一エラー variant が返ることを assert。`collect_named_feature_ids` の DFS 順序が決定的であることの回帰。
