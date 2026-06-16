## 自律判断ログ (自律バッチモード)

- Issue body は完了条件 (7 ケース構成・data-testid・実 API サーバ起動方針) まで明記。intent-check `aligned: yes`。
- **GET /api/v0/document が存在しない** ことを確認 (web/src/api.ts と crates/engawa-api/src/ を grep)。代替: `page.on("request")` / `page.waitForRequest()` で POST capture して T07 (`.engawa 記録順序` 検証) を満たす。Issue body 「もしくは現行の文書状態取得経路」の柔軟枠で対応。
- 担当ファイルは `web/tests/phase7_sketch_acceptance.spec.ts` のみ (新規)。Rust 側は触らず、UI も触らない (#163〜#165 で完了済)。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `web/tests/phase7_sketch_acceptance.spec.ts` 新規、計 7 ケース | 新 UI 機能の追加 (#163〜#165 で完了) |
| 3 RefPlane × Extrude (E01-E03): bodies が `1 → 2` 増 | 既存 Phase 6 acceptance spec の改変 |
| 3 RefPlane × ExtrudeCut (C01-C03): create_sketch + extrude_cut POST、target=既存 body の feature_id | 負荷テスト / fuzzing (#124 別) |
| D01: Front × Extrude 後の POST 順序 = create_sketch (plane_ref=Front) → extrude (sketch=create_sketch.id) | 視覚回帰スクリーンショット |
| `drawClosedRect(page, planeRefId): Promise<void>` 共通ヘルパ (spec 内 inline) | 1 セッション内の複数 RefPlane 切替シナリオ |
| 実 API サーバ (playwright.config.ts の webServer) を流用、mock しない | Phase 8 モデル面上のスケッチ |
| 各テスト独立 (page.goto から開始) | バックエンド (Rust) への変更 |

## Non-Goals

- UI 実装の追加・修正 — 本 Issue は純粋なテスト追加
- 既存 Phase 6 acceptance spec の変更
- engawa-api への新エンドポイント追加 (GET /api/v0/document)
- スクリーンショット視覚回帰

## 実装対象

Issue: #166
影響ファイル:
- `web/tests/phase7_sketch_acceptance.spec.ts` (新規)

### 設計

#### 共通ヘルパ

参照元: `web/tests/refplane_pick.spec.ts` の `projectRefPlaneCenter` (Iso 視点で各 RefPlane 上の代表点 +5,+5 角を Three.js `Vector3.project(camera)` 経由でスクリーン座標に投影)。本 spec にも同等関数を inline 定義する:

```typescript
async function projectRefPlaneCenter(page: Page, id: "Front"|"Top"|"Right"): Promise<{x:number;y:number}|null> {
  return await page.evaluate((id: string) => {
    const viewer = (window as any).__viewer;
    if (!viewer || !viewer.camera || !viewer.renderer) return null;
    const cam = viewer.camera;
    cam.updateMatrixWorld(true);
    cam.matrixWorldInverse.copy(cam.matrixWorld).invert();
    let wx = 0, wy = 0, wz = 0;
    if (id === "Front") { wx = 5; wy = 5; wz = 0; }
    else if (id === "Top") { wx = 5; wy = 0; wz = 5; }
    else { wx = 0; wy = 5; wz = 5; }
    const v = cam.position.clone();
    v.set(wx, wy, wz);
    v.project(cam);
    const canvas = viewer.renderer.domElement;
    const rect = canvas.getBoundingClientRect();
    const sx = rect.left + ((v.x + 1) / 2) * rect.width;
    const sy = rect.top + ((1 - v.y) / 2) * rect.height;
    if (!Number.isFinite(sx) || !Number.isFinite(sy)) return null;
    return { x: sx, y: sy };
  }, id);
}

// countBodies: viewer の __meshData (#163 で expose 済) の長さを返す
async function countBodies(page: Page): Promise<number> {
  return await page.evaluate(() => ((window as any).__meshData ?? []).length);
}

// getInitialFeatureIds: /api/v0/features の現在値を取得 (string[])
async function getInitialFeatureIds(page: Page): Promise<string[]> {
  return await page.evaluate(async () => {
    const res = await fetch("/api/v0/features");
    if (!res.ok) return [];
    return await res.json();
  });
}

async function selectRefPlane(page: Page, id: "Front"|"Top"|"Right"): Promise<void> {
  const pt = await projectRefPlaneCenter(page, id);
  if (!pt) throw new Error("projection failed");
  await page.mouse.click(pt.x, pt.y);
  await expect(page.locator('[data-testid="selected-refplane"]')).toHaveText(id);
}

async function drawClosedRect(page: Page, planeRefId: "Front"|"Top"|"Right"): Promise<void> {
  await selectRefPlane(page, planeRefId);
  await page.click('[data-testid="btn-start-sketch"]');
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");
  const cx = box.x + box.width * 0.5;
  const cy = box.y + box.height * 0.5;
  // 中心付近に小さい矩形 (一辺 2.0 単位を画面ピクセル換算で約 50px と仮定)
  const off = 50;
  await page.mouse.click(cx - off, cy - off);
  await page.waitForTimeout(40);
  await page.mouse.click(cx + off, cy - off);
  await page.waitForTimeout(40);
  await page.mouse.click(cx + off, cy + off);
  await page.waitForTimeout(40);
  await page.mouse.click(cx - off, cy + off);
  await page.waitForTimeout(40);
  await page.mouse.click(cx - off, cy - off); // close
  await expect(page.locator('[data-testid="sketch-canvas"]'))
    .toHaveAttribute("data-state", "closed", { timeout: 5000 });
}
```

#### テスト本体

各テストは独立 `test()` で:
1. `await page.goto("/")` + canvas 待ち
2. `drawClosedRect(page, planeRefId)`
3. `await page.fill('[data-testid="sketch-depth"]', "2")`
4. POST capture (`page.on("request")` で `/api/v0/features` への POST body を配列に蓄積)
5. ボタン clic (`btn-sketch-extrude` or `btn-sketch-extrude-cut`)
6. bodies 数 (window.__meshDataVersion + scene.children traverse) を確認

実 API サーバ起動経由なので、`/api/v0/mesh` の初期 bodies は `examples/simple_box.engawa` ベース (= 1 body)。

#### Extrude ケース (E01-E03)

```typescript
test("E01_front_extrude bodies 1->2", async ({ page }) => {
  await page.goto("/");
  await page.waitForSelector("canvas");
  await page.waitForTimeout(500);
  // 既存 body は simple_box (1 件)
  const before = await countBodies(page);
  await drawClosedRect(page, "Front");
  await page.fill('[data-testid="sketch-depth"]', "2");
  await page.click('[data-testid="btn-sketch-extrude"]');
  await expect.poll(async () => await countBodies(page), { timeout: 10_000 }).toBeGreaterThan(before);
});
```

`countBodies` は `window.__meshData.length` を取得。

E02/E03 は Top/Right で同様。

#### ExtrudeCut ケース (C01-C03)

```typescript
test("C01_front_extrude_cut posts extrude_cut with target=existing body id", async ({ page }) => {
  const posts: any[] = [];
  page.on("request", async (req) => {
    if (req.method() === "POST" && req.url().endsWith("/api/v0/features")) {
      try { posts.push(await req.postDataJSON()); } catch {}
    }
  });
  await page.goto("/");
  await page.waitForSelector("canvas");
  await page.waitForTimeout(500);
  const initialBodies = await getInitialFeatureIds(page); // /api/v0/features の初期取得結果
  expect(initialBodies.length).toBeGreaterThan(0);
  await drawClosedRect(page, "Front");
  await page.fill('[data-testid="sketch-depth"]', "1");
  await page.click('[data-testid="btn-sketch-extrude-cut"]');
  await expect.poll(() => posts.filter(p => p.type === "extrude_cut").length, { timeout: 10_000 }).toBeGreaterThan(0);
  const cutPost = posts.find(p => p.type === "extrude_cut")!;
  expect(cutPost.target).toBe(initialBodies[0]);
  expect(posts.find(p => p.type === "create_sketch")?.plane_ref).toBe("Front");
});
```

#### 整合性ケース (D01)

```typescript
test("D01_engawa_record_order Front extrude posts create_sketch then extrude with matching sketch id", async ({ page }) => {
  const posts: any[] = [];
  page.on("request", async (req) => {
    if (req.method() === "POST" && req.url().endsWith("/api/v0/features")) {
      try { posts.push(await req.postDataJSON()); } catch {}
    }
  });
  await page.goto("/");
  await page.waitForSelector("canvas");
  await page.waitForTimeout(500);
  await drawClosedRect(page, "Front");
  await page.fill('[data-testid="sketch-depth"]', "2");
  await page.click('[data-testid="btn-sketch-extrude"]');
  await expect.poll(() => posts.length, { timeout: 10_000 }).toBeGreaterThanOrEqual(2);
  // POST 順序: create_sketch (plane_ref=Front) → extrude (sketch=create_sketch.id)
  const csIdx = posts.findIndex(p => p.type === "create_sketch");
  const exIdx = posts.findIndex(p => p.type === "extrude");
  expect(csIdx).toBeLessThan(exIdx);
  expect(posts[csIdx].plane_ref).toBe("Front");
  expect(posts[exIdx].sketch).toBe(posts[csIdx].id);
});
```

### 数値モデル

- 矩形サイズ: 一辺 ~2.0 単位 (canvas 内ピクセル 50px の off-center クリック; カメラ Iso 視点で 1 ピクセル ≒ 0.04 単位想定)。RefPlane ±10 範囲内、SNAP_RADIUS=0.1 を超える間隔。
- depth: 2.0 (Extrude) / 1.0 (ExtrudeCut)
- 5 点目クリックは 1 点目と完全同一のピクセル座標で行う → 幾何的に閉じる (SNAP_RADIUS 不依存。同一座標なら raycaster の uv が同一値になり、addPoint 内の snap → equal-to-points[0] 経路で必ず close 判定に乗る)。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| E01_front_extrude | Front × Extrude | Front 選択 → 矩形 → depth=2 → btn-sketch-extrude → bodies 数増 | __meshData.length が before より大 |
| E02_top_extrude | Top × Extrude | 同上を Top で | 同上 |
| E03_right_extrude | Right × Extrude | 同上を Right で | 同上 |
| C01_front_extrude_cut | Front × ExtrudeCut | Front 矩形 → depth=1 → btn-sketch-extrude-cut → extrude_cut POST、target=既存 body | posts に extrude_cut が含まれ target=initial_feature_ids[0] |
| C02_top_extrude_cut | Top × ExtrudeCut | 同上を Top で | 同上 |
| C03_right_extrude_cut | Right × ExtrudeCut | 同上を Right で | 同上 |
| D01_engawa_record_order | 整合性 | Front × Extrude POST 順序 create_sketch (plane_ref=Front) → extrude (sketch=create_sketch.id) | order assert + plane_ref + sketch ref |

## 幾何的不変条件チェックリスト

- [ ] N/A — E2E テスト追加のみ、kernel に触らない
- [ ] N/A
- [ ] N/A
- [ ] N/A

## 影響範囲

- `web/tests/phase7_sketch_acceptance.spec.ts` (新規, ~200 行)
- バックエンド: 無変更
- 既存テスト基盤 (playwright.config.ts / cargo xtask): 無変更
