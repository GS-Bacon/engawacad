# debug-spec.md — Codex F01 修正仕様

## 仮説
`count_naked_edges` の union-find 実装は推移性の問題（A≈B≈C が連鎖マージされる）を抱えており、
実際の穴を見逃す可能性がある。また seam vertex の微小ジッターで偽陽性になるリスクもある。
既存の `box_sphere_void_acceptance.rs` が使う「整数グリッド量子化」方式が正解。

## 関連ファイル
- `crates/mycad-kernel/tests/bool_naked_edge_acceptance.rs` — `count_naked_edges` を修正する
- `crates/mycad-kernel/tests/box_sphere_void_acceptance.rs` — 参考: `quantize` helper の使い方

## 修正方針

`count_naked_edges` の union-find を **整数グリッド量子化** に置き換える。

### Before
```rust
fn count_naked_edges(mesh: &TriangleMesh) -> usize {
    let eps = 1e-10;
    // ... pairwise union-find with O(n²) comparison ...
}
```

### After
```rust
fn count_naked_edges(mesh: &TriangleMesh, tol: f64) -> usize {
    let quantize = |p: &[f64; 3]| -> [i64; 3] {
        [
            (p[0] / tol).round() as i64,
            (p[1] / tol).round() as i64,
            (p[2] / tol).round() as i64,
        ]
    };

    let qpos: Vec<[i64; 3]> = mesh.positions.iter().map(quantize).collect();
    let tri_count = mesh.indices.len() / 3;

    let mut edge_count: std::collections::HashMap<[[i64; 3]; 2], usize> =
        std::collections::HashMap::new();

    for tri in 0..tri_count {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let q0 = qpos[i0];
        let q1 = qpos[i1];
        let q2 = qpos[i2];
        if q0 == q1 || q1 == q2 || q2 == q0 { continue; }
        for [qa, qb] in &[[q0, q1], [q1, q2], [q2, q0]] {
            let key = if qa <= qb { [*qa, *qb] } else { [*qb, *qa] };
            *edge_count.entry(key).or_insert(0) += 1;
        }
    }

    edge_count.values().filter(|&&c| c == 1).count()
}
```

呼び出し側はすべて `count_naked_edges(&mesh, 1e-9)` に変更する（T02/T03/T05 等）。

## 試した修正と結果
- [x] union-find → 量子化グリッドへの置き換え（GLM 実施済み、ただし下記 2 点が残存）
  - F01: tol が 1e-9（計画の 1e-10 より緩い）
  - F02: 三角形丸ごと skip が縮退 edge を隠す

## 次にやること（GLM 修正対象）

### 修正 1: F01 — tol を 1e-10 に揃える
全呼び出し `count_naked_edges(&mesh, 1e-9)` を `count_naked_edges(&mesh, 1e-10)` へ変更。

### 修正 2: F02 — 三角形 skip を edge 単位 skip に変更
`count_naked_edges` 内の:
```rust
if q0 == q1 || q1 == q2 || q2 == q0 { continue; }
for [qa, qb] in &[[q0, q1], [q1, q2], [q2, q0]] {
    let key = ...;
    *edge_count.entry(key).or_insert(0) += 1;
}
```
を:
```rust
for [qa, qb] in &[[q0, q1], [q1, q2], [q2, q0]] {
    if qa == qb { continue; }    // 縮退 edge のみ skip
    let key = if qa <= qb { [*qa, *qb] } else { [*qb, *qa] };
    *edge_count.entry(key).or_insert(0) += 1;
}
```
へ変更（三角形レベルの skip 行は削除する）。

### 修正 3: T08 の期待値更新
F02 修正後、T08 は v0==v1（量子化後同一点）で edge (v0,v1) が縮退 skip され、
残り 2 edge（(v1,v2) と (v2,v0)）が naked になる → 期待値を 2 に変更:
```rust
assert_eq!(count_naked_edges(&mesh, 1e-10), 2, "-0.0/+0.0 welded, 2 boundary edges exposed");
```

### 修正 4: T09 の頂点距離を 1e-11 に変更
tol=1e-10 で 5e-11 は境界ケース（5e-11/1e-10 = 0.5 → 四捨五入して 1 → 同一セルにならない）。
1e-11 に変更すると 1e-11/1e-10 = 0.1 → 0 に丸め → 確実に同一セルに入る:
```rust
[0.0, 0.0, 1e-11],  // 3 — within tol=1e-10 of vertex 0
[1.0, 0.0, 1e-11],  // 4 — within tol=1e-10 of vertex 1
```
コメントも `eps=1e-10` → `tol=1e-10, offset=1e-11` に修正。

## 追加で書いてほしいテスト
なし。
