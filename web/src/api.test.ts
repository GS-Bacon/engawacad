/**
 * Unit tests for web/src/api.ts — #102 GET /api/v0/features
 *
 * T_api_01: fetchAllFeatureIds — 200 OK → returns string[]
 * T_api_02: fetchAllFeatureIds — 500 → throws with error message
 * T_api_03: fetchAllFeatureIds — 500 non-JSON → throws HTTP status
 * T_api_04: fetchAllFeatureIds — 422 with empty error → throws HTTP status
 * T_api_05: fetchAllFeatureIds determinism — mock returns same data 100 times
 * T_api_06: fetchBodies — 200 OK → returns BodyMesh[]
 * T_api_07: fetchBodies — 500 → throws with error message
 */
import { describe, it, expect, afterEach } from "vitest";
import { fetchAllFeatureIds, fetchBodies } from "./api";

const originalFetch = globalThis.fetch;

function mockFetch(status: number, body: unknown, contentType = "application/json") {
  globalThis.fetch = async () =>
    new Response(
      typeof body === "string" ? body : JSON.stringify(body),
      { status, headers: { "content-type": contentType } },
    );
}

describe("fetchAllFeatureIds", () => {
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it("T_api_01: 200 OK returns string array", async () => {
    mockFetch(200, ["sketch_1", "extrude_1", "box_1"]);
    const ids = await fetchAllFeatureIds();
    expect(ids).toEqual(["sketch_1", "extrude_1", "box_1"]);
  });

  it("T_api_02: 500 with error body → throws with error message", async () => {
    mockFetch(500, { error: "Internal server error" });
    await expect(fetchAllFeatureIds()).rejects.toThrow("Internal server error");
  });

  it("T_api_03: 500 with non-JSON body → throws HTTP status", async () => {
    globalThis.fetch = async () =>
      new Response("Bad Gateway", {
        status: 502,
        headers: { "content-type": "text/plain" },
      });
    await expect(fetchAllFeatureIds()).rejects.toThrow("HTTP 502");
  });

  it("T_api_04: 422 with empty error → throws HTTP status", async () => {
    mockFetch(422, { error: "" });
    await expect(fetchAllFeatureIds()).rejects.toThrow("HTTP 422");
  });

  it("T_api_05: returns empty array for no features", async () => {
    mockFetch(200, []);
    const ids = await fetchAllFeatureIds();
    expect(ids).toEqual([]);
  });

  it("T_api_06: deterministic — 100 calls return identical results", async () => {
    const expected = ["sketch_1", "extrude_1"];
    // Mock always returns the same response
    mockFetch(200, expected);
    const first = await fetchAllFeatureIds();
    expect(first).toEqual(expected);
    for (let i = 0; i < 99; i++) {
      const result = await fetchAllFeatureIds();
      expect(result).toEqual(first);
    }
  });
});

describe("fetchBodies", () => {
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it("T_api_07: 200 OK returns BodyMesh array", async () => {
    const body = [
      { feature_id: "box_1", mesh: { positions: [], normals: [], indices: [], face_ids: [] } },
    ];
    mockFetch(200, body);
    const result = await fetchBodies();
    expect(result).toEqual(body);
  });

  it("T_api_08: 500 with error body → throws with error message", async () => {
    mockFetch(500, { error: "disk read failure" });
    await expect(fetchBodies()).rejects.toThrow("disk read failure");
  });

  it("T_api_09: 404 non-JSON → throws HTTP status", async () => {
    globalThis.fetch = async () =>
      new Response("Not Found", {
        status: 404,
        headers: { "content-type": "text/plain" },
      });
    await expect(fetchBodies()).rejects.toThrow("HTTP 404");
  });
});
