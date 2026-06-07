# fix(viewer): render loop を rAF に統一し damping と DPR を設定する

## 概要

現状の OrbitControls は damping なし・DPR 未設定で操作感が粗い。
本 Issue で renderer/controls の初期設定 1 箇所を修正し、回転の滑らかさと解像度を改善する。
（フィット表示・標準ビューボタンは別 Issue: viewer-camera-controls）

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `OrbitControls.enableDamping = true` + `dampingFactor` 設定 | フィット表示（viewer-camera-controls） |
| `requestAnimationFrame` ループで毎フレーム `controls.update()` を呼ぶ | 標準ビューボタン（viewer-camera-controls） |
| `renderer.setPixelRatio(window.devicePixelRatio)` の設定 | 面ピッキング・Feature 書き込み |

## Non-Goals

- フィット表示・標準ビューボタン（viewer-camera-controls）
- 面ピッキング・Feature 操作

## 完了条件

- Playwright: `renderer.getPixelRatio()` を JS eval して `window.devicePixelRatio` と一致する
- `OrbitControls.enableDamping` が `true`（JS eval で確認）
- `cargo xtask ci` および Playwright E2E が通る

## 関連 ADR

- ADR-003: ビューア・アプリ全体アーキテクチャ
- ADR-008: Phase 6 Milestone（viewer-polish は完了判定外、refactor 扱い）

## 実装ヒント

`web/src/viewer.ts` の `OrbitControls` 設定と render loop を修正。約 10 行の変更。
