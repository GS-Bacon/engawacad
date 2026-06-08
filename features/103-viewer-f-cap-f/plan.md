## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| extrude.ts に planeForFaceNormal() 追加（法線ベクトルから軸判定） | 曲面 (f_cylinder_* 等) の軸判定 |
| main.ts の onSelect で planeForFaceId が null のとき法線ベースにフォールバック | f_cap_* / f_side_* の face の offset 計算（#104 で対応） |
| extrude.test.ts にテスト追加 | カーネル側の face_id 命名変更 |

## Non-Goals
- f_cap_* / f_side_* の offset 計算や正確な押出位置の修正（Issue #104 で対応）
- 曲面・球面の face ID 対応
- カーネル側の face_id フォーマット変更

## 実装対象
- Issue: #103
- 影響ファイル:
  - `web/src/extrude.ts` — planeForFaceNormal() 追加
  - `web/src/main.ts` — onSelect の法線フォールバック追加
  - `web/src/extrude.test.ts` — f_cap_* / f_side_* のテスト追加

**修正箇所 (before/after)**

before: `main.ts` の onSelect
```typescript
function onSelect(faceId: string | null): void {
  panel.style.display = faceId && planeForFaceId(faceId) ? "block" : "none";
}
```

after:
```typescript
function onSelect(faceId: string | null): void {
  if (!faceId) { panel.style.display = "none"; return; }
  const sel = handle.getSelectedFaceVertices();
  const plane = planeForFaceId(faceId) ??
    (sel ? planeForFaceNormal(sel.positions, sel.indices, sel.faceIds, faceId) : null);
  panel.style.display = plane ? "block" : "none";
}
```

## 設計方針
- 選択面の最初の三角形の頂点から外積で法線を計算し、主軸（絶対値最大成分）でスケッチ平面を決定
- 主軸 Z → xy / Y → xz / X → yz（既存 planeForFaceId と同じ対応）
- 零ベクトル（縮退三角形）の場合は null を返す（EPSILON_GUARD: 1e-9 を流用）
- planeForFaceId は削除せず、既知パターンの高速パスとして維持
- 決定性要件: 純粋関数、同一入力→同一出力

### 数値モデル
- ε_cross = 1e-9（法線クロス積の長さが ε 以下なら縮退と判定）

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_normal_cap_z | 正常系 | z 方向法線の三角形から planeForFaceNormal が "xy" を返す | "xy" |
| T02_normal_side_y | 正常系 | y 方向法線の三角形から "xz" を返す | "xz" |
| T03_normal_side_x | 正常系 | x 方向法線の三角形から "yz" を返す | "yz" |
| T04_degen_zero_cross | 縮退 | 縮退三角形（クロス積長 ≤ 1e-9）から planeForFaceNormal が null を返す | null |
| T05_boundary_no_match | 境界 | faceId が存在しない場合 null を返す | null |

## 幾何的不変条件チェックリスト
- N/A
- N/A
- N/A
- N/A
