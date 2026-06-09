/**
 * Promo chain tests (@stage2) — multi-step operation sequences.
 *
 * Demonstrates: face selection → extrude → extrude-cut → view angles.
 * Tagged @stage2 so xtask tiles them into the "operations" segment of
 * the promotional dashboard video (acceptance-promo.mp4).
 *
 * PLAYWRIGHT_VIDEO=1 guard: small pauses between steps aid visual clarity
 * in recorded video without slowing down normal CI runs.
 */
import { test, expect } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import { assertMeshHealthy } from "./helpers.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const RECORD = process.env.PLAYWRIGHT_VIDEO === "1";

/** Brief pause inserted only during video recording for visual clarity. */
async function beat(page: import("@playwright/test").Page, ms = 600): Promise<void> {
  if (RECORD) await page.waitForTimeout(ms);
}

function loadFixture(name: string): unknown {
  return JSON.parse(
    fs.readFileSync(path.join(__dirname, "fixtures", `${name}.json`), "utf-8"),
  );
}

/** Set up page with simple_box_faces fixture (has face_ids for picking). */
async function setupPage(page: import("@playwright/test").Page): Promise<void> {
  const fixture = loadFixture("simple_box_faces");

  await page.route("/api/v0/mesh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(fixture),
    }),
  );
  // POST /features returns the same fixture as the "updated" bodies
  // (demonstrates viewer rebuild; shape change is exercised in acceptance_extrude)
  await page.route("/api/v0/features", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(fixture),
    }),
  );

  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  await page.waitForFunction(() => (window as any).__meshData !== undefined, {
    timeout: 10_000,
  });
  // Allow OrbitControls + first render to settle
  await page.waitForTimeout(800);
}

/** Click the canvas centre (hits the box top face). */
async function clickCenter(page: import("@playwright/test").Page): Promise<void> {
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas not found");
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);
}

// ---------------------------------------------------------------------------
// P01: Single extrude chain — face pick → depth input → extrude → scene rebuild
// ---------------------------------------------------------------------------
test("P01 extrude chain: face pick to scene rebuild @stage2", async ({ page }) => {
  await setupPage(page);
  await beat(page);

  // Step 1: select a face
  await clickCenter(page);
  await beat(page);

  const panel = page.locator('[data-testid="extrude-panel"]');
  await expect(panel).not.toHaveCSS("display", "none");

  // Step 2: fill depth and confirm
  const versionBefore = await page.evaluate(
    () => (window as any).__meshDataVersion ?? 0,
  );
  await page.fill('[data-testid="extrude-depth"]', "5");
  await beat(page, 400);
  await page.click('[data-testid="btn-extrude"]');
  await beat(page);

  // Step 3: assert scene rebuilt + panel hidden
  await page.waitForFunction(
    (v: number) => ((window as any).__meshDataVersion ?? 0) > v,
    versionBefore,
    { timeout: 5_000 },
  );
  await expect(panel).toHaveCSS("display", "none");
  await assertMeshHealthy(page);
});

// ---------------------------------------------------------------------------
// P02: Extrude-cut chain — face pick → cut → scene rebuild
// ---------------------------------------------------------------------------
test("P02 extrude-cut chain: face pick to scene rebuild @stage2", async ({ page }) => {
  await setupPage(page);
  await beat(page);

  await clickCenter(page);
  await beat(page);

  const panel = page.locator('[data-testid="extrude-panel"]');
  await expect(panel).not.toHaveCSS("display", "none");

  const versionBefore = await page.evaluate(
    () => (window as any).__meshDataVersion ?? 0,
  );
  await page.fill('[data-testid="extrude-depth"]', "3");
  await beat(page, 400);
  await page.click('[data-testid="btn-extrude-cut"]');
  await beat(page);

  await page.waitForFunction(
    (v: number) => ((window as any).__meshDataVersion ?? 0) > v,
    versionBefore,
    { timeout: 5_000 },
  );
  await expect(panel).toHaveCSS("display", "none");
  await assertMeshHealthy(page);
});

// ---------------------------------------------------------------------------
// P03: View angle tour — iso → front → top → iso
// Demonstrates camera controls and model from multiple angles.
// ---------------------------------------------------------------------------
test("P03 view angle tour: iso front top @stage2", async ({ page }) => {
  await setupPage(page);
  await beat(page);

  // Start from iso (default) — wait for initial render
  await assertMeshHealthy(page);
  await beat(page);

  // Front view
  await page.click('[data-testid="btn-view-front"]');
  await beat(page);
  const frontDelta = await page.evaluate(() => {
    const { camera, controls } = (window as any).__viewer;
    const t = controls.target;
    return { dz: camera.position.z - t.z };
  });
  expect(frontDelta.dz).toBeGreaterThan(0);
  await beat(page);

  // Top view
  await page.click('[data-testid="btn-view-top"]');
  await beat(page);
  const topDelta = await page.evaluate(() => {
    const { camera, controls } = (window as any).__viewer;
    const t = controls.target;
    return { dy: camera.position.y - t.y };
  });
  expect(topDelta.dy).toBeGreaterThan(0);
  await beat(page);

  // Back to iso
  await page.click('[data-testid="btn-view-iso"]');
  await beat(page);
  await assertMeshHealthy(page);
});

// ---------------------------------------------------------------------------
// P04: Boolean model tour — load each fixture and verify mesh health
// Shows the full primitive / boolean catalogue in a single sequential test.
// ---------------------------------------------------------------------------
const SHOWCASE_MODELS = [
  "simple_box",
  "cylinder",
  "sphere",
  "extruded_rect",
  "boolean_box_cut",
  "boolean_box_fuse",
  "boolean_box_intersect",
  "boolean_cut_cylinder_hole",
  "boolean_fuse_box_cyl",
  "boolean_intersect_box_cyl",
];

test("P04 boolean model tour: primitives and booleans @stage2", async ({ page }) => {
  // Start with the first model
  const first = loadFixture(SHOWCASE_MODELS[0]);
  let currentFixture = first;

  await page.route("/api/v0/mesh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(currentFixture),
    }),
  );
  await page.route("/api/v0/features", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(currentFixture),
    }),
  );

  await page.goto("/");
  await page.waitForSelector("canvas", { timeout: 10_000 });
  await page.waitForFunction(() => (window as any).__meshData !== undefined, {
    timeout: 10_000,
  });
  await page.waitForTimeout(600);

  for (const name of SHOWCASE_MODELS) {
    // Hot-swap the mesh via fetch re-route
    currentFixture = loadFixture(name);
    await page.route("/api/v0/mesh", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(currentFixture),
      }),
    );

    // Trigger reload by re-navigating (simplest way to re-run main()).
    // After goto(), the JS context resets so __meshDataVersion starts fresh
    // from undefined; just wait until __meshData is populated (≥ 1 frame rendered).
    await page.goto("/");
    await page.waitForSelector("canvas", { timeout: 10_000 });
    await page.waitForFunction(
      () => (window as any).__meshData !== undefined,
      { timeout: 10_000 },
    );
    await beat(page, 800);

    // Iso view for consistency
    await page.click('[data-testid="btn-view-iso"]');
    await beat(page, 300);
    // Note: health check (naked edges / manifold) is covered by viewer.spec.ts T03
    // per model. P04 is a visual tour — we just confirm the scene rendered at all.
  }
});
