# feat(viewer): 選択面からの押出(Extrude)を UI で実行

## 概要

viewer-pick (#N) で面が選択できるようになった後、選択面に対して押出を実行する UI を追加する。
深さを入力 → `POST /api/v0/features` で `Extrude` Feature を積む → レスポンスでシーンを差し替える。
既存の `make_extrusion` / `Feature::Extrude` を再利用し、新しい幾何実装は不要。

前提: write-api (#N) および viewer-pick (#N) が closed であること。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| 面選択中に `data-testid="extrude-panel"` パネルを表示（深さ `<input>` + 「押出」`<button>`） | 押出カット（ExtrudeCut）（cut-op） |
| 「押出」ボタンクリック → `POST /api/v0/features` に `Extrude` Feature JSON を送信 | テーパー角・両側押出・任意方向 |
| レスポンス `Vec<BodyMesh>` でシーンを差し替え（既存 mesh 描画ロジックを再利用） | スケッチ描画（Phase 7） |
| 送信する Feature の `sketch` を選択 `face_id` から正準平面（Xy/Xz/Yz）で解決 | undo/redo |
| 押出後に選択状態をクリア・パネルを非表示に | アニメーションプレビュー |

## Non-Goals

- ExtrudeCut（cut-op）
- テーパー角・両側押出
- スケッチ描画 UI（Phase 7 以降）

## 完了条件

- Playwright: 面を選択 → `extrude-panel` が visible になる
- Playwright: 深さを入力して「押出」ボタンクリック → `POST /api/v0/features` が呼ばれ 200 レスポンス
- Playwright 完了後に `GET /api/v0/mesh` でメッシュ頂点数が押出前より増加している
- `mycad export` で `.mycad` を STL 化したとき `Feature::Extrude` が 1 件以上存在する（CLI or YAML parse）
- `cargo xtask ci` および Playwright E2E が通る

## 関連 ADR

- ADR-008: Decision 1（Extrude は加算 Feature）、Decision 3（committed 操作は POST で実行）

## 実装ヒント

面選択時に `web/` に深さ入力パネルを表示（素の DOM: `<input type="number" data-testid="extrude-depth">` +
`<button data-testid="btn-extrude">`）。
face_id → 正準平面の解決: face_id 文字列に含まれる法線情報 or API からの平面情報を使う（詳細は plan.md 実装時に確定）。
`POST /api/v0/features` 完了後に `main.ts` の `initScene` 相当を呼び直してシーンを更新する。
