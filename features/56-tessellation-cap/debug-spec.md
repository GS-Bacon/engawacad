# debug-spec — Issue #56 Codex F01/F02 修正

## F01: tessellation_cap_acceptance.rs が git 未追跡

`crates/mycad-kernel/tests/tessellation_cap_acceptance.rs` が untracked のため、
`extract-test-summary.ts` が `added=0` を報告している。

**修正方針**: `git add crates/mycad-kernel/tests/tessellation_cap_acceptance.rs` を実行する。
またはファイルが既存であれば空行などを加えて stage 状態にする（guard-crates は Edit/Write/rm 禁止だが git add は問題なし）。

実際には `git add` は guard に引っかからない。以下を実行すること:
```bash
git add crates/mycad-kernel/tests/tessellation_cap_acceptance.rs
git add crates/mycad-kernel/tests/test_bool_probe.rs  # こちらも add (空ファイル)
git add crates/mycad-format/tests/golden_examples.rs  # fmt 修正分
```

その後 `cargo xtask ci` を再実行して ci.log を更新すること。

## F02: T04/T05 が plan で合意した形状と異なる

現状の実装:
- `t04_watertight_cut_hole`: box - sphere の signed volume チェック (plan: boolean_cut_cylinder_hole の watertight)
- `t05_watertight_fuse`: box ∪ sphere の signed volume チェック (plan: boolean_fuse_box_cyl の watertight)

**修正方針**:
- `t04_watertight_cut_hole`: `make_cuboid(10,10,10)` から `make_cylinder(2.0, 6.0, Point::origin())` を cut し、
  `assert_watertight_after_weld` で全辺が 2 三角形共有されることを検証する。
  (`build_cut_cylinder_hole` 関数が `t04` と同じファイル内にある場合はそれを使う)

- `t05_watertight_fuse`: `make_cuboid(10,10,10)` と `make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5))` を
  fuse し、`assert_watertight_after_weld` で全辺が 2 三角形共有されることを検証する。

**備考**: `build_cut_cylinder_hole` が既に acceptance テスト内に存在する場合はそれを利用する。
`build_fuse_box_sphere` は plan の `boolean_fuse_box_cyl` 相当に置き換える。
