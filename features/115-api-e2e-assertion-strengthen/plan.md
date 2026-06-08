## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| extrude_cut_acceptance.rs A01 のアサーション強化（面数・face_id 一意性・体積） | 新規 E2E エンドポイントの追加 |
| e2e_api_scenarios.rs S04 の期待値修正（offset=5.0,depth=5.0 → depth≈face_dist パターン） | Playwright E2E テスト |
| 面数・face_id ユニーク数を同一チェックで検証 | TriangleMesh 構造の変更 |

## Non-Goals
- validate_manifold() の実装変更（#111 完了済み）
- 新 API エンドポイントの追加
- body 数（volumes/シェル数）の検証（#116 に委譲）

## 実装対象
Issue: #115
影響ファイル:
- `crates/mycad-api/tests/extrude_cut_acceptance.rs` — A01 アサーション強化
- `crates/mycad-api/tests/e2e_api_scenarios.rs` — S04 期待値修正

### A01 強化内容（before / after）

**before**: 頂点数が変化したかどうかだけを確認
```rust
assert!(
    after_vertices != initial_vertices,
    "vertices must change after extrude_cut: ..."
);
```

**after**: 面数・face_id 一意性・体積の追加検証
```rust
// 1) 面数: unique face_id 数が期待値と一致
let all_face_ids: Vec<&str> = cut_bodies.iter()
    .flat_map(|b| b.mesh.face_ids.iter().map(|s| s.as_str()))
    .collect();
let unique_faces: std::collections::HashSet<&str> =
    all_face_ids.iter().copied().collect();
assert_eq!(unique_faces.len(), 10,
    "pocket cut: 6-1+4+1=10 faces expected: {unique_faces:?}");

// 2) 体積: after < before (material が除去された)
let vol_before = mesh_volume(&initial_bodies);
let vol_after  = mesh_volume(&cut_bodies);
assert!(vol_after < vol_before,
    "extrude_cut must reduce volume: before={vol_before}, after={vol_after}");
```

ヘルパー関数:
```rust
fn mesh_volume(bodies: &[BodyMesh]) -> f64 {
    // 符号付き四面体体積の総和: V = Σ (p0·(p1×p2)) / 6
    bodies.iter().map(|b| {
        let m = &b.mesh;
        let v: f64 = m.indices.chunks(3).map(|tri| {
            let p0 = m.positions[tri[0] as usize];
            let p1 = m.positions[tri[1] as usize];
            let p2 = m.positions[tri[2] as usize];
            (p0[0]*(p1[1]*p2[2]-p1[2]*p2[1])
            +p0[1]*(p1[2]*p2[0]-p1[0]*p2[2])
            +p0[2]*(p1[0]*p2[1]-p1[1]*p2[0])) / 6.0
        }).sum();
        v.abs()
    }).sum()
}
```

### S04 期待値修正

simple_box は 10×20×30、中心原点 → X ∈ [-5, 5]。
YZ 平面に sketch、offset=5.0 → face_dist=5.0 → depth=5.0 は境界（退化）でエラーが期待される。
現行 S04 は depth=5.0 で 422 を期待しているが、実際の boundary 幾何が #111 後にエラーになるかどうか
を再確認し、エラーなら現行の assert を維持するか、`depth = face_dist - ε`（≒4.999999）で 200 になる
パターンを追加テスト S04b として追加する。

実装者 GLM は `cargo test -p mycad-api` を実行し実際の挙動を確認してから期待値を決定すること。

## 設計方針
- API は変更なし。テストのみ強化。
- mesh_volume は符号付き四面体公式（Gauss divergence theorem）を使用。
- face_id の一意性確認は `HashSet::len()` で行う。
- 決定性要件: テスト自体が冪等（外部依存なし、temp_copy で毎回クリーン）

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 面数 | pocket cut 後の unique face_id 数 | == 10 |
| T02 | 体積 | cut 後体積 < 初期体積 | vol_after < vol_before |
| T02_boundary | 境界 | face_id が 0 の場合にエラーせず関数が機能する | デグレなし |
| T03 | 回帰 | S04: offset=5.0, depth=5.0 → 422 (boundary) | 期待値は実行で確認 |

## 幾何的不変条件チェックリスト
- N/A（テスト強化のみ、カーネル変更なし）
