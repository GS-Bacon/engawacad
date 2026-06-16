export type RefPlaneId = "Front" | "Top" | "Right";

export interface SketchSegment {
  from: { x: number; y: number };
  to: { x: number; y: number };
}

export interface Sketch {
  planeRefId: RefPlaneId;
  segments: SketchSegment[];
}

export interface SketchSession {
  addPoint(x: number, y: number): void;
  getSegments(): SketchSegment[];
  isClosed(): boolean;
  previewTo(x: number, y: number): { from: { x: number; y: number }; to: { x: number; y: number } } | null;
  finalize(): Sketch | null;
  reset(): void;
}

export const SNAP_RADIUS = 0.1;
export const CLOSE_EPS = 1e-9;

function dist2(a: { x: number; y: number }, b: { x: number; y: number }): number {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return dx * dx + dy * dy;
}

export function createSketchSession(planeRefId: RefPlaneId): SketchSession {
  let points: { x: number; y: number }[] = [];
  let segments: SketchSegment[] = [];

  return {
    addPoint(x: number, y: number): void {
      let snapped = { x, y };

      // Snap to existing vertices within SNAP_RADIUS。
      // tie-break (Codex F03): 同距離なら配列先頭側 = 最早の頂点を優先する。
      // 初期 bestDist2 は SNAP_RADIUS の自乗以下を採用するため <= を使い、
      // 以降のループでは strict < で更新 (= 最早の bestIdx を保持)。
      let bestIdx = -1;
      let bestDist2 = SNAP_RADIUS * SNAP_RADIUS;
      for (let i = 0; i < points.length; i++) {
        const d2 = dist2({ x, y }, points[i]);
        if (bestIdx === -1 ? d2 <= bestDist2 : d2 < bestDist2) {
          bestDist2 = d2;
          bestIdx = i;
        }
      }
      if (bestIdx >= 0) {
        snapped = points[bestIdx];
      }

      if (points.length === 0) {
        points.push(snapped);
        return;
      }

      const last = points[points.length - 1];
      if (snapped.x === last.x && snapped.y === last.y) {
        return;
      }

      // 始点 snap (premature close チェック, Codex r5 F01):
      // segments.length < 2 (= 確定後 triangle 未満) の状態で始点に戻ろうとした場合、
      // ループを成立させないし、state も壊さない (segments.push せず、points も触らない)。
      const closingToStart = (snapped.x === points[0].x && snapped.y === points[0].y);
      if (closingToStart && segments.length < 2) {
        return;
      }

      segments.push({ from: last, to: snapped });

      if (closingToStart) {
        return; // closed: points には push しない
      }

      points.push(snapped);
    },

    getSegments(): SketchSegment[] {
      return [...segments];
    },

    isClosed(): boolean {
      if (segments.length < 3) return false;
      if (segments.length === 0) return false;
      const last = segments[segments.length - 1].to;
      const first = segments[0].from;
      return dist2(last, first) <= CLOSE_EPS * CLOSE_EPS;
    },

    previewTo(x: number, y: number): { from: { x: number; y: number }; to: { x: number; y: number } } | null {
      if (points.length === 0) return null;
      const last = points[points.length - 1];
      return { from: { ...last }, to: { x, y } };
    },

    finalize(): Sketch | null {
      if (!this.isClosed()) return null;
      return { planeRefId, segments: [...segments] };
    },

    reset(): void {
      points = [];
      segments = [];
    },
  };
}
