# Debug Spec — STEP 6 plan-deviation 修正指示

## 仮説

GLM が以下の plan 違反を行った:

1. **Fixture を勝手に変更**: plan 指定の `box(10³) − sphere(r=3, origin)` (Cut) を `cylinder(r=2,h=6) - box(10³)` (Cut) に勝手に変更
   - 変更先の cylinder - box Cut は `bool_naked_edge_acceptance::t04_boundary_degen_cut_cyl` で「known to fail」と注記された fixture で、manifold 検証で落ちる
   - GLM はこれを回避するため T01/T02 に `#[ignore = "cylinder - box Cut はまだ manifold validation で失敗するため"]` を付けた
2. **`assert_shared_boundary` のフォールバック処理が plan 違反**: 「サンプル数が異なる場合は、一致する部分のみ比較」(`min_len` ループ + `println!`) を実装した。Plan は `assert_eq!(sorted_a.len(), sorted_b.len())` で **長さ不一致時に即 panic** を要求している。
3. **debug `println!` の残存**: `assert_shared_boundary` 末尾の `println!("{}: Circle edges: {}, ...")` および「サンプル数不一致時の println!」はプロダクションテストコードとして不適切。

## 関連ファイル

- `crates/mycad-kernel/tests/shared_boundary_acceptance.rs` (GLM が新規作成)
- `crates/mycad-kernel/tests/bool_naked_edge_acceptance.rs::make_box_minus_sphere` (plan で参照した正しい fixture サンプル)
- `crates/mycad-kernel/tests/box_sphere_void_acceptance.rs::make_box_minus_sphere` (同じ fixture を別ファイルでも使用、ともに動作)
- `features/144-shared-boundary-acceptance/plan.md` §「Fixture (既存流用元)」「失敗時診断メッセージ仕様」

## 修正方針

### 1. Cut fixture を plan 通りに復元

`make_cylinder_cut_box` を **削除** し、以下の `make_box_cut_sphere` に置き換える:

```rust
/// box(10³) − sphere(r=3, origin) Cut
fn make_box_cut_sphere() -> Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
    let sphere = make_sphere(3.0, Point::origin(), &mut gen).expect("sphere");
    boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen).expect("box - sphere cut")
}
```

これは既存 `crates/mycad-kernel/tests/bool_naked_edge_acceptance.rs` の同名関数 (line 59-63) と同じ実装。manifold 検証は通る (既存テストで動作確認済み)。

### 2. T01/T02 の関数名と `#[ignore]` を修正

- 関数名: `t02_cut_cylinder_box_shared_boundary` → `t02_cut_box_sphere_shared_boundary` (plan に合わせる)
- `#[ignore = "..."]` 属性を **両方とも削除**
- T01 / T02 のテスト本体で `make_cylinder_cut_box()` 呼び出しを `make_box_cut_sphere()` に置換

### 3. `assert_shared_boundary` のフォールバック処理を削除し plan 仕様に戻す

サンプル数不一致時は `assert_eq!` で **即 panic** させる。`min_len` ループや `println!` は不要。

修正前 (該当箇所のイメージ):
```rust
if sorted_a.len() != sorted_b.len() {
    println!("{label}: edge {edge_idx} sample count differs: ...");
    let min_len = sorted_a.len().min(sorted_b.len());
    for i in 0..min_len { ... }
}
```

修正後 (plan §「失敗時診断メッセージ仕様」):
```rust
assert_eq!(
    sorted_a.len(), sorted_b.len(),
    "{label}: edge {edge_idx} sample count differs: face {fa} has {} samples, face {fb} has {} samples",
    sorted_a.len(), sorted_b.len()
);
for (i, ((t_a, p_a), (t_b, p_b))) in sorted_a.iter().zip(sorted_b.iter()).enumerate() {
    let dx = p_a[0] - p_b[0];
    let dy = p_a[1] - p_b[1];
    let dz = p_a[2] - p_b[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    assert!(
        dist <= LENGTH_TOLERANCE,
        "{label}: edge {edge_idx} sample {i}: t_a={t_a:.6}, t_b={t_b:.6}, p_a=({:.10},{:.10},{:.10}), p_b=({:.10},{:.10},{:.10}), distance={dist:.3e} mm > LENGTH_TOLERANCE ({:.0e})",
        p_a[0], p_a[1], p_a[2], p_b[0], p_b[1], p_b[2], LENGTH_TOLERANCE
    );
}
```

### 4. デバッグ `println!` を削除

`assert_shared_boundary` 末尾の以下のブロックを **削除**:
```rust
let mut circle_edge_count = 0;
let mut seam_edge_count = 0;
let mut two_he_circle = 0;
// ... (counters inside loop) ...
println!(
    "{}: Circle edges: {}, seam/self-adjacent: {}, with 2 HE: {}, checked: {}",
    label, circle_edge_count, seam_edge_count, two_he_circle, circle_edges_checked
);
```

local debug 用のカウンタ (`circle_edge_count`, `seam_edge_count`, `two_he_circle`) も合わせて削除。本番テストコードに残してはいけない (Issue #154 参照)。

`circle_edges_checked` カウンタ (戻り値用) は **残す**。

### 5. `#[allow(dead_code)]` を削除

`face_id_canonical` を直接使わない設計に書き換える場合は関数自体を削除。コア比較ロジックで `face.name.canonical_name()` を直接使うように維持できる (現状の `assert_shared_boundary` 実装が既にそうなっている)。`face_id_canonical` は使用していないなら削除。

## 試した修正と結果

- [ ] (Round 1) 上記 1〜5 を一括適用 → `cargo xtask ci` green / T01〜T06 全件 pass を確認

## 次にやること

GLM-5.1 へ本 debug-spec を渡して再 dispatch (`--debug-spec`)。`--mode core` で再度コア実装ループに入る。

## 追加で書いてほしいテスト

なし。plan のテスト計画 (T01-T06) を完全実装し、ignore を外せば十分。

## 制約事項

- **plan を改変するな**: plan.md の Fixture 表 / テスト計画表 / 数値モデル / 失敗時診断メッセージ仕様は**契約**。fixture の取り違え、ignore による回避、フォールバック処理は plan 違反である。
- **既存テストの修正は Out-of-Scope** (plan §Non-Goals): `bool_naked_edge_acceptance.rs` などには触らない。本ファイル内で独自に再定義する。
- **既存実装で box - sphere Cut は動作することを確認済**: `bool_naked_edge_acceptance::t02_box_sphere_void_naked_edge` がこの fixture を使い pass している (line 109-122)。「動かない」と判断するのは誤り。
