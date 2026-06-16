/**
 * Playwright E2E tests — #164 2D スケッチキャンバス
 *
 * 仕様 (Issue #164):
 *  - RefPlane 未選択時に btn-start-sketch を押しても sketch-canvas は idle のまま
 *  - 矩形 4 点で開ループ → data-state="open" + .sketch-canvas--open
 *  - 5 点目 SNAP_RADIUS 内クリックで自動スナップ → data-state="closed"
 *  - Esc で全破棄 → data-state="idle"
 *  - Enter で open finalize → null 返却、state は "open" のまま
 *  - ゼロ長セグメントは棄却
 */
import { test, expect, type Page } from "@playwright/test";

/** Empty document で起動 — RefPlane だけが表示される */
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
  await page.route("http://127.0.0.1:7879/**", (route) =>
    route.fulfill({
      status: 200,
      headers: {
        "Access-Control-Allow-Origin": "*",
        "Access-Control-Allow-Methods": "POST, DELETE, OPTIONS",
        "Access-Control-Allow-Headers": "content-type",
      },
      body: "",
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

/** sketch-canvas の data-state を取得 */
async function getSketchState(page: Page): Promise<string> {
  const el = page.locator('[data-testid="sketch-canvas"]');
  return (await el.getAttribute("data-state")) ?? "idle";
}

/** sketch-canvas--open class が付いているか */
async function hasSketchOpenClass(page: Page): Promise<boolean> {
  const el = page.locator('[data-testid="sketch-canvas"]');
  const className = await el.getAttribute("class");
  return className?.includes("sketch-canvas--open") ?? false;
}

test("T01 RefPlane unselected btn-start-sketch is no-op", async ({ page }) => {
  await setupEmptyDocPage(page);
  const btn = page.locator('[data-testid="btn-start-sketch"]');
  await btn.click();
  expect(await getSketchState(page)).toBe("idle");
});

test("T02 four-click open rectangle shows open state", async ({ page }) => {
  await setupEmptyDocPage(page);
  const front = await projectRefPlaneCenter(page, "Front");
  if (!front) throw new Error("projection failed");

  // Select Front RefPlane
  await page.mouse.click(front.x, front.y);
  await page.waitForTimeout(100);

  // Start sketch
  const btn = page.locator('[data-testid="btn-start-sketch"]');
  await btn.click();
  await page.waitForTimeout(100);

  // Click 4 points for an open rectangle (not closed yet)
  // (0,0), (5,0), (5,5), (0,5) - approximate screen positions
  // We click near the center of the visible plane for simplicity
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  const cx = box.x + box.width * 0.5;
  const cy = box.y + box.height * 0.5;

  // Click 4 points in a rectangle pattern
  await page.mouse.click(cx - 50, cy - 50);
  await page.waitForTimeout(50);
  await page.mouse.click(cx + 50, cy - 50);
  await page.waitForTimeout(50);
  await page.mouse.click(cx + 50, cy + 50);
  await page.waitForTimeout(50);
  await page.mouse.click(cx - 50, cy + 50);
  await page.waitForTimeout(100);

  expect(await getSketchState(page)).toBe("open");
  expect(await hasSketchOpenClass(page)).toBe(true);
});

test("T03 fifth click within SNAP_RADIUS closes the loop", async ({ page }) => {
  await setupEmptyDocPage(page);
  const front = await projectRefPlaneCenter(page, "Front");
  if (!front) throw new Error("projection failed");

  await page.mouse.click(front.x, front.y);
  await page.waitForTimeout(100);

  const btn = page.locator('[data-testid="btn-start-sketch"]');
  await btn.click();
  await page.waitForTimeout(100);

  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  const cx = box.x + box.width * 0.5;
  const cy = box.y + box.height * 0.5;

  // Click 4 points for rectangle
  await page.mouse.click(cx - 50, cy - 50);
  await page.waitForTimeout(50);
  await page.mouse.click(cx + 50, cy - 50);
  await page.waitForTimeout(50);
  await page.mouse.click(cx + 50, cy + 50);
  await page.waitForTimeout(50);
  await page.mouse.click(cx - 50, cy + 50);
  await page.waitForTimeout(50);

  expect(await getSketchState(page)).toBe("open");

  // Click near the first point to close the loop
  await page.mouse.click(cx - 50, cy - 50);
  await page.waitForTimeout(100);

  expect(await getSketchState(page)).toBe("closed");
  expect(await hasSketchOpenClass(page)).toBe(false);
});

test("T04 Esc discards segments back to idle", async ({ page }) => {
  await setupEmptyDocPage(page);
  const front = await projectRefPlaneCenter(page, "Front");
  if (!front) throw new Error("projection failed");

  await page.mouse.click(front.x, front.y);
  await page.waitForTimeout(100);

  const btn = page.locator('[data-testid="btn-start-sketch"]');
  await btn.click();
  await page.waitForTimeout(100);

  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  const cx = box.x + box.width * 0.5;
  const cy = box.y + box.height * 0.5;

  // Click a few points
  await page.mouse.click(cx - 50, cy - 50);
  await page.waitForTimeout(50);
  await page.mouse.click(cx + 50, cy - 50);
  await page.waitForTimeout(50);

  expect(await getSketchState(page)).toBe("open");

  // Press Escape
  await page.keyboard.press("Escape");
  await page.waitForTimeout(100);

  expect(await getSketchState(page)).toBe("idle");
  expect(await hasSketchOpenClass(page)).toBe(false);
});

test("T05_boundary_open_finalize Enter on open returns null", async ({ page }) => {
  await setupEmptyDocPage(page);
  const front = await projectRefPlaneCenter(page, "Front");
  if (!front) throw new Error("projection failed");

  await page.mouse.click(front.x, front.y);
  await page.waitForTimeout(100);

  const btn = page.locator('[data-testid="btn-start-sketch"]');
  await btn.click();
  await page.waitForTimeout(100);

  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  const cx = box.x + box.width * 0.5;
  const cy = box.y + box.height * 0.5;

  // Click 3 points for open triangle (not closed)
  await page.mouse.click(cx - 50, cy - 50);
  await page.waitForTimeout(50);
  await page.mouse.click(cx + 50, cy - 50);
  await page.waitForTimeout(50);
  await page.mouse.click(cx, cy + 50);
  await page.waitForTimeout(100);

  const stateBefore = await getSketchState(page);
  expect(stateBefore).toBe("open");

  // Press Enter on open loop - finalize should return null
  // We can't directly test the return value, but state should remain "open"
  await page.keyboard.press("Enter");
  await page.waitForTimeout(100);

  expect(await getSketchState(page)).toBe("open");
});

test("T06_degen_zero_length zero-length segment is rejected", async ({ page }) => {
  await setupEmptyDocPage(page);
  const front = await projectRefPlaneCenter(page, "Front");
  if (!front) throw new Error("projection failed");

  await page.mouse.click(front.x, front.y);
  await page.waitForTimeout(100);

  const btn = page.locator('[data-testid="btn-start-sketch"]');
  await btn.click();
  await page.waitForTimeout(100);

  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  const cx = box.x + box.width * 0.5;
  const cy = box.y + box.height * 0.5;

  // Click first point
  await page.mouse.click(cx, cy);
  await page.waitForTimeout(50);

  // Click same position again (zero-length)
  await page.mouse.click(cx, cy);
  await page.waitForTimeout(100);

  // State should remain idle (no segments added)
  expect(await getSketchState(page)).toBe("idle");
});

// T11_auto_finalize_on_close (FN03 対応): 閉ループ達成時に自動 finalize が呼ばれ、
// console.log("[sketch] finalized:", ...) が出力されることを確認する。
test("T11_auto_finalize_on_close emits sketch finalized log", async ({ page }) => {
  const consoleMessages: string[] = [];
  page.on("console", (msg) => consoleMessages.push(msg.text()));

  await setupEmptyDocPage(page);
  const front = await projectRefPlaneCenter(page, "Front");
  if (!front) throw new Error("projection failed");
  await page.mouse.click(front.x, front.y);
  await page.waitForTimeout(100);

  const btn = page.locator('[data-testid="btn-start-sketch"]');
  await btn.click();
  await page.waitForTimeout(100);

  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");
  const cx = box.x + box.width * 0.5;
  const cy = box.y + box.height * 0.5;

  // Draw rectangle: 4 corners + close back to first
  await page.mouse.click(cx - 50, cy - 50);
  await page.waitForTimeout(30);
  await page.mouse.click(cx + 50, cy - 50);
  await page.waitForTimeout(30);
  await page.mouse.click(cx + 50, cy + 50);
  await page.waitForTimeout(30);
  await page.mouse.click(cx - 50, cy + 50);
  await page.waitForTimeout(30);
  await page.mouse.click(cx - 50, cy - 50); // close
  await page.waitForTimeout(150);

  expect(await getSketchState(page)).toBe("closed");
  // 自動 finalize で console に "[sketch] finalized:" が出力されている
  expect(consoleMessages.some((m) => m.includes("[sketch] finalized:"))).toBe(true);
});
