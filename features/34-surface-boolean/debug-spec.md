# Debug Spec — Issue #34 GLM Loop 3

## 仮説

`crates/mycad-kernel/src/geometry/surface_intersect.rs` の `intersect_plane_plane` 関数で
`origin_3d` の計算式が間違っている。

```rust
// 現在の間違ったコード
let t = b_normal.dot(&diff) / denom;
let origin_3d = a_origin + t * direction;
```

`direction = a_normal × b_normal` (正規化済み)。この方向は両平面に平行なので
`b_normal · direction = 0`。したがって：

```
b_normal · (a_origin + t * direction) = b_normal · a_origin + t * 0 = b_normal · a_origin
```

これは `c_b = b_normal · b_origin` と一般には一致しない。つまり `origin_3d` が
**平面 B 上に乗っていない** = 交差直線上にない。

2D pcurve_on_a の origin `a_orig_2d = project_to_face_uv(a, origin_3d)` が
正しい交差直線上にないため、`clip_line_to_polygon_2d(p0, dir_norm, polygon_2d)` が
正しい直線でなく平行にずれた別の直線をクリップする。

結果として PSLG subdivision が誤った IntersectionSegment を受け取り、
manifold violation "edge must have exactly 2 half-edges" が発生する。

## 関連ファイル

- `crates/mycad-kernel/src/geometry/surface_intersect.rs:57-105`
  - `intersect_plane_plane` 関数
  - 特に `origin_3d` 計算部分 (現在の `let t = ...; let origin_3d = a_origin + t * direction;`)

## 修正方針

`intersect_plane_plane` の `origin_3d` 計算を、元の `partition.rs:intersect_planes`
が使っていた正しい公式に置き換える。

**正しい公式** (両平面の方程式を同時に解く):
```rust
// c_a = a_normal · a_origin, c_b = b_normal · b_origin
let c_a = a_origin.coords.dot(&a_normal);
let c_b = b_origin.coords.dot(&b_normal);
let n_dot = a_normal.dot(&b_normal);
let denom2 = 1.0 - n_dot * n_dot;
if denom2.abs() < ANGLE_TOLERANCE {
    return Ok(vec![]);
}
let origin_3d = Point::from(
    (c_a - n_dot * c_b) / denom2 * a_normal
    + (c_b - n_dot * c_a) / denom2 * b_normal,
);
```

この公式は `a_normal · P = c_a` かつ `b_normal · P = c_b` を同時に満たす点 P を
`α * a_normal + β * b_normal` の形で求めるもの。元の `partition.rs` の `intersect_planes`
(コミット前の状態 = `git show HEAD:crates/mycad-kernel/src/booleans/partition.rs` の
495-526 行) に相当する正しい実装。

`direction` の計算・正規化は現状通り `a_normal.cross(&b_normal).normalize()` でよい。
冗長な `let d = direction.norm(); if d < ANGLE_TOLERANCE` チェックは削除してよい
(正規化後は常に 1.0)。

## 追加で書いてほしいテスト

```rust
// surface_intersect.rs の tests に追加
#[test]
fn t01b_plane_plane_intersection_origin_on_both_planes() {
    // Plane Z: z=5, Plane X: x=3
    let plane_z = Surface::Plane {
        origin: Point::new(0.0, 0.0, 5.0),
        normal: Vec3::z(),
        u_axis: Vec3::x(),
        v_axis: Vec3::y(),
    };
    let plane_x = Surface::Plane {
        origin: Point::new(3.0, 0.0, 0.0),
        normal: Vec3::x(),
        u_axis: Vec3::y(),
        v_axis: Vec3::z(),
    };
    let loops = intersect_surfaces(&plane_z, &plane_x).unwrap();
    assert_eq!(loops.len(), 1);
    let il = &loops[0];
    // intersection line must be z=5, x=3, y=free
    if let Curve::Line { origin, direction } = &il.curve_3d {
        // origin must be on both planes
        assert!((origin.z - 5.0).abs() < LENGTH_TOLERANCE, "origin.z must be 5.0, got {}", origin.z);
        assert!((origin.x - 3.0).abs() < LENGTH_TOLERANCE, "origin.x must be 3.0, got {}", origin.x);
        // direction must be Y axis
        assert!((direction.y.abs() - 1.0).abs() < LENGTH_TOLERANCE, "direction must be ±Y");
    } else {
        panic!("expected Line");
    }
    // pcurve_on_a (plane_z UV): direction=(0,1) or (0,-1), origin.u must be 3.0
    if let Curve2D::Line2D { origin, direction } = &il.pcurve_on_a {
        assert!((origin.0 - 3.0).abs() < LENGTH_TOLERANCE,
            "pcurve_on_a origin.u must be 3.0, got {}", origin.0);
    }
}
```

このテストで "origin が両平面上にある" ことを直接 assert し、回帰防止にする。

## その他の注意事項

- `cargo fmt --all` を実行してからコミットすること (loop 2 でフォーマット違反が出た)
- `features/34-surface-boolean/ci.log` には現在 loop 2 の結果が入っている (formatting fix後のCI)
- 既存テスト t01-t08 + t01b が全部 green になること、かつ feature_dispatcher の
  既存 Plane×Plane テスト群 (t02-t20) が全部 green になることを確認すること
