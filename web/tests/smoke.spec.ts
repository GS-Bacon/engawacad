/**
 * Smoke test for Phase 9 — minimal viewer hello-world.
 *
 * Verifies basic viewer functionality: page loads and has a title.
 * Full E2E tests will be added in Phase 21+.
 */
import { test, expect } from "@playwright/test";

test("viewer hello-world: page loads and has title", async ({ page }) => {
  await page.goto("/");
  // Page should have a non-empty title
  const title = await page.title();
  expect(title).toBeTruthy();
  expect(title.length).toBeGreaterThan(0);
});
