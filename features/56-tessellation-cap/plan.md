## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `tessellate_sphere_face_trimmed` の巻き方向バグ修正 (中間バンド + pole fan) | Cone サーフェスのテッセレーション |
| boolean 結果 3 種 (intersect_cyl_sphere, cut_cylinder_hole, fuse_box_cyl) のウォータータイト + 外向き法線テスト | earcut のウィンディング修正 (same_sense=false 平面) — 今回は sphere fix に集中 |
| 修正の決定性検証 | ビューア自体の z-fighting 問題 (レンダリングレイヤー) |
| degen / boundary ガードテスト | 新しいサーフェスタイプの追加 |

## Non-Goals
- earcut パスの same_sense=false ウィンディング修正 (別 Issue で対応)
- Cone テッセレーション
- ビューア側の描画品質改善
- 既存 sphere / cylinder / cuboid テストケースの変更

## 実装対象

**影響ファイル:** `crates/mycad-kernel/src/tessellation/mod.rs`

**根本原因:**

`tessellate_sphere_face_trimmed` 内の 2 箇所で `!face.same_sense` を判定条件に使っているが、正しくは:

1. **中間バンド (行 813):** 上 cap (trim_lower=false) では traverse 方向がフルスフィアと同じなので `face.same_sense` をそのまま使う必要がある。下 cap (trim_lower=true) では traverse が逆転しているので反転が必要。XOR で統一: `face.same_sense ^ trim_lower`。
2. **pole fan (行 836, 843):** pole fan は traverse の向きに関わらず、フルスフィアと同じ幾何配置になる。条件は `face.same_sense` をそのまま使う。

**Before / After:**

### 中間バンド (line ~813)
```rust
// Before
if !face.same_sense {
    push_triangle(mesh, a0, a1, b0);
    push_triangle(mesh, a1, b1, b0);
} else {
    push_triangle(mesh, a0, b0, a1);
    push_triangle(mesh, a1, b0, b1);
}

// After
if face.same_sense != trim_lower {
    push_triangle(mesh, a0, a1, b0);
    push_triangle(mesh, a1, b1, b0);
} else {
    push_triangle(mesh, a0, b0, a1);
    push_triangle(mesh, a1, b0, b1);
}
```

### south pole fan (line ~836)
```rust
// Before
if !face.same_sense {
    push_triangle(mesh, pole_idx, next, cur);
} else {
    push_triangle(mesh, pole_idx, cur, next);
}

// After
if face.same_sense {
    push_triangle(mesh, pole_idx, next, cur);
} else {
    push_triangle(mesh, pole_idx, cur, next);
}
```

### north pole fan (line ~843)
```rust
// Before
if !face.same_sense {
    push_triangle(mesh, cur, next, pole_idx);
} else {
    push_triangle(mesh, next, cur, pole_idx);
}

// After
if face.same_sense {
    push_triangle(mesh, cur, next, pole_idx);
} else {
    push_triangle(mesh, next, cur, pole_idx);
}
```

## 設計方針
- **決定性:** 変更は巻き方向の選択のみ。乱数・ID 生成なし。決定性は既存テストで担保される。
- **退化幾何:** `push_triangle` の AREA_EPS ガードは変更しない。
- **derive 規約 / エラーハンドリング / deps:** 変更なし。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_determinism | 決定性 | intersect_cyl_sphere を 2 run 実行し positions/indices 一致 | assert_eq! |
| T02_watertight_intersect | 正常系 | intersect_cyl_sphere テッセレーション全辺が 2 三角形に共有 | 全エッジ count == 2 |
| T03_outward_normals_intersect | 正常系 | intersect_cyl_sphere 全三角形の facet normal が winding-centroid dot > 0 | dot > 0 |
| T04_watertight_cut_hole | 正常系 | boolean_cut_cylinder_hole テッセレーション watertight | 全エッジ count == 2 |
| T05_watertight_fuse | 正常系 | boolean_fuse_box_cyl テッセレーション watertight | 全エッジ count == 2 |
| T06_degen_sphere_equator | 退化/境界 | 球面 cap の切断面が赤道 (v_lat=0) のケース | パニックなし + 正常終了 |
| T07_boundary_same_sense_false | 境界 | trim_lower=false + same_sense=false の上 cap | dot > 0 (外向き法線) |

## 幾何的不変条件チェックリスト
- [x] `flip_normals / same_sense` の意味論が明確か — assemble.rs の same_sense 割り当てを確認済み (sphere/cylinder は flip_normals=true → same_sense=false)
- [x] `tessellate_sphere_face_trimmed` の traverse 方向と winding の整合 — XOR ロジックで修正
- N/A: partition 出力 (テッセレーション層のみの修正)
- N/A: pslg_subdivide
