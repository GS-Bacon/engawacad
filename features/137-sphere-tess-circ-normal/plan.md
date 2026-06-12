## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `tessellate_sphere_face_trimmed` (mod.rs:967-1110) を `circ_normal` ベースの球ローカル軸計算に書き換える | `Surface::Sphere` enum への axis フィールド追加 (球は回転対称、評価関数の引数 v を「軸方向の緯度」と再解釈すれば十分) |
| `face.surface.evaluate(u, v)` を本関数内で使用せず、circ_normal 軸を極とする UV 計算をインライン化 | 他の sphere 関連関数 (full sphere tessellation、primitive sphere) の変更 |
| `rel_z.clamp(-1.0, 1.0)` の暗黙吸収を `signed_offset` バリデーション + `InvalidTrimCircle` エラー化 | `surface_intersect` の MVP 制約 (plane×sphere/cyl×sphere 軸 ±Z のみ) を解除すること |
| 境界リング再サンプリングの隣接面整合を直接比較する acceptance テスト追加 (#129-F02 同種指摘) | upstream の Boolean partition.rs の交線生成変更 (ADR-009 実装本体マター) |
| 接円 (tangent) と回転済み球ディンプル (任意軸 circ_normal) の acceptance テスト追加 | 完全な「sphere の任意回転」サポート (現状は circ_normal が global Z 以外でも tessellation が正しいことを保証するだけ) |
| 上流制約コメント (現状の `surface_intersect` MVP は plane×sphere/cyl×sphere とも軸 ±Z のみ) を関数冒頭に明記 | Phase 5 の rotation 配線 (#135 で対応済み) |

## Non-Goals

- `Surface::Sphere` enum 構造の変更 (axis フィールド追加等)
- 他の sphere tessellation 関数 (full sphere、untrimmed) の変更
- `tessellate_sphere_face_trimmed` が扱う inner_loop 数の上限変更 (現状の 1 inner_loop 制約を維持)
- 上流の MVP 制約 (surface_intersect の軸 ±Z 限定) の解除
- ADR-009 案 A 実装本体 (交線エッジを周期円として保持する) — 本 Issue とは独立に Phase 4 再訪時に処理
- 共有境界比較テストの境界状態超強化 (#144 が別 Issue で扱う、本 Issue では「sphere の trim 境界のみ」をテスト)

## 実装対象

**Issue**: #137
**影響範囲**:
- 修正: `crates/mycad-kernel/src/tessellation/mod.rs` の `tessellate_sphere_face_trimmed` (lines 967-1110)
- 追加 (errors): `crates/mycad-kernel/src/tessellation/mod.rs` の `TessellationError` enum に `InvalidTrimCircle` variant
- 追加: `crates/mycad-kernel/tests/trim_sphere_circ_normal_acceptance.rs` (新規 acceptance テスト)

### `tessellate_sphere_face_trimmed` の主要変更 (before / after)

#### 1. 軸とトリム方向の導出

**before** (line 1007-1018):
```rust
// The circle center z gives us the latitude
let center_z = circ_center.coords.z;
// v_lat = asin((center_z - sph_center_z) / R)
let rel_z = (center_z - center.coords.z) / radius;
let rel_z = rel_z.clamp(-1.0, 1.0);
let v_lat = rel_z.asin();

// Both intersection circles share normal +Z, so circ_normal cannot tell ...
let trim_lower = center_z < center.coords.z;
```

**after**:
```rust
// 球ローカル軸 = circ_normal 方向。球は回転対称なので circ_normal を
// 「極軸」として扱い、v_lat = この軸方向の緯度として再解釈する。
let axis = circ_normal.normalize();

// circ_center から sph_center へのベクトルの axis 方向成分が signed_offset。
// |signed_offset| = sph_radius * sin(v_lat)
let signed_offset = (circ_center - center).coords.dot(&axis);

// axis 長 ≈ 0 (退化した circ_normal) → release build でも validation エラー化
// (debug_assert! は release で消えて NaN が下流に伝搬するため不可、INVARIANT IN01)
if !axis.norm().is_finite() || axis.norm() <= LENGTH_TOLERANCE {
    return Err(TessellationError::InvalidTrimCircle {
        signed_offset: f64::NAN,
        sphere_radius: radius,
    });
}

// 球外円は退化吸収せず InvalidTrimCircle としてエラー化する。
// 接円 (|signed_offset| ≈ sph_radius) は許容、それを超える場合は逸脱。
if signed_offset.abs() > radius + LENGTH_TOLERANCE {
    return Err(TessellationError::InvalidTrimCircle {
        signed_offset,
        sphere_radius: radius,
    });
}
// 上の validation で |signed_offset| <= radius + ε まで絞ったため、
// clamp は接円 (|signed_offset| = radius) 近傍の浮動小数誤差で asin が
// NaN を返すのを防ぐ「丸め誤差吸収のみ」の役割 (INVARIANT IN02)
let rel_axis = (signed_offset / radius).clamp(-1.0, 1.0);
let v_lat = rel_axis.asin();

// trim_lower (axis 基準): cut 円が axis 負側 → axis 負側の極 (-axis) のキャップを生成
let trim_lower = signed_offset < 0.0;
```

#### 2. UV-grid 評価を axis ベースにインライン化

**before** (line 1041-1059):
```rust
for iu in 0..n_u {
    let u = du * iu as f64;
    let p = face.surface.evaluate(u, v_boundary);    // ← グローバル Z 仮定
    let normal = face.surface.normal_at(u, v_boundary);
    mesh.positions.push([p.x, p.y, p.z]);
    mesh.normals.push(normal_arr(&normal, face.same_sense));
}
// ... (内部リングも同様に face.surface.evaluate)
```

**after**:
```rust
let (bu, bv) = orthonormal_basis(&axis);
// 球面 (u, v) 評価: sph_center + R * (sin(v) * axis + cos(v) * (cos(u) * bu + sin(u) * bv))
let eval_axis = |u: f64, v: f64| -> Point {
    let radial = u.cos() * bu + u.sin() * bv;
    center + radius * (v.sin() * axis + v.cos() * radial)
};
// 法線: 球面外向きは (point - sph_center) / R
let normal_axis = |u: f64, v: f64| -> Vec3 {
    ((eval_axis(u, v) - center) / radius).normalize()
};

for iu in 0..n_u {
    let u = du * iu as f64;
    let p = eval_axis(u, v_boundary);
    let n = normal_axis(u, v_boundary);
    mesh.positions.push([p.x, p.y, p.z]);
    mesh.normals.push(normal_arr(&n, face.same_sense));
}
// 内部リングも同様に eval_axis / normal_axis を使用
```

#### 3. 極の計算

**before**:
```rust
let pole_p = face.surface.evaluate(0.0, pole_v);
let pole_normal = face.surface.normal_at(0.0, pole_v);
```

**after**:
```rust
// pole_v は ±π/2、eval_axis(0, ±π/2) = center ± R * axis
let pole_p = eval_axis(0.0, pole_v);
let pole_normal = normal_axis(0.0, pole_v);
```

#### 4. KernelError 追加

`TessellationError` enum に新 variant 追加:
```rust
#[error("trim circle outside sphere surface: signed_offset={signed_offset}, radius={sphere_radius}")]
InvalidTrimCircle { signed_offset: f64, sphere_radius: f64 },
```

#### 5. 上流制約コメント

関数冒頭 (line 967 直後) に追記:
```rust
// 上流制約 (2026-06 時点): surface_intersect の MVP では plane×sphere /
// cyl×sphere とも軸 ±Z のみをサポートする。すなわち本関数が受け取る
// circ_normal は実際上 ±Z しか到達しない。ただし将来 (rotation 配線 #135 や
// 任意軸 surface_intersect 拡張) で他の circ_normal が来ても、本実装は
// circ_normal を局所軸として正しく動作する。
```

## 設計方針

- **決定性**: `orthonormal_basis(axis)` は決定的 (math.rs:50-60、入力軸が同じなら同じ basis)。`signed_offset` 計算と asin/sin/cos は決定的浮動小数演算。同じ入力 → 同じ出力。
- **B-rep トポロジー妥当性**: 本関数は B-rep を変更しない。`TriangleMesh` を生成するのみ。`validate_manifold` への影響なし。Euler-Poincaré (V-E+F=2S) は本関数の責務外。
- **退化幾何の扱い**:
  - `|signed_offset| > sph_radius + LENGTH_TOLERANCE` → 球外円 → `InvalidTrimCircle` エラー化 (退化吸収せず)
  - `|signed_offset| ≈ sph_radius` (接円) → `clamp(-1.0, 1.0)` で安全クランプ後 v_lat = ±π/2、極のみで構成される退化メッシュ。production code path で到達しない想定だが test で挙動確定
  - `axis` ベクトル長 ≈ 0 (退化した circ_normal) → release build でも有効な validation で `InvalidTrimCircle` エラー化 (INVARIANT IN01)。`axis.norm() <= LENGTH_TOLERANCE` または `axis.norm().is_finite() == false` を判定基準とする。debug_assert! の defensive check は採用しない (release で NaN 伝搬する決定性リスクを避ける)。
- **derive 規約**: `TessellationError` enum は既存の Debug/Clone/Display/Error を踏襲。新 variant `InvalidTrimCircle` の追加のみ。
- **エラーハンドリング**: 既存の `Result<(), TessellationError>` シグネチャを維持。新 variant 追加で thiserror 規約に従う。
- **workspace.dependencies 規約**: 新規依存追加なし。`nalgebra::Vector3` の `dot/normalize` を流用。
- **既存 `face.surface.evaluate/normal_at` 呼び出しを撤去**: 本関数内に閉じた変更。他関数 (`tessellate_sphere_face_full` 等) は touched しない。

### 数値モデル (Phase 4 / 6+ で必須)

- **tolerance**: `LENGTH_TOLERANCE = 1e-9` (`geometry::math`) を流用。`|signed_offset| > radius + LENGTH_TOLERANCE` の比較に使用。新 ε は追加しない。
- **退化判定基準**: `signed_offset.abs() > radius + LENGTH_TOLERANCE` → `InvalidTrimCircle` エラー。境界 `signed_offset == ±radius` は接円として許容 (v_lat = ±π/2)。
- **角度方向 ε**: 別途 ANGLE_EPS は導入しない (NUMERIC NU01)。極近傍 (v_lat ≈ ±π/2) の数値不安定性は `signed_offset` ベース validation で上流から制限されるため実害なし。asin の結果は clamp で domain 内に保証されているので NaN が発生しない。将来「極近傍の grid 生成スキップ」等が必要になれば別 Issue で `ANGLE_TOLERANCE` 流用を検討。
- **ADR-004 準拠方針**: per-entity tolerance 適用は将来 Issue (Phase 5 候補で `Face.tolerance` field 追加時)。本 Issue ではグローバル `LENGTH_TOLERANCE` 流用。tolerant モデルとの整合は変更なし。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 既存の boolean_cut_sphere_dimple (cyl×sphere) を 2 回 tessellate し、メッシュが完全一致 (positions / indices / normals) | assert_eq! |
| T02 | 正常系 (回帰) | 既存の boolean_cut_sphere_dimple (cyl軸 +Z) で改修後も watertight・naked_edge 0 | validate_manifold 成功、naked_edge カウント == 0 |
| T03 | 正常系 (新機能) | 任意軸 circ_normal を持つ trim circle で tessellate。Face を直接構築: sphere + 任意軸 (+X 方向) を normal とする circle を inner_loop に。eval_axis 由来の頂点が球面上にあることを `\|point - center\| == radius` で検証 | watertight、極が circ_normal 方向、メッシュ頂点が球面上 |
| T_boundary_tangent_circle | 境界 | `signed_offset == sph_radius` (接円、circ_radius ≈ 0)。v_lat = ±π/2 になり境界ring が極点に縮退する。実装方針: 境界 ring を生成せず極のみ生成 (or 全 boundary 頂点が極の縮退三角形になる) いずれかを採用し test で固定する (AMBIG AM01) | (a) v_lat = ±π/2 で境界 ring と極が同一点に縮退 → 結果メッシュは三角形 0 件、 (b) または `TrimmedFaceUnsupported` で reject。GLM 実装でどちらを採用したかを test 関数名で明示する |
| T_boundary_great_circle | 境界 | `signed_offset == 0` (大円、v_lat == 0)。半球のメッシュ生成 | 半球メッシュ、極が片側 (axis 基準) |
| T_degen_out_of_sphere | 退化 (エラー化) | `signed_offset > sph_radius + LENGTH_TOLERANCE` (球外円) を渡すと `InvalidTrimCircle` を返す | `Err(TessellationError::InvalidTrimCircle { .. })` |
| T_degen_zero_axis | 退化 (エラー化) | `circ_normal = Vec3::zeros()` を含む退化 inner_loop を渡すと `InvalidTrimCircle` を返す (release/debug 共通、IN01 採用) | `Err(TessellationError::InvalidTrimCircle { .. })` |
| T04_shared_boundary | 共有境界整合 (#129-F02 同種) | `boolean_cut_sphere_dimple` 結果の Solid 内で、TrimmedSphereFace の inner_loop と CylindricalLateralFace の outer_loop が共有する HalfEdge ペアを `solid.half_edges` の `twin` index 経由で特定する。両側の境界 ring 頂点 (それぞれ tessellate_sphere_face_trimmed と tessellate_cyl_lateral が生成した頂点列) を index 順に比較し全頂点で `LENGTH_TOLERANCE` 以内であることを確認。隣接面の特定方法: `inner_loop.half_edges[k].twin` から対面 Face と対応 HalfEdge を辿る (AMBIG AM01 round 2) | assert_eq! per vertex、index 順差なし |

## 幾何的不変条件チェックリスト

- partition 出力との整合: **N/A** (本関数は tessellation 側、partition 出力は変更しない)
- プリミティブ face outer_loop 2D 向き: **N/A** (本関数は trimmed sphere face、プリミティブではない)
- flip_normals / same_sense の意味論: 既存 `face.same_sense` 使用パターンを維持 (eval_axis の normal は外向き、normal_arr で same_sense に応じて符号反転)
- pslg_subdivide 出向き整合: **N/A** (本関数は subdivision を行わない)

## /3ai フロー方針 (本 Issue 限定)

- **STEP 3 (GLM 多ペルソナ設計レビュー)**: **保持** (Phase 4 関連、`### 数値モデル` あり → SCOPE / INVARIANT / AMBIG / NUMERIC の 4 ペルソナ)
- **STEP 5 (branch)**: ユーザー memo `feedback_workflow.md` に従い main へ直 push、branch 不要
- **STEP 5.5 (Acceptance Skeleton)**: 保持。T01-T04 / T_boundary_* / T_degen_* を `tests/trim_sphere_circ_normal_acceptance.rs` に skeleton 化
- **STEP 6 (GLM コア実装)**: 保持。`crates/**` の `tessellate_sphere_face_trimmed` 修正と新 variant 追加は GLM dispatch 経由
- **STEP 6.6 (GLM テスト実装)**: 保持
- **STEP 7 (GLM 最終レビュー)**: 保持
- **STEP 7.5 (Codex 独立技術ゲート)**: **保持** (幾何不変量リスクが高いため、別モデル系の確認必須)
- **STEP 8**: main へ direct push

## 検証

- 既存 sphere trim テスト (`trim_surface_tessellation_acceptance.rs` 等) が引き続き green
- 追加した T01-T04 / T_boundary_* / T_degen_* テスト green
- `cargo xtask ci` で Playwright 14 件失敗 (#145 既知 infrastructure 問題) を除いて green
- `cargo test --workspace` 単体実行で green (回帰なし)
- 接円・回転済み球ディンプルのメッシュ目視 (任意): viewer 用 STL export で確認
