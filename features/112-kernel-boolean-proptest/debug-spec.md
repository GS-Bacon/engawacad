## Codex STEP 7.5 round 1 — 修正指示

### 仮説

Codex レビュアーが 2 件の High を指摘した。

### 関連ファイル

- `crates/mycad-kernel/tests/boolean_proptest.rs`

---

### F01: T02 が surface cut Err をサイレントに無視している

**発生箇所**: `t02_prop_surface_cut_reduces_volume` (L88〜L108)

**現状コード**:
```rust
if let Ok(cut) = result {
    // volume assertion
}
```

x_offset ∈ [4.1, 5.4] の範囲で boolean が常に `Err` を返す退行が起きてもテストがパスしてしまう。

**修正方針**:
```rust
// Err は surface cut 範囲で発生してはならない（明確な Error として失敗させる）
prop_assert!(result.is_ok(), "surface cut must succeed for x_offset={x_offset}: {result:?}");
let cut = result.unwrap();
// 以降の volume assertion は変更なし
```

`prop_assert!(result.is_ok(), ...)` を追加してから `let cut = result.unwrap();` で取り出す。
proptest マクロ内では `unwrap()` はパニックになるが、前の `prop_assert!` が先に失敗を伝える。

---

### F02: T03/T03_boundary/T03_repeated/T06 が entity 数と ID しか比較していない

**発生箇所**: `t03_determinism_fixed_seed` (L134〜L154), `t03_boundary_seed_determinism`, `t03_repeated_determinism_100_runs`, `t06_make_cuboid_determinism`

**修正方針**: 全比較に `tessellate_solid()` の `positions` / `indices` 比較を追加。

```rust
// 既存の vertex/edge/face count + id 比較の後に追加:
let mesh_a = tessellate_solid(&a).unwrap();
let mesh_b = tessellate_solid(&b).unwrap();
assert_eq!(mesh_a.positions, mesh_b.positions, "vertex positions mismatch");
assert_eq!(mesh_a.indices, mesh_b.indices, "triangle indices mismatch");
```

`t03_boundary_seed_determinism` の `Ok(sa), Ok(sb)` アームにも同様に追加する。
`t03_repeated_determinism_100_runs` は比較スタイルが異なる場合は同等の追加を行う。
`t06_make_cuboid_determinism` も同様（`boolean()` ではなく `make_cuboid()` の結果を比較）。

---

### 試した修正と結果

- (なし — 初回)

### 次にやること

1. `t02_prop_surface_cut_reduces_volume` に `prop_assert!(result.is_ok(), ...)` を追加
2. `t03_determinism_fixed_seed` / `t03_boundary_seed_determinism` / `t03_repeated_determinism_100_runs` / `t06_make_cuboid_determinism` に mesh positions/indices 比較を追加
3. `cargo xtask ci` で green を確認
4. 結果を `features/112-kernel-boolean-proptest/glm-test-result.json` に書き出す

### 追加で書いてほしいテスト

なし（既存テストの修正のみ）
