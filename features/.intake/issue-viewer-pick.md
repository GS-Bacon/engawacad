# feat(viewer): 押出入力のための面ピッキングと選択ハイライト

## 概要

mesh-face-ids (#N) で `TriangleMesh.face_ids` が利用可能になった後、ブラウザ上でモデルの面をクリックして
選択できるようにする。raycaster で hit した三角形の `face_id` を解決し、選択面をハイライト表示する。
選択状態は transient（`.mycad` 非変更）。これが extrude-op (#N) の入力インターフェースになる。

前提: mesh-face-ids (#N) が closed であること。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| canvas クリックイベント + `Raycaster.intersectObjects` で交差三角形を特定 | 押出操作の実行（extrude-op） |
| 交差三角形の index を `face_ids[faceIndex]` で `face_id` 文字列に解決 | 複数面同時選択 |
| 選択面の三角形を別マテリアルでハイライト描画 | エッジ/頂点ピッキング |
| 別面クリックで選択切替・空白クリックで選択解除 | キーボードによる選択 |
| 選択中の `face_id` を JS モジュール変数 `selectedFaceId: string \| null` に保持 | |
| `.mycad` への変更なし（transient 状態） | |

## Non-Goals

- 押出操作の実行（extrude-op）
- 複数面同時選択
- エッジ/頂点ピッキング

## 完了条件

- Playwright: `simple_box.mycad` を開き、canvas 上の面座標をクリック → `data-testid="selected-face-id"` 要素に非 null 文字列が表示される
- Playwright: 別の面座標をクリック → `selected-face-id` が更新された別文字列になる
- Playwright: canvas 外（ページ余白）をクリック → `selected-face-id` が空または要素が非表示になる
- Playwright 完了後に `GET /api/v0/mesh` を fetch して `simple_box.mycad` の内容が変化していない（`.mycad` 非変更）
- `cargo xtask ci` および Playwright E2E が通る

## 関連 ADR

- ADR-005: EntityRef（安定命名 — 生 index 禁止）
- ADR-008: Decision 2（face_ids の契約）、transient 状態の扱い

## 実装ヒント

`web/src/viewer.ts` に `addEventListener('click', ...)` を追加。
`raycaster.intersectObjects(scene.children, true)` → `intersects[0].faceIndex` → `face_ids[faceIndex]`。
ハイライトは `MeshStandardMaterial` 色変更（`userData.originalMaterial` に元を退避）。
`selectedFaceId` を `data-testid="selected-face-id"` DOM 要素にも書き出して E2E から観測可能にする。
