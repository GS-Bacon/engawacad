/**
 * Full-stack integration tests — real browser + real engawa-api.
 *
 * Design: GET /api/v0/mesh and GET /api/v0/features are mocked with the
 * simple_box_faces fixture so that face IDs are deterministic and plane
 * detection works reliably.  POST /api/v0/features is passed through to the
 * real server via route.continue(), so the actual API validation, feature
 * storage, and mesh rebuild all run against the real binary.
 *
 * This covers the gap that mocked extrude_panel tests cannot: the real server
 * must accept the payload the UI constructs and return parseable geometry.
 *
 * Tagged @stage2: tests involve user operations.
 */
import { test, expect } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import { assertMeshHealthy } from "./helpers.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function loadFacesFixture(): unknown {
  return JSON.parse(
    fs.readFileSync(
      path.join(__dirname, "fixtures", "simple_box_faces.json"),
      "utf-8",
    ),
  );
}

/**
 * Set up a page where:
 * - GET /api/v0/mesh    → simple_box_faces fixture (known face IDs)
 * - GET /api/v0/features → empty list
 * - POST /api/v0/features → real server (route.continue())
 */
async function setupIntegrationPage(
  page: import("@playwright/test").Page,
): Promise<void> {
  const fixture = loadFacesFixture();

  await page.route("/api/v0/mesh", (route) => {
    if (route.request().method() === "GET") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(fixture),
      });
    }
    return route.continue();
  });

  // All /api/v0/features requests (GET for feature IDs, POST for operations)
  // go to the real server so that usedFeatureIds is correctly populated and
  // ID collisions across tests are avoided.
  // (No route intercept needed — real server handles it.)

  // Suppress logger sidecar CORS noise (port 7879 not started in test env)
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
  await page.waitForFunction(() => (window as any).__meshData !== undefined, {
    timeout: 10_000,
  });
  await page.waitForTimeout(800); // OrbitControls settle
}

// ---------------------------------------------------------------------------
// I1: Full-stack extrude — UI computes plane, POSTs to real API, renders result
// ---------------------------------------------------------------------------
test(
  "I1 full-stack: face pick → extrude → real API updates scene @stage2",
  async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });

    await setupIntegrationPage(page);

    // Select top face (center of canvas in iso view)
    const canvas = page.locator("canvas");
    const box = await canvas.boundingBox();
    if (!box) throw new Error("canvas not found");
    await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);

    const panel = page.locator('[data-testid="extrude-panel"]');
    await expect(panel).not.toHaveCSS("display", "none");

    const vBefore = await page.evaluate(
      () => (window as any).__meshDataVersion ?? 0,
    );

    await page.fill('[data-testid="extrude-depth"]', "3");
    await page.click('[data-testid="btn-extrude"]');

    // Real server processes the POST and returns updated mesh → scene rebuilds
    await page.waitForFunction(
      (v: number) => ((window as any).__meshDataVersion ?? 0) > v,
      vBefore,
      { timeout: 8_000 },
    );

    // Panel hides on success (selection cleared)
    await expect(panel).toHaveCSS("display", "none");

    // Resulting mesh must be geometrically sound
    await assertMeshHealthy(page);
    expect(consoleErrors).toHaveLength(0);
  },
);

// ---------------------------------------------------------------------------
// I2: Full-stack extrude-cut — same real-API round trip for cut operation
// ---------------------------------------------------------------------------
test(
  "I2 full-stack: face pick → extrude-cut → real API updates scene @stage2",
  async ({ page }) => {
    await setupIntegrationPage(page);

    const canvas = page.locator("canvas");
    const box = await canvas.boundingBox();
    if (!box) throw new Error("canvas not found");
    await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);

    const panel = page.locator('[data-testid="extrude-panel"]');
    await expect(panel).not.toHaveCSS("display", "none");

    const vBefore = await page.evaluate(
      () => (window as any).__meshDataVersion ?? 0,
    );

    await page.fill('[data-testid="extrude-depth"]', "2");
    await page.click('[data-testid="btn-extrude-cut"]');

    await page.waitForFunction(
      (v: number) => ((window as any).__meshDataVersion ?? 0) > v,
      vBefore,
      { timeout: 8_000 },
    );

    await expect(panel).toHaveCSS("display", "none");
    // extrude-cut creates a hole — mesh must still be manifold
    await assertMeshHealthy(page);
  },
);
