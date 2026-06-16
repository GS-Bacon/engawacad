# Codex 非 blocking findings — #164

## R6 medium

### F01 (medium) — RefPlane mesh 境界外でクリックが no-op

- 指摘: viewer.ts の pointerup は `raycaster.intersectObject(plane)` で 20×20 メッシュに当たった場合のみ受け付ける。一方、main.ts の pointermove preview は無限平面として投影。結果、可視メッシュ外で灰色 preview が出ても click は無効になり、RefPlane の 20×20 境界を越える輪郭が描けない。
- 受容判断: Phase 7 の前提として「正準平面の ±10 (RefPlane 20×20) 内でスケッチを完成させる」のがスコープ。ADR-010 にも明示なし。本 Issue 単独では境界外での描画は仕様外。
- 後続対応: #165/#166 でユーザビリティが問題化したら、preview を viewer.ts に閉じ込めて click/preview 共通投影 helper にする (Codex suggestion)。または「無限平面で投影 → 20×20 内に clamp」方針も検討余地。

### F02 (medium) — SNAP_RADIUS の境界閾値と tie-break の単体検証が不足

- 指摘: E2E T03 / T11 は 5 点目を始点と同じ画面座標でクリックしているので「SNAP_RADIUS 内の別座標からの snap」を経由していない。unit T08 も等距離 tie-break ケースが不在で、`bestIdx === -1 ? d2 <= bestDist2 : d2 < bestDist2` の earliest 優先分岐が壊れても検出されない。
- 受容判断: T08 を等距離シナリオに戻すと「ロジック上の premature close と区別がつかない」現象 (Codex r5 F01) と衝突する。tie-break の真の検証は public API だけでは難しく、内部 hook expose は overdesign。
- 後続対応: viewer.ts に投影 helper を公開してから (#165 の文脈で) tie-break テストを再設計する。
