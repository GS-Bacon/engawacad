# test-spec.md — #95 選択面からの押出(Extrude) UI

## 不足テスト（plan 計画分）

### Playwright E2E（E01〜E04 — 全て `test.skip` 状態）

`web/tests/extrude_panel.spec.ts` の 4 テストは skeleton 段階（`test.skip`）。
GLM-test-implementer が実装すること。

| ID | 内容 | 実装メモ |
|---|---|---|
| E01 | 面クリック → `extrude-panel` visible | `onSelectionChange` が `"block"` にする経路。`planeForFaceId(faceId)` が非 null のときのみ表示 |
| E02 | 背景クリック → パネル非表示 | `onSelectionChange(null)` → `display:none` |
| E03 | 深さ入力 + 押出 → POST `create_sketch` → `extrude` の順、各 200 | `page.route` で intercept し `postedBodies` 配列を確認。順序が `["create_sketch","extrude"]` であること |
| E04 | 押出後シーン差し替え + 選択クリア + パネル非表示 | `handle.updateBodies` 後に `setSelection(null)` → `onSelectionChange(null)` → `display:none` |

**Playwright テスト共通前提:**
- `page.route("/api/v0/mesh", ...)` で `simple_box_faces.json` fixture を返す
- canvas クリックで面が選択される想定（face-picking は #94 で実装済み）
- E03/E04 は `page.route("/api/v0/features", ...)` で 200 + fixture を返す mock が必要

## 実装差分から追加すべきテスト

### `viewer.ts` — ViewerHandle の dispose

`initViewer` が `ViewerHandle.dispose()` を返すようになった。
`dispose()` の呼び出しで `cancelAnimationFrame` と `renderer.dispose()` が正しく呼ばれることを確認する
vitest テストは Three.js 依存のため browser 環境が必要 → E2E か Playwright 経由で確認。

現状 dispose のユニットテストは存在しない → **medium priority**（STEP 6.6 の対象外、フォローアップ）

### `viewer.ts` — updateBodies 後の選択クリア

`updateBodies` が `setSelection(null)` を呼び、`onSelectionChange(null)` → `display:none` になること。
E04 がカバーしているため追加テスト不要。

### `api.ts` — postFeature の HTTP エラー展開

`postFeature` が 4xx/5xx で Error をスローし、`body.error` を message に含めること。
現状ユニットテストなし → vitest で `fetch` をモックしてテスト可能。
**medium priority**（E03 の mock が 200 を返すため E2E カバー不足）

## エッジケース・退化入力（plan 計画外で生じた分岐）

### `buildExtrudeFeatures` — depth ≤ 0 / NaN → null

実装では `if (!Number.isFinite(depth) || depth <= 0) return null;` でガード済み。
vitest テストで以下を追加することを推奨:
- `depth = 0` → null
- `depth = -1` → null
- `depth = NaN` → null
- `depth = Infinity` → null（`Number.isFinite` が false）

### `viewer.ts` — getSelectedFaceVertices: 選択なし → null

`selectedFaceId` が null のとき null を返す。
現在テストなし → vitest でユニットテスト可能だが Three.js 依存のため優先度低。

## 数値境界

- `ε_guard = 1e-9`: T06_boundary で 0-vertex(faceId 不一致)を確認済み。
  追加: extent が ε_guard の境界値（exactly ε_guard → null, ε_guard+ε → non-null）を検証可能。
  **low priority**（実用上 box 面は O(モデル寸法) ≫ ε_guard）

## 決定性

T01 で `buildExtrudeFeatures` の決定性を確認済み。
追加: `planeForFaceId` と `footprintProfile` 単体でも同一入力→同一出力を追加してもよいが、
これらは純粋関数で副作用なしのため決定性は自明 → 省略可。
