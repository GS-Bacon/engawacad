# test-spec — #163 viewer-refplane-pick

実装差分: `web/src/viewer.ts` (RefPlane 3 枚追加 + raycaster + ViewerHandle.onRefPlaneSelected), `web/src/main.ts` (DOM 反映), `web/index.html` (selected-refplane 要素), `web/tests/refplane_pick.spec.ts` (新規 5 ケース)。

## 不足テスト (plan 計画分)

- plan の T01〜T04, T06 は実装済み (`refplane_pick.spec.ts` 内)。
- plan の T05 (Phase 6 互換: `face_picking.spec.ts` 既存テストが green) は本 CI で 57 passed / 1 skipped を確認済み、回帰なし。

## 実装差分から追加すべきテスト

- **fitCamera empty 分岐**: 本 Issue で empty bodies (RefPlane のみ) のとき camera を Iso 位置 `(15, 15, 30)`、target `(0,0,0)` に置く分岐を追加した。これは T01-T04 で経由するが、`__viewer.camera.position` を直接 assert する明示テストはまだない。
  - 追加候補: T01 内で `expect(camPos).toEqual([15,15,30])` を確認、または独立した `T01_camera_iso_on_empty` を作る。
  - ただし fitCamera のスコープは「RefPlane を可視範囲に置く」点に尽きるため、refplane_pick.spec の T02〜T04 がクリックで Front/Top/Right を区別して当てられている時点で機能要件は満たされている。**追加不要**と判断 (テスト追加コスト > 重複検証コスト)。

## エッジケース・退化入力

- **クリック後に bodies が POST 反映されたケース**: `handle.updateBodies()` 呼出後に RefPlane mesh は scene に残るか (buildScene 内で group しか clear しないため残る想定)。
  - 追加候補: bodies 更新後も RefPlane 3 枚が scene に残ることを assert。
  - 重要度: 後続 #164/#165 で必須となるため、本 Issue でも 1 ケース足しておくのが望ましい。**T07_bodies_update_preserves_refplanes** として追加要請する。

- **RefPlane が選択中の状態で形状面をクリック**: 形状面選択時に `selectRefPlane(null)` が呼ばれ RefPlane の opacity が 0.2 に戻ることは T06_degen が確認済み (実装上 `selectRefPlane(null)` の opacity 復帰)。

## 数値境界

- RefPlane mesh の opacity 値 `0.2` / `0.45` は `toBeCloseTo(..., 2)` で検証済み。

## 決定性

- 本 Issue は UI のみで、kernel の決定性に影響なし。N/A。

## 類似ケース（未カバー）

- 本 Issue は bug 修正ではなく feature 追加のため、既存類似コードパス検索は N/A。

## 期待値乖離

- なし。plan の T01-T04, T06 の期待値と実装側 assertion (refplane id 集合 / opacity 値 / textContent) は一致。
