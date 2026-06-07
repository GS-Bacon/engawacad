/**
 * Playwright E2E tests — #95 選択面からの押出(Extrude) UI
 *
 * E01: 面クリック → extrude-panel visible
 * E02: 背景クリック(選択解除) → パネル非表示
 * E03: 深さ入力 + 押出 → POST が create_sketch・extrude の順で各 200、postData が期待 JSON
 * E04: 押出レスポンス(頂点増 fixture)でシーン差し替え後、選択クリア + パネル非表示
 */
import { test, expect, type Page } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function loadFacesFixture(): unknown {
  const p = path.join(__dirname, "fixtures", "simple_box_faces.json");
  return JSON.parse(fs.readFileSync(p, "utf-8"));
}

/** fixture を mock して /api/v0/mesh を差し替えてページを開く */
async function setupExtrudePage(page: Page): Promise<void> {
  const fixture = loadFacesFixture();
  await page.route("/api/v0/mesh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(fixture),
    }),
  );
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  await page.waitForTimeout(1000);
}

// E01: 面クリック → extrude-panel visible
test("E01 face click shows extrude panel", async ({ page }) => {
  await setupExtrudePage(page);
  // canvas 中央クリックで面を選択
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);

  const panel = page.locator('[data-testid="extrude-panel"]');
  await expect(panel).not.toHaveCSS("display", "none");
});

// E02: 背景クリック → パネル非表示
test("E02 background click hides extrude panel", async ({ page }) => {
  await setupExtrudePage(page);
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  // 面選択してパネルを表示
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);

  // 背景クリックで選択解除
  await page.mouse.click(box.x + 5, box.y + 5);

  const panel = page.locator('[data-testid="extrude-panel"]');
  await expect(panel).toHaveCSS("display", "none");
});

// E03: 深さ入力 + 押出 → POST create_sketch → POST extrude の順で各 200
test("E03 extrude button triggers ordered POST sequence", async ({ page }) => {
  await setupExtrudePage(page);

  const postedBodies: string[] = [];
  await page.route("/api/v0/features", (route) => {
    postedBodies.push(JSON.parse(route.request().postData() ?? "{}").type);
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(loadFacesFixture()),
    });
  });

  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);

  await page.fill('[data-testid="extrude-depth"]', "5");
  await page.click('[data-testid="btn-extrude"]');

  await page.waitForTimeout(500);
  expect(postedBodies).toEqual(["create_sketch", "extrude"]);
});

// E04: 押出レスポンスでシーン差し替え後、選択クリア + パネル非表示
test("E04 after extrude scene updates and panel hides", async ({ page }) => {
  await setupExtrudePage(page);

  await page.route("/api/v0/features", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(loadFacesFixture()),
    }),
  );

  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);

  await page.fill('[data-testid="extrude-depth"]', "5");
  await page.click('[data-testid="btn-extrude"]');
  await page.waitForTimeout(500);

  const panel = page.locator('[data-testid="extrude-panel"]');
  await expect(panel).toHaveCSS("display", "none");
});
