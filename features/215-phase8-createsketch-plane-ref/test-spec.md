# Test Spec for #215 — CreateSketch.plane_ref EntityRef 互換

## 不足テスト（plan 計画分）

### T02 (Legacy string PlaneRef) — 直接 unit test 未実装

plan.md T02 は `plane_ref: Some(PlaneRef::RefPlane("Front".into()))` で従来通り Extrude 成功する経路を要求している。現状 `examples/sketch_via_refplane.engawa` の YAML smoke は通っているが、Rust 構造体直接構築の単体テスト (PlaneRef::RefPlane variant 専用) が `face_entity_ref_planeref.rs` に存在しない。

追加要請: `face_entity_ref_planeref.rs` (または同じ tests/ ディレクトリの新ファイル) に次のテストを追加する:

```rust
#[test]
fn t02_legacy_string_planeref_backward_compat() {
    // ref_planes に "Front" を登録 → CreateSketch.plane_ref = PlaneRef::RefPlane("Front") → Extrude 成功
    let ref_planes = vec![RefPlane {
        id: "Front".into(),
        plane: SketchPlane::Xy,
        offset: 0.0,
    }];
    let features = vec![/* CreateSketch with plane_ref: Some(PlaneRef::RefPlane("Front".into())), then Extrude */];
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap();
    assert_eq!(built.live().count(), 1);
    let solid = &built.live().next().unwrap().solid;
    assert!(solid.faces.len() >= 6);
}
```

### T03 期待結果 (2) Plane 4 ベクトル一致 — 未実装

現状の `t03_plane_ref_entity_extrude` は (1) bodies.len() == 2 と (3) face 数 ≥ 6 を assert しているが、**plan.md T03 期待結果 (2)** 「`resolve_plane` が返す Plane の `origin/normal/u_axis/v_axis` 4 ベクトルが、`built.live()` から取った cuboid 上面 Face の `Surface::Plane` 4 フィールドと完全一致 (`assert_eq!`)」が抜けている。

追加要請: `t03_plane_ref_entity_extrude` 内、または別関数 `t03b_resolve_plane_matches_face_surface` で:

```rust
// 元 cuboid (bodies[0]) から f_z_pos の Face を引いて、その Surface::Plane の 4 ベクトル
let cuboid = &bodies[0].solid;
let face_idx = cuboid.find_face_by_entity_ref(&EntityRef::Named {
    feature_id: "cuboid".into(),
    kind: EntityKind::Face,
    role: "f_z_pos".into(),
}).expect("f_z_pos face exists");
let face_surface = &cuboid.faces[face_idx].surface;
let (face_origin, face_normal, face_u, face_v) = match face_surface {
    Surface::Plane { origin, normal, u_axis, v_axis } => (*origin, *normal, *u_axis, *v_axis),
    _ => panic!("Surface::Plane expected"),
};

// extrusion 側の最下面 Surface::Plane を取り出して 4 ベクトル一致を assert
// (extrusion の Face[0] (= 底面、入力 sketch plane と同一平面) を仮定)
let bottom_surface = &body_extrusion.solid.faces[0].surface;
match bottom_surface {
    Surface::Plane { origin, normal, u_axis, v_axis } => {
        assert_eq!(*origin, face_origin, "底面 origin が Face EntityRef 由来の Plane と一致");
        assert_eq!(*normal, face_normal, "底面 normal が一致");
        assert_eq!(*u_axis, face_u, "底面 u_axis が一致");
        assert_eq!(*v_axis, face_v, "底面 v_axis が一致");
    },
    _ => panic!("Surface::Plane expected"),
}
```

※ make_extrusion が返す Solid の最下面が input plane と一致する想定。一致するか実装で確認し、一致しない場合は plan T03 期待結果と実装挙動の乖離を test-spec に明記する (期待値乖離扱い)。

### T05 (YAML roundtrip legacy) — 未実装

plan.md T05 は `examples/sketch_via_refplane.engawa` を deserialize → serialize → deserialize で同一を要求している。`examples_smoke.rs` の sketch_via_refplane は build pipeline まで通すが、純粋な YAML roundtrip テストは無い。

追加要請: 新規 `crates/engawa-format/tests/planeref_yaml_roundtrip.rs` または既存 `golden_examples.rs` に:

```rust
#[test]
fn t05_yaml_roundtrip_sketch_via_refplane_example() {
    let yaml = include_str!("../../../examples/sketch_via_refplane.engawa");
    let doc1: Document = serde_yaml::from_str(yaml).unwrap();
    let yaml2 = serde_yaml::to_string(&doc1).unwrap();
    let doc2: Document = serde_yaml::from_str(&yaml2).unwrap();
    assert_eq!(doc1, doc2, "roundtrip 同一");
    // CreateSketch.plane_ref が PlaneRef::RefPlane variant であること
    let root_features = &doc1.root_component.features;
    let plane_ref = root_features.iter().find_map(|f| match f {
        Feature::CreateSketch { plane_ref, .. } => plane_ref.clone(),
        _ => None,
    }).expect("plane_ref がある");
    match plane_ref {
        PlaneRef::RefPlane(s) => assert_eq!(s, "Front"),
        PlaneRef::Entity(_) => panic!("legacy 形式は PlaneRef::RefPlane variant"),
    }
}
```

(Document に `PartialEq` derive が無い場合は serialize 後の文字列比較で代替する)

### T06 (YAML roundtrip Entity) — 未実装

plan.md T06 は新 example sketch_via_face_entity_ref.engawa を roundtrip + 文字列形式と区別される YAML 出力を要求している。

追加要請:

```rust
#[test]
fn t06_yaml_roundtrip_sketch_via_face_entity_ref_example() {
    let yaml = include_str!("../../../examples/sketch_via_face_entity_ref.engawa");
    let doc1: Document = serde_yaml::from_str(yaml).unwrap();
    let yaml2 = serde_yaml::to_string(&doc1).unwrap();
    let doc2: Document = serde_yaml::from_str(&yaml2).unwrap();
    // 再 serialize の YAML には "plane_ref:" の下に "ref:" "feature_id:" "kind:" "role:" が含まれる (map 形式)
    assert!(yaml2.contains("ref:"));
    assert!(yaml2.contains("feature_id:"));
    // legacy string 形式ではないこと
    assert!(!yaml2.contains("plane_ref: cuboid\n"));
    // roundtrip 同一性
    let yaml3 = serde_yaml::to_string(&doc2).unwrap();
    assert_eq!(yaml2, yaml3, "roundtrip after first cycle = stable");
}
```

## 実装差分から追加すべきテスト

特になし。GLM の core 実装は `crates/engawa-build/src/lib.rs` に `surface_to_plane` ヘルパを追加し、ExtrudeCut / Extrude 両方の経路で `match &entry.plane_ref` 3 分岐を実装している (plan に沿っている)。

## エッジケース・退化入力

T07 (unknown EntityRef) / T08 (Cylinder lat_face = 非平面) / T09 (Derived variant) は実装済。

不足: **同一 Solid 内で同じ feature_id+role を持つ Face が複数 (cuboid 1 個では発生しないが、`make_extrusion` の底面と上面が両方とも `EntityRef::Named { feature_id: "extrusion", kind: Face, role: "bot"/"top" }` の形式で持つ場合) のときの「最初にマッチした index」決定性検証** — これは `find_face_by_entity_ref` の責務で #214 (8a) で実装済 + テスト済。本 Issue では追加不要。

## 数値境界

本 Issue は数値モデルなし (Phase 8 は Sketch plane の解決のみ、tolerance 計算なし)。N/A。

## 決定性

T01 で IdGenerator(0) 起点で 2 回 build → Face IDs vec! 一致を検証済。**ただし plan T01 では「Plane 4 ベクトル全要素 assert_eq」も要求している**ため、T01 末尾に以下を追加:

```rust
// 追加: 2 回の build で resolve された Plane が完全一致 (Solid Face Surface)
let face_surfaces1: Vec<_> = body1.faces.iter().map(|f| &f.surface).collect();
let face_surfaces2: Vec<_> = body2.faces.iter().map(|f| &f.surface).collect();
assert_eq!(face_surfaces1.len(), face_surfaces2.len());
for (s1, s2) in face_surfaces1.iter().zip(face_surfaces2.iter()) {
    // PartialEq with f64 — Plane の 4 ベクトルが完全一致 (IdGenerator(0) 由来で決定的)
    assert_eq!(s1, s2);
}
```
