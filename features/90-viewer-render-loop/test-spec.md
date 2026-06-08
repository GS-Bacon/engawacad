# test-spec: #90 viewer-render-loop

## 不足テスト（plan 計画分）

plan の T05/T06 はすでに viewer.spec.ts に実装済み（GLM が skeleton をそのまま確認）。追加実装不要。

## 実装差分から追加すべきテスト

なし。変更は:
1. `controls.dampingFactor = 0.1` — 値を変えた場合に T06 が落ちるため間接的にカバー
2. `window.__viewer` 露出 — T05/T06 が依存するため間接的にカバー

## エッジケース・退化入力

- `window.__viewer` 未設定時: optional chaining `?.` により `undefined` を返し例外なし（T05/T06のコード自体がこの保護を実装済み）

## 数値境界

- `dampingFactor = 0.1`: Three.js のデフォルト(0.05)〜1.0 の範囲内。境界テスト不要（UI 設定値）

## 決定性

レンダリング設定変更のみ。決定性テスト対象外。

## 結論

T05/T06 が実装済みのため STEP 6.6 GLM テスト追加実装は不要。state shim で passed にセット。
