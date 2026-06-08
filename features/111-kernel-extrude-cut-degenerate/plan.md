## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `extrude.ts` EPSILON_GUARD を 1e-9 → 1e-6 に変更 | E2E アサーション強化 (#114) |
| `classify.rs` 共面判定 `dist < len_eps` → `dist <= len_eps` | proptest 追加 (#112) |
| `topology.rs` `validate_manifold()` に共面重複面チェック追加 | S04 幾何の修正 (#114) |
| 退化 B-rep を再現する受け入れテスト | #110 負側押し出し方向バグ |

## Non-Goals

- S04/extrude_cut_acceptance の幾何修正（#114 で対応）
- proptest 追加（#112 で対応）
- #110 負側押し出し方向バグの修正

## 実装対象

影響ファイル:
- `web/src/extrude.ts` — EPSILON_GUARD (Claude 直接編集)
- `crates/mycad-kernel/src/booleans/classify.rs` — 共面判定 (GLM)
- `crates/mycad-kernel/src/brep/topology.rs` — validate_manifold() (GLM)
- `crates/mycad-kernel/tests/extrude_cut_degenerate_acceptance.rs` — 受け入れテスト (STEP 5.5 skeleton)

### 修正1: EPSILON_GUARD (extrude.ts)

```typescript
// Before (web/src/extrude.ts:8)
const EPSILON_GUARD = 1e-9;

// After
const EPSILON_GUARD = 1e-6;
```

理由: kernel の LENGTH_TOLERANCE = 1e-9 と同値では clearance にならない。1e-6 はカーネルトレランスより 1000× 大きく、浮動小数点演算誤差に対して十分なマージンを確保する。

### 修正2: classify.rs 共面判定 (GLM)

```rust
// Before (crates/mycad-kernel/src/booleans/classify.rs:73)
if dist < len_eps {

// After
if dist <= len_eps {
```

理由: dist = 1e-9 のとき strict `<` で境界ケースを素通りする。`<=` に変更して境界値も共面として扱う。

### 修正3: validate_manifold() 共面重複面チェック (GLM)

`crates/mycad-kernel/src/brep/topology.rs` の `validate_manifold()` に追加:

```rust
// 新規追加: 共面重複面チェック
// 各面ペアについて、法線が平行かつ平面間距離 <= len_eps かつ 2D オーバーラップがある場合 Err
```

実装は GLM が行う。`geometry/surface.rs` の `Surface::Plane` から法線・原点を取得し、
`classify.rs` の `polygons_have_2d_overlap` を再利用する。

## 設計方針

- EPSILON_GUARD 変更は純粋な値変更のみ。UI 動作への影響: depth clamp が 1e-6 広がるだけで実質無視できる範囲
- classify.rs の変更は `<=` のみ。既存テストへの影響は境界値テストのみ
- validate_manifold() の新チェックは `O(F²)` (面数二乗)。通常の CAD モデルでは面数 < 1000 なので許容範囲内

### 数値モデル

- EPSILON_GUARD = 1e-6 (viewer 側 clearance)
- LENGTH_TOLERANCE = 1e-9 (kernel 内部)
- 1e-6 >> 1e-9: clearance は トレランスの 1000× → 浮動小数点演算誤差 (eps ≈ 2e-16 × 値) に対して 6 桁のマージン

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T01 | 正常系 | 10×20×30 box に depth=4.999999 extrude_cut → validate_manifold() OK | Ok(()) |
| T01_degen_boundary | 境界 | depth=face_dist-1e-10 (旧 EPSILON_GUARD 誤差圏) でも OK | Ok(()) (EPSILON_GUARD 1e-6 で回避) |
| T02 | 正常系 | classify_fragment が dist==len_eps で SharedOppositeDirection を返す | 共面として分類 |
| T02_boundary_degen | 境界 | dist = len_eps + 1e-15 (境界より僅かに大きい) は共面扱いにならない | OutsideOther / InsideOther |
| T03 | 正常系 | validate_manifold() が 2 面共面重複を検出して Err | Err(ManifoldViolation) |
| T03_boundary_degen | 境界 | 共面ただし 2D オーバーラップなし → Ok | Ok(()) |
| T04 | 決定性 | 同一 box に同一 extrude_cut を 2 回 → 結果が toEqual | 決定的 |

## 幾何的不変条件チェックリスト

- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか — 変更なし
- [x] 各プリミティブの face ごとの outer_loop 2D 向き — 変更なし
- [x] flip_normals / same_sense の意味論 — 変更なし
- [x] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか — 変更なし
