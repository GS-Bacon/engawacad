# Debug Spec for #215 — Codex r2 F02 修正

## 仮説

Codex review r2 で F02 (medium) として `crates/engawa-format/src/feature.rs` の `Feature::CreateSketch` の inline golden test (`test_create_sketch_yaml_golden` 付近、l.440 周辺) に `PlaneRef::RefPlane` / `PlaneRef::Entity` の YAML 形状検証が抜けている指摘があった。現状は `plane_ref: None` ケースのみで、新 enum の retrograde 検出ができない。

## 関連ファイル

- `crates/engawa-format/src/feature.rs` — `CreateSketch` 周辺の inline test `test_create_sketch_yaml_golden` (l.430-462) と新 `PlaneRef` 定義 (l.260 付近)

## 修正方針

`feature.rs` inline test mod (`#[cfg(test)] mod tests`) に **既存テストを変更せず** 2 つの新規 inline test を追加する:

```rust
#[test]
fn test_create_sketch_yaml_golden_with_plane_ref_refplane() {
    let f = Feature::CreateSketch {
        id: "sketch_1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        profile: vec![],
        plane_ref: Some(PlaneRef::RefPlane("Front".to_string())),
    };
    let yaml = serde_yaml::to_string(&f).unwrap();
    // legacy string 形式: plane_ref: Front
    assert!(yaml.contains("plane_ref: Front"), "got:\n{}", yaml);
    // roundtrip 同一
    let back: Feature = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(f, back);
}

#[test]
fn test_create_sketch_yaml_golden_with_plane_ref_entity() {
    let f = Feature::CreateSketch {
        id: "sketch_1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "cuboid".to_string(),
            kind: EntityKind::Face,
            role: "f_z_pos".to_string(),
        })),
    };
    let yaml = serde_yaml::to_string(&f).unwrap();
    // Entity 形式: map で ref: named, feature_id, kind, role
    assert!(yaml.contains("ref: named"), "got:\n{}", yaml);
    assert!(yaml.contains("feature_id: cuboid"), "got:\n{}", yaml);
    assert!(yaml.contains("kind: face"), "got:\n{}", yaml);
    assert!(yaml.contains("role: f_z_pos"), "got:\n{}", yaml);
    // legacy string 形式と衝突しない (untagged enum で variant が区別される)
    let back: Feature = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(f, back);
}
```

`Feature` の `derive` に `PartialEq` が無い場合は serialize-string 比較に変更する (現状 `derive(Debug, Clone, Serialize, Deserialize, ...)` で `PartialEq` の有無を確認した上で対応)。

## 試した修正と結果

- [試行 1 を埋める]

## 次にやること

1. `feature.rs` inline test mod に上記 2 関数を追加 (PartialEq が無い場合は roundtrip 比較を string 比較に置き換える)
2. `cargo xtask ci` green 確認

## 追加で書いてほしいテスト

上記 2 関数のみ。Codex r2 F01 (primitive 固定 fid) は **本 Issue (#215) のスコープ外** で、`#219` として別 Issue 起票済み。`examples/sketch_via_face_entity_ref.engawa` には #219 への明示的なコメントがあり、`face_entity_ref_planeref.rs` のテストも cuboid/cylinder 固定 fid を使った workaround で本 Issue の機能 (`PlaneRef::Entity` 経路の動作) を検証している。F01 の primitive 名前張り替えは触らない。
