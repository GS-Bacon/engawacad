import type { Page } from "@playwright/test";
import { expect } from "@playwright/test";

/**
 * Assert mesh geometric invariants using window.__meshData exposed by viewer.ts.
 *
 * Checks per body:
 *   - triangle count > 0
 *   - no NaN / Infinity in positions or normals
 *   - bounding box non-degenerate (not all vertices at a single point)
 *   - edge-manifold: every undirected edge shared by exactly 2 triangles
 *     (naked edges > 0 means holes / non-manifold topology)
 *
 * Must be called after canvas has rendered at least one frame so that
 * buildScene has populated window.__meshData.
 */
export async function assertMeshHealthy(page: Page): Promise<void> {
  const result = await page.evaluate(() => {
    const meshData: Array<{ positions: number[]; normals: number[]; indices: number[] }> =
      (window as any).__meshData ?? null;

    if (meshData === null) {
      return { missing: true, errors: [] as string[] };
    }

    if (meshData.length === 0) {
      return { missing: false, errors: ["no bodies in __meshData (empty mesh)"] };
    }

    const errors: string[] = [];

    for (let b = 0; b < meshData.length; b++) {
      const { positions, normals, indices } = meshData[b];
      const tag = `body[${b}]`;

      // Triangle count > 0
      const triCount = indices.length / 3;
      if (triCount === 0) {
        errors.push(`${tag}: 0 triangles`);
        continue; // remaining checks require indices
      }

      // NaN / Infinity in positions
      let badPos = -1;
      for (let i = 0; i < positions.length; i++) {
        if (!isFinite(positions[i])) { badPos = i; break; }
      }
      if (badPos >= 0) {
        errors.push(`${tag}: position[${badPos}]=${positions[badPos]} is non-finite`);
      }

      // NaN / Infinity in normals
      let badNorm = -1;
      for (let i = 0; i < normals.length; i++) {
        if (!isFinite(normals[i])) { badNorm = i; break; }
      }
      if (badNorm >= 0) {
        errors.push(`${tag}: normal[${badNorm}]=${normals[badNorm]} is non-finite`);
      }

      // Bounding box non-degenerate
      let minX = Infinity, maxX = -Infinity;
      let minY = Infinity, maxY = -Infinity;
      let minZ = Infinity, maxZ = -Infinity;
      for (let i = 0; i < positions.length; i += 3) {
        if (positions[i]     < minX) minX = positions[i];
        if (positions[i]     > maxX) maxX = positions[i];
        if (positions[i + 1] < minY) minY = positions[i + 1];
        if (positions[i + 1] > maxY) maxY = positions[i + 1];
        if (positions[i + 2] < minZ) minZ = positions[i + 2];
        if (positions[i + 2] > maxZ) maxZ = positions[i + 2];
      }
      const eps = 1e-9;
      if (maxX - minX < eps && maxY - minY < eps && maxZ - minZ < eps) {
        errors.push(`${tag}: degenerate bounding box — all vertices at a single point`);
      }

      // Edge-manifold: each undirected edge must appear exactly 2 times
      // for a closed solid.  Use POSITION-based edge keys (rounded to 6dp)
      // rather than index-based, because flat-shaded meshes duplicate vertices
      // at face boundaries — adjacent faces share the same POSITION but use
      // different vertex indices.  Position-based grouping correctly identifies
      // genuinely shared edges regardless of the indexing scheme.
      const PREC = 6;
      // Normalize -0 → 0 and near-zero values before toFixed so that
      // e.g. "-0.000000" (from -6e-17) and "0.000000" (from +6e-17)
      // are treated as the same position key.
      const round = (x: number): string => {
        const r = parseFloat(x.toFixed(PREC));
        return (r === 0 ? 0 : r).toFixed(PREC);
      };
      // Map each vertex index to a canonical position key
      const posKey = (idx: number) =>
        `${round(positions[idx * 3])},${round(positions[idx * 3 + 1])},${round(positions[idx * 3 + 2])}`;

      const edgeCounts = new Map<string, number>();
      for (let t = 0; t < indices.length; t += 3) {
        const ka = posKey(indices[t]);
        const kb = posKey(indices[t + 1]);
        const kc = posKey(indices[t + 2]);
        for (const [u, v] of [[ka, kb], [kb, kc], [kc, ka]] as [string, string][]) {
          const key = u < v ? `${u}|${v}` : `${v}|${u}`;
          edgeCounts.set(key, (edgeCounts.get(key) ?? 0) + 1);
        }
      }
      let nakedEdges = 0;
      for (const count of edgeCounts.values()) {
        if (count !== 2) nakedEdges++;
      }
      if (nakedEdges > 0) {
        errors.push(`${tag}: ${nakedEdges} naked edge(s) — mesh has holes or non-manifold topology`);
      }
    }

    return { missing: false, errors };
  });

  if (result.missing) {
    // __meshData not yet populated — fail with a clear message
    throw new Error(
      "assertMeshHealthy: window.__meshData is not set. " +
      "Ensure the viewer has rendered at least one frame before calling this function.",
    );
  }
  expect(result.errors, "mesh geometric invariants").toEqual([]);
}

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
  // ロガーサイドカー (port 7879) は test 環境では起動しない。
  // CORS preflight が console.error を出すのを防ぐためにモックする。
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
