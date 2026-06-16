# test-spec — #164 viewer-sketch-canvas

## 不足テスト (plan 計画分)

Plan の T01〜T10 のうち、現状の `web/tests/sketch_canvas.spec.ts` は skeleton (test.skip 6 件、T07-T10 の vitest 単体は未着手)。  
本 STEP 6.6 で GLM が実装する必要があるテスト:

- **Playwright (web/tests/sketch_canvas.spec.ts)** — skeleton から実装に切替えて 6 ケース通す
  - T01 RefPlane unselected btn-start-sketch is no-op
  - T02 four-click open rectangle shows open state
  - T03 fifth click within SNAP_RADIUS closes the loop
  - T04 Esc discards segments back to idle
  - T05_boundary_open_finalize Enter on open returns null (実装で finalize 呼出 hook を expose する必要あり)
  - T06_degen_zero_length zero-length segment is rejected

- **vitest 単体 (web/src/sketch.test.ts)** — T07-T10 を ID 付きで追加
  - T07_unit_determinism: 同一入力列を 2 回 `createSketchSession→addPoint*N` → segments と isClosed() が完全一致
  - T08_unit_snap_tie_break: points=[(0,0),(0,0.05)] に対し (0,0.03) を addPoint → 最早の (0,0) にスナップ
  - T09_unit_isclosed_lt3: segments=2 で最初の点に戻っても isClosed()=false
  - T10_unit_finalize_open: 開ループで finalize() → null

## 実装差分から追加すべきテスト

- **viewer.ts に sketchOverlay の visibility / sketchSegments の addSketchSegment フックが追加された**: visibility が `setSketchMode(true)` で true、`clearSketchOverlay()` で false になることを Playwright で確認。
  - 追加候補: T11_overlay_visible_on_sketch_mode (Playwright、page.evaluate で `__viewer.scene.children` から sketch overlay group を取り、visible を確認)
  - 重要度: medium。設計仕様 (data-state) の検証を Playwright で経由すれば実質的に確認できるため、本 Issue 単独では追加不要 (#166 E2E でカバーする予定)。

- **main.ts に `currentSession.finalize()` を expose する hook が必要**: T05_boundary_open_finalize の検証には `window.__currentSketchSession` のような expose が必要。GLM が main.ts の最後で `(window as any).__sketchDebug = { finalize: () => currentSession?.finalize() ?? null }` のような hook を expose することを test-spec で要請する。

## エッジケース・退化入力

- **複数 RefPlane を切り替えた直後にスケッチ開始**: Front 選択 → スケッチ開始 → Esc → Top 選択 → スケッチ開始 → 新しい session が Top で作られることの確認。
  - 重要度: medium。#165 で extrude 接続時に「どの plane_ref で送信するか」が問題化するため、本 Issue で「セッション作り直し」を検証しておくと将来助かる。追加候補: T12_replanted_session。
  - ただし主要要件ではないので、必須にしない。

## 数値境界

- SNAP_RADIUS = 0.1 (ちょうど境界): (0,0) と (0.1, 0) は snap される?
  - 仕様上「2D 距離 SNAP_RADIUS 以下」なので、距離 ちょうど 0.1 は snap 対象 (`<=`)。
  - 追加候補: T13_unit_snap_boundary。ただし vitest で T07/T08 のバリエーションとして含めれば十分。

- CLOSE_EPS = 1e-9 (ちょうど境界): スナップ後 (0,0) と (1e-9, 0) は閉じたとみなされる?
  - 仕様上「2D 距離 1e-9 以下」だが、SNAP_RADIUS=0.1 で先にスナップされるため事実上 0 距離になる。
  - 重要度: low (実用上発生しない)。**追加不要**と判断。

## 決定性

- T07_unit_determinism で純粋関数の決定性 (snap tie-break 含む) を確認。

## 類似ケース（未カバー）

- 本 Issue は feature 追加なので類似 bug の grep は N/A。

## 期待値乖離

- 現状 sketch.ts 実装と plan の API シグネチャ (`addPoint(x,y)`, `getSegments()`, `isClosed()`, `previewTo(x,y)`, `finalize()`, `reset()`, `SNAP_RADIUS=0.1`, `CLOSE_EPS=1e-9`) は一致。
