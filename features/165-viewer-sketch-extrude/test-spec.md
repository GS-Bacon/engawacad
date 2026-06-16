# test-spec — #165 viewer-sketch-extrude

## 不足テスト (plan 計画分)

Plan の T01〜T08 のうち、現状実装済み:
- Playwright sketch_extrude.spec.ts: T01 (panel hidden) / T02 (CreateSketch+Extrude) / T03 (CreateSketch+ExtrudeCut) / T04_boundary_depth_guard (4 ガード)
- vitest sketch.test.ts: T06_unit_buildCreateSketch (構造 + deterministic seg id) / T07_unit_plane_mapping (Front/Top/Right→xy/xz/yz)

未実装:
- **T08_unit_id_determinism (nextId 決定性)**: GLM が test を追加していない。本 STEP 6.6 で `extrude.ts` の `nextId(prefix, used)` を 2 回呼んで同じ ID 列が返ることを assert。

## 実装差分から追加すべきテスト

- **Enter で finalize=null のとき currentSession を維持して継続描画可能** — sketch_canvas T05 で担保済。
- **連続スケッチ操作**: 1 回目 Extrude → 2 回目別 RefPlane でスケッチ → Extrude → 各々独立した CreateSketch/Extrude POST が発火する。中間で usedFeatureIds が monotonically 増える。
  - 追加候補: T09_consecutive_extrudes。重要度: medium、本 Issue 単独では必須ではない (#166 E2E で網羅予定)。

## エッジケース・退化入力

- **三角形 (3 セグメント) で Extrude**: T02 (4 セグメント矩形) と同様に動く想定。本 Issue では矩形ケースで担保し、明示的テスト不要。

## 数値境界

- depth ガード (T04_boundary_depth_guard) で 0 / 負値 / Infinity をカバー済。`Number.isFinite` の振る舞いに依存。

## 決定性

- T06_unit_buildCreateSketch で deterministic segment ID を確認済。
- T08_unit_id_determinism を **追加要請**: nextId 戦略の決定性を 2 回 call で確認。

## 類似ケース（未カバー）

- 本 Issue は feature 追加なので類似 bug の grep は N/A。

## 期待値乖離

- なし。plan の API シグネチャと実装は一致 (buildCreateSketchFromSketch / nextId / Feature 型)。
