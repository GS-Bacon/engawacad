## 仮説

sphere face の outer_loop は seam edge を 2 方向で通る 2 本の half-edge しか持たないため、
`partition.rs::get_loop_vertices` が `[south_pole=(0,0,-r), north_pole=(0,0,r)]` の
**2 点しか返さない**。この polygon_3d を持つ FaceFragment が classify.rs に渡ると、
`get_fragment_interior_point` の `poly.len() < 3` チェック (line 106) で
`Err("fragment has < 3 vertices")` となり、A3 テストが BooleanInternal エラーで失敗する。

## 関連ファイル

- `crates/mycad-kernel/src/booleans/classify.rs:104-108`
  ```rust
  fn get_fragment_interior_point(frag: &FaceFragment, _len_eps: f64) -> Result<Point, String> {
      let poly = &frag.polygon_3d;
      if poly.len() < 3 {
          return Err("fragment has < 3 vertices".to_string()); // ← ここが爆発
      }
      ...
  }
  ```

- `crates/mycad-kernel/src/booleans/partition.rs:602-608`
  ```rust
  fn get_all_face_polygons(solid: &Solid) -> Vec<Vec<Point>> {
      solid.faces.iter()
          .map(|f| get_loop_vertices(solid, f.outer_loop))
          .collect()
  }
  ```
  → sphere の outer_loop は he_up(south→north) + he_down(north→south) の 2 HE のみ → 2 点

- `crates/mycad-kernel/src/geometry/surface.rs:69-75`
  sphere parametrization: `u` = longitude [0, 2π)、`v` = latitude [-π/2, π/2]
  `(u, v) = (π/2, 0)` → `Point(center.x, center.y + radius, center.z)` (equator +Y)

- `crates/mycad-kernel/src/primitives/sphere.rs:10`
  seam は +X 経線 (XZ 半平面)。seam 上の点は u≈0 または u≈2π。

## 修正方針

`classify.rs::get_fragment_interior_point` の `poly.len() < 3` 分岐に
**sphere 面専用の early-return** を追加する。

```rust
if poly.len() < 3 {
    // Self-adjacent periodic sphere face: outer_loop has only 2 vertices (poles).
    // Use the equatorial point at u=π/2, v=0 (away from the +X seam) as interior point.
    if let Surface::Sphere { center, radius } = &frag.surface {
        return Ok(Point::new(center.x, center.y + radius, center.z));
    }
    return Err("fragment has < 3 vertices".to_string());
}
```

この点 `(cx, cy + r, cz)` は:
- sphere surface 上に存在 (`unproject_from_face_uv(Sphere, π/2, 0)` の結果と同値)
- seam (+X 経線, u≈0) から最も離れた位置 (equator +Y)
- A3 では sphere center=origin, radius=3, box half-extent=5 なので
  この点 (0, 3, 0) は box 内部 → `point_in_polyhedron` が true → `FragmentLabel::InsideOther` ✓

## 追加で書いてほしいテスト

既存 T14 (`partition_faces` に sphere face 含む入力) では panic-free は確認できるが、
A3 (classify まで走る統合テスト) が通れば内部テストは不要。
ただし念のため `classify.rs::tests` に単体テストを追加しても可:

```rust
#[test]
fn t14b_sphere_fragment_interior_point() {
    // polygon_3d が 2 点 (south_pole + north_pole) の sphere FaceFragment に対して
    // get_fragment_interior_point がエラーを返さないことを確認
    let frag = FaceFragment {
        polygon_3d: vec![
            Point::new(0.0, 0.0, -3.0),
            Point::new(0.0, 0.0,  3.0),
        ],
        surface: Surface::Sphere { center: Point::origin(), radius: 3.0 },
        source_face_index: 0,
        parent_name: EntityRef::try_named("s", EntityKind::Face, "f").unwrap(),
        traversal_index: 0,
        is_tool_side: true,
        boundary_partners: vec![None, None],
    };
    let result = get_fragment_interior_point(&frag, 1e-9);
    assert!(result.is_ok(), "sphere face interior point must succeed");
    let pt = result.unwrap();
    // should be on sphere surface: distance from center ≈ radius
    let dist = pt.coords.norm();
    assert!((dist - 3.0).abs() < 1e-9, "interior point must be on sphere surface");
}
```
