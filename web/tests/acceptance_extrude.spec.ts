/**
 * Acceptance tests — Phase 7 Extrude / ExtrudeCut テストマトリクス (Issue #123)
 *
 * 対象: POST /api/v0/features への Extrude/ExtrudeCut × 境界値
 * 実行: cargo xtask acceptance  (Playwright API テスト、ブラウザ不要)
 *
 * simple_box.mycad: width=10, height=20, depth=30, 原点中心 z∈[-15,15]
 * XY 平面スケッチ → z 正方向に Extrude / box_1 へ ExtrudeCut
 * face距離 = 15.0 (XY 平面 z=0 から top face z=+15 まで)
 *
 * 各テストは独立して実行可能（累積状態に依存しない絶対アサート）。
 * 一意の ID を使用するため並列実行（--workers N）に対応。
 *
 * Phase 7 拡張ポイント: CreateSketch 起点ケースは末尾コメントを参照
 */
import { test, expect } from "@playwright/test";

const API_BASE = "http://127.0.0.1:7878";

// XY 平面上の矩形スケッチ (origin centered, [-2,2]×[-4,4])
function makeSketch(id: string) {
  return {
    type: "create_sketch",
    id,
    plane: "xy",
    profile: [
      { id: `${id}_s0`, from: [-2, -4], to: [2, -4] },
      { id: `${id}_s1`, from: [2, -4], to: [2, 4] },
      { id: `${id}_s2`, from: [2, 4], to: [-2, 4] },
      { id: `${id}_s3`, from: [-2, 4], to: [-2, -4] },
    ],
  };
}

async function postFeature(
  request: any,
  body: Record<string, unknown>,
): Promise<{ status: number; text: string }> {
  const res = await request.post(`${API_BASE}/api/v0/features`, { data: body });
  const text = await res.text();
  return { status: res.status(), text };
}

function hasMeshVertices(responseText: string): boolean {
  try {
    const bodies = JSON.parse(responseText) as Array<{
      mesh: { positions: unknown[] };
    }>;
    return (
      Array.isArray(bodies) &&
      bodies.length > 0 &&
      bodies.some((b) => b.mesh.positions.length > 0)
    );
  } catch {
    return false;
  }
}

// ---------------------------------------------------------------------------
// Extrude 正側面
// ---------------------------------------------------------------------------

// T01: Extrude 正側面 depth=2.0
test("T01_extrude_normal depth=2.0", async ({ request }) => {
  const { status: s1 } = await postFeature(request, makeSketch("sk_t01"));
  expect(s1).toBe(200);

  const { status, text } = await postFeature(request, {
    type: "extrude",
    id: "ext_t01",
    sketch: "sk_t01",
    depth: 2.0,
  });
  expect(status).toBe(200);
  expect(text).not.toContain('"error"');
  expect(hasMeshVertices(text)).toBeTruthy();
  expect(text).toContain('"feature_id":"ext_t01"');
});

// T02: Extrude 正側面 depth=0.01 — 最小値
test("T02_extrude_min depth=0.01", async ({ request }) => {
  const { status: s1 } = await postFeature(request, makeSketch("sk_t02"));
  expect(s1).toBe(200);

  const { status, text } = await postFeature(request, {
    type: "extrude",
    id: "ext_t02",
    sketch: "sk_t02",
    depth: 0.01,
  });
  expect(status).toBe(200);
  expect(text).not.toContain('"error"');
  expect(hasMeshVertices(text)).toBeTruthy();
});

// T03: Extrude 正側面 depth=10.0 — 大値
test("T03_extrude_large depth=10.0", async ({ request }) => {
  const { status: s1 } = await postFeature(request, makeSketch("sk_t03"));
  expect(s1).toBe(200);

  const { status, text } = await postFeature(request, {
    type: "extrude",
    id: "ext_t03",
    sketch: "sk_t03",
    depth: 10.0,
  });
  expect(status).toBe(200);
  expect(text).not.toContain('"error"');
  expect(hasMeshVertices(text)).toBeTruthy();
});

// ---------------------------------------------------------------------------
// Extrude 負側面 (#110 回帰)
// ---------------------------------------------------------------------------

// T04: Extrude 負側面 depth=-2.0
test("T04_degen_extrude_neg_face depth=-2.0 (#110 regression)", async ({
  request,
}) => {
  const { status: s1 } = await postFeature(request, makeSketch("sk_t04"));
  expect(s1).toBe(200);

  const { status, text } = await postFeature(request, {
    type: "extrude",
    id: "ext_t04",
    sketch: "sk_t04",
    depth: -2.0,
  });
  expect(status).toBe(200);
  expect(text).not.toContain('"error"');
  expect(hasMeshVertices(text)).toBeTruthy();
});

// T05: Extrude 負側面 depth=-0.01 — 最小負値
test("T05_degen_extrude_neg_min depth=-0.01 (#110 regression)", async ({
  request,
}) => {
  const { status: s1 } = await postFeature(request, makeSketch("sk_t05"));
  expect(s1).toBe(200);

  const { status, text } = await postFeature(request, {
    type: "extrude",
    id: "ext_t05",
    sketch: "sk_t05",
    depth: -0.01,
  });
  expect(status).toBe(200);
  expect(text).not.toContain('"error"');
  expect(hasMeshVertices(text)).toBeTruthy();
});

// ---------------------------------------------------------------------------
// ExtrudeCut
// ---------------------------------------------------------------------------

// T06: ExtrudeCut depth=1.0 — 正常値
test("T06_extrude_cut_normal depth=1.0", async ({ request }) => {
  const { status: s1 } = await postFeature(request, makeSketch("sk_t06"));
  expect(s1).toBe(200);

  const { status, text } = await postFeature(request, {
    type: "extrude_cut",
    id: "cut_t06",
    sketch: "sk_t06",
    depth: 1.0,
    target: "box_1",
  });
  expect(status).toBe(200);
  expect(text).not.toContain('"error"');
  expect(hasMeshVertices(text)).toBeTruthy();
});

// T07: ExtrudeCut depth=face距離-0.01=14.999 — 境界手前
test("T07_extrude_cut_near_boundary depth=14.999", async ({ request }) => {
  const { status: s1 } = await postFeature(request, makeSketch("sk_t07"));
  expect(s1).toBe(200);

  const { status, text } = await postFeature(request, {
    type: "extrude_cut",
    id: "cut_t07",
    sketch: "sk_t07",
    depth: 14.999,
    target: "box_1",
  });
  expect(status).toBe(200);
  expect(text).not.toContain('"error"');
});

// T08: ExtrudeCut depth=face距離=15.0 — 境界値 (#111 回帰)
test("T08_degen_extrude_cut_at_boundary depth=15.0 (#111 regression)", async ({
  request,
}) => {
  const { status: s1 } = await postFeature(request, makeSketch("sk_t08"));
  expect(s1).toBe(200);

  const { status, text } = await postFeature(request, {
    type: "extrude_cut",
    id: "cut_t08",
    sketch: "sk_t08",
    depth: 15.0,
    target: "box_1",
  });
  expect(status).toBe(200);
  expect(text).not.toContain('"error"');
});

// T09: ExtrudeCut depth=face距離+0.1=15.1 — 境界超え
test("T09_extrude_cut_beyond_boundary depth=15.1", async ({ request }) => {
  const { status: s1 } = await postFeature(request, makeSketch("sk_t09"));
  expect(s1).toBe(200);

  const { status, text } = await postFeature(request, {
    type: "extrude_cut",
    id: "cut_t09",
    sketch: "sk_t09",
    depth: 15.1,
    target: "box_1",
  });
  expect(status).toBe(200);
  expect(text).not.toContain('"error"');
});

// ---------------------------------------------------------------------------
// Phase 7 拡張ポイント
// ---------------------------------------------------------------------------
// TODO(Phase 7): CreateSketch 起点のケースをここに追加する。
// ブラウザで正準平面を選択し → スケッチを描き → Extrude/ExtrudeCut するフローを
// setupPageWithServer() + face クリック操作で実装する。
// 参考: web/tests/extrude_panel.spec.ts の E03/E04
