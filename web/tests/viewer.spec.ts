import { test, expect, type Page } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const FIXTURES_DIR = path.join(__dirname, "fixtures");

const NON_ASSEMBLY_EXAMPLES = [
  "simple_box",
  "cylinder",
  "sphere",
  "boolean_box_cut",
  "boolean_box_fuse",
  "boolean_box_intersect",
  "boolean_box_void",
  "boolean_cut_cylinder_hole",
  "boolean_cut_sphere_dimple",
  "boolean_fuse_box_cyl",
  "boolean_intersect_box_cyl",
  "boolean_intersect_cyl_sphere",
  "cylinder_offset",
  "sphere_offset",
  "extruded_rect",
  "two_bodies",
];

function loadFixture(name: string): unknown {
  return JSON.parse(
    fs.readFileSync(path.join(FIXTURES_DIR, `${name}.json`), "utf-8"),
  );
}

async function setupPageWithFixture(
  page: Page,
  fixtureName: string,
): Promise<string[]> {
  const consoleErrors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") {
      consoleErrors.push(msg.text());
    }
  });

  const fixture = loadFixture(fixtureName);
  await page.route("/api/v0/mesh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(fixture),
    }),
  );
  await page.route("/api/v0/features", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([]),
    }),
  );

  return consoleErrors;
}

// T01: Console no error - simple_box
test("T01 console no error - simple_box", async ({ page }) => {
  const consoleErrors = await setupPageWithFixture(page, "simple_box");
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });

  expect(consoleErrors).toHaveLength(0);
});

// T02: Screenshot comparison - simple_box
test("T02 screenshot - simple_box", async ({ page }) => {
  await setupPageWithFixture(page, "simple_box");
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  await page.waitForTimeout(1000);

  await expect(page).toHaveScreenshot("simple_box.png", {
    maxDiffPixelRatio: 0.03,
  });
});

// T03: Console no error - all non-assembly examples
const LOOPED_EXAMPLES = NON_ASSEMBLY_EXAMPLES.filter((n) => n !== "simple_box");

for (const name of LOOPED_EXAMPLES) {
  test(`T03 console no error - ${name}`, async ({ page }) => {
    const consoleErrors = await setupPageWithFixture(page, name);
    await page.goto("/");
    await page.waitForSelector("canvas", { timeout: 10_000 });

    expect(consoleErrors).toHaveLength(0);
  });
}

// T04: Boundary - empty response (no crash)
test("T04 boundary - empty response", async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") {
      consoleErrors.push(msg.text());
    }
  });

  await page.route("/api/v0/mesh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: "[]",
    }),
  );

  await page.goto("/");
  await page.waitForTimeout(2000);

  // "No bodies found" displayed in #error div
  const errorEl = page.locator("#error");
  await expect(errorEl).toBeVisible();
  await expect(errorEl).toContainText("No bodies found");

  // No console errors beyond the expected UI message
  expect(consoleErrors.length).toBeLessThanOrEqual(1);
});

// T05: renderer pixelRatio — skeleton (#90)
test("T05 renderer pixelRatio matches devicePixelRatio", async ({ page }) => {
  await setupPageWithFixture(page, "simple_box");
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  await page.waitForTimeout(500);
  const ratio = await page.evaluate(() => (window as any).__viewer?.renderer.getPixelRatio());
  const winRatio = await page.evaluate(() => window.devicePixelRatio);
  expect(ratio).toBe(winRatio);
});

// T06: OrbitControls enableDamping — skeleton (#90)
test("T06 OrbitControls enableDamping is true", async ({ page }) => {
  await setupPageWithFixture(page, "simple_box");
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  await page.waitForTimeout(500);
  const damping = await page.evaluate(
    () => (window as any).__viewer?.controls.enableDamping,
  );
  expect(damping).toBe(true);
});
