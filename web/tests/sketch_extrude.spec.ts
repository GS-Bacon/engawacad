/**
 * Playwright E2E tests — #165 スケッチ確定 → Extrude / ExtrudeCut
 *
 * 仕様 (Issue #165):
 *  - lastFinalizedSketch=null では sketch-extrude-panel は hidden (btn 操作不可)
 *  - 矩形閉ループ + depth>0 で POST 順序 = create_sketch (plane_ref) → extrude (fuse_target=null)
 *  - ExtrudeCut: POST 順序 = create_sketch → extrude_cut (target=最初の body)
 *  - depth=0 / 負値 / NaN / Infinity の 4 パターンで button disabled
 */
import { test, expect, type Page } from "@playwright/test";

/** Empty document で起動 */
async function setupEmptyDocPage(page: Page): Promise<void> {
  await page.route("/api/v0/mesh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([]),
    }),
  );
  await page.route("/api/v0/features", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([]),
    }),
  );
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  await page.waitForTimeout(500);
}

/** 各 RefPlane 上の代表点をスクリーン座標に投影 */
async function projectRefPlaneCenter(
  page: Page,
  id: "Front" | "Top" | "Right",
): Promise<{ x: number; y: number } | null> {
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

/** スケッチで矩形を描いて閉じる (Front 平面で。box 中心ベースの 4 点 + 始点クリック) */
async function drawClosedRectangle(page: Page): Promise<void> {
  const front = await projectRefPlaneCenter(page, "Front");
  if (!front) throw new Error("projection failed");
  await page.mouse.click(front.x, front.y);
  await page.waitForTimeout(100);
  await page.click('[data-testid="btn-start-sketch"]');
  await page.waitForTimeout(100);

  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");
  const cx = box.x + box.width * 0.5;
  const cy = box.y + box.height * 0.5;
  await page.mouse.click(cx - 50, cy - 50);
  await page.waitForTimeout(30);
  await page.mouse.click(cx + 50, cy - 50);
  await page.waitForTimeout(30);
  await page.mouse.click(cx + 50, cy + 50);
  await page.waitForTimeout(30);
  await page.mouse.click(cx - 50, cy + 50);
  await page.waitForTimeout(30);
  await page.mouse.click(cx - 50, cy - 50); // close
  await page.waitForTimeout(200);
}

test.describe("Sketch → Extrude/Cut (T01-T04)", () => {
  test("T01: unfinalized sketch keeps sketch-extrude-panel hidden", async ({ page }) => {
    await setupEmptyDocPage(page);
    const front = await projectRefPlaneCenter(page, "Front");
    if (!front) throw new Error("projection failed");
    await page.mouse.click(front.x, front.y);
    await page.waitForTimeout(100);
    await page.click('[data-testid="btn-start-sketch"]');
    await page.waitForTimeout(100);

    // 1 点だけ打って開ループのまま
    const canvas = page.locator("canvas");
    const box = await canvas.boundingBox();
    if (!box) throw new Error("canvas not found");
    await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);
    await page.waitForTimeout(100);

    await expect(page.locator('[data-testid="sketch-extrude-panel"]')).not.toBeVisible();
    await expect(page.locator('[data-testid="btn-sketch-extrude"]')).toBeDisabled();
  });

  test("T02: finalize + depth=2 + btn-sketch-extrude posts create_sketch then extrude in order", async ({ page }) => {
    const posts: any[] = [];
    await page.route("/api/v0/mesh", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
    );
    await page.route("/api/v0/features", async (route) => {
      const req = route.request();
      const body = req.postDataJSON();
      if (req.method() === "POST") {
        posts.push(body);
        if (body.type === "extrude") {
          await route.fulfill({
            status: 200,
            contentType: "application/json",
            body: JSON.stringify([{
              feature_id: body.id,
              mesh: { positions: [], normals: [], indices: [], face_ids: [] },
            }]),
          });
        } else {
          await route.fulfill({ status: 200, contentType: "application/json", body: "[]" });
        }
      } else {
        await route.fulfill({ status: 200, contentType: "application/json", body: "[]" });
      }
    });
    await page.goto("/");
    await page.waitForSelector("canvas", { timeout: 10_000 });
    await page.waitForTimeout(500);

    await drawClosedRectangle(page);
    await expect(page.locator('[data-testid="sketch-extrude-panel"]')).toBeVisible({ timeout: 5000 });

    await page.fill('[data-testid="sketch-depth"]', "2");
    await page.waitForTimeout(50);
    await page.click('[data-testid="btn-sketch-extrude"]');
    await page.waitForTimeout(400);

    expect(posts.length).toBeGreaterThanOrEqual(2);
    expect(posts[0].type).toBe("create_sketch");
    expect(posts[0].plane_ref).toBe("Front");
    expect(posts[0].plane).toBe("xy");
    expect(posts[1].type).toBe("extrude");
    expect(posts[1].fuse_target).toBeNull();
    expect(posts[1].depth).toBe(2);
    expect(posts[1].sketch).toBe(posts[0].id);
  });

  test("T03: finalize + btn-sketch-extrude-cut posts create_sketch then extrude_cut targeting first body", async ({ page }) => {
    const posts: any[] = [];
    // 既存の body 1 件を返す mesh API mock + features POST 動作
    await page.route("/api/v0/mesh", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([{
          feature_id: "box1",
          mesh: { positions: [], normals: [], indices: [], face_ids: [] },
        }]),
      }),
    );
    await page.route("/api/v0/features", async (route) => {
      const req = route.request();
      if (req.method() === "POST") {
        const body = req.postDataJSON();
        posts.push(body);
        // 簡易: 常に body=box1 を返す
        await route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify([{
            feature_id: "box1",
            mesh: { positions: [], normals: [], indices: [], face_ids: [] },
          }]),
        });
      } else {
        await route.fulfill({ status: 200, contentType: "application/json", body: "[]" });
      }
    });
    await page.goto("/");
    await page.waitForSelector("canvas", { timeout: 10_000 });
    await page.waitForTimeout(500);

    await drawClosedRectangle(page);
    await expect(page.locator('[data-testid="sketch-extrude-panel"]')).toBeVisible({ timeout: 5000 });

    await page.fill('[data-testid="sketch-depth"]', "1");
    await page.waitForTimeout(50);
    await page.click('[data-testid="btn-sketch-extrude-cut"]');
    await page.waitForTimeout(400);

    expect(posts.length).toBeGreaterThanOrEqual(2);
    expect(posts[0].type).toBe("create_sketch");
    expect(posts[0].plane_ref).toBe("Front");
    expect(posts[1].type).toBe("extrude_cut");
    expect(posts[1].target).toBe("box1");
    expect(posts[1].depth).toBe(1);
    expect(posts[1].sketch).toBe(posts[0].id);
  });

  test("T04_boundary_depth_guard: depth=0 / 負値 / Infinity でボタン disabled", async ({ page }) => {
    await setupEmptyDocPage(page);
    await drawClosedRectangle(page);
    await expect(page.locator('[data-testid="sketch-extrude-panel"]')).toBeVisible({ timeout: 5000 });
    const extrudeBtn = page.locator('[data-testid="btn-sketch-extrude"]');
    const depthInput = page.locator('[data-testid="sketch-depth"]');

    // depth=5 (default) で enabled
    await expect(extrudeBtn).toBeEnabled();

    // depth=0
    await depthInput.fill("0");
    await page.waitForTimeout(50);
    await expect(extrudeBtn).toBeDisabled();

    // 負値
    await depthInput.fill("-5");
    await page.waitForTimeout(50);
    await expect(extrudeBtn).toBeDisabled();

    // Infinity 相当の極大値 (1e309 は Number.isFinite(Number("1e309"))=false)
    await depthInput.fill("1e309");
    await page.waitForTimeout(50);
    await expect(extrudeBtn).toBeDisabled();

    // depth=2 で enabled に戻る
    await depthInput.fill("2");
    await page.waitForTimeout(50);
    await expect(extrudeBtn).toBeEnabled();
  });
});
