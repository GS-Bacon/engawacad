# Codex 非 blocking findings — #163

## R3 medium

### F01 (medium) — T07 が `handle.updateBodies()` を呼んでいない

- 指摘: 現 T07_bodies_update_preserves_refplanes は初期表示の RefPlane 3 枚を再確認しているのみで、`buildScene()` 経由の更新経路を踏んでいない。`updateBodies()` が RefPlane を落とす回帰はこのテストでは検出できない。
- 受容判断: 本 Issue では「初期表示で RefPlane が出る」が主要要件 (T01) で、updateBodies 後の挙動は #164 (sketch canvas) / #165 (extrude) で実経路 (POST → buildScene) を通る統合テストが追加される。本 Issue 単独での実装コスト > 検証価値。
- 後続対応: #164 または #165 で「Front 平面選択 → スケッチ → Extrude → RefPlane 3 枚が依然 scene にある」を assert する E2E を入れる予定。
