# debug-spec F01b — build_fuse() のバグ修正

## バグ
`crates/mycad-kernel/tests/tessellation_cap_acceptance.rs` の line 340 付近:

```rust
fn build_fuse() -> mycad_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let mut sph = make_sphere(3.0, Point::origin(), &mut gen).unwrap();
    shift_solid(&mut sph, 0.0, 0.0, 6.0);
    boolean(&box_solid, &sph, BooleanOp::Intersect, &mut gen).expect("intersect should succeed")
}
```

`BooleanOp::Intersect` になっているが、`build_fuse` という名前なので `BooleanOp::Fuse` + 適切な形状に変更すること。

## 修正方針
`build_fuse()` を以下の実装に置き換える:
```rust
/// Build boolean_fuse_box_cyl: make_cuboid(10,10,10) ∪ cylinder(r=2, h=15, origin=(0,0,-7.5)).
fn build_fuse() -> mycad_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let cyl = make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5), &mut gen).unwrap();
    boolean(&box_solid, &cyl, BooleanOp::Fuse, &mut gen).expect("fuse box cyl should succeed")
}
```

## 注意
- `shift_solid` の呼び出しも不要になるので削除
- `build_fuse_box_sphere` (別名) は既に存在するかもしれないので確認して重複しないようにする
- `cargo xtask ci` が green になることを確認
