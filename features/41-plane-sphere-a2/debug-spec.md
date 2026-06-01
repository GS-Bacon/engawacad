# debug-spec — #41 Plane×Sphere A2 Run 1 失敗分析

## 仮説

Run 1 の実装は概ね正しいが、既存テスト 2 件が大円ガードと trimmed sphere tessellation の副作用で壊れた。

---

## 失敗テスト 1: `booleans::partition::tests::t15_cyl_sph_face_pair_skipped`

### 失敗メッセージ
```
panicked at crates/mycad-kernel/src/booleans/partition.rs:2003:13:
partition with cylinder+sphere failed: unsupported boolean case: plane through sphere center (great circle)
```

### 根本原因
- `make_cylinder(2.0, 4.0)` は高さ 4 の円柱。底面が z=0 にある (要確認)。
- `make_sphere(3.0)` は中心が origin (0,0,0)、R=3 の球。
- 円柱の底面平面 (z=0) が球の中心 (z=0) を通るため、大円ガード `d.abs() < LENGTH_TOLERANCE` が発火。
- t15 の本来の意図は「cyl lateral × sph face pair がスキップされる」ことの検証であり、大円ケースのテストではない。

### 修正方針
**`t15_cyl_sph_face_pair_skipped` テストの球を z 方向にオフセットして大円を回避する。**

```rust
// 変更前
let sphere = make_sphere(3.0, &mut gen).unwrap();

// 変更後: 球の中心を z=2 に移動 (どの cylinder cap も球中心を通らない)
let mut sphere = make_sphere(3.0, &mut gen).unwrap();
// Solid の全 vertex を z+2 にシフト
for v in sphere.vertices_mut() {
    v.position.coords.z += 2.0;
}
```

または別方法: sphere 生成後に Transform を適用するヘルパがあれば使う。
もし vertex 直接シフトが最も簡単なら、`sphere.vertices_mut()` で全頂点の z を +2.0 加算。

あるいは sphere を再生成せず、t15 の `partition_faces` の入力を少し変えるだけでも良い。ただし **テスト本体のロジック (cyl×sph face pairs のスキップ確認) は変えないこと**。

---

## 失敗テスト 2: `tessellation::tests::test_non_canonical_sphere_face`

### 失敗メッセージ
```
panicked at crates/mycad-kernel/src/tessellation/mod.rs:1378:13:
assertion failed: matches!(tessellate_solid(&s), Err(TessellationError::TrimmedFaceUnsupported))
```

### 根本原因
- テストの "Case 2: inner loops present" は **無効な** inner loop (`lp_inner = loop with 1 HE = he0`) を持つ sphere face を作成している。
- A2 の実装で `tessellate_sphere_face_trimmed` が inner_loops を持つ sphere face を処理するようになったため、この無効ケースが `TrimmedFaceUnsupported` を返さず、内部で別の挙動をしている。
- テストの意図: 不正な inner loop を持つ sphere face は `TrimmedFaceUnsupported` であること。

### Case 2 の無効内容 (詳細)
```rust
let lp_inner = s.add_loop(7, vec![he0]);  // 1 HE のみ → 無効
// outer_loop = 2 HE (seam の自己参照型), inner_loop = 1 HE
// ← 有効な緯度線 circle inner loop は: outer=2HE + inner=2HE (Curve::Circle seam-like 経路)
```

### 修正方針
`tessellate_sphere_face_trimmed` (または呼び出し元の分岐) に **inner loop の妥当性チェック** を追加し、無効な場合は `Err(TessellationError::TrimmedFaceUnsupported)` を返す。

有効な trimmed sphere inner loop の条件:
- `inner_loops` の長さが **ちょうど 1** (複数内側ループは未対応)
- その inner loop の HE 列に **Curve::Circle** を持つ edge が含まれる
- その edge の curve が `Surface::Sphere` と幾何的に整合 (latitude line として valid)

実装例:
```rust
fn tessellate_sphere_face_trimmed(face, solid, ...) -> Result<Vec<Triangle>, TessellationError> {
    // 1. inner_loops が 1 件か確認
    if face.inner_loops.len() != 1 {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }
    let inner_loop = &solid.loops[face.inner_loops[0]];
    
    // 2. inner loop の HE から Curve::Circle edge を取得
    let circle_curve = inner_loop.half_edges.iter()
        .find_map(|he_idx| {
            let he = &solid.half_edges[*he_idx];
            let edge = &solid.edges[he.edge];
            if matches!(edge.curve, Curve::Circle { .. }) {
                Some(&edge.curve)
            } else {
                None
            }
        });
    
    let Some(Curve::Circle { center, normal, radius }) = circle_curve else {
        return Err(TessellationError::TrimmedFaceUnsupported);
    };
    
    // 3. v_lat を circle center と sphere surface から計算
    // (Surface::Sphere { center: sph_c, radius: sph_r } から)
    // d = (circle_center - sph_c).dot(normal.normalize())
    // v_lat = asin(d / sph_r)
    
    // 4. v_range UV グリッドでメッシュ生成...
}
```

---

## 関連ファイル

- `crates/mycad-kernel/src/booleans/partition.rs` — t15 テスト (行 1994 付近)、大円ガード (行 308 付近、496 付近、871 付近)
- `crates/mycad-kernel/src/tessellation/mod.rs` — `test_non_canonical_sphere_face` (行 1303)、`tessellate_sphere_face_trimmed` (新設関数)

---

## 試した修正と結果

- [ ] Run 1: 実装完了、fmt 失敗 → `cargo fmt --all` で修正 → 2 テスト残存
  - t15: 大円ガードの誤発火
  - test_non_canonical: trimmed sphere tessellation が invalid inner loop を弾かない

---

## 次にやること

1. **t15**: sphere vertex を z+2 シフト or sphere を (0,0,2) で生成して大円を回避
2. **test_non_canonical**: `tessellate_sphere_face_trimmed` または分岐箇所に inner loop 妥当性チェック追加
3. `cargo xtask ci` 全緑確認

---

## 追加で書いてほしいテスト

(なし — 既存テストの修正のみで十分)
