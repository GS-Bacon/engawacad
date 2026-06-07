# Debug Spec — #94 GLM round 1 後の実装不足

## 問題
GLM コア実装が `web/src/mesh.ts` の face_ids 伝搬(グループ分割)のみ実装し、
以下の必須ファイルへの変更が漏れている:
1. `web/src/viewer.ts` — Raycaster + クリックハンドラ + ハイライト 未実装
2. `web/index.html` — `data-testid="selected-face-id"` DOM 要素 未追加

CI は green になっているが、vitest の T01-T04 が `.todo()` のまま(実装なし)、
Playwright の E01-E04 が `.fixme()` のまま、という状態。face 選択機能は一切動かない。

## 仮説
GLM が mesh.ts の face_ids 伝搬を実装した後、viewer.ts の変更に進まずに終了した(glm_exit: 1)。

## 残実装 1: `web/src/viewer.ts`

plan.md「実装対象 §2」に従って実装すること。

追加すべき import (viewer.ts 先頭):
```ts
import { Raycaster, Vector2 } from "three";
// 既存 imports に追加
```

モジュール変数追加 (viewer.ts モジュールトップレベル、BODY_COLORS の下):
```ts
export let selectedFaceId: string | null = null;
```

`initViewer` 内の Mesh 生成部分 (for ループ) を変更して 2 要素マテリアル + pickMeshes 収集:
```ts
  const group = new Group();
  const pickMeshes: Mesh[] = [];
  const highlightMaterial = new MeshPhongMaterial({ color: 0xffaa00, specular: 0x333333, shininess: 30 });

  for (let i = 0; i < bodies.length; i++) {
    const geometry = meshToGeometry(bodies[i].mesh);
    geometry.computeBoundingBox();
    const color = BODY_COLORS[i % BODY_COLORS.length];
    const baseMaterial = new MeshPhongMaterial({ color, specular: 0x333333, shininess: 30 });
    const threeMesh = new Mesh(geometry, [baseMaterial, highlightMaterial]);
    group.add(threeMesh);
    pickMeshes.push(threeMesh);
  }
  scene.add(group);
```

`window.addEventListener("resize", onResize)` の前（render loop の前）に以下を追加:
```ts
  const raycaster = new Raycaster();
  const ndc = new Vector2();
  const selectedEl = document.querySelector<HTMLElement>('[data-testid="selected-face-id"]');

  function setSelection(faceId: string | null): void {
    selectedFaceId = faceId;
    if (selectedEl) selectedEl.textContent = faceId ?? "";
    for (const m of pickMeshes) {
      const gfids: string[] = m.geometry.userData.groupFaceIds ?? [];
      const groups = m.geometry.groups;
      for (let k = 0; k < groups.length; k++) {
        groups[k].materialIndex =
          faceId !== null && faceId !== "" && gfids[k] === faceId ? 1 : 0;
      }
    }
  }

  let downX = 0, downY = 0;
  const DRAG_THRESHOLD = 5;
  renderer.domElement.addEventListener("pointerdown", (e) => { downX = e.clientX; downY = e.clientY; });
  renderer.domElement.addEventListener("pointerup", (e) => {
    if (e.button !== 0) return;
    if (Math.hypot(e.clientX - downX, e.clientY - downY) > DRAG_THRESHOLD) return;
    const rect = renderer.domElement.getBoundingClientRect();
    ndc.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    ndc.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
    raycaster.setFromCamera(ndc, camera);
    const hits = raycaster.intersectObjects(pickMeshes, false);
    if (hits.length > 0 && hits[0].faceIndex != null) {
      const fids: string[] = hits[0].object.geometry.userData.faceIds ?? [];
      setSelection(fids[hits[0].faceIndex] ?? null);
    } else {
      setSelection(null);
    }
  });
```

## 残実装 2: `web/index.html`

`<div id="info"></div>` の次の行に追加:
```html
      <div id="selected-face" data-testid="selected-face-id"></div>
```

CSS の `<style>` ブロックに追加:
```css
      #selected-face {
        display: block;
        position: absolute;
        bottom: 1rem;
        left: 1rem;
        background: rgba(0, 0, 0, 0.5);
        color: white;
        padding: 0.4rem 0.6rem;
        border-radius: 4px;
        font-family: monospace;
        font-size: 12px;
        pointer-events: none;
        z-index: 10;
        min-height: 1em;
      }
```

## 残実装 3: `web/src/mesh.test.ts` の T01-T04 スケルトンを実装

T01-T04 の `.todo()` を実際のテストに変更する。T01-T04 の実装は
`web/src/mesh.ts` の実装と整合すること。

## 試した修正と結果
- (初回 debug-spec)

## 次にやること
1. viewer.ts を上記仕様で修正
2. index.html を上記仕様で修正
3. mesh.test.ts の T01-T04 を実装
4. `npm run typecheck && npm run test` green 確認
5. `cargo xtask ci` green 確認
