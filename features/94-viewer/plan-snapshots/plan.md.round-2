# #94 feat(viewer): 押出入力のための面ピッキングと選択ハイライト

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `web/src/viewer.ts` に Raycaster + pointer クリックハンドラを追加し、交差三角形を特定 | 押出操作の実行 (extrude-op #95) |
| `meshToGeometry` を拡張し `face_ids` を `geometry.userData.faceIds`(三角形ごと) に退避 + face_id 連続 run ごとに `addGroup` | 複数面同時選択 |
| `intersection.faceIndex` → `userData.faceIds[faceIndex]` で `face_id` 文字列に解決 | エッジ/頂点ピッキング |
| 選択面の group を別マテリアル(2要素 material 配列)でハイライト、別面で切替・空白で解除 | キーボード選択 |
| `selectedFaceId: string \| null` をモジュール変数で保持 + `data-testid="selected-face-id"` DOM 要素へ書き出し | `.mycad` への書き込み(transient のみ) |
| ピッキング用 fixture `simple_box_faces.json` を kernel から再生成(face_ids 入り) | サーバ往復・POST(本 Issue は純フロントエンド・transient) |

## Non-Goals
- 押出操作の実行(extrude-op) — #95 で対応
- 複数面同時選択
- エッジ/頂点ピッキング
- キーボードによる選択
- `.mycad` への永続化(本 Issue の選択状態は transient、POST しない)

## 実装対象
<!-- Issue: #94 -->
<!-- 影響ファイル: web/src/mesh.ts, web/src/viewer.ts, web/index.html, web/tests/viewer.spec.ts, web/src/mesh.test.ts, web/tests/fixtures/simple_box_faces.json(新規) -->

### 1. `web/src/mesh.ts` — `meshToGeometry` に face_ids 伝搬 + group 分割（既存関数修正）

**before** (mesh.ts:82-89):
```ts
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new BufferAttribute(positionData, 3));
  geometry.setAttribute("normal", new BufferAttribute(normalData, 3));
  if (mesh.indices.length > 0) {
    geometry.setIndex(Array.from(mesh.indices));
  }

  return geometry;
}
```

**after**:
```ts
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new BufferAttribute(positionData, 3));
  geometry.setAttribute("normal", new BufferAttribute(normalData, 3));
  if (mesh.indices.length > 0) {
    geometry.setIndex(Array.from(mesh.indices));
  }

  // face_ids は三角形ごと(length === indices.length / 3)。raycast の
  // intersection.faceIndex で引けるよう userData に退避する。
  const faceIds = mesh.face_ids ?? [];
  geometry.userData.faceIds = faceIds;

  // 同一 face_id の三角形は tessellation 上で連続するため、連続 run ごとに
  // addGroup する。ハイライト時に group の materialIndex を差し替える。
  // group の materialIndex は既定 0(=ベースマテリアル)。
  const groupFaceIds: string[] = [];
  let runStart = 0;
  for (let t = 1; t <= faceIds.length; t++) {
    if (t === faceIds.length || faceIds[t] !== faceIds[runStart]) {
      geometry.addGroup(runStart * 3, (t - runStart) * 3, 0);
      groupFaceIds.push(faceIds[runStart]);
      runStart = t;
    }
  }
  geometry.userData.groupFaceIds = groupFaceIds;

  return geometry;
}
```
> 戻り値型は `BufferGeometry` のまま。既存呼び出し(viewer.ts・テスト)は無改修。group は materialIndex 0 のみなので単一マテリアルでも描画不変(既存 T02 screenshot 維持)。

### 2. `web/src/viewer.ts` — Raycaster + クリック選択 + ハイライト（既存関数修正）

モジュール先頭に選択状態を追加:
```ts
export let selectedFaceId: string | null = null;
```

**before** (viewer.ts:38-53, 91, 93-98):
```ts
  const group = new Group();

  for (let i = 0; i < bodies.length; i++) {
    const geometry = meshToGeometry(bodies[i].mesh);
    geometry.computeBoundingBox();
    const color = BODY_COLORS[i % BODY_COLORS.length];
    const material = new MeshPhongMaterial({
      color,
      specular: 0x333333,
      shininess: 30,
    });
    const threeMesh = new Mesh(geometry, material);
    group.add(threeMesh);
  }

  scene.add(group);
```

**after**:
```ts
  const group = new Group();
  const pickMeshes: Mesh[] = [];
  const highlightMaterial = new MeshPhongMaterial({
    color: 0xffaa00,
    specular: 0x333333,
    shininess: 30,
  });

  for (let i = 0; i < bodies.length; i++) {
    const geometry = meshToGeometry(bodies[i].mesh);
    geometry.computeBoundingBox();
    const color = BODY_COLORS[i % BODY_COLORS.length];
    const baseMaterial = new MeshPhongMaterial({
      color,
      specular: 0x333333,
      shininess: 30,
    });
    // index 0 = ベース, index 1 = ハイライト。group の materialIndex を切替える。
    const threeMesh = new Mesh(geometry, [baseMaterial, highlightMaterial]);
    group.add(threeMesh);
    pickMeshes.push(threeMesh);
  }

  scene.add(group);
```

レンダーループ前にピッキング配線を追加(新規ブロック、viewer.ts:91 付近):
```ts
  const raycaster = new Raycaster();
  const ndc = new Vector2();
  const selectedEl = document.querySelector<HTMLElement>(
    '[data-testid="selected-face-id"]',
  );

  function setSelection(faceId: string | null): void {
    selectedFaceId = faceId;
    if (selectedEl) selectedEl.textContent = faceId ?? "";
    // 全 mesh の group を一旦ベース(0)へ戻し、一致 group のみ 1 に。
    for (const m of pickMeshes) {
      const gfids: string[] = m.geometry.userData.groupFaceIds ?? [];
      const groups = m.geometry.groups;
      for (let k = 0; k < groups.length; k++) {
        groups[k].materialIndex =
          faceId !== null && faceId !== "" && gfids[k] === faceId ? 1 : 0;
      }
    }
  }

  // pointerdown/up の移動量でクリックとドラッグ(OrbitControls 回転)を区別する。
  let downX = 0, downY = 0;
  const DRAG_THRESHOLD = 5;
  renderer.domElement.addEventListener("pointerdown", (e) => {
    downX = e.clientX; downY = e.clientY;
  });
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
      setSelection(null); // 空白(モデル無し領域)クリックで選択解除
    }
  });
```
> import 追加: `Raycaster`, `Vector2`(three)。`intersectObjects` は AxesHelper/light を避けるため `pickMeshes` のみを対象にする(`scene.children` は使わない)。

### 3. `web/index.html` — 観測用 DOM 要素追加

**before** (index.html:43-46):
```html
    <div id="app">
      <div id="error"></div>
      <div id="info"></div>
    </div>
```
**after**:
```html
    <div id="app">
      <div id="error"></div>
      <div id="info"></div>
      <div id="selected-face" data-testid="selected-face-id"></div>
    </div>
```
`#selected-face` は `#info` と同様の絶対配置オーバーレイ。`pointer-events: none` を付与しキャンバスクリックを阻害しない(CSS を inline `<style>` に追加)。

## 設計方針
- **決定性**: `meshToGeometry` は純関数のまま。同一 `TriangleMesh` 入力 → 同一 group 構成・同一 `userData.faceIds`。乱数・時刻非依存。
- **B-rep トポロジー妥当性**: N/A(本 Issue は描画層のみ、トポロジー生成なし)。
- **退化幾何の扱い**: 既存 `validateMesh` が退化三角形を排除済み。`face_ids` 空配列(indices 0)・空文字 face_id(無名面)を非クラッシュで扱う。
- **derive 規約**: N/A(TypeScript 層、Rust 型変更なし)。`face_ids` は既存 ts-rs 生成型に存在。
- **エラーハンドリング**: raycast ミス → `setSelection(null)`。`faceIndex` 欠落/範囲外 → null フォールバック。
- **transient 保証**: 選択は描画状態のみ。POST/書き込み経路を一切持たない → `.mycad` 構造的に不変。

### 数値モデル
**N/A** — 本 Issue は新たな tolerance/ε を導入しない。face_id 解決は厳密な整数インデックス(`faceIndex → face_ids[faceIndex]`)であり浮動小数判定を含まない。ray-triangle 交差は Three.js `Raycaster` の既定実装に委譲(独自 ε なし)。ドラッグ判別の `DRAG_THRESHOLD=5px` は許容操作量であり幾何 tolerance ではない。ADR-004 の対象外。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 (vitest) | 同一 mesh で `meshToGeometry` を2回。`toEqual` で検証: `userData.faceIds` / `userData.groupFaceIds` / `geometry.groups.map(g=>[g.start,g.count,g.materialIndex])` | 3項目すべて一致 |
| T02 | 正常系 (vitest) | face_ids 入り mesh → `userData.faceIds` が入力と一致、group が全三角形を被覆 | 一致・被覆 |
| T03_boundary | 境界 (vitest) | indices 空(三角形0)の mesh → group 0個・`userData.faceIds` 空・例外なし | クラッシュなし |
| T04_degen | 退化 (vitest) | 無名面で face_id `""` を含む run → group は作られるが選択時にハイライト対象外(空文字はハイライトしない) | 非クラッシュ・空文字非ハイライト |
| E01 | E2E (Playwright) | `simple_box_faces` 開く → canvas 中央クリック → `selected-face-id` が非空文字列 | textContent.length > 0 |
| E02 | E2E | 別の面座標をクリック → `selected-face-id` が別の非空文字列に更新 | 値が E01 と異なる |
| E03 | E2E | モデル無しの canvas 隅をクリック → `selected-face-id` が空 | textContent === "" |
| E04 | E2E (transient) | クリック前後で `GET /api/v0/mesh` を fetch → レスポンスが deep-equal(.mycad 非変更) | 一致 |

## 幾何的不変条件チェックリスト
- N/A（Boolean/Partition/Assemble 非該当。本 Issue は描画層の面ピッキングのみ）

## フィクスチャ生成手順（GLM 向けメモ）
`web/tests/fixtures/simple_box_faces.json` は kernel から再生成する(既存16 fixture は #92 以前生成で face_ids 欠落のため無改修・別ファイルにする):
1. `cargo run -p mycad-cli -- view --input examples/simple_box.mycad --port 7878 &`(ローカル一時起動)
2. `curl -s localhost:7878/api/v0/mesh > web/tests/fixtures/simple_box_faces.json`
3. サーバ停止。生成 JSON に `face_ids`(非空文字列)が含まれることを確認。
> E04 の「.mycad 非変更」は mock route が定数 fixture を返すため fetch 前後 deep-equal で表現。真の transient 保証はコード上 POST 経路を持たないこと(レビューで担保)。
