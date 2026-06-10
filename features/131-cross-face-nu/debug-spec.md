# Codex レビュー指摘修正仕様 (r4)

## F01 (high) — adjacent_face_idx が実際に担保されていない

**Codex の指摘**: 両分岐が arcs_per_rev を返すため、adjacent_face_idx を削除しても全テストが通る。
#131 の「cross-face adjacency で n_u を決める」成果物が証明できていない。

**修正方針**: `crates/mycad-kernel/src/tessellation/mod.rs` の `#[cfg(test)] mod tests` ブロックに、
`adjacent_face_idx` を直接呼ぶユニットテストを追加する。

**追加するテスト（インライン unit test — GLM がこれを tessellation/mod.rs の末尾に追加する）**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::booleans::{boolean, BooleanOp};
    use crate::brep::topology::IdGenerator;
    use crate::geometry::Point;
    use crate::primitives::{make_cuboid, make_cylinder, make_sphere};

    /// Verify that adjacent_face_idx returns a Sphere face for cyl∩sphere intersect's
    /// cylinder lateral face Circle arcs. If adjacent_face_idx is removed or broken,
    /// this test fails.
    #[test]
    fn test_adjacent_face_idx_sphere_cap() {
        let mut gen = IdGenerator::new(0);
        let cyl = make_cylinder(3.0, 20.0, Point::origin(), &mut gen).unwrap();
        let mut sph = make_sphere(4.0, Point::origin(), &mut gen).unwrap();
        // shift sphere to create cyl∩sphere
        for v in &mut sph.vertices {
            v.point.coords.z += 10.0;
        }
        let solid = boolean(&cyl, &sph, BooleanOp::Intersect, &mut gen)
            .expect("cyl∩sphere intersect");

        // Find the cylinder lateral face (Surface::Cylinder)
        let cyl_face_idx = solid.faces.iter().position(|f| {
            matches!(f.surface, crate::geometry::surface::Surface::Cylinder { .. })
        });
        assert!(cyl_face_idx.is_some(), "No cylinder lateral face found");
        let cyl_face_idx = cyl_face_idx.unwrap();
        let cyl_face = &solid.faces[cyl_face_idx];
        let outer_loop = &solid.loops[cyl_face.outer_loop];

        // Find a Circle-arc half-edge and verify its adjacent face is Sphere
        let sphere_adj_found = outer_loop.half_edges.iter().any(|&he_idx| {
            let he = &solid.half_edges[he_idx];
            matches!(solid.edges[he.edge].curve, crate::geometry::curve::Curve::Circle { .. })
                && adjacent_face_idx(&solid, he_idx)
                    .map(|f| matches!(solid.faces[f].surface, crate::geometry::surface::Surface::Sphere { .. }))
                    .unwrap_or(false)
        });
        assert!(
            sphere_adj_found,
            "No Circle-arc HE in cyl lateral face has a Sphere adjacent face — \
             adjacent_face_idx or the Sphere detection is broken"
        );
    }

    /// Verify that adjacent_face_idx returns a Plane face for box∩cyl intersect's
    /// cylinder lateral face Circle arcs. If adjacent_face_idx is removed or broken,
    /// this test fails.
    #[test]
    fn test_adjacent_face_idx_plane_cap() {
        let mut gen = IdGenerator::new(0);
        let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
        let cyl = make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5), &mut gen).unwrap();
        let solid = boolean(&box_solid, &cyl, BooleanOp::Intersect, &mut gen)
            .expect("box∩cyl intersect");

        let cyl_face_idx = solid.faces.iter().position(|f| {
            matches!(f.surface, crate::geometry::surface::Surface::Cylinder { .. })
        });
        assert!(cyl_face_idx.is_some(), "No cylinder lateral face found");
        let cyl_face_idx = cyl_face_idx.unwrap();
        let cyl_face = &solid.faces[cyl_face_idx];
        let outer_loop = &solid.loops[cyl_face.outer_loop];

        let plane_adj_found = outer_loop.half_edges.iter().any(|&he_idx| {
            let he = &solid.half_edges[he_idx];
            matches!(solid.edges[he.edge].curve, crate::geometry::curve::Curve::Circle { .. })
                && adjacent_face_idx(&solid, he_idx)
                    .map(|f| matches!(solid.faces[f].surface, crate::geometry::surface::Surface::Plane { .. }))
                    .unwrap_or(false)
        });
        assert!(
            plane_adj_found,
            "No Circle-arc HE in cyl lateral face has a Plane adjacent face — \
             adjacent_face_idx or the Plane detection is broken"
        );
    }
}
```

**確認事項**:
- テストが `tessellation/mod.rs` の `#[cfg(test)] mod tests` 内に追加されている
- `adjacent_face_idx` を無効にすると `test_adjacent_face_idx_sphere_cap` と `test_adjacent_face_idx_plane_cap` が FAIL する
- `cargo test -p mycad-kernel` で全テスト green

## 試した修正と結果
- Round 1: `_adj_is_sphere` 計算 + 使わない → Codex「Sphere 分岐未反映」
- Round 2: Sphere→angular_segments → t03_cyl_sph_intersect_naked_edge FAIL
- Round 3: 両分岐 arcs_per_rev + コメント → Codex「adjacent_face_idx 削除しても全テスト通る」
- Round 4: adjacent_face_idx を直接検証する unit test 追加（本 debug-spec）

## 次にやること
1. GLM が上記 unit test を tessellation/mod.rs に追加する
2. `cargo test -p mycad-kernel` green を確認する（新テスト 2 件も含む）
