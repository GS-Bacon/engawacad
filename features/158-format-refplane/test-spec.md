# Test Spec for Issue #158 — STEP 6.6 GLM テスト実装指示

## 不足テスト (plan 計画分)

plan.md のテスト計画 ID 表 T01〜T14 すべて、STEP 5.5 で配置したスケルトンが `#[ignore]` + `todo!()` のまま。STEP 6.6 GLM が以下 14 件を実装し、`#[ignore]` を解除する。

### format 層: `crates/engawa-format/tests/refplane_acceptance.rs` (8 件)

実装すべきテスト関数とその assertion 要件:

| ID | 関数名 | 実装すべき内容 |
|----|--------|---------------|
| T02 | `t02_document_new_seeds_three_refplanes` | `Document::new("x")` 直後、`doc.root_component.ref_planes` が `vec![RefPlane{id:"Front",plane:Xy,offset:0.0}, RefPlane{id:"Top",plane:Xz,offset:0.0}, RefPlane{id:"Right",plane:Yz,offset:0.0}]` と一致 (id・plane・offset すべて `assert_eq!`) |
| T03 | `t03_from_yaml_seeds_three_refplanes` | `Document::from_yaml("schema_version: 1\nversion: '0.1.0'\nroot_component:\n  name: x\n").unwrap()` 後、`doc.root_component.ref_planes.len() == 3` かつ id が `["Front","Top","Right"]` の順で並ぶ |
| T04 | `t04_legacy_examples_roundtrip` | `examples/extruded_rect.engawa` と `examples/two_bodies.engawa` を `std::fs::read_to_string` で読み、`Document::from_yaml(&s).unwrap().to_yaml().unwrap()` 後、元文字列と byte-identical で一致 (`assert_eq!(original, reserialized)`) |
| T08 | `t08_yaml_golden_new_format` | `examples/sketch_via_refplane.engawa` を parse → serialize → 既存 `golden_examples.rs` の golden 文字列 (`golden_sketch_via_refplane` 関数で使われているのと同じもの) と一致する。golden 文字列はこのテスト内に同等の concat! 形式で再掲してよい |
| T09 | `t09_yaml_golden_document_with_explicit_refplanes` | `Document::new("x")` をベースに、`root_component.ref_planes` の Front の offset を 5.0 に書き換えた `Document` を作る。`to_yaml().unwrap()` 出力に `ref_planes:` と `offset: 5` 行が含まれることを `assert!(yaml.contains("ref_planes:"))` `assert!(yaml.contains("offset: 5"))` で確認 |
| T12 | `t12_degen_duplicate_refplane_id` | id 重複の YAML (例: `ref_planes:\n- id: Front\n  plane: xy\n- id: Front\n  plane: xz\n`) を含む `.engawa` 文字列を `Document::from_yaml(&s)` で parse → `Err(FormatError::DuplicateRefPlaneId { id })` が返ることを `assert!(matches!(err, FormatError::DuplicateRefPlaneId { .. }))` |
| T13 | `t13_default_three_skips_serialize` | `Document::new("x").to_yaml().unwrap()` の出力に `ref_planes:` 文字列が含まれない (`assert!(!yaml.contains("ref_planes"))`) |
| T14 | `t14_explicit_three_with_custom_offset_serializes` | `Document::new("x")` の root_component.ref_planes の Front の offset を 5.0 に書き換えて serialize → 出力に `ref_planes:` と `offset: 5` が含まれる |

### build 層: `crates/engawa-build/tests/refplane_acceptance.rs` (6 件)

| ID | 関数名 | 実装すべき内容 |
|----|--------|---------------|
| T01 | `t01_determinism` | `examples/sketch_via_refplane.engawa` を `Document::from_yaml` で 2 回 parse → 各 Document を `build_bodies_from_features(&features, &ref_planes, &mut gen)` で 2 回 build → 得られた Solid の face_count / edge_count / vertex_count + 各 Face の EntityId 列が完全一致。**GLM IN02 採用**: mesh struct `assert_eq!` だけでなく Face/Edge/Vertex EntityId 列の `assert_eq!` も含める |
| T05 | `t05_new_format_e2e` | `examples/sketch_via_refplane.engawa` (plane_ref:Front, plane:xy 併記) を parse→build した出力 Solid と、`examples/extruded_rect.engawa` (plane:xy 単独) を parse→build した Solid が同じ face/edge/vertex 数を持ち、bounding box が一致 (`approx::abs_diff_eq!` 等で許容差ゼロ確認) |
| T06 | `t06_plane_ref_priority_over_plane` | `plane_ref: Some("Front")`, `plane: SketchPlane::Yz`, `offset: 100.0` を持つ `Feature::CreateSketch` を含む features を build → 出力 Solid が **xy 平面で extrude された結果と一致** (= plane_ref が plane/offset を上書きしている)。具体的には bounding box の z 軸が `[0, depth]` に入る (yz だと z 軸ではなく x 軸 + 100 オフセットになるはず) |
| T07 | `t07_legacy_plane_offset_still_works` | `plane_ref: None`, `plane: SketchPlane::Xy`, `offset: 5.0` の Feature::CreateSketch を build → 出力 Solid の z 軸 bounding box が `[5.0, 5.0 + depth]` 範囲に入る (従来の plane+offset 経路が機能) |
| T10 | `t10_degen_unknown_plane_ref` | `plane_ref: Some("Nonexistent")` の Feature::CreateSketch を空の `ref_planes: &Vec::new()` で build → `Err(BuildError::UnknownRefPlane { id })` が返ることを `assert!(matches!(err, BuildError::UnknownRefPlane { ref id }) if id == "Nonexistent")` |
| T11 | `t11_boundary_empty_plane_ref_string` | `plane_ref: Some("".to_string())` の Feature::CreateSketch を build → `Err(BuildError::UnknownRefPlane { id })` で空文字列が返る (空文字列は通常 id にマッチしない) |

## 実装差分から追加すべきテスト

特になし。STEP 6 GLM core 実装は plan 通り完了している。

## エッジケース・退化入力

T10 (不明 plane_ref) / T11 (空文字列 plane_ref) / T12 (id 重複) で網羅。これら 3 件は本 Issue 固有の退化条件であり、それぞれエラー型を分岐検証する。

## 数値境界

本 Issue は形式変更のみで数値判断 (ε, tolerance) を持たない。N/A。

## 決定性

T01 で `examples/sketch_via_refplane.engawa` の決定性を 2 回 build 実行 + EntityId 列比較で検証。

## 実装注意点 (GLM-test-implementer 向け)

- 既存 `#[ignore = "STEP 6 で実装後に解除"]` 属性は **削除** する (各関数のテスト本体実装後に `#[ignore]` 行を取り除く)
- `crates/engawa-format/tests/refplane_acceptance.rs` と `crates/engawa-build/tests/refplane_acceptance.rs` の **2 ファイル両方** を実装すること
- 既存 `Component`/`Document`/`Feature`/`BuildError`/`FormatError`/`SketchPlane`/`SketchSegment` の import は適宜追加 (例: `use engawa_format::{Document, Component, RefPlane, FormatError, Feature, SketchPlane, SketchSegment};`)
- T01/T05/T06/T07 の build テストでは `IdGenerator::new(0)` で初期化、`build_bodies_from_features(&features, &ref_planes, &mut gen)` を呼ぶ
- T06/T07/T10/T11 は Feature::CreateSketch を **直接 vec! で構築** する必要がある (`plane_ref: Some/None` を明示)
- T04/T08 の golden は **byte-identical 一致**が要件。文字列の最後の改行や YAML キー順は parse → serialize の挙動に依存するので、まず実装してテスト失敗時に actual を見て golden 文字列を調整する流れで OK
- T01/T05/T06/T07 で extrude depth は `8.0` 等の固定値を使ってよい (plan で特定値の指定なし)
- すべてのテストで Issue #158 のスコープ (RefPlane + plane_ref) のみを扱う。kernel 不変条件等の追加テストは不要

## 完了条件

- 14 件すべての `#[ignore]` 解除 + 実装完了
- `cargo test -p engawa-format --test refplane_acceptance` で全件 pass
- `cargo test -p engawa-build --test refplane_acceptance` で全件 pass
- `cargo xtask ci` が green
