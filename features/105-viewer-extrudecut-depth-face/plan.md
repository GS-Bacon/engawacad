## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| buildExtrudeCutFeatures で depth を face-to-canonical-plane 距離でクランプ | カーネル側の coplanar face ロバスト化 |
| faceToCanonicalPlaneDistance() ヘルパー関数追加（web/src/extrude.ts） | ExtrudeCut のエラーメッセージ UI 改善 |
| extrude.test.ts にクランプのテスト追加 | ExtrudeCut offset 修正（#104 スコープ外） |

## Non-Goals
- カーネルの boolean 演算を coplanar face に対してロバストにすること（大きな変更）
- ExtrudeCut のエラーメッセージを UI に表示すること（#106 E2E テスト後に別途）
- depth=0 になる場合の UI フィードバック（クランプで対処）

## 実装対象
- Issue: #105
- 影響ファイル:
  - `web/src/extrude.ts` — faceToCanonicalPlaneDistance() 追加、buildExtrudeCutFeatures 修正

**修正箇所 (before/after)**

before: `buildExtrudeCutFeatures` の先頭バリデーション
```typescript
if (!Number.isFinite(depth) || depth <= 0) return null;
```
after: depth を face-to-canonical-plane 距離でクランプ
```typescript
if (!Number.isFinite(depth) || depth <= 0) return null;
const plane = planeForFaceId(faceId);
if (!plane) return null;
const maxSafeDepth = faceToCanonicalPlaneDistance(positions, indices, faceIds, faceId, plane) - EPSILON_GUARD;
const effectiveDepth = Math.min(depth, maxSafeDepth);
if (effectiveDepth <= 0) return null;
// 以降 depth → effectiveDepth に置換
```

## 設計方針
- `faceToCanonicalPlaneDistance`: face の最初の三角形の代表頂点から、スケッチ平面の正準原点までの法線方向距離
  - yz → |頂点の x 座標|
  - xz → |頂点の y 座標|
  - xy → |頂点の z 座標|
- クランプ後の effectiveDepth ≤ 0 なら null を返す（押出カット不可として UI 何もしない）
- EPSILON_GUARD = 1e-9（既存定数を流用）
- 決定性: 純粋関数、同一入力→同一出力

### 数値モデル
- ε_guard = 1e-9（既存 EPSILON_GUARD を使用、新規 epsilon 追加なし）

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_normal_under | 正常系 | f_x_pos (x=5), depth=4 → クランプなし | effectiveDepth=4 |
| T02_boundary_exact | 境界 | f_x_pos (x=5), depth=5 → クランプされる | effectiveDepth=5-ε |
| T03_boundary_over | 境界 | f_x_pos (x=5), depth=10 → クランプされる | effectiveDepth≈5-ε |
| T04_degen_zero_offset | 縮退 | face が原点上 (x=0, depth=0) → null 返す | null |

## 幾何的不変条件チェックリスト
- N/A
- N/A
- N/A
- N/A
