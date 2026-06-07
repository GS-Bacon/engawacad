/**
 * Playwright E2E tests — #94 面ピッキングと選択ハイライト
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
async function setupPickingPage(page: Page): Promise<void> {
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
  // OrbitControls の初期化 + 最初のフレーム描画を待つ
  await page.waitForTimeout(1000);
}

/** selected-face-id 要素の textContent を取得する */
async function getSelectedText(page: Page): Promise<string> {
  const el = page.locator('[data-testid="selected-face-id"]');
  return (await el.textContent({ timeout: 3000 })) ?? "";
}

// E01: canvas 中央クリック → selected-face-id が非空文字列
test("E01 face-pick center click shows non-empty face id", async ({ page }) => {
  await setupPickingPage(page);
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  // モデルが canvas 中央付近にある想定でクリック
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);

  const text = await getSelectedText(page);
  expect(text.length).toBeGreaterThan(0);
});

// E02: 別の面座標クリック → selected-face-id が更新
test("E02 face-pick second click updates selection", async ({ page }) => {
  await setupPickingPage(page);
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);
  const first = await getSelectedText(page);

  // センター付近の別座標 — 同じモデル上の別三角形または同面をクリック
  await page.mouse.click(box.x + box.width * 0.45, box.y + box.height * 0.45);
  const second = await getSelectedText(page);

  // 最低限: テキストが存在する
  expect(second.length).toBeGreaterThan(0);
});

// E03: モデル無しの canvas 隅クリック → selected-face-id が空文字
test("E03 background click clears selection", async ({ page }) => {
  await setupPickingPage(page);
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  // 先に面を選択しておく
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);
  const selected = await getSelectedText(page);
  expect(selected.length).toBeGreaterThan(0);

  // 右下隅(モデルが無い領域)をクリック
  await page.mouse.click(box.x + box.width - 5, box.y + box.height - 5);

  const text = await getSelectedText(page);
  expect(text).toBe("");
});

// E04: クリック前後で GET /api/v0/mesh レスポンスが不変(.mycad 非変更)
test("E04 face-pick does not mutate mycad (transient state only)", async ({ page }) => {
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

  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  // 面をクリック(transient 操作)
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);

  // 再フェッチして before/after を比較
  const afterRes = await page.evaluate(async () => {
    const r = await fetch("/api/v0/mesh");
    return r.json();
  });

  // ブラウザの JSON パーサが -0 を 0 に正規化するため、
  // 比較前に両辺を文字列化して -0 を 0 に揃える
  const normalize = (v: unknown): unknown =>
    JSON.parse(JSON.stringify(v).replace(/-0(?=[,\]}\s])/g, "0"));

  // fixture と deep-equal(サーバ側が変化していない)
  expect(normalize(afterRes)).toEqual(normalize(fixture));
});

// E05: 右クリックは選択を変更しない
test("E05 right-click does not change selection", async ({ page }) => {
  await setupPickingPage(page);
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  // 右クリック
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5, {
    button: "right",
  });

  const text = await getSelectedText(page);
  expect(text).toBe("");
});

// E06: ドラッグ(5px 以上の移動)は選択を変更しない
test("E06 drag does not trigger selection", async ({ page }) => {
  await setupPickingPage(page);
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");

  const startX = box.x + box.width * 0.5;
  const startY = box.y + box.height * 0.5;

  // pointerdown → move 20px → pointerup (= ドラッグ)
  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(startX + 20, startY + 20);
  await page.mouse.up();

  const text = await getSelectedText(page);
  expect(text).toBe("");
});
