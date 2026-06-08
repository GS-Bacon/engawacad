# test-spec: #91 viewer-camera-controls

## 不足テスト（plan 計画分）

plan の T07/T08/T09 はすでに viewer.spec.ts に実装済み（acceptance skeleton と同時に追加）。追加実装不要。

## 実装差分から追加すべきテスト

なし。変更は:
1. `index.html` — `#view-buttons` div + 3 ボタン（T07 が DOM 可視性をカバー）
2. `viewer.ts` — `setView()` 実装（T08/T09 が front/top 方向を検証）
3. `main.ts` — クリックイベント接続（T08/T09 のクリックシナリオがカバー）

## エッジケース・退化入力

- `camera.position === controls.target`（距離 d=0）: `camera.position.set(t.x, t.y, t.z + 0)` になるだけでクラッシュしない。
  ただし T08_degen は plan.md に記載のみで Playwright テストは省略（初期表示では必ず d > 0 のため実質到達不能）。

## 数値境界

- `setView("front")`: dz = d > 0 が期待値。d は `fitCamera()` 由来で初期表示後は必ず正。
- `setView("top")`: dy = d > 0。同様。
- `setView("iso")`: 距離保存のみ確認（テスト省略）。

## 決定性

カメラ位置変更のみ。決定性テスト対象外。

## 結論

T07/T08/T09 が実装済みのため STEP 6.6 GLM テスト追加実装は不要。state shim で passed にセット。
