# Test Spec — Issue #38 (コア実装後の不足テスト)

## 不足テスト（plan 計画分 — 未実装）

plan T09/T10 相当は `test_sphere_entity_names` / `test_sphere_name_determinism` としてコア実装済み。
plan T14b 相当は `t14b_sphere_fragment_interior_point` としてコア実装済み (debug-spec 由来)。

**コア実装フェーズで全 plan テスト (T07/T08/T09相当/T10相当/T13/T14/T14b/T15/T16a-d/A3/T22(A3)/T32) が実装・通過済み。
STEP 6.6 で追加すべき未実装テストはほぼ無いが、以下の補強を実施すること。**

## 実装差分から追加すべきテスト

### TX1 — `validate_boolean_input` が sphere + cuboid の混在 solid を受け入れる (統合確認)
```
test対象: booleans/mod.rs::validate_boolean_input
ケース: sphere 1面 + cuboid 6面 を持つ「手作り混在 Solid」を渡す
期待: Ok(()) (面ごとに Plane/Sphere をミックスしても全体を reject しない)
なぜ: validate_boolean_input はは面ごとに kind を見ており、Sphere 面の convex
      チェックをスキップする経路が正しく動くことを直接確認していない
配置: crates/mycad-kernel/src/booleans/mod.rs #[cfg(test)]
```

### TX2 — `partition_faces` で box (planar) + sphere の face pair が intersection 呼び出しを行う
```
test対象: booleans/partition.rs::partition_faces
ケース: box+sphere の partition (A3 の低レベル版) で box face と sphere face の pair
        が intersect_surfaces を呼び、その結果が空ループ (no intersection) であることを確認
期待: target_fragments.len() == 6 (box faces)、tool_fragments.len() == 1 (sphere face)
なぜ: A3 統合テストは通るが、partition の内部でどのフラグメントが生成されるかを
      単体レベルで assert していない。regression 追加で価値あり
配置: crates/mycad-kernel/src/booleans/partition.rs #[cfg(test)]
```

### TX3 — sphere 面のみの FaceFragment が `classify_fragment_against_solid` を通る
```
test対象: booleans/classify.rs::classify_fragment_against_solid
ケース: polygon_3d が 2 点の sphere FaceFragment を作り、box 相当の other solid を渡す
        → InsideOther が返ることを確認
期待: Ok(FragmentLabel::InsideOther)
なぜ: t14b は interior_point が求まることを確認したが、point_in_polyhedron
      の結果まで検証していない。追加することで classify の sphere 経路を end-to-end カバー
配置: crates/mycad-kernel/src/booleans/classify.rs #[cfg(test)]
注意: classify_fragment_against_solid の可視性が pub(crate) であれば直接呼べる
```

## エッジケース・退化入力

### TX4 — CW 向き polygon (負面積) は `validate_planar_face_outer_loop_basic` を通過する
```
ケース: 時計回り (CW) の 3 頂点 triangle solid を渡す → Ok(()) を期待
なぜ: 実装は area.abs() <= area_eps で判定するため CW は reject しないはずだが、
      符号の扱いを明示的に確認する
配置: crates/mycad-kernel/src/booleans/mod.rs #[cfg(test)]
```

### TX5 — sphere 中心が原点以外でも interior_point が sphere surface 上にある
```
ケース: center=(3.0, 4.0, 5.0), radius=2.0 の sphere FaceFragment の interior_point を取得
期待: 点 (3.0, 4.0 + 2.0, 5.0) = (3.0, 6.0, 5.0) が返る
なぜ: t14b は center=origin のみ検証。center が非 origin のケースを確認
配置: crates/mycad-kernel/src/booleans/classify.rs #[cfg(test)]
```

## 数値境界

### TX6 — 半径 LENGTH_TOLERANCE ちょうどの analytic Circle は reject される
```
ケース: radius = LENGTH_TOLERANCE (= 1e-9) の Curve::Circle outer_loop planar face
        → Err(UnsupportedBooleanInput) を期待
なぜ: T16d は LENGTH_TOLERANCE/2 で確認。ちょうど equal のケースの境界動作を確認
配置: crates/mycad-kernel/src/booleans/mod.rs #[cfg(test)]
```

## 決定性

### TX7 — A3 の sphere 面 entity name (feature_id / role) が不変
```
ケース: A3 の Cut 結果の inner shell sphere face の `parent_name` が
        EntityRef::Named { feature_id: "sphere", kind: Face, role: "surface" } と一致
期待: 決定的 naming チェーン (T22(A3) が全エンティティ一致を確認済みだが、
      sphere 面の name を直接 assert するテストは無い)
配置: crates/mycad-build/tests/feature_dispatcher.rs
注意: A3 テストに 1 assert を追加する形でも可
```

---

> **実装メモ**: コア実装フェーズで全 plan テストが実装済みのため、STEP 6.6 では
> TX1–TX7 の追加テストのみを実装すること。既存テストの修正は不要。
