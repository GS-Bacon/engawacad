# #216 test-spec.md

## 不足テスト (plan 計画分)

すべての plan T01〜T05_degen が STEP 6 で実装済み (`crates/engawa-build/tests/face_sketch_placement_acceptance.rs`)。残不足なし。

## 実装差分から追加すべきテスト

GLM 実装差分は acceptance file (5 テスト関数のみ) + examples_smoke.rs にエントリ 1 件 + 新規 example 1 ファイル。crates/ src/ 変更ゼロ。

差分から追加すべきテストは以下:

| ID | 内容 | 期待結果 |
|----|------|---------|
| T06_u_v_axis_orientation | T03 で確認した plane.normal に加え、cuboid 上面の `plane.u_axis` / `plane.v_axis` が `cuboid.rs:135-136` 定義通り `(-1,0,0)` / `(0,1,0)` であることを assert (Plane の **回転方向**まで含めた完全な座標系の固定)。plan が「回転・原点が期待通り」と言及していた "回転" 部分の deeper assertion。 | `assert!((plane.u_axis - Vec3::new(-1.0,0.0,0.0)).norm() < 1e-9); assert!((plane.v_axis - Vec3::new(0.0,1.0,0.0)).norm() < 1e-9);` |
| T07_other_role_returns_different_face | 同 cuboid の `f_z_neg` を引いて別 face が返り (face_idx が異なる)、その plane.normal が `(0,0,-1)` であることを assert。`find_face_by_entity_ref` が role を正しく区別している保証。 | T03 と同じ cuboid から、`f_z_neg` 引いた `face_idx != face_idx_pos` かつ `plane.normal ≈ (0,0,-1)` |

## エッジケース・退化入力

| ID | 内容 | 期待結果 |
|----|------|---------|
| T08_unknown_role | 存在しない role (`f_xxx`) を `find_face_by_entity_ref` に渡し、`None` が返ることを assert。 | `assert!(solid.find_face_by_entity_ref(&EntityRef::Named { feature_id:"box_1", kind:Face, role:"f_xxx".to_string() }).is_none());` |
| T09_unknown_feature_id | 存在しない feature_id (`box_999`) を渡して `None` が返ることを assert。 | 同上 with `feature_id:"box_999"` |

## 数値境界

| ID | 内容 | 期待結果 |
|----|------|---------|
| T10_negative_radius | 半径 -1 の 8 角形 → 各点座標は有限で profile_closed → ただし反時計回り/時計回り反転で extrude 方向が想定外。実装の挙動を実測し pin 留めする (extrude が成功して負 volume、または validate_profile_closed reject、または build error)。本 Issue では「panic でない」のみ assert (実装挙動を test 内コメントで documents)。 | `let result = build_assembly(...); assert!(true);` (= panic でないこと自体を test 通過で示す) — このテストは plan T05_degen と同じ "graceful" 基準で記述する |

## 決定性

| ID | 内容 | 期待結果 |
|----|------|---------|
| T11_t03_repeated_determinism | T03 を 3 回繰り返し、毎回同じ `face_idx` が返ることを assert (`find_face_by_entity_ref` の決定性の十分性検証)。STL byte 列 T01 と相補的。 | `let idx1 = ...; let idx2 = ...; let idx3 = ...; assert_eq!(idx1, idx2); assert_eq!(idx2, idx3);` |

## 類似ケース (Bug 修正 Issue のみ — 本 Issue は feature のため N/A)

N/A — 本 Issue は feature 追加であり、修正バグなし。

## 期待値乖離

`check-spec-divergence.ts` の結果: 「git diff main..HEAD で変更された .rs ファイルなし」(branch は cad/216-phase8-modelface-e2e-a で main 比較が空)。手動 diff (`git diff HEAD --stat`) では `examples_smoke.rs` 修正 + 新規 `face_sketch_placement_acceptance.rs` + 新規 `examples/sketch_circle_on_face.engawa` のみが見える。

実装の assertion 値と plan の期待値を突合:
- T01: 3-run STL byte eq → 実装 `assert_eq!(a,b); assert_eq!(b,c);` ✓ 一致
- T02: build success + `bodies.len() == 1` → 実装は `bodies.len() == 2` (create_box の box_1 と extrude_1 の両方が live)。**乖離**だが実装側の値が現実 (extrude は box_1 を fuse_target で消費しないため両 body が live)。plan の "bodies.len() == 1" は推測 — 実装の `== 2` を採用し plan に追記する。
- T03: Plane.origin.z = 2.5 + normal = (0,0,1) → 実装一致 ✓
- T04: build success → 実装 `assert!(!bodies.is_empty())` ✓ 一致
- T05_degen: panic でない + KernelError variant 内 → 実装 `format!("{:?}", e).contains(...)` で 5 variant 候補マッチ ✓ 一致

**乖離: T02 のみ** — plan を「bodies.len() == 2 (box_1 + extrude_1 が live)」に修正し、本 Issue 内で吸収する。
