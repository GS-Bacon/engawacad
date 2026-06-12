import { test, expect, type Page } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import { setupPageWithServer, assertMeshHealthy, mockLoggerSidecar } from "./helpers.js";

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

  await mockLoggerSidecar(page);

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

// T01: Console no error — real API server (no fixture mock)
test("T01 console no error - simple_box @stage1", async ({ page }) => {
  const consoleErrors = await setupPageWithServer(page);
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });

  expect(consoleErrors).toHaveLength(0);
});

// T02: Screenshot comparison - simple_box
// Platform note: baseline filename is "simple-box-linux.png" on Linux (Playwright appends platform).
test("T02 screenshot - simple_box @stage1", async ({ page }) => {
  await setupPageWithFixture(page, "simple_box");
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  // Wait for at least one GPU frame to be sent instead of an arbitrary fixed delay.
  await page.waitForFunction(
    () => (window as any).__viewer?.renderer?.info?.render?.frame > 0,
    { timeout: 10_000 },
  );

  await expect(page).toHaveScreenshot("simple_box.png", {
    maxDiffPixelRatio: 0.03,
  });
});

// T03: Console no error + mesh geometric invariants - all non-assembly examples
// Checks: triangle count > 0, no NaN/Infinity, non-degenerate bbox, edge-manifold.
//
// Known-broken curved-surface booleans that still have naked edges:
// - boolean_intersect_box_cyl: n_u resolution mismatch across face boundary (#131 follow-up)
const KNOWN_NAKED_EDGE_EXAMPLES = new Set([
  "boolean_intersect_box_cyl",
]);

const LOOPED_EXAMPLES = NON_ASSEMBLY_EXAMPLES.filter((n) => n !== "simple_box");

for (const name of LOOPED_EXAMPLES) {
  test(`T03 console no error - ${name} @stage1`, async ({ page }) => {
    test.skip(KNOWN_NAKED_EDGE_EXAMPLES.has(name), `${name} has known naked-edge defects`);
    const consoleErrors = await setupPageWithFixture(page, name);
    await page.goto("/");
    await page.waitForSelector("canvas", { timeout: 10_000 });
    // Wait for at least one rendered frame so __meshData is populated
    await page.waitForFunction(
      () => (window as any).__meshData !== undefined,
      { timeout: 10_000 },
    );

    expect(consoleErrors).toHaveLength(0);
    await assertMeshHealthy(page);
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
  // Wait for the error element to appear instead of an arbitrary fixed delay.
  await page.waitForSelector("#error", { state: "visible", timeout: 10_000 });

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

// T07: view buttons exist in DOM — skeleton (#91)
test("T07 view buttons exist in DOM", async ({ page }) => {
  await setupPageWithFixture(page, "simple_box");
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  await expect(page.locator('[data-testid="btn-view-front"]')).toBeVisible();
  await expect(page.locator('[data-testid="btn-view-top"]')).toBeVisible();
  await expect(page.locator('[data-testid="btn-view-iso"]')).toBeVisible();
});

// T08: front view button — skeleton (#91)
test("T08 btn-view-front positions camera on +Z axis of target", async ({ page }) => {
  await setupPageWithFixture(page, "simple_box");
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  await page.waitForTimeout(500);
  await page.click('[data-testid="btn-view-front"]');
  await page.waitForTimeout(200);
  const delta = await page.evaluate(() => {
    const { camera, controls } = (window as any).__viewer;
    const t = controls.target;
    return { dx: camera.position.x - t.x, dy: camera.position.y - t.y, dz: camera.position.z - t.z };
  });
  expect(delta.dz).toBeGreaterThan(0);
  expect(Math.abs(delta.dx)).toBeLessThan(0.01);
  expect(Math.abs(delta.dy)).toBeLessThan(0.01);
});

// T09: top view button — skeleton (#91)
test("T09 btn-view-top positions camera on +Y axis of target", async ({ page }) => {
  await setupPageWithFixture(page, "simple_box");
  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  await page.waitForTimeout(500);
  await page.click('[data-testid="btn-view-top"]');
  await page.waitForTimeout(200);
  const result = await page.evaluate(() => {
    const { camera, controls } = (window as any).__viewer;
    const t = controls.target;
    return {
      dx: camera.position.x - t.x,
      dy: camera.position.y - t.y,
      dz: camera.position.z - t.z,
      upZ: camera.up.z,
    };
  });
  expect(result.dy).toBeGreaterThan(0);
  expect(Math.abs(result.dx)).toBeLessThan(0.01);
  expect(Math.abs(result.dz)).toBeLessThan(0.01);
  expect(result.upZ).toBe(-1); // up=(0,0,-1) で look 方向と平行にならない
});
