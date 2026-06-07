# fix(viewer): 押出操作に備えるカメラ体験の改善（damping + フィット表示 + 標準ビュー）

## 概要

現状の `mycad view` ビューアはマウスドラッグで回転できるが、慣性（damping）がなく OrbitControls の
デフォルト render loop も非 rAF のため操作感が粗い。また初期表示でモデルが画面外に出ることがあり、
標準ビュー切替もない。Phase 6 の対話ループを毎回このビューアで実機確認するため先に整える。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `OrbitControls.enableDamping = true` + `dampingFactor` 設定 | 面ピッキング・選択ハイライト（#viewer-pick） |
| `requestAnimationFrame` ループで毎フレーム `controls.update()` を呼ぶ | Feature 追加・編集操作（別 Issue） |
| `renderer.setPixelRatio(window.devicePixelRatio)` の設定 | WebSocket / リアルタイム更新 |
| 初期ロード時に `Box3` AABB → camera/target を自動計算してモデルが収まる位置に配置 | キーボードショートカット |
| Front / Top / Iso の 3 ボタンを HTML に追加し、クリックで `camera.position` と `controls.target` を更新 | タッチ操作カスタマイズ |

## Non-Goals

- 面ピッキング実装（viewer-pick）
- Feature 書き込み操作
- キーボードショートカット

## 完了条件

- Playwright: ページロード後にモデル AABB が viewport 内に完全に収まっている（canvas スナップショット回帰）
- HTML に `data-testid="btn-view-front"` / `"btn-view-top"` / `"btn-view-iso"` が 3 つ存在する（`page.locator` で検証）
- Front ボタンクリック後、`camera.position` の z 成分が正で x=0, y=0 に近い値になる（コンソールログ or JS eval）
- `renderer.getPixelRatio()` が `window.devicePixelRatio` と等しい（JS eval）
- `cargo xtask ci`（Rust）および `bun run test` / Playwright E2E が通る

## 関連 ADR

- ADR-003: ビューア・アプリ全体アーキテクチャ
- ADR-008: Phase 6 設計決定（viewer-polish は完了判定外だが Phase 6 Milestone に含む）

## 実装ヒント

`web/src/viewer.ts` の `OrbitControls` 設定と render loop を修正。
フィット表示は `new THREE.Box3().setFromObject(group)` → `sphere.radius` で距離を計算して `camera.position` をセット。
ボタンは `web/index.html` に追加し `web/src/main.ts` からイベントを接続。
