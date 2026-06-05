## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `tessellate_sphere_face_trimmed` の trim 方向判定を位置ベースに修正 | partition.rs / classify.rs の変更（B-rep は正常） |
| cyl×sphere intersect の上下両 cap がメッシュに出ることを保証 | box−sphere `TrimmedFaceUnsupported`（#50, 別 Issue） |
| メッシュ z 範囲を検証する受入テスト T08/T09 追加 | viewer all()→live() バグ（#51, 別 Issue） |
| 修正後 `circ_normal` 未使用化に伴う clippy 対応 | 非軸整列・Cut/Fuse with sphere の cap |

## Non-Goals
- partition.rs:931-941 の未使用 `is_top_cap` / `_interior` のリファクタは行わない（tessellation 側のみで完結）
- 球面 cap に「保持する極」を明示エンコードする B-rep 拡張は行わない
- 多重 inner loop の trimmed sphere（現状 `TrimmedFaceUnsupported`）は対象外

## 実装対象
<!-- Issue: #54 -->
<!-- 影響クレート: mycad-kernel (tessellation), mycad-build (tests) -->

**修正ファイル**: `crates/mycad-kernel/src/tessellation/mod.rs`

変更箇所 1: `tessellate_sphere_face_trimmed` 753-756 行

before:
```rust
    // Determine trim direction: if cutting plane normal is +Z and plane is above center,
    // the lower cap remains. If below center, the upper cap remains.
    // The circ_normal indicates the plane normal direction.
    let trim_lower = circ_normal.z > 0.0;
```

after:
```rust
    // Both intersection circles share normal +Z, so circ_normal cannot tell the
    // upper cap from the lower cap. Decide from the circle's position relative to
    // the sphere center (same criterion as classify::get_fragment_interior_point):
    // a cutting circle below the center keeps the lower (south-pole) cap.
    let trim_lower = center_z < center.coords.z;
```

変更箇所 2: 842 行（`circ_normal` が未使用になるため破棄リストに追加）

before:
```rust
    let _ = (circ_radius, base_idx);
```
after:
```rust
    let _ = (circ_radius, circ_normal, base_idx);
```

**テスト追記ファイル**: `crates/mycad-build/tests/cyl_sph_intersect_acceptance.rs`

## 設計方針

- **決定性**: 変更は純粋な浮動小数比較のみ。IdGenerator 不使用。決定性は維持（T01/T22b 既存）。
- **B-rep トポロジー妥当性**: tessellation のみ修正。B-rep は変更なし。T04 (Euler-Poincaré) / T05 (faces==3) は引き続き pass。
- **退化幾何の扱い**: 交線円は必ず球中心の上下に分かれる（h_offset>0）ため、`center_z == center.coords.z` は本 Issue では発生しない。接線退化は T07 でカバー済みで cap を生成しない。
- **判定基準の出典**: `classify.rs:113` の `inner_z < center.z` と同一基準で一貫性を保つ。
- **derive 規約・workspace.dependencies**: 変更なし（既存実装内の 1 行置換のみ）。
- **エラーハンドリング**: 既存の `TrimmedFaceUnsupported` 分岐は変更しない。

### 数値モデル
- 比較 `center_z < center.coords.z` は厳密な浮動小数比較。ε 不要（等号は発生しない）。
- ADR-004 準拠: tolerant モード採用済み（本修正は比較方向の変更のみでトレランス値に触れない）。
- 上 cap 修正後の最大頂点 z = `sph_center.z + radius` = +5.0（`evaluate(0, π/2)` の厳密値）。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T08 | 正常系 | example ビルド→tessellate→mesh.positions の z 範囲 | max_z ≈ +5.0 (tol 1e-3), min_z ≈ -5.0 |
| T09 | 正常系 | z>+4.0+ε の頂点が複数存在する（上 cap 三角形の実在） | z>4.0+1e-3 の頂点数 >= 12 |
| T01 | 決定性 | 既存（変更なし） | pass 継続 |
| T05 | faces | 既存（変更なし） | pass 継続 |
| T18 | tess | 既存 triangle_count > 0（変更なし） | pass 継続 |
| T22b | 決定性 | 既存（変更なし） | pass 継続 |
| T34 | 交線 | 既存（変更なし） | pass 継続 |

## 幾何的不変条件チェックリスト
- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか — tessellation のみ修正、B-rep 不変
- N/A flip_normals / same_sense の変更なし（既存コードそのまま）
- N/A pslg_subdivide — 本 Issue と無関係
- [x] watertight: 上 cap が rim(z=+4) から北極(z=+5) まで張られ、開口が塞がること
