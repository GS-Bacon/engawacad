import { describe, it, expect } from "vitest";
import { createSketchSession, SNAP_RADIUS, CLOSE_EPS } from "./sketch";

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
});
