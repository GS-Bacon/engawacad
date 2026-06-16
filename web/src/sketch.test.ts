import { describe, it, expect } from "vitest";
import { createSketchSession, SNAP_RADIUS, CLOSE_EPS, buildCreateSketchFromSketch } from "./sketch";
import { nextId } from "./extrude";

describe("SketchSession", () => {
  describe("determinism (T07)", () => {
    it("should produce identical results for same input sequence", () => {
      const session1 = createSketchSession("Front");
      const session2 = createSketchSession("Front");

      const points = [
        { x: 0, y: 0 },
        { x: 5, y: 0 },
        { x: 5, y: 5 },
        { x: 0, y: 5 },
      ];

      for (const p of points) {
        session1.addPoint(p.x, p.y);
        session2.addPoint(p.x, p.y);
      }

      expect(session1.getSegments()).toEqual(session2.getSegments());
      expect(session1.isClosed()).toBe(session2.isClosed());
    });
  });

  describe("snap to closest vertex (T08)", () => {
    // triangle を 3 辺描いてから、4 点目を始点 A 近傍に置いて snap → 閉じる
    it("should snap fourth point to first vertex and close the loop", () => {
      const session = createSketchSession("Front");
      session.addPoint(0, 0);    // A
      session.addPoint(5, 0);    // B
      session.addPoint(5, 5);    // C
      // (0.05, 0.08) は A(0,0) との距離 ~0.094 で SNAP_RADIUS=0.1 内、B/C からは離れている
      session.addPoint(0.05, 0.08);

      const segments = session.getSegments();
      expect(segments.length).toBe(3);
      expect(segments[2].to).toEqual({ x: 0, y: 0 }); // snap to A
      expect(session.isClosed()).toBe(true);
    });

    // 中間頂点への snap: triangle を作って 4 点目を B 近傍へ → snap to B (非閉)
    it("should snap fourth point to an intermediate vertex (not first)", () => {
      const session = createSketchSession("Front");
      session.addPoint(0, 0);    // A
      session.addPoint(5, 0);    // B
      session.addPoint(5, 5);    // C
      // (5.05, 0.05) は B(5,0) との距離 ~0.071 で snap、他は離れている
      session.addPoint(5.05, 0.05);

      const segments = session.getSegments();
      expect(segments.length).toBe(3);
      expect(segments[2].to).toEqual({ x: 5, y: 0 }); // snap to B (中間頂点)
      expect(session.isClosed()).toBe(false);
    });
  });

  describe("isClosed with <3 segments (T09)", () => {
    it("should return false for 2 segments (line)", () => {
      const session = createSketchSession("Front");
      session.addPoint(0, 0);
      session.addPoint(5, 0);
      session.addPoint(5, 5);

      expect(session.getSegments().length).toBe(2);
      expect(session.isClosed()).toBe(false);
    });

    it("should return true for closed triangle (3 segments)", () => {
      const session = createSketchSession("Front");
      session.addPoint(0, 0);
      session.addPoint(5, 0);
      session.addPoint(5, 5);
      session.addPoint(0, 0);

      expect(session.getSegments().length).toBe(3);
      expect(session.isClosed()).toBe(true);
    });

    // T09b (Codex r5 F01 / F02 対応): A → B → A (premature close, triangle 未満) は
    // no-op として扱われ、segments は 1 のまま、isClosed=false で、次の addPoint(C) が
    // A から再開せず、最後の確定点 (B) から継続する。
    it("should treat A->B->A as no-op (premature close) keeping state consistent", () => {
      const session = createSketchSession("Front");
      session.addPoint(0, 0);    // A
      session.addPoint(5, 0);    // B
      session.addPoint(0, 0);    // A 再 → premature close, no-op 期待

      expect(session.getSegments().length).toBe(1);
      expect(session.isClosed()).toBe(false);

      // 続けて C を addPoint。segments[1].from は B でなければならない (state corruption 防止)。
      session.addPoint(5, 5);    // C
      const segs = session.getSegments();
      expect(segs.length).toBe(2);
      expect(segs[1].from).toEqual({ x: 5, y: 0 });
      expect(segs[1].to).toEqual({ x: 5, y: 5 });
    });
  });

  describe("finalize open loop (T10)", () => {
    it("should return null for open loop", () => {
      const session = createSketchSession("Front");
      session.addPoint(0, 0);
      session.addPoint(5, 0);
      session.addPoint(5, 5);

      expect(session.finalize()).toBeNull();
    });
  });

  // T06_degen_zero_length (unit, FN02 対応): ゼロ長セグメントは addPoint で棄却される
  describe("zero-length segment rejection (T06_unit)", () => {
    it("should not add a segment when the new point coincides with the last point", () => {
      const session = createSketchSession("Front");
      session.addPoint(0, 0);
      // 2 回目も (0, 0) — snap 経由で同点になり segments に積まれない
      session.addPoint(0, 0);
      expect(session.getSegments().length).toBe(0);
    });

    it("should not add a segment when the new point is within SNAP_RADIUS of the last point", () => {
      const session = createSketchSession("Front");
      session.addPoint(0, 0);
      // SNAP_RADIUS (0.1) 内 — last (0,0) に snap されて同点 → segments に積まない
      session.addPoint(SNAP_RADIUS * 0.5, 0);
      expect(session.getSegments().length).toBe(0);
    });

    it("constants SNAP_RADIUS=0.1 / CLOSE_EPS=1e-9 (spec sanity)", () => {
      expect(SNAP_RADIUS).toBeCloseTo(0.1, 9);
      expect(CLOSE_EPS).toBeCloseTo(1e-9, 12);
    });
  });

  // T06_unit_buildCreateSketch: buildCreateSketchFromSketch 正常系
  describe("buildCreateSketchFromSketch (T06_unit)", () => {
    it("should build create_sketch Feature with correct structure", () => {
      const sketch = {
        planeRefId: "Front" as const,
        segments: [
          { from: { x: 0, y: 0 }, to: { x: 5, y: 0 } },
        ],
      };
      const result = buildCreateSketchFromSketch(sketch, "sk1");
      expect(result).toEqual({
        type: "create_sketch",
        id: "sk1",
        plane: "xy",
        profile: [
          { id: "sk1_seg_0", from: [0, 0], to: [5, 0] },
        ],
        plane_ref: "Front",
      });
    });

    it("should assign deterministic segment IDs", () => {
      const sketch = {
        planeRefId: "Top" as const,
        segments: [
          { from: { x: 0, y: 0 }, to: { x: 5, y: 0 } },
          { from: { x: 5, y: 0 }, to: { x: 5, y: 5 } },
        ],
      };
      const result = buildCreateSketchFromSketch(sketch, "sk2");
      if (result.type !== "create_sketch") throw new Error("expected create_sketch");
      expect(result.profile[0].id).toBe("sk2_seg_0");
      expect(result.profile[1].id).toBe("sk2_seg_1");
    });
  });

  // T07_unit_plane_mapping: RefPlaneId → SketchPlane 対応
  describe("RefPlane to SketchPlane mapping (T07_unit)", () => {
    it("should map Front to xy", () => {
      const sketch = { planeRefId: "Front" as const, segments: [] };
      const result = buildCreateSketchFromSketch(sketch, "sk");
      if (result.type !== "create_sketch") throw new Error("expected create_sketch");
      expect(result.plane).toBe("xy");
      expect(result.plane_ref).toBe("Front");
    });

    it("should map Top to xz", () => {
      const sketch = { planeRefId: "Top" as const, segments: [] };
      const result = buildCreateSketchFromSketch(sketch, "sk");
      if (result.type !== "create_sketch") throw new Error("expected create_sketch");
      expect(result.plane).toBe("xz");
      expect(result.plane_ref).toBe("Top");
    });

    it("should map Right to yz", () => {
      const sketch = { planeRefId: "Right" as const, segments: [] };
      const result = buildCreateSketchFromSketch(sketch, "sk");
      if (result.type !== "create_sketch") throw new Error("expected create_sketch");
      expect(result.plane).toBe("yz");
      expect(result.plane_ref).toBe("Right");
    });
  });

  // T08_unit_id_determinism: nextId 決定性
  describe("nextId determinism (T08_unit)", () => {
    it("should return the same ID for same prefix and empty existing set", () => {
      const existing1 = new Set<string>();
      const existing2 = new Set<string>();
      expect(nextId("sketch_", existing1)).toBe("sketch_0");
      expect(nextId("sketch_", existing2)).toBe("sketch_0");
    });

    it("should return the same ID when called with identical existing set states", () => {
      const existing1 = new Set(["sketch_0", "sketch_1", "extrude_0"]);
      const existing2 = new Set(["sketch_0", "sketch_1", "extrude_0"]);
      expect(nextId("sketch_", existing1)).toBe("sketch_2");
      expect(nextId("sketch_", existing2)).toBe("sketch_2");
    });

    it("should return smallest non-colliding ID monotonically", () => {
      const existing = new Set<string>();
      expect(nextId("sketch_", existing)).toBe("sketch_0");
      existing.add("sketch_0");
      expect(nextId("sketch_", existing)).toBe("sketch_1");
      existing.add("sketch_1");
      expect(nextId("sketch_", existing)).toBe("sketch_2");
      existing.add("sketch_2");
      existing.add("sketch_3");
      expect(nextId("sketch_", existing)).toBe("sketch_4");
    });

    it("should handle different prefixes independently", () => {
      const existing = new Set<string>();
      expect(nextId("sketch_", existing)).toBe("sketch_0");
      expect(nextId("extrude_", existing)).toBe("extrude_0");
      expect(nextId("extrude_cut_", existing)).toBe("extrude_cut_0");
      existing.add("sketch_0");
      expect(nextId("sketch_", existing)).toBe("sketch_1");
      expect(nextId("extrude_", existing)).toBe("extrude_0"); // unchanged
    });
  });

  // エッジケース・退化入力テスト (phase 6.6 敵対ペルソナ追加)
  describe("edge cases: degenerate input handling", () => {
    it("should handle empty segments array gracefully", () => {
      const sketch = { planeRefId: "Front" as const, segments: [] };
      const result = buildCreateSketchFromSketch(sketch, "sk_empty");
      if (result.type !== "create_sketch") throw new Error("expected create_sketch");
      expect(result.profile).toEqual([]);
      expect(result.plane_ref).toBe("Front");
    });

    it("should handle single segment (line)", () => {
      const sketch = {
        planeRefId: "Top" as const,
        segments: [{ from: { x: 0, y: 0 }, to: { x: 10, y: 0 } }],
      };
      const result = buildCreateSketchFromSketch(sketch, "sk_line");
      if (result.type !== "create_sketch") throw new Error("expected create_sketch");
      expect(result.profile.length).toBe(1);
      expect(result.profile[0].id).toBe("sk_line_seg_0");
    });

    it("should handle coincident endpoints (degenerate segment)", () => {
      const sketch = {
        planeRefId: "Right" as const,
        segments: [{ from: { x: 5, y: 5 }, to: { x: 5, y: 5 } }],
      };
      const result = buildCreateSketchFromSketch(sketch, "sk_degen");
      if (result.type !== "create_sketch") throw new Error("expected create_sketch");
      expect(result.profile[0].from).toEqual([5, 5]);
      expect(result.profile[0].to).toEqual([5, 5]);
    });

    it("should handle large coordinate values (near f64 limits)", () => {
      const large = 1e100;
      const sketch = {
        planeRefId: "Front" as const,
        segments: [{ from: { x: -large, y: -large }, to: { x: large, y: large } }],
      };
      const result = buildCreateSketchFromSketch(sketch, "sk_large");
      if (result.type !== "create_sketch") throw new Error("expected create_sketch");
      expect(result.profile[0].from[0]).toBe(-large);
      expect(result.profile[0].to[0]).toBe(large);
    });

    it("should handle very small coordinate values (near zero)", () => {
      const tiny = 1e-100;
      const sketch = {
        planeRefId: "Top" as const,
        segments: [{ from: { x: 0, y: 0 }, to: { x: tiny, y: tiny } }],
      };
      const result = buildCreateSketchFromSketch(sketch, "sk_tiny");
      if (result.type !== "create_sketch") throw new Error("expected create_sketch");
      expect(result.profile[0].to[0]).toBe(tiny);
    });
  });

  // 決定性・再現性テスト
  describe("determinism: repeated calls produce identical results", () => {
    it("should produce identical segment IDs for same sketch across multiple calls", () => {
      const sketch = {
        planeRefId: "Front" as const,
        segments: [
          { from: { x: 0, y: 0 }, to: { x: 5, y: 0 } },
          { from: { x: 5, y: 0 }, to: { x: 5, y: 5 } },
          { from: { x: 5, y: 5 }, to: { x: 0, y: 5 } },
          { from: { x: 0, y: 5 }, to: { x: 0, y: 0 } },
        ],
      };
      const sketchId = "sk_det";

      const results = [];
      for (let i = 0; i < 10; i++) {
        const result = buildCreateSketchFromSketch(sketch, sketchId);
        if (result.type !== "create_sketch") throw new Error("expected create_sketch");
        results.push(result);
      }

      // 全ての結果が最初の結果と一致することを確認
      const first = results[0];
      for (let i = 1; i < results.length; i++) {
        expect(results[i]).toEqual(first);
      }
    });
  });

  // ラウンドトリップテスト (Feature → serialize → deserialize)
  describe("round-trip: Feature serialization consistency", () => {
    it("should produce Feature that survives JSON.stringify/parse", () => {
      const sketch = {
        planeRefId: "Front" as const,
        segments: [
          { from: { x: 0, y: 0 }, to: { x: 5, y: 0 } },
          { from: { x: 5, y: 0 }, to: { x: 5, y: 5 } },
        ],
      };
      const original = buildCreateSketchFromSketch(sketch, "sk_rt");
      if (original.type !== "create_sketch") throw new Error("expected create_sketch");

      const serialized = JSON.stringify(original);
      const deserialized = JSON.parse(serialized);

      expect(deserialized).toEqual(original);
      expect(deserialized.id).toBe(original.id);
      expect(deserialized.plane_ref).toBe(original.plane_ref);
    });
  });
});
