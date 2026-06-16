## 自律判断ログ (自律バッチモード)

- Issue body は機械検証可能な完了条件・固定値 (SNAP_RADIUS=0.1 / 閉ループ 1e-9 / data-state 列挙 / DOM testid / API シグネチャ) まで明記。intent-check `aligned: yes`。
- 担当ファイルは `web/` 配下のみ (sketch.ts 新規 / viewer.ts / main.ts / index.html / 新規 spec)。Rust 側は触らない。
- #163 マージ済 (RefPlane 描画 + ViewerHandle.onRefPlaneSelected が存在)。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `web/src/sketch.ts` 新規: `Sketch` / `SketchSegment` 型と `createSketchSession(planeRefId)` factory | Extrude / ExtrudeCut への接続 (#165) |
| `SketchSession` API: `addPoint(x,y)` / `getSegments()` / `isClosed()` / `previewTo(x,y)` / `finalize()` / `reset()` | 円・円弧・スプライン (ADR-010 §Out of scope) |
| 端点スナップ `SNAP_RADIUS = 0.1`、既存全頂点に対する 2D 距離判定 | 中点 / 中心 / 延長線 / グリッドスナップ |
| 閉ループ判定: 最後の to と最初の from が 2D 距離 1e-9 以下 | 幾何拘束ソルバ |
| `data-testid="sketch-canvas"` + `data-state="idle"/"open"/"closed"` 切替、`sketch-canvas--open` で赤枠 | スケッチ寸法表示 |
| ラバーバンド: 最後の確定点 → 現マウス位置の preview 線 (Three.js Line) | 履歴編集 / undo-redo |
| 確定: 閉ループ自動 finalize / Enter で明示 finalize / Esc で全破棄 | モデル面 / 任意平面の上での描画 (Phase 8) |
| スケッチモード中は OrbitControls 無効化 (`controls.enabled = false`) | バックエンド変更 (engawa-format / build / kernel) |
| `btn-start-sketch` ボタン + RefPlane 選択ガード | RefPlane offset UI 露出 |
| Playwright `sketch_canvas.spec.ts` (5 ケース) | sketch.ts と Extrude UI の統合 (E2E は #166) |

## Non-Goals

- Extrude/ExtrudeCut UI: #165
- 円弧・スプライン: ADR-010 で Phase 7 外
- グリッド・中点等のスナップ: ADR-010 で「端点スナップ以外は後回し」
- スケッチの後追い編集 / undo-redo: 将来 Phase
- バックエンド (Rust) への変更

## 実装対象

Issue: #164
影響ファイル:
- `web/src/sketch.ts` (新規)
- `web/src/viewer.ts` (改修)
- `web/src/main.ts` (改修)
- `web/index.html` (要素追加 + style)
- `web/tests/sketch_canvas.spec.ts` (新規)

### 1. `web/src/sketch.ts` (新規)

```typescript
export type RefPlaneId = "Front" | "Top" | "Right";

export interface SketchSegment {
  from: { x: number; y: number };
  to: { x: number; y: number };
}

export interface Sketch {
  planeRefId: RefPlaneId;
  segments: SketchSegment[];
}

export interface SketchSession {
  addPoint(x: number, y: number): void;
  getSegments(): SketchSegment[];
  isClosed(): boolean;
  previewTo(x: number, y: number): { from: { x: number; y: number }; to: { x: number; y: number } } | null;
  finalize(): Sketch | null;
  reset(): void;
}

export const SNAP_RADIUS = 0.1;
export const CLOSE_EPS = 1e-9;

export function createSketchSession(planeRefId: RefPlaneId): SketchSession;
```

実装ロジック:
- セッション内部 state: `points: {x,y}[]` (確定済 vertex 列)、`segments: SketchSegment[]`
- `addPoint(x, y)`:
  1. `snap`: 既存 `points` のうち 2D 距離が SNAP_RADIUS 以下のものがあれば、最近接の既存頂点座標に置換 (距離同値の場合は `points` 配列の先頭側 = 最早の頂点を優先 → tie-break 決定性を担保)
  2. `points.length === 0` のとき: `points.push(snapped)`、`segments` は変化なし
  3. それ以外: `last = points[points.length - 1]`、`snapped == last` (ゼロ長) なら no-op、それ以外は `segments.push({ from: last, to: snapped })` し、`snapped === points[0]` (= 最初の点に戻った = 閉じた) なら `points` には push せず終了、そうでなければ `points.push(snapped)`
- `isClosed()`: `segments.length >= 3` かつ最後の `segments[last].to` と `segments[0].from` の 2D 距離が `CLOSE_EPS` 以下
- `previewTo(x, y)`: `points.length === 0` → null、それ以外 → `{ from: points[last], to: { x, y } }`
- `finalize()`: `isClosed()` なら `{ planeRefId, segments: [...segments] }` を返す。開ループは null。
- `reset()`: state を初期化

### 2. `web/src/viewer.ts` 改修

- 既存 ViewerHandle に追加:
  - `setSketchMode(active: boolean): void`
  - `setSketchPreview(from: {x,y,z} | null, to: {x,y,z} | null): void`
  - `addSketchSegment(from: {x,y,z}, to: {x,y,z}): void`
  - `clearSketchOverlay(): void`
- `initViewer` の `opts` に追加:
  - `onSketchPoint?: (u: number, v: number) => void` — スケッチモード中の click で 2D 座標を通知
- スケッチモード中:
  - `controls.enabled = false`
  - スケッチオーバーレイ用 `Group` (`sketchOverlay`) を scene に add
  - 確定済セグメントは個別 `Line` で描画 (青 `0x2080ff`)
  - preview は別 `Line` (薄い灰色 `0x808080`)、null で hide
- raycaster ハンドラ: スケッチモード中は通常の face/RefPlane pick を bypass し、選択中 RefPlane の平面 (法線 + 通過点 (0,0,0)) と ray の交点を計算、2D 座標 (u, v) に変換して `opts.onSketchPoint?.(u, v)` を呼ぶ
- 2D ↔ 3D 変換 (RefPlane ごと、Three.js は右手系):
  - Front (XY plane, normal +Z): (u, v) → world (u, v, 0)
  - Top (XZ plane, normal +Y): (u, v) → world (u, 0, v)
  - Right (YZ plane, normal +X): (u, v) → world (0, u, v)

### 3. `web/src/main.ts` 改修

- `btn-start-sketch` クリック handler:
  - `selectedRefPlaneId` が null なら何もしない (button は visible のまま、押しても no-op)
  - 非 null なら `setSketchMode(true)` + `currentSession = createSketchSession(id)` + sketch-canvas data-state = "idle"
- viewer の `onSketchPoint` callback:
  - `currentSession.addPoint(u, v)`
  - 確定したセグメントを viewer.addSketchSegment 経由で描画
  - `currentSession.isClosed()` なら data-state = "closed"、`sketch-canvas--open` class を外す
  - 開いていれば data-state = "open"、`sketch-canvas--open` class を付ける
  - segments=0 のときは data-state = "idle"
- マウス移動 (canvas pointermove): `currentSession.previewTo(u, v)` → viewer.setSketchPreview
  (RefPlane 投影は viewer 側で行うので、main は座標を流すだけ)
- キーイベント:
  - Enter: `currentSession.finalize()` → 閉なら console.log(sketch) (Phase 7 では先送り)、開なら何もしない
  - Esc: `currentSession.reset()` + viewer.clearSketchOverlay() + setSketchMode(false) + data-state = "idle"

### 4. `web/index.html` 改修

- 追加要素:
  - `<button data-testid="btn-start-sketch" id="btn-start-sketch">スケッチ開始</button>` を `#extrude-panel` の上、もしくは `#view-buttons` 横に追加
  - `<div data-testid="sketch-canvas" id="sketch-canvas" data-state="idle"></div>` を `#app` 内
- CSS:
  ```css
  #btn-start-sketch {
    position: absolute;
    top: 1rem;
    right: 9rem;
    background: rgba(0,0,0,0.5); color: white; border: none;
    padding: 0.4rem 0.6rem; border-radius: 4px;
    font-family: monospace; font-size: 12px; cursor: pointer;
    z-index: 10;
  }
  #sketch-canvas {
    position: absolute; inset: 0; pointer-events: none; z-index: 9;
  }
  #sketch-canvas[data-state="idle"] { display: none; }
  #sketch-canvas[data-state="open"], #sketch-canvas[data-state="closed"] { display: block; }
  #sketch-canvas.sketch-canvas--open { outline: 3px solid #e74c3c; outline-offset: -3px; }
  ```

### 数値モデル

- `SNAP_RADIUS = 0.1` (UI 物理単位 = カメラスケール)
- `CLOSE_EPS = 1e-9` (バックエンド `validate_profile_closed` と同値で先回り判定)
- ゼロ長セグメント (`snapped === last`) は addPoint で棄却 (segments に積まない)
- ADR-004 準拠: UI レイヤは tolerant、kernel 側は exact。本 Issue では kernel に渡さないため互換のみ意識。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | RefPlane ガード | RefPlane 未選択時に `btn-start-sketch` クリック → sketch-canvas data-state は "idle" のまま | `data-state === "idle"` |
| T02 | 開ループ表示 | RefPlane 選択 → start-sketch → 矩形 4 点クリック (0,0)(5,0)(5,5)(0,5) | `data-state="open"` + `sketch-canvas--open` class |
| T03 | 閉ループ + SNAP | T02 の後、5 点目を (0.05, 0.05) でクリック → 最初の点にスナップ | `data-state="closed"`、`sketch-canvas--open` 外れる |
| T04 | Esc 破棄 | T02 状態で Esc | `data-state="idle"`、segments=0、OrbitControls 再有効 |
| T05_boundary_open_finalize | 退化/境界 | T02 (open) 状態で Enter | finalize は null を返す、`data-state="open"` のまま |
| T06_degen_zero_length | 退化 | 1 点目クリック後、同じ位置 (距離 < SNAP_RADIUS) を再度クリック | segments=0 のまま (ゼロ長棄却) |
| T07_unit_determinism | 単体 (vitest) | 同一入力列を 2 回 `createSketchSession→addPoint*N` → segments と isClosed() が完全一致 | 決定性 assert |
| T08_unit_snap_tie_break | 単体 (vitest) | 既存 points=[(0,0),(0,0.05)] に対し (0,0.03) を addPoint → 0,0 にスナップ (= 最早の頂点) | tie-break ルール検証 |
| T09_unit_isclosed_lt3 | 単体 (vitest) | segments=2 (三角形未満) で最初の点に戻っても isClosed()=false | 3 セグメント未満は閉と認めない |
| T10_unit_finalize_open | 単体 (vitest) | 開ループで finalize() → null | finalize null 確認 |


## 幾何的不変条件チェックリスト

- [ ] N/A — partition/assemble 系ではない (UI のみ)
- [ ] N/A
- [ ] N/A
- [ ] N/A

## 影響範囲

- `web/src/sketch.ts` (新規, ~120 行)
- `web/src/sketch.test.ts` (新規, vitest 単体テスト)
- `web/src/viewer.ts`: ~80 行追加 (sketch overlay + 2D↔3D 変換 + raycaster 分岐)
- `web/src/main.ts`: ~40 行追加 (ボタン購読 + キー入力 + onSketchPoint コールバック)
- `web/index.html`: 数行追加 (button + sketch-canvas div + style)
- `web/tests/sketch_canvas.spec.ts` (新規, 6 ケース)
- バックエンド: 無変更
