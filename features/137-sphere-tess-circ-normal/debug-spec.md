# Debug spec — #137 Codex F01 (round 2)

STEP 7.5 round 2 Codex review が F01 (high) を指摘:

## F01 (high) — 1HE 周期エッジの periodic 検証不在

### 現状コード (mod.rs:1001-1009)

```rust
// Validate inner loop structure: exactly 1 inner loop with ≥1 HE
if face.inner_loops.len() != 1 {
    return Err(TessellationError::TrimmedFaceUnsupported);
}
let il_idx = face.inner_loops[0];
let il = &solid.loops[il_idx];
if il.half_edges.is_empty() {
    return Err(TessellationError::TrimmedFaceUnsupported);
}
let he = &solid.half_edges[il.half_edges[0]];
```

**問題**: `half_edges.is_empty()` でなければ即座に最初の HE を読み始めるが、1HE ループの場合「その HE が周期エッジ (start_vertex == end_vertex かつ `t_range` が full sweep 2π) かどうか」を検証していない。半円 1HE (t_range = [0, π]) でも `Ok(())` で進み、`du = 2π/n_u` の full sweep 計算で 360° のトリムキャップが生成されてしまう。

### 修正方針

1HE ループの場合のみ追加 validation:
- `edge.vertices[0] == edge.vertices[1]` (start vertex == end vertex の周期エッジ)
- `(edge.t_range[1] - edge.t_range[0]).abs()` が `2π` に `ANGLE_TOLERANCE` 以内

満たさない場合は `TrimmedFaceUnsupported` を返す。

```rust
// 1HE ループは周期エッジ (full circle) のみ許容
if il.half_edges.len() == 1 {
    let he = &solid.half_edges[il.half_edges[0]];
    let edge = &solid.edges[he.edge];
    // start == end vertex (周期エッジ)
    if edge.vertices[0] != edge.vertices[1] {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }
    // t_range が full sweep 2π
    let span = (edge.t_range[1] - edge.t_range[0]).abs();
    if (span - 2.0 * std::f64::consts::PI).abs() > ANGLE_TOLERANCE {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }
}
```

(2HE 以上のループは既存ロジック通りなので無変更)

### 追加回帰テスト (in `tests/trim_sphere_circ_normal_acceptance.rs`)

```rust
#[test]
fn t_degen_partial_arc_1he_rejected() {
    // 1HE ループだが t_range が半円 [0, π] のみ → TrimmedFaceUnsupported
    // F01 round 2 回帰テスト: 周期エッジ検証
    todo!()
}
```

Setup:
- `make_sphere(5.0, Origin)` で球を作る
- inner_loop に 1 HE を持つ Face を直接構築するが、edge の `t_range = [0.0, std::f64::consts::PI]` (半円)
- start_vertex != end_vertex でも OK (どちらの違反パターンも reject される)

期待: `tessellate_sphere_face_trimmed` が `Err(TessellationError::TrimmedFaceUnsupported)` を返す。

### F02 (medium) は本 round では対応しない

F02 (`circ_center`/`circ_radius` 整合性検証) は別 Issue (#147 を想定) で follow-up として処理する。本 round では F01 のみ修正。

### 検証

- `t_degen_partial_arc_1he_rejected` が pass
- 既存 9 テストが引き続き pass
- `cargo test -p mycad-kernel --test trim_sphere_circ_normal_acceptance` で 全 10 件 pass
- `cargo test --workspace` で回帰なし
