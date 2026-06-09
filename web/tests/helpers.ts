import type { Page } from "@playwright/test";

/**
 * Set up a page connected to the real API server (no route mocking).
 * The API server is started automatically by playwright.config.ts webServer.
 * Returns an array of console error messages captured during the test.
 */
export async function setupPageWithServer(page: Page): Promise<string[]> {
  const consoleErrors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") {
      consoleErrors.push(msg.text());
    }
  });
  return consoleErrors;
}

/**
 * Original fixture-based setup (kept for backward compatibility).
 * Intercepts /api/v0/mesh and /api/v0/features with static fixture data.
 */
export async function setupPageWithFixture(
  page: Page,
  fixture: unknown,
): Promise<string[]> {
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
