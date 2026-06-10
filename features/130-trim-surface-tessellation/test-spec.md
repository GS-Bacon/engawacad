# Test Spec — Issue #130 (trim surface tessellation)

## 不足テスト（plan 計画分）

以下の 2 件が plan のテスト計画表に記載されているが、skeleton で `todo!()` のまま `#[ignore]` になっている。
GLM で実装・解除すること。

### T07: `t07_degen_inner_small_loop_no_panic`

**目的**: 非常に小さい inner_loop（例えば 3 点でほぼ同一座標）でパニックしないこと。

**実装方針**:
- 再現しにくいため、`tessellate_trimmed_uv_face` の `if outer_3d.len() < 3 { return Ok(()); }` と同様に inner_loop が退化した場合はスキップするパスを確認する。
- 実際の退化ケースを直接作れないため、`assert_eq!(naked, 0)` のあとに `assert!(mesh.triangle_count() >= 0)` でパニックしないことを確認する最小テストで OK。
- 近似的なアプローチ: `make_cuboid(10,10,10)` + `make_sphere(0.01, ...)` で `BooleanOp::Cut` → tessellate がエラーを返さない。

```rust
#[test]
fn t07_degen_inner_small_loop_no_panic() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
    // very small sphere that barely clips the top face → tiny inner_loop
    let sphere = make_sphere(0.05, Point::new(0.0, 0.0, 6.0), &mut gen).expect("sphere");
    // may or may not produce inner_loop; main contract is: no panic
    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen);
    match result {
        Ok(solid) => {
            let mesh = tessellate_solid(&solid);
            assert!(mesh.is_ok(), "tessellate should not error on tiny dimple");
        }
        Err(_) => {
            // boolean itself may fail for degenerate overlap — acceptable
        }
    }
}
```

### T08: `t08_boundary_seam_cylinder_unwrap`

**目的**: seam を跨ぐ円筒面（`boolean_box_cut` がこのケース）で `tessellate_trimmed_uv_face` の UV アンラップが機能し、naked_edge = 0 になること。

T02 (`t02_box_cut_naked_edge_zero`) が既に boolean_box_cut をカバーしているが、このテストでは **seam 近傍** を明示的に確認する。

**実装方針**:
```rust
#[test]
fn t08_boundary_seam_cylinder_unwrap() {
    // boolean_box_cut: cylinder with seam line edge on face boundary
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(4.0, 4.0, 4.0, &mut gen).expect("cuboid");
    let cyl = make_cylinder(1.0, 6.0, Point::new(0.0, 0.0, -3.0), &mut gen).expect("cyl");
    let result = boolean(&box_solid, &cyl, BooleanOp::Cut, &mut gen).expect("cut");
    let mesh = tessellate_solid(&result).expect("tessellate");

    // Confirm no naked edges (seam line HE should not break UV continuity)
    let naked = count_naked_edges(&mesh, 1e-6);
    assert_eq!(naked, 0, "seam cylinder should have zero naked edges after UV unwrap");
    // Confirm triangle count is plausible (at least top + bottom caps + cylindrical side)
    assert!(mesh.triangle_count() >= 32 * 2, "should have cylinder ring triangles");
}
```

---

## 実装差分から追加すべきテスト

### T05: `assertMeshHealthy` 近ゼロ正規化（Playwright helpers.ts）

`web/tests/helpers.ts` の `round()` 関数で `parseFloat(x.toFixed(PREC))` 経由の near-zero 正規化を追加した。
これが正しく機能することを確認する JavaScript テスト（vitest）を追加:

ファイル: `web/src/mesh.test.ts` の末尾 or 新規 `web/tests/helpers.test.ts` に追加

```typescript
// near-zero normalization
it("round(-6e-17) and round(6e-17) produce the same key", () => {
    const PREC = 6;
    const round = (x: number): string => {
        const r = parseFloat(x.toFixed(PREC));
        return (r === 0 ? 0 : r).toFixed(PREC);
    };
    expect(round(-6.123e-17)).toBe("0.000000");
    expect(round( 6.123e-17)).toBe("0.000000");
    expect(round(-0)).toBe("0.000000");
    expect(round(0.995185)).toBe("0.995185");
});
```

> ただし Playwright `helpers.ts` は `@playwright/test` に依存しているため、vitest では直接インポートできない。上記の `round()` ロジックのみを抽出してユニットテストすること。

---

## エッジケース・退化入力

| ケース | 確認方法 | 備考 |
|--------|----------|------|
| inner_loop の点数が 3 未満 | T07 参照 | `if il_3d.len() < 3 { continue; }` でスキップ |
| seam 境界の UV アンラップ | T08 参照 | `unwrap_periodic_uv` がなければ earcut が破綻 |
| sphere dimple (inner_loop が outer より多く点を持つ) | T04 / t04_sphere_dimple_naked_edge_zero で検証済み | |
| cylinder 全周（inner_loops なし）の既存パスが壊れていない | `t05_watertight_fuse`, `t01_determinism` で検証済み | 回帰なし |

---

## 数値境界

- `unwrap_periodic_uv`: u の差が π 超の場合にのみシフト。テスト T08 がカバー。
- `collect_loop_points` の `segments` が 0 → `opts.angular_segments.max(3)` で最低 3 を保証。

---

## 決定性

- T01 (`t01_box_cut_determinism`) が 2 回実行で全座標・法線・インデックスの一致を確認済み。
- `tessellate_sphere_face_trimmed` の n_u は `collect_loop_points` 点数から決定的に算出される。
