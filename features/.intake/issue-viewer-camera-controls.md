# fix(viewer): 初期フィット表示と Front/Top/Iso 標準ビューボタンを追加する

## 概要

現状はモデルが画面外に出ることがあり、標準ビューへの切替手段もない。
本 Issue でロード時の自動フィット表示と 3 つの標準ビューボタンを追加する。
（damping/render loop は別 Issue: viewer-render-loop）

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| 初期ロード時に `Box3` AABB → camera/target を計算してモデルを収める | damping/render loop（viewer-render-loop） |
| HTML に `data-testid="btn-view-front/top/iso"` の 3 ボタンを追加 | 面ピッキング・Feature 書き込み |
| ボタンクリックで `camera.position` と `controls.target` を更新 | キーボードショートカット |

## Non-Goals

- damping/render loop（viewer-render-loop）
- 面ピッキング・Feature 操作
- キーボードショートカット

## 完了条件

- Playwright: ページロード直後に canvas スナップショットを撮り、モデルが viewport 中央に収まっている（スナップショット回帰）
- Playwright: `btn-view-front` をクリック後に `camera.position.z > 0` かつ `x ≈ 0, y ≈ 0`（JS eval）
- Playwright: `btn-view-top` をクリック後に `camera.position.y > 0` かつ `x ≈ 0, z ≈ 0`（JS eval）
- `data-testid="btn-view-front/top/iso"` の 3 要素が DOM に存在する（`page.locator` で確認）
- `cargo xtask ci` および Playwright E2E が通る

## 関連 ADR

- ADR-003: ビューア・アプリ全体アーキテクチャ
- ADR-008: Phase 6 Milestone（viewer-polish 系は完了判定外、refactor 扱い）

## 実装ヒント

`web/src/main.ts` の mesh ロード後に `new THREE.Box3().setFromObject(group)` → `sphere.radius` で
distance を計算して `camera.position` をセット。ボタンは `web/index.html` に追加し `main.ts` からイベント接続。
