# Test Spec — #94 面ピッキング

## 不足テスト（plan 計画分）

| ID | 状態 | 対応 |
|----|------|------|
| T01 | ✅ 実装済み (vitest) | determinism — faceIds/groupFaceIds/groups 構成 3項目 toEqual |
| T02 | ✅ 実装済み (vitest) | face_ids 伝搬・group 全三角形被覆 |
| T03_boundary | ✅ 実装済み (vitest) | indices 空 → group 0個・例外なし |
| T04_degen | ✅ 実装済み (vitest) | 無名面 face_id `""` のグループは materialIndex が 1 にならない |
| E01 | ⚠️ `.fixme` (Playwright) | canvas 中央クリック → selected-face-id 非空 → **GLM が .fixme を外して実装すること** |
| E02 | ⚠️ `.fixme` (Playwright) | 別面クリック → 更新 → **GLM が .fixme を外して実装すること** |
| E03 | ⚠️ `.fixme` (Playwright) | 空白クリック → 解除 → **GLM が .fixme を外して実装すること** |
| E04 | ⚠️ `.fixme` (Playwright) | transient 保証 (GET /api/v0/mesh が不変) → **GLM が .fixme を外して実装すること** |

**Playwright スケルトン位置**: `web/tests/face_picking.spec.ts`
**必要な fixture**: `web/tests/fixtures/simple_box_faces.json`（作成済み。face_ids: 12件, 例: `"N(box_1;face:f_z_neg)"`）

## 実装差分から追加すべきテスト

以下のロジックが `web/src/viewer.ts` に追加されたが plan テスト計画表に未記載:

### A: 右クリックは選択を変更しない (`button !== 0` ガード)
```
pointerup のハンドラ: if (e.button !== 0) return;
→ 右クリックイベントは無視される
```
**追加テスト**: `it("right-click does not trigger selection")` — Playwright で `page.mouse.click(x, y, { button: 'right' })` を実行し、selected-face-id が空のままであることを確認。

### B: ドラッグは選択を変更しない (DRAG_THRESHOLD=5px ガード)
```
if (Math.hypot(e.clientX - downX, e.clientY - downY) > DRAG_THRESHOLD) return;
```
**追加テスト**: `it("drag does not trigger selection")` — 5px 以上マウスを動かしてから pointerup → selected-face-id が空のまま。

### C: `face_ids` 配列の undefined フォールバック
```
const fids = (hits[0].object as Mesh).geometry.userData.faceIds ?? [];
setSelection(fids[hits[0].faceIndex] ?? null);
```
→ faceIds が空配列でも faceIndex が範囲外でも null 選択で非クラッシュ。
T03_boundary (vitest) でグループ 0 個の geometry に対して間接的にカバー済み。Playwright での明示テストは不要（unit level で十分）。

## エッジケース・退化入力

| ケース | 現在の対応 |
|--------|------------|
| face_ids が空の body (no named faces) | T03_boundary で `userData.faceIds = []`、group 0個 → raycaster hit時 `fids[idx] → undefined → null → setSelection(null)` |
| face_id が `""` の面をクリック | T04_degen + `setSelection` 内 `faceId !== ""` ガードで非ハイライト |
| 複数 body (pickMeshes が複数) | plan の設計で pickMeshes 全件を反復 → Playwright E02 で 1 body、複数 body テストはスコープ外 |

## 数値境界

| 値 | 根拠 |
|----|------|
| `DRAG_THRESHOLD = 5px` | pixel 距離で人間のクリックとドラッグを区別する慣習的閾値 |
| `pointer-events: none` (CSS) | ハイライトオーバーレイが canvas クリックを阻害しない |

## 決定性

- T01 (vitest) で `meshToGeometry` の決定性を検証済み。
- `selectedFaceId` はモジュール変数 → 同一 face クリックで同一 face_id 文字列が設定される（EntityRef は安定命名）。

## GLM テスト実装指示（STEP 6.6 向け）

1. `web/tests/face_picking.spec.ts` の E01-E04 すべての `.fixme` を外して実装すること。
2. `setupPickingPage` ヘルパーの型シグネチャを単純化（`Page` 型を直接 import する）。
3. E01: 単純に canvas 中央をクリックし、`data-testid="selected-face-id"` の textContent が非空か確認。
4. E02: E01 後に別座標（例: 上部 20%）をクリック → textContent が length > 0 であれば OK（同面ヒットの場合も許容）。
5. E03: 先に面を選択してから、canvas の端(左上5px)をクリック → textContent が空文字になるか確認（three.js のモデルがない位置）。モデルが端にある場合に E03 が誤 pass しないよう、AxesHelper の領域を避けること。
6. E04: route 記録 + 実際の mock fetch で before/after deep-equal。
7. `web/tests/face_picking.spec.ts` の末尾に追加テスト:
   - `it("right-click does not change selection")` (button: 'right' で mouse.click)
   - `it("drag does not trigger selection")` (mouse.move > 5px から pointerup)

**注意**: Playwright テストは `npm run build && npm run playwright` で実行。`preview` はビルド済み `dist/` を配信するため、事前ビルドが必要。
