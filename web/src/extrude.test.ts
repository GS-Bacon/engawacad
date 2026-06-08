/**
 * Unit tests for web/src/extrude.ts — #95 選択面からの押出(Extrude) UI
 *
 * T01: 決定性  — buildExtrudeFeatures を同入力で2回 → toEqual
 * T02: 正常系  — planeForFaceId: f_z_* →xy / f_y_* →xz / f_x_* →yz
 * T03: 正常系  — footprintProfile: box 上面投影 → 4 seg・凸閉矩形・座標一致
 * T04: 正常系  — buildExtrudeFeatures: 既存 id {sketch_1} 与え → 衝突回避採番
 * T05_degen   — 無名面 "" / 非対応ロール → planeForFaceId null・buildExtrudeFeatures null
 * T06_boundary — faceIds に対象 faceId が0件 → footprintProfile null
 */
import { describe, it, expect, afterEach } from "vitest";
import {
  planeForFaceId,
  planeForFaceNormal,
  footprintProfile,
  buildExtrudeFeatures,
  buildExtrudeCutFeatures,
  faceToCanonicalPlaneDistance,
  faceOffsetFromPlane,
  insetRect,
  CUT_INSET_RATIO,
} from "./extrude";
import { postFeature } from "./api";
import type { Feature } from "./generated/Feature";

// Helper: build positions/indices/faceIds for a 10×10×10 box top face (f_z_pos).
// 4 vertices of the Z=10 face: v0=(0,0,10) v1=(10,0,10) v2=(10,10,10) v3=(0,10,10)
// 2 triangles: [0,1,2] and [0,2,3], both with faceId f_z_pos
function boxTopFaceData() {
  const positions = new Float32Array([
    0, 0, 10,
    10, 0, 10,
    10, 10, 10,
    0, 10, 10,
  ]);
  const indices = new Uint32Array([0, 1, 2, 0, 2, 3]);
  const faceIds = [
    "N(v0;face:f_z_pos)",
    "N(v0;face:f_z_pos)",
  ];
  return { positions, indices, faceIds };
}

// ---------------------------------------------------------------------------
// T01: 決定性
// ---------------------------------------------------------------------------
describe("T01 determinism", () => {
  it("buildExtrudeFeatures returns identical output on repeated calls with same input", () => {
    const { positions, indices, faceIds } = boxTopFaceData();
    const faceId = "N(v0;face:f_z_pos)";
    const depth = 5;
    const existing = new Set<string>();
    const a = buildExtrudeFeatures(faceId, positions, indices, faceIds, depth, existing);
    const b = buildExtrudeFeatures(faceId, positions, indices, faceIds, depth, existing);
    expect(a).not.toBeNull();
    expect(b).not.toBeNull();
    expect(a).toEqual(b);
  });
});

// ---------------------------------------------------------------------------
// T02: planeForFaceId — ロール → 平面
// ---------------------------------------------------------------------------
describe("T02 planeForFaceId", () => {
  it("f_z_pos → xy", () => {
    expect(planeForFaceId("N(fid;face:f_z_pos)")).toBe("xy");
  });

  it("f_z_neg → xy", () => {
    expect(planeForFaceId("N(fid;face:f_z_neg)")).toBe("xy");
  });

  it("f_y_pos → xz", () => {
    expect(planeForFaceId("N(fid;face:f_y_pos)")).toBe("xz");
  });

  it("f_x_neg → yz", () => {
    expect(planeForFaceId("N(fid;face:f_x_neg)")).toBe("yz");
  });
});

// ---------------------------------------------------------------------------
// T03: footprintProfile — box 上面投影
// ---------------------------------------------------------------------------
describe("T03 footprintProfile", () => {
  it("box top face → 4 segments, convex closed rectangle", () => {
    const { positions, indices, faceIds } = boxTopFaceData();
    const faceId = "N(v0;face:f_z_pos)";
    const segments = footprintProfile(positions, indices, faceIds, faceId, "xy");
    expect(segments).not.toBeNull();
    expect(segments!.length).toBe(4);
    // closed: last seg.to === first seg.from
    expect(segments![3].to).toEqual(segments![0].from);
    // Bounding rectangle of (0..10, 0..10) on xy plane
    expect(segments![0].from).toEqual([0, 0]);
    expect(segments![0].to).toEqual([10, 0]);
    expect(segments![1].from).toEqual([10, 0]);
    expect(segments![1].to).toEqual([10, 10]);
    expect(segments![2].from).toEqual([10, 10]);
    expect(segments![2].to).toEqual([0, 10]);
    expect(segments![3].from).toEqual([0, 10]);
    expect(segments![3].to).toEqual([0, 0]);
  });
});

// ---------------------------------------------------------------------------
// T04: buildExtrudeFeatures — 衝突回避採番
// ---------------------------------------------------------------------------
describe("T04 collision-free id naming", () => {
  it("existing {sketch_1} causes sketch_0 to be used", () => {
    const { positions, indices, faceIds } = boxTopFaceData();
    const faceId = "N(v0;face:f_z_pos)";
    const existing = new Set(["sketch_1"]);
    const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, 5, existing);
    expect(result).not.toBeNull();
    expect(result!.sketch.id).not.toBe("sketch_1");
    // Should use sketch_0 (lowest non-colliding)
    expect(result!.sketch.id).toBe("sketch_0");
    // Extrude references the sketch
    if (result!.extrude.type === "extrude") {
      expect(result!.extrude.sketch).toBe("sketch_0");
    }
  });
});

// ---------------------------------------------------------------------------
// T05_degen: 無名面 / 非対応ロール → null
// ---------------------------------------------------------------------------
describe("T05_degen degenerate face ids", () => {
  it("empty string faceId → planeForFaceId null", () => {
    expect(planeForFaceId("")).toBeNull();
  });

  it("unsupported role → planeForFaceId null", () => {
    expect(planeForFaceId("N(fid;face:f_w_pos)")).toBeNull();
  });

  it("empty faceId → buildExtrudeFeatures null", () => {
    const { positions, indices, faceIds } = boxTopFaceData();
    const result = buildExtrudeFeatures("", positions, indices, faceIds, 5, new Set());
    expect(result).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// T06_boundary: faceIds に対象 faceId が0件 → footprintProfile null
// ---------------------------------------------------------------------------
describe("T06_boundary no matching triangles", () => {
  it("faceId not present in faceIds → footprintProfile null", () => {
    const positions = new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["face_other"];
    const result = footprintProfile(positions, indices, faceIds, "face_target", "xy");
    expect(result).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// T07: depth 境界値テスト — buildExtrudeFeatures が不正 depth を弾く
// ---------------------------------------------------------------------------
describe("T07 depth boundary values", () => {
  const { positions, indices, faceIds } = boxTopFaceData();
  const faceId = "N(v0;face:f_z_pos)";

  it("depth = 0 → null", () => {
    expect(buildExtrudeFeatures(faceId, positions, indices, faceIds, 0, new Set())).toBeNull();
  });

  it("depth = -1 → null", () => {
    expect(buildExtrudeFeatures(faceId, positions, indices, faceIds, -1, new Set())).toBeNull();
  });

  it("depth = NaN → null", () => {
    expect(buildExtrudeFeatures(faceId, positions, indices, faceIds, NaN, new Set())).toBeNull();
  });

  it("depth = Infinity → null (Number.isFinite is false)", () => {
    expect(buildExtrudeFeatures(faceId, positions, indices, faceIds, Infinity, new Set())).toBeNull();
  });

  it("depth = -Infinity → null", () => {
    expect(buildExtrudeFeatures(faceId, positions, indices, faceIds, -Infinity, new Set())).toBeNull();
  });

  it("depth = -0 → null (0 === -0, depth <= 0 is true)", () => {
    expect(buildExtrudeFeatures(faceId, positions, indices, faceIds, -0, new Set())).toBeNull();
  });

  it("depth = Number.MIN_VALUE (positive tiny) → valid", () => {
    // smallest positive > 0, isFinite=true, > 0 → should succeed
    const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, Number.MIN_VALUE, new Set());
    expect(result).not.toBeNull();
  });

  it("depth = Number.MAX_VALUE → valid", () => {
    const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, Number.MAX_VALUE, new Set());
    expect(result).not.toBeNull();
  });
});

// ---------------------------------------------------------------------------
// T08: ε_guard 境界値テスト — footprintProfile が退化 extent を弾く
// ---------------------------------------------------------------------------
describe("T08 epsilon guard boundary", () => {
  const faceId = "N(v0;face:f_z_pos)";

  it("extent exactly ε_guard (1e-9) → null (<= ε_guard)", () => {
    // All U coords the same → extent = 0 ≤ 1e-9
    const positions = new Float32Array([
      5, 0, 10,
      5, 10, 10,
      5, 5, 10,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = [faceId];
    const result = footprintProfile(positions, indices, faceIds, faceId, "xy");
    expect(result).toBeNull();
  });

  it("extent > ε_guard → non-null profile", () => {
    // U range = 1 (0→1), V range = 10 → both > 1e-9
    const positions = new Float32Array([
      0, 0, 10,
      1, 0, 10,
      1, 10, 10,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = [faceId];
    const result = footprintProfile(positions, indices, faceIds, faceId, "xy");
    expect(result).not.toBeNull();
    expect(result!.length).toBe(4);
  });

  it("all vertices coincident (point face) → null", () => {
    const positions = new Float32Array([
      5, 5, 10,
      5, 5, 10,
      5, 5, 10,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = [faceId];
    const result = footprintProfile(positions, indices, faceIds, faceId, "xy");
    expect(result).toBeNull();
  });

  it("collinear vertices (line face, V extent = 0) → null", () => {
    const positions = new Float32Array([
      0, 5, 10,
      5, 5, 10,
      10, 5, 10,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = [faceId];
    const result = footprintProfile(positions, indices, faceIds, faceId, "xy");
    expect(result).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// T09: 決定性 100 回 — buildExtrudeFeatures が常に同一結果を返す
// ---------------------------------------------------------------------------
describe("T09 determinism 100 runs", () => {
  it("buildExtrudeFeatures produces identical output 100 times", () => {
    const { positions, indices, faceIds } = boxTopFaceData();
    const faceId = "N(v0;face:f_z_pos)";
    const existing = new Set<string>();
    const first = buildExtrudeFeatures(faceId, positions, indices, faceIds, 5, existing);
    expect(first).not.toBeNull();
    for (let i = 0; i < 100; i++) {
      const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, 5, existing);
      expect(result).toEqual(first);
    }
  });

  it("planeForFaceId produces identical output 100 times", () => {
    const input = "N(fid;face:f_z_pos)";
    const expected = "xy";
    for (let i = 0; i < 100; i++) {
      expect(planeForFaceId(input)).toBe(expected);
    }
  });

  it("footprintProfile produces identical output 100 times", () => {
    const { positions, indices, faceIds } = boxTopFaceData();
    const faceId = "N(v0;face:f_z_pos)";
    const first = footprintProfile(positions, indices, faceIds, faceId, "xy");
    expect(first).not.toBeNull();
    for (let i = 0; i < 100; i++) {
      expect(footprintProfile(positions, indices, faceIds, faceId, "xy")).toEqual(first);
    }
  });
});

// ---------------------------------------------------------------------------
// T10: planeForFaceId 追加ケース
// ---------------------------------------------------------------------------
describe("T10 planeForFaceId additional cases", () => {
  it("f_x_pos → yz", () => {
    expect(planeForFaceId("N(fid;face:f_x_pos)")).toBe("yz");
  });

  it("f_y_neg → xz", () => {
    expect(planeForFaceId("N(fid;face:f_y_neg)")).toBe("xz");
  });

  it("face_id without face: prefix → null", () => {
    expect(planeForFaceId("N(fid;something_else)")).toBeNull();
  });

  it("face_id with numeric-only fid → still works", () => {
    expect(planeForFaceId("N(42;face:f_z_neg)")).toBe("xy");
  });

  it("plain string without N() wrapper → null if no f_*_ pattern", () => {
    expect(planeForFaceId("just_a_face")).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// T11: buildExtrudeFeatures ID 採番の追加ケース
// ---------------------------------------------------------------------------
describe("T11 buildExtrudeFeatures ID allocation", () => {
  const { positions, indices, faceIds } = boxTopFaceData();
  const faceId = "N(v0;face:f_z_pos)";

  it("empty existing set → sketch_0 and extrude_0", () => {
    const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, 5, new Set());
    expect(result).not.toBeNull();
    expect(result!.sketch.id).toBe("sketch_0");
    expect(result!.extrude.id).toBe("extrude_0");
  });

  it("sketch_0 occupied → sketch_1", () => {
    const existing = new Set(["sketch_0"]);
    const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, 5, existing);
    expect(result).not.toBeNull();
    expect(result!.sketch.id).toBe("sketch_1");
  });

  it("sketch_0 and sketch_1 occupied → sketch_2", () => {
    const existing = new Set(["sketch_0", "sketch_1"]);
    const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, 5, existing);
    expect(result).not.toBeNull();
    expect(result!.sketch.id).toBe("sketch_2");
  });

  it("extrude references correct sketch id", () => {
    const existing = new Set(["sketch_0"]);
    const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, 5, existing);
    expect(result).not.toBeNull();
    if (result!.extrude.type === "extrude") {
      expect(result!.extrude.sketch).toBe(result!.sketch.id);
    }
  });
});

// ---------------------------------------------------------------------------
// T12: footprintProfile の異常平面投影
// ---------------------------------------------------------------------------
describe("T12 footprintProfile different planes", () => {
  it("xz plane projects (x,z)", () => {
    const faceId = "N(v0;face:f_y_pos)";
    // Y face at y=5, x range 0..10, z range 0..20
    const positions = new Float32Array([
      0, 5, 0,
      10, 5, 0,
      10, 5, 20,
      0, 5, 20,
    ]);
    const indices = new Uint32Array([0, 1, 2, 0, 2, 3]);
    const faceIds = [faceId, faceId];
    const result = footprintProfile(positions, indices, faceIds, faceId, "xz");
    expect(result).not.toBeNull();
    expect(result!.length).toBe(4);
    // Bounding rect on xz: x=[0,10], z=[0,20]
    expect(result![0].from).toEqual([0, 0]);
    expect(result![1].to).toEqual([10, 20]);
  });

  it("yz plane projects (y,z)", () => {
    const faceId = "N(v0;face:f_x_neg)";
    const positions = new Float32Array([
      -3, 0, 0,
      -3, 10, 0,
      -3, 10, 20,
      -3, 0, 20,
    ]);
    const indices = new Uint32Array([0, 1, 2, 0, 2, 3]);
    const faceIds = [faceId, faceId];
    const result = footprintProfile(positions, indices, faceIds, faceId, "yz");
    expect(result).not.toBeNull();
    expect(result!.length).toBe(4);
    // y=[0,10], z=[0,20]
    expect(result![0].from).toEqual([0, 0]);
    expect(result![1].to).toEqual([10, 20]);
  });
});

// ---------------------------------------------------------------------------
// T13: postFeature HTTP エラー展開テスト
// ---------------------------------------------------------------------------
describe("T13 postFeature error handling", () => {
  const originalFetch = globalThis.fetch;

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it("4xx with error body → throws with error message", async () => {
    globalThis.fetch = async () =>
      new Response(JSON.stringify({ error: "Invalid sketch profile" }), {
        status: 422,
        headers: { "content-type": "application/json" },
      });
    const feature: Feature = { type: "extrude", id: "e0", sketch: "s0", depth: 5 };
    await expect(postFeature(feature)).rejects.toThrow("Invalid sketch profile");
  });

  it("5xx with error body → throws with error message", async () => {
    globalThis.fetch = async () =>
      new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "content-type": "application/json" },
      });
    const feature: Feature = { type: "extrude", id: "e0", sketch: "s0", depth: 5 };
    await expect(postFeature(feature)).rejects.toThrow("Internal server error");
  });

  it("4xx with non-JSON body → throws HTTP status", async () => {
    globalThis.fetch = async () =>
      new Response("Bad Gateway", {
        status: 502,
        headers: { "content-type": "text/plain" },
      });
    const feature: Feature = { type: "extrude", id: "e0", sketch: "s0", depth: 5 };
    await expect(postFeature(feature)).rejects.toThrow("HTTP 502");
  });

  it("4xx with empty error → throws HTTP status", async () => {
    globalThis.fetch = async () =>
      new Response(JSON.stringify({ error: "" }), {
        status: 400,
        headers: { "content-type": "application/json" },
      });
    const feature: Feature = { type: "extrude", id: "e0", sketch: "s0", depth: 5 };
    await expect(postFeature(feature)).rejects.toThrow("HTTP 400");
  });

  it("200 → returns parsed BodyMesh array", async () => {
    const body = [{ feature_id: "box_1", mesh: { positions: [], normals: [], indices: [], face_ids: [] } }];
    globalThis.fetch = async () =>
      new Response(JSON.stringify(body), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    const feature: Feature = { type: "create_sketch", id: "s0", plane: "xy", profile: [] };
    const result = await postFeature(feature);
    expect(result).toEqual(body);
  });
});

// ---------------------------------------------------------------------------
// T14: insetRect unit tests
// ---------------------------------------------------------------------------
describe("T14 insetRect", () => {
  it("10×10 rectangle with ratio 0.25 → 5×5 centered", () => {
    const rect = [
      { id: "seg_0", from: [0, 0] as [number, number], to: [10, 0] as [number, number] },
      { id: "seg_1", from: [10, 0] as [number, number], to: [10, 10] as [number, number] },
      { id: "seg_2", from: [10, 10] as [number, number], to: [0, 10] as [number, number] },
      { id: "seg_3", from: [0, 10] as [number, number], to: [0, 0] as [number, number] },
    ];
    const result = insetRect(rect, 0.25);
    expect(result).not.toBeNull();
    // extent = 10, shrink per side = 10 * 0.25 = 2.5, new range = [2.5, 7.5]
    expect(result![0].from).toEqual([2.5, 2.5]);
    expect(result![1].to).toEqual([7.5, 7.5]);
    // 4 segments, closed
    expect(result!.length).toBe(4);
    expect(result![3].to).toEqual(result![0].from);
  });

  it("collapse case: tiny extent → null", () => {
    // extent = 2, ratio = 0.5 → shrink = 1 each side → new extent = 0
    const rect = [
      { id: "seg_0", from: [0, 0] as [number, number], to: [2, 0] as [number, number] },
      { id: "seg_1", from: [2, 0] as [number, number], to: [2, 2] as [number, number] },
      { id: "seg_2", from: [2, 2] as [number, number], to: [0, 2] as [number, number] },
      { id: "seg_3", from: [0, 2] as [number, number], to: [0, 0] as [number, number] },
    ];
    const result = insetRect(rect, 0.5);
    expect(result).toBeNull();
  });

  it("non-square rectangle: 20×4 with ratio 0.25 → 10×2", () => {
    const rect = [
      { id: "seg_0", from: [0, 0] as [number, number], to: [20, 0] as [number, number] },
      { id: "seg_1", from: [20, 0] as [number, number], to: [20, 4] as [number, number] },
      { id: "seg_2", from: [20, 4] as [number, number], to: [0, 4] as [number, number] },
      { id: "seg_3", from: [0, 4] as [number, number], to: [0, 0] as [number, number] },
    ];
    const result = insetRect(rect, 0.25);
    expect(result).not.toBeNull();
    // U: [5, 15], V: [1, 3]
    expect(result![0].from).toEqual([5, 1]);
    expect(result![1].to).toEqual([15, 3]);
  });

  it("deterministic: same input → same output 100 times", () => {
    const rect = [
      { id: "seg_0", from: [0, 0] as [number, number], to: [10, 0] as [number, number] },
      { id: "seg_1", from: [10, 0] as [number, number], to: [10, 10] as [number, number] },
      { id: "seg_2", from: [10, 10] as [number, number], to: [0, 10] as [number, number] },
      { id: "seg_3", from: [0, 10] as [number, number], to: [0, 0] as [number, number] },
    ];
    const first = insetRect(rect, 0.25);
    expect(first).not.toBeNull();
    for (let i = 0; i < 100; i++) {
      expect(insetRect(rect, 0.25)).toEqual(first);
    }
  });
});

// ---------------------------------------------------------------------------
// T15: buildExtrudeCutFeatures determinism (T01_ui)
// ---------------------------------------------------------------------------
describe("T15 buildExtrudeCutFeatures determinism", () => {
  it("same input produces identical output", () => {
    const { positions, indices, faceIds } = boxTopFaceData();
    const faceId = "N(v0;face:f_z_pos)";
    const depth = 5;
    const target = "box_1";
    const existing = new Set<string>();
    const a = buildExtrudeCutFeatures(faceId, positions, indices, faceIds, depth, target, existing);
    const b = buildExtrudeCutFeatures(faceId, positions, indices, faceIds, depth, target, existing);
    expect(a).not.toBeNull();
    expect(b).not.toBeNull();
    expect(a).toEqual(b);
  });

  it("result has correct type and target", () => {
    const { positions, indices, faceIds } = boxTopFaceData();
    const faceId = "N(v0;face:f_z_pos)";
    const depth = 5;
    const target = "box_1";
    const existing = new Set<string>();
    const result = buildExtrudeCutFeatures(faceId, positions, indices, faceIds, depth, target, existing);
    expect(result).not.toBeNull();
    expect(result!.extrudeCut.type).toBe("extrude_cut");
    if (result!.extrudeCut.type === "extrude_cut") {
      expect(result!.extrudeCut.target).toBe("box_1");
      expect(result!.extrudeCut.depth).toBe(5);
      expect(result!.extrudeCut.sketch).toBe(result!.sketch.id);
    }
  });

  it("100 repeated calls produce identical output", () => {
    const { positions, indices, faceIds } = boxTopFaceData();
    const faceId = "N(v0;face:f_z_pos)";
    const depth = 5;
    const target = "box_1";
    const existing = new Set<string>();
    const first = buildExtrudeCutFeatures(faceId, positions, indices, faceIds, depth, target, existing);
    expect(first).not.toBeNull();
    for (let i = 0; i < 100; i++) {
      expect(
        buildExtrudeCutFeatures(faceId, positions, indices, faceIds, depth, target, existing),
      ).toEqual(first);
    }
  });

  it("empty existingFeatureIds → sketch_0 and extrude_cut_0", () => {
    const { positions, indices, faceIds } = boxTopFaceData();
    const faceId = "N(v0;face:f_z_pos)";
    const existing = new Set<string>();
    const result = buildExtrudeCutFeatures(faceId, positions, indices, faceIds, 5, "box_1", existing);
    expect(result).not.toBeNull();
    expect(result!.sketch.id).toBe("sketch_0");
    expect(result!.extrudeCut.id).toBe("extrude_cut_0");
  });
});

// ---------------------------------------------------------------------------
// T16: buildExtrudeCutFeatures degenerate inputs (T02_ui_degen)
// ---------------------------------------------------------------------------
describe("T16 buildExtrudeCutFeatures degenerate inputs", () => {
  const { positions, indices, faceIds } = boxTopFaceData();
  const faceId = "N(v0;face:f_z_pos)";

  it("empty target → null", () => {
    const result = buildExtrudeCutFeatures(faceId, positions, indices, faceIds, 5, "", new Set());
    expect(result).toBeNull();
  });

  it("depth = 0 → null", () => {
    expect(
      buildExtrudeCutFeatures(faceId, positions, indices, faceIds, 0, "box_1", new Set()),
    ).toBeNull();
  });

  it("depth = -1 → null", () => {
    expect(
      buildExtrudeCutFeatures(faceId, positions, indices, faceIds, -1, "box_1", new Set()),
    ).toBeNull();
  });

  it("depth = NaN → null", () => {
    expect(
      buildExtrudeCutFeatures(faceId, positions, indices, faceIds, NaN, "box_1", new Set()),
    ).toBeNull();
  });

  it("depth = Infinity → null", () => {
    expect(
      buildExtrudeCutFeatures(faceId, positions, indices, faceIds, Infinity, "box_1", new Set()),
    ).toBeNull();
  });

  it("depth = -0 → null", () => {
    expect(
      buildExtrudeCutFeatures(faceId, positions, indices, faceIds, -0, "box_1", new Set()),
    ).toBeNull();
  });

  it("inset collapse: zero-width face → null", () => {
    // All U coords the same → footprintProfile gets a degenerate extent → null
    const degeneratePositions = new Float32Array([
      5, 0, 10,
      5, 10, 10,
      5, 5, 10,
    ]);
    const degenerateIndices = new Uint32Array([0, 1, 2]);
    const degenerateFaceIds = [faceId];
    const result = buildExtrudeCutFeatures(
      faceId, degeneratePositions, degenerateIndices, degenerateFaceIds,
      5, "box_1", new Set(),
    );
    expect(result).toBeNull();
  });

  it("empty faceId → null", () => {
    const result = buildExtrudeCutFeatures("", positions, indices, faceIds, 5, "box_1", new Set());
    expect(result).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// T17: usedFeatureIds collision avoidance — simulates main.ts initialization
// ---------------------------------------------------------------------------
describe("T17 usedFeatureIds collision avoidance (simulates main.ts init)", () => {
  const { positions, indices, faceIds } = boxTopFaceData();
  const faceId = "N(v0;face:f_z_pos)";

  it("existing {sketch_0} from server → buildExtrudeFeatures uses sketch_1", () => {
    // Simulates: fetchAllFeatureIds() returns ["sketch_0"]
    // usedFeatureIds = new Set(["sketch_0"])
    const usedFeatureIds = new Set(["sketch_0"]);
    const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, 5, usedFeatureIds);
    expect(result).not.toBeNull();
    expect(result!.sketch.id).toBe("sketch_1");
  });

  it("existing {sketch_0, sketch_1, extrude_0} from server → sketch_2 and extrude_1", () => {
    const usedFeatureIds = new Set(["sketch_0", "sketch_1", "extrude_0"]);
    const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, 5, usedFeatureIds);
    expect(result).not.toBeNull();
    expect(result!.sketch.id).toBe("sketch_2");
    expect(result!.extrude.id).toBe("extrude_1");
  });

  it("extrudeCut avoids existing extrude_cut_0", () => {
    const usedFeatureIds = new Set(["sketch_0", "extrude_cut_0"]);
    const result = buildExtrudeCutFeatures(faceId, positions, indices, faceIds, 5, "box_1", usedFeatureIds);
    expect(result).not.toBeNull();
    expect(result!.sketch.id).toBe("sketch_1");
    expect(result!.extrudeCut.id).toBe("extrude_cut_1");
  });

  it("empty usedFeatureIds from server (no features) → sketch_0 and extrude_0", () => {
    const usedFeatureIds = new Set<string>();
    const result = buildExtrudeFeatures(faceId, positions, indices, faceIds, 5, usedFeatureIds);
    expect(result).not.toBeNull();
    expect(result!.sketch.id).toBe("sketch_0");
    expect(result!.extrude.id).toBe("extrude_0");
  });
});

// ---------------------------------------------------------------------------
// #103: planeForFaceNormal — face id パターンマッチに依存しない法線ベースの平面判定
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// T18_normal_cap_z: z 方向法線の三角形 → "xy"
// ---------------------------------------------------------------------------
describe("T18_normal_cap_z: planeForFaceNormal z-axis normal → xy", () => {
  it("triangle with z-normal returns xy plane", () => {
    // Triangle on z=5 plane: normal is (0,0,1) → dominant z → "xy"
    const positions = new Float32Array([
      0, 0, 5,
      10, 0, 5,
      10, 10, 5,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["N(v0;face:f_cap_z_pos)"];
    const result = planeForFaceNormal(positions, indices, faceIds, "N(v0;face:f_cap_z_pos)");
    expect(result).toBe("xy");
  });
});

// ---------------------------------------------------------------------------
// T19_normal_side_y: y 方向法線の三角形 → "xz"
// ---------------------------------------------------------------------------
describe("T19_normal_side_y: planeForFaceNormal y-axis normal → xz", () => {
  it("triangle with y-normal returns xz plane", () => {
    // Triangle on y=5 plane: normal is (0,1,0) → dominant y → "xz"
    const positions = new Float32Array([
      0, 5, 0,
      10, 5, 0,
      10, 5, 20,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["N(v0;face:f_side_y)"];
    const result = planeForFaceNormal(positions, indices, faceIds, "N(v0;face:f_side_y)");
    expect(result).toBe("xz");
  });
});

// ---------------------------------------------------------------------------
// T20_normal_side_x: x 方向法線の三角形 → "yz"
// ---------------------------------------------------------------------------
describe("T20_normal_side_x: planeForFaceNormal x-axis normal → yz", () => {
  it("triangle with x-normal returns yz plane", () => {
    // Triangle on x=3 plane: normal is (1,0,0) → dominant x → "yz"
    const positions = new Float32Array([
      3, 0, 0,
      3, 10, 0,
      3, 10, 20,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["N(v0;face:f_side_x)"];
    const result = planeForFaceNormal(positions, indices, faceIds, "N(v0;face:f_side_x)");
    expect(result).toBe("yz");
  });
});

// ---------------------------------------------------------------------------
// T21_degen_zero_cross: 縮退三角形（クロス積長 ≤ 1e-9）→ null
// ---------------------------------------------------------------------------
describe("T21_degen_zero_cross: degenerate triangle → null", () => {
  it("collapsed triangle (all vertices same) → null", () => {
    const positions = new Float32Array([
      5, 5, 5,
      5, 5, 5,
      5, 5, 5,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["N(v0;face:f_cap_z)"];
    const result = planeForFaceNormal(positions, indices, faceIds, "N(v0;face:f_cap_z)");
    expect(result).toBeNull();
  });

  it("collinear triangle → null", () => {
    const positions = new Float32Array([
      0, 0, 0,
      5, 0, 0,
      10, 0, 0,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["N(v0;face:f_side_y)"];
    const result = planeForFaceNormal(positions, indices, faceIds, "N(v0;face:f_side_y)");
    expect(result).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// T22_boundary_no_match: faceId が faceIds に存在しない → null
// ---------------------------------------------------------------------------
describe("T22_boundary_no_match: faceId not in faceIds → null", () => {
  it("non-existent faceId returns null", () => {
    const positions = new Float32Array([
      0, 0, 5,
      10, 0, 5,
      10, 10, 5,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["N(v0;face:f_z_pos)"];
    const result = planeForFaceNormal(positions, indices, faceIds, "N(v0;face:f_cap_unknown)");
    expect(result).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// T23: faceToCanonicalPlaneDistance — face 位置から正準面までの距離
// ---------------------------------------------------------------------------
describe("T23 faceToCanonicalPlaneDistance", () => {
  function makeTriangle(x: number, y: number, z: number): {
    positions: Float32Array; indices: Uint32Array; faceIds: string[];
  } {
    return {
      positions: new Float32Array([x, y, z, x + 1, y, z, x, y + 1, z]),
      indices: new Uint32Array([0, 1, 2]),
      faceIds: ["face_a"],
    };
  }

  it("T23_01: yz plane, x=5 → distance 5", () => {
    const { positions, indices, faceIds } = makeTriangle(5, 0, 0);
    expect(faceToCanonicalPlaneDistance(positions, indices, faceIds, "face_a", "yz")).toBeCloseTo(5);
  });

  it("T23_02: xz plane, y=3 → distance 3", () => {
    const { positions, indices, faceIds } = makeTriangle(0, 3, 0);
    expect(faceToCanonicalPlaneDistance(positions, indices, faceIds, "face_a", "xz")).toBeCloseTo(3);
  });

  it("T23_03: xy plane, z=10 → distance 10", () => {
    const { positions, indices, faceIds } = makeTriangle(0, 0, 10);
    expect(faceToCanonicalPlaneDistance(positions, indices, faceIds, "face_a", "xy")).toBeCloseTo(10);
  });

  it("T23_normal_under: f_x_pos at x=5, depth=4 → no clamp (effectiveDepth=4)", () => {
    const positions = new Float32Array([5, 0, 0, 5, 5, 0, 5, 0, 5]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["N(v0;face:f_x_pos)"];
    const result = buildExtrudeCutFeatures(
      "N(v0;face:f_x_pos)", positions, indices, faceIds, 4, "box_0", new Set()
    );
    expect(result).not.toBeNull();
    if (result!.extrudeCut.type === "extrude_cut") {
      expect(result!.extrudeCut.depth).toBeCloseTo(4);
    }
  });

  it("T23_boundary_exact: depth == faceOffset → effectiveDepth < depth (clamped)", () => {
    // f_x_pos at x=5, depth=5 → should be clamped
    const positions = new Float32Array([5, 0, 0, 5, 5, 0, 5, 0, 5]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["N(v0;face:f_x_pos)"];
    const result = buildExtrudeCutFeatures(
      "N(v0;face:f_x_pos)", positions, indices, faceIds, 5, "box_0", new Set()
    );
    expect(result).not.toBeNull();
    if (result!.extrudeCut.type === "extrude_cut") {
      expect(result!.extrudeCut.depth).toBeLessThan(5);
    }
  });

  it("T23_boundary_over: f_x_pos at x=5, depth=10 → clamped to ≈5-ε", () => {
    const positions = new Float32Array([5, 0, 0, 5, 5, 0, 5, 0, 5]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["N(v0;face:f_x_pos)"];
    const result = buildExtrudeCutFeatures(
      "N(v0;face:f_x_pos)", positions, indices, faceIds, 10, "box_0", new Set()
    );
    expect(result).not.toBeNull();
    if (result!.extrudeCut.type === "extrude_cut") {
      expect(result!.extrudeCut.depth).toBeLessThan(5);
      expect(result!.extrudeCut.depth).toBeGreaterThan(5 - 1e-6);
    }
  });

  it("T23_degen_zero_offset: face at origin (x=0) → null (depth clamped to ≤0)", () => {
    const positions = new Float32Array([0, 0, 0, 0, 5, 0, 0, 0, 5]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["N(v0;face:f_x_pos)"];
    const result = buildExtrudeCutFeatures(
      "N(v0;face:f_x_pos)", positions, indices, faceIds, 5, "box_0", new Set()
    );
    expect(result).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// #104: faceOffsetFromPlane — face の法線方向オフセット
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// T01_unit_offset_yz: yz 面の頂点(x=5) から offset 5.0 を返す
// ---------------------------------------------------------------------------
describe("T01_unit_offset_yz: faceOffsetFromPlane yz face returns x offset", () => {
  it("triangle on x=5 → offset 5.0 for yz plane", () => {
    const positions = new Float32Array([
      5, 0, 0,
      5, 10, 0,
      5, 0, 10,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["face_a"];
    expect(faceOffsetFromPlane(positions, indices, faceIds, "face_a", "yz")).toBeCloseTo(5);
  });
});

// ---------------------------------------------------------------------------
// T02_unit_offset_xy: xy 面の頂点(z=10) から offset 10.0 を返す
// ---------------------------------------------------------------------------
describe("T02_unit_offset_xy: faceOffsetFromPlane xy face returns z offset", () => {
  it("triangle on z=10 → offset 10.0 for xy plane", () => {
    const positions = new Float32Array([
      0, 0, 10,
      10, 0, 10,
      0, 10, 10,
    ]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["face_b"];
    expect(faceOffsetFromPlane(positions, indices, faceIds, "face_b", "xy")).toBeCloseTo(10);
  });
});

// ---------------------------------------------------------------------------
// T03_degen_no_match: faceId が存在しない場合 0.0 を返す
// ---------------------------------------------------------------------------
describe("T03_degen_no_match: faceOffsetFromPlane returns 0.0 for missing faceId", () => {
  it("faceId not in faceIds → 0.0", () => {
    const positions = new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]);
    const indices = new Uint32Array([0, 1, 2]);
    const faceIds = ["face_other"];
    expect(faceOffsetFromPlane(positions, indices, faceIds, "face_missing", "xy")).toBeCloseTo(0);
  });

  it("empty faceIds array → 0.0", () => {
    const positions = new Float32Array([0, 0, 0]);
    const indices = new Uint32Array([]);
    const faceIds: string[] = [];
    expect(faceOffsetFromPlane(positions, indices, faceIds, "any", "xy")).toBeCloseTo(0);
  });
});
