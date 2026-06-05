# test-spec.md — Issue #48: position params

## 不足テスト（plan 計画分）

T01〜T06 の acceptance test は `crates/mycad-build/tests/position_params_acceptance.rs` にスケルトン配置済みだが、本体は `todo!()` のまま。GLM はすべての `#[ignore]` を外して実装すること。

### T01 決定性
```rust
// IdGenerator::new(0) で 2 回 build → assert_solids_equal_with_names
let path = examples_dir.join("cylinder_offset.mycad");
let doc = Document::from_path(&path).unwrap();
let mut g1 = IdGenerator::new(0);
let mut g2 = IdGenerator::new(0);
let b1 = build_bodies_from_features(&doc.root_component.features, &mut g1).unwrap();
let b2 = build_bodies_from_features(&doc.root_component.features, &mut g2).unwrap();
assert_solids_equal_with_names(&b1.all()[0].solid, &b2.all()[0].solid);
```

### T02 origin 反映（cylinder）
```rust
// Feature::CreateCylinder { origin: [0.0, 0.0, -10.0], .. } で build
// Surface::Cylinder.origin == Point::new(0.0, 0.0, -10.0)
// 底面 Vertex の z 座標がすべて -10.0 付近 (< 1e-9)
let features = vec![Feature::CreateCylinder {
    id: "c".into(), radius: 5.0, height: 20.0, origin: [0.0, 0.0, -10.0]
}];
let mut gen = IdGenerator::new(0);
let bodies = build_bodies_from_features(&features, &mut gen).unwrap();
let solid = &bodies.all()[0].solid;
// Verify Surface::Cylinder.origin:
let cyl_face = solid.faces.iter().find(|f| matches!(solid.surface_of(f), Surface::Cylinder { .. })).unwrap();
if let Surface::Cylinder { origin, .. } = solid.surface_of(cyl_face) {
    assert!((origin - Point::new(0.0, 0.0, -10.0)).norm() < 1e-9);
}
// Verify bottom vertices z == -10:
let bottom_vs: Vec<_> = solid.vertices.iter().filter(|v| (v.point.z + 10.0).abs() < 1e-9).collect();
assert!(bottom_vs.len() >= 1);
```

### T03 後方互換（origin 省略 == origin=[0,0,0]）
```rust
// origin 省略版と origin=[0.0,0.0,0.0] 明示版が byte-identical Solid
let f_implicit = Feature::CreateCylinder { id:"c".into(), radius:3.0, height:5.0, origin:[0.0,0.0,0.0] };
let f_explicit = Feature::CreateCylinder { id:"c".into(), radius:3.0, height:5.0, origin:[0.0,0.0,0.0] };
// YAML に origin が出力されないことも確認
let yaml = serde_yaml::to_string(&f_implicit).unwrap();
assert!(!yaml.contains("origin"), "origin:[0,0,0] should not appear in YAML");
// round-trip で既存 cylinder.mycad が変化しないことも確認
let path = examples_dir.join("cylinder.mycad");
let doc = Document::from_path(&path).unwrap();
let yaml1 = doc.to_yaml().unwrap();
let doc2 = Document::from_yaml(&yaml1).unwrap();
assert_eq!(yaml1, doc2.to_yaml().unwrap());
```

### T04 center 反映（sphere）
```rust
// Feature::CreateSphere { center: [2.0, 0.0, 0.0], .. } で build
// Surface::Sphere.center == Point::new(2.0, 0.0, 0.0)
// 北極・南極頂点が center 周り
let features = vec![Feature::CreateSphere { id: "s".into(), radius: 5.0, center: [2.0, 0.0, 0.0] }];
let mut gen = IdGenerator::new(0);
let bodies = build_bodies_from_features(&features, &mut gen).unwrap();
let solid = &bodies.all()[0].solid;
if let Surface::Sphere { center, .. } = solid.surface_of(&solid.faces[0]) {
    assert!((center - Point::new(2.0, 0.0, 0.0)).norm() < 1e-9);
}
let north = solid.vertices.iter().max_by(|a,b| a.point.z.partial_cmp(&b.point.z).unwrap()).unwrap();
assert!((north.point - Point::new(2.0, 0.0, 5.0)).norm() < 1e-9);
```

### T05 派生名不変（origin/center を変えても role 名は不変）
```rust
// 原点版と offset 版の face/edge/vertex name が一致
let f_origin  = vec![Feature::CreateCylinder { id:"c".into(), radius:3.0, height:5.0, origin:[0.0,0.0,0.0] }];
let f_offset  = vec![Feature::CreateCylinder { id:"c".into(), radius:3.0, height:5.0, origin:[1.0,2.0,-3.0] }];
let s1 = build_one(&f_origin);
let s2 = build_one(&f_offset);
assert_solids_equal_with_names(&s1, &s2);
// sphere 版も同様
```

### T06 YAML roundtrip（offset examples）
```rust
// examples/cylinder_offset.mycad と examples/sphere_offset.mycad が byte-identical roundtrip
for name in &["cylinder_offset.mycad", "sphere_offset.mycad"] {
    let path = examples_dir.join(name);
    let doc = Document::from_path(&path).unwrap();
    let yaml1 = doc.to_yaml().unwrap();
    let doc2 = Document::from_yaml(&yaml1).unwrap();
    assert_eq!(yaml1, doc2.to_yaml().unwrap(), "roundtrip mismatch for {name}");
}
```

### T07 NaN/Inf 検証（mycad-format inline test）
T07 は `crates/mycad-format/src/document.rs` または `feature.rs` の inline test として追加。
```rust
#[test]
fn t07_nan_position_rejected() {
    use crate::feature::Feature;
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: test
  features:
    - type: create_cylinder
      id: c1
      radius: 5.0
      height: 10.0
      origin: [.nan, 0.0, 0.0]
"#;
    let result = Document::from_yaml(yaml);
    assert!(matches!(result, Err(FormatError::InvalidPosition { .. })),
        "expected InvalidPosition, got {result:?}");
}
```

## 実装差分から追加すべきテスト

- **build ディスパッチ経路** (lib.rs L130-151): `[f64;3]` → `Point::new` の変換で負値・大値が正しく渡るか確認。これは T02/T04 のカバー範囲に含まれる。
- **既存テストの `origin:[0.0,0.0,0.0]` 明示追加**: feature_dispatcher.rs / a1_1 / surface_boolean_a2 の多数箇所でフィールドを明示追加した。これは後方互換確認のためだが、YAML との往復は T03/T06 でカバーできる。

## エッジケース・退化入力

- **大座標値** (`origin:[1e15, 1e15, 1e15]`): is_finite はパス。build は成功すること。
- **負座標値** (`origin:[-100.0, -50.0, -200.0]`): T02 に準じて surface.origin が正しいこと。
- **ゼロ座標混在** (`center:[0.0, 0.0, 5.0]`): YAML で center フィールドが出力される（is_origin = false）こと。

## 数値境界

- `origin:[f64::NAN, 0.0, 0.0]` → `FormatError::InvalidPosition` (T07)
- `origin:[f64::INFINITY, 0.0, 0.0]` → `FormatError::InvalidPosition` (T07 追加ケース)

## 決定性

T01 で `IdGenerator::new(0)` 2 回 → `assert_solids_equal_with_names` でカバー。

## 前提ファイル（GLM が作成すること）

- `examples/cylinder_offset.mycad`: `origin: [0.0, 0.0, -10.0]`、radius=5.0、height=20.0
- `examples/sphere_offset.mycad`: `center: [2.0, 0.0, 0.0]`、radius=5.0

これらは T01・T06 の入力ファイルとして必須。
