import { fetchAllFeatureIds, fetchBodies, postFeature } from "./api";
import { initViewer, type RefPlaneId, type SketchPoint3D } from "./viewer";
import { planeForFaceId, planeForFaceNormal, buildExtrudeFeatures, buildExtrudeCutFeatures } from "./extrude";
import { createSketchSession, type Sketch, type SketchSession as SketchSessionType } from "./sketch";
import { log, clearLog, getEntries, formatLog } from "./logger";

const app = document.getElementById("app")!;
const errorEl = document.getElementById("error")!;
const infoEl = document.getElementById("info")!;

function showError(msg: string): void {
  errorEl.textContent = msg;
  errorEl.style.display = "block";
}

function showInfo(msg: string): void {
  infoEl.textContent = msg;
  infoEl.style.display = "block";
}

// Ctrl+Shift+L でログをクリップボードにコピー
document.addEventListener("keydown", (e) => {
  if (e.ctrlKey && e.shiftKey && e.key === "L") {
    e.preventDefault();
    navigator.clipboard.writeText(formatLog()).then(() => {
      console.info("[action-log] copied to clipboard");
    });
  }
});

// ブラウザコンソールからも参照できるように公開
(window as any).__actionLog = {
  get entries() { return getEntries(); },
  copy() { navigator.clipboard.writeText(formatLog()); return "copied!"; },
  clear() { clearLog(); return "cleared!"; },
  format: formatLog,
};

async function main(): Promise<void> {
  try {
    // Empty document (no bodies) is valid: RefPlane (#163) を選択して新規スケッチを開始できる必要がある。
    let currentBodies = await fetchBodies();

    // Tracks ALL feature IDs ever used (bodies + sketches), grows monotonically.
    // Prevents duplicate IDs across consecutive extrude / extrude-cut operations.
    const allFeatureIds = await fetchAllFeatureIds();
    const usedFeatureIds = new Set<string>(allFeatureIds);

    const handle = initViewer(app, currentBodies, {
      onSelectionChange: onSelect,
      onSketchPoint: (u: number, v: number) => {
        if (!currentSession || !selectedRefPlaneId) return;
        const segmentsBefore = currentSession.getSegments().length;
        currentSession.addPoint(u, v);
        const segmentsAfter = currentSession.getSegments().length;

        if (segmentsAfter > segmentsBefore) {
          const newSeg = currentSession.getSegments()[segmentsAfter - 1];
          const from = uvToWorld(newSeg.from.x, newSeg.from.y, selectedRefPlaneId);
          const to = uvToWorld(newSeg.to.x, newSeg.to.y, selectedRefPlaneId);
          handle.addSketchSegment(from, to);
        }
        updateSketchCanvasState();

        // Issue #163 完了条件 ①: 閉ループ達成時に自動 finalize。
        // finalize 後はセッションを終了 + スケッチモードを抜けて OrbitControls を再有効化する
        // (Codex r3 F01: sketchMode が残ると UI 閉じ込め)。閉じた輪郭の overlay は次の
        // start-sketch クリックまで残す (視覚的に finalized されたことを示す)。
        if (currentSession.isClosed()) {
          const sketch = currentSession.finalize();
          if (sketch) {
            log("sketch_finalized_auto", { planeRefId: sketch.planeRefId, segments: sketch.segments.length });
            console.log("[sketch] finalized:", sketch);
          }
          currentSession = null;
          handle.setSketchMode(false);
        }
      },
    });
    log("init", { bodies: currentBodies.length });

    // RefPlane selection (#163)
    const selectedRefPlaneEl = document.querySelector<HTMLElement>('[data-testid="selected-refplane"]')!;
    let selectedRefPlaneId: RefPlaneId | null = null;
    handle.onRefPlaneSelected = (id: RefPlaneId | null) => {
      selectedRefPlaneId = id;
      selectedRefPlaneEl.textContent = id ?? "";
      selectedRefPlaneEl.style.display = id ? "block" : "none";
    };

    // Sketch (#164)
    let currentSession: SketchSessionType | null = null;
    const sketchCanvasEl = document.querySelector<HTMLElement>('[data-testid="sketch-canvas"]')!;

    function uvToWorld(u: number, v: number, planeId: RefPlaneId): SketchPoint3D {
      switch (planeId) {
        case "Front":
          return { x: u, y: v, z: 0 };
        case "Top":
          return { x: u, y: 0, z: v };
        case "Right":
          return { x: 0, y: u, z: v };
      }
    }

    function updateSketchCanvasState(): void {
      if (!currentSession) {
        sketchCanvasEl.dataset.state = "idle";
        sketchCanvasEl.classList.remove("sketch-canvas--open");
        return;
      }
      if (currentSession.isClosed()) {
        sketchCanvasEl.dataset.state = "closed";
        sketchCanvasEl.classList.remove("sketch-canvas--open");
      } else if (currentSession.getSegments().length > 0) {
        sketchCanvasEl.dataset.state = "open";
        sketchCanvasEl.classList.add("sketch-canvas--open");
      } else {
        sketchCanvasEl.dataset.state = "idle";
        sketchCanvasEl.classList.remove("sketch-canvas--open");
      }
    }

    const btnStartSketch = document.querySelector<HTMLButtonElement>('[data-testid="btn-start-sketch"]')!;
    btnStartSketch.addEventListener("click", () => {
      if (!selectedRefPlaneId) return;
      // 新規セッション開始前に既存 overlay を破棄 (Codex F01 指摘: 古い線分が残る)
      handle.clearSketchOverlay();
      handle.setSketchMode(true);
      currentSession = createSketchSession(selectedRefPlaneId);
      updateSketchCanvasState();
    });

    const viewFront = document.querySelector<HTMLButtonElement>('[data-testid="btn-view-front"]')!;
    const viewTop   = document.querySelector<HTMLButtonElement>('[data-testid="btn-view-top"]')!;
    const viewIso   = document.querySelector<HTMLButtonElement>('[data-testid="btn-view-iso"]')!;
    viewFront.addEventListener("click", () => handle.setView("front"));
    viewTop.addEventListener("click",   () => handle.setView("top"));
    viewIso.addEventListener("click",   () => handle.setView("iso"));

    const panel = document.querySelector<HTMLElement>('[data-testid="extrude-panel"]')!;
    const depthInput = document.querySelector<HTMLInputElement>('[data-testid="extrude-depth"]')!;
    const btn = document.querySelector<HTMLButtonElement>('[data-testid="btn-extrude"]')!;

    function onSelect(faceId: string | null): void {
      log("face_pick", { faceId });
      if (!faceId) { panel.style.display = "none"; return; }
      const sel = handle.getSelectedFaceVertices();
      const plane = planeForFaceId(faceId) ??
        (sel ? planeForFaceNormal(sel.positions, sel.indices, sel.faceIds, faceId) : null);
      panel.style.display = plane ? "block" : "none";
    }

    btn.addEventListener("click", async () => {
      const sel = handle.getSelectedFaceVertices();
      const depth = Number(depthInput.value);
      if (!sel || !Number.isFinite(depth) || depth <= 0) return;
      const built = buildExtrudeFeatures(sel.faceId, sel.positions, sel.indices, sel.faceIds, depth, usedFeatureIds);
      if (!built) return;
      log("extrude_submit", { faceId: sel.faceId, depth });
      // Reserve IDs before any POST so partial failures don't cause reuse on retry.
      usedFeatureIds.add(built.sketch.id);
      usedFeatureIds.add(built.extrude.id);
      try {
        await postFeature(built.sketch);
        const updated = await postFeature(built.extrude);
        currentBodies = updated;
        handle.updateBodies(updated);
        log("extrude_ok", { bodies: updated.length });
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        log("extrude_error", { error: msg });
        showError(msg);
      }
    });

    const cutBtn = document.querySelector<HTMLButtonElement>('[data-testid="btn-extrude-cut"]')!;
    cutBtn.addEventListener("click", async () => {
      const sel = handle.getSelectedFaceVertices();
      const depth = Number(depthInput.value);
      if (!sel || !Number.isFinite(depth) || depth <= 0) return;
      const target = currentBodies.find((b) => b.mesh.face_ids.some((fid: string) => fid === sel.faceId))?.feature_id;
      if (!target) return;
      const built = buildExtrudeCutFeatures(sel.faceId, sel.positions, sel.indices, sel.faceIds, depth, target, usedFeatureIds);
      if (!built) return;
      log("extrude_cut_submit", { faceId: sel.faceId, depth, target });
      // Reserve IDs before any POST so partial failures don't cause reuse on retry.
      usedFeatureIds.add(built.sketch.id);
      usedFeatureIds.add(built.extrudeCut.id);
      try {
        await postFeature(built.sketch);
        const updated = await postFeature(built.extrudeCut);
        currentBodies = updated;
        handle.updateBodies(updated);
        log("extrude_cut_ok", { bodies: updated.length });
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        log("extrude_cut_error", { error: msg });
        showError(msg);
      }
    });

    // Sketch mouse move handler (#164)
    const canvasEl = document.querySelector("canvas")!;
    canvasEl.addEventListener("pointermove", (e) => {
      if (!currentSession || !selectedRefPlaneId) return;
      const rect = canvasEl.getBoundingClientRect();
      const ndc = {
        x: ((e.clientX - rect.left) / rect.width) * 2 - 1,
        y: -((e.clientY - rect.top) / rect.height) * 2 + 1,
      };

      // Convert NDC to ray and find intersection with RefPlane
      const camera = (window as any).__viewer?.camera;
      const raycaster = (window as any).__viewer?.raycaster;
      if (!camera || !raycaster) return;

      raycaster.setFromCamera(ndc, camera);

      // Simple plane intersection (planes pass through origin)
      const planeNormal = selectedRefPlaneId === "Front" ? { x: 0, y: 0, z: 1 } :
                         selectedRefPlaneId === "Top" ? { x: 0, y: 1, z: 0 } :
                         { x: 1, y: 0, z: 0 };
      const rayOrigin = raycaster.ray.origin;
      const rayDir = raycaster.ray.direction;
      const denom = planeNormal.x * rayDir.x + planeNormal.y * rayDir.y + planeNormal.z * rayDir.z;
      if (Math.abs(denom) < 1e-6) return;

      const t = -(planeNormal.x * rayOrigin.x + planeNormal.y * rayOrigin.y + planeNormal.z * rayOrigin.z) / denom;
      if (t < 0) return;

      const point = {
        x: rayOrigin.x + t * rayDir.x,
        y: rayOrigin.y + t * rayDir.y,
        z: rayOrigin.z + t * rayDir.z,
      };

      const uv = selectedRefPlaneId === "Front" ? { u: point.x, v: point.y } :
                 selectedRefPlaneId === "Top" ? { u: point.x, v: point.z } :
                 { u: point.y, v: point.z };

      const preview = currentSession.previewTo(uv.u, uv.v);
      if (preview) {
        const from3d = uvToWorld(preview.from.x, preview.from.y, selectedRefPlaneId);
        const to3d = uvToWorld(preview.to.x, preview.to.y, selectedRefPlaneId);
        handle.setSketchPreview(from3d, to3d);
      } else {
        handle.setSketchPreview(null, null);
      }
    });

    // Sketch key handlers (#164)
    document.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && currentSession) {
        const finalized = currentSession.finalize();
        if (finalized) {
          console.log("[sketch] finalized:", finalized);
        }
      }
      if (e.key === "Escape" && currentSession) {
        currentSession.reset();
        handle.clearSketchOverlay();
        handle.setSketchMode(false);
        currentSession = null;
        updateSketchCanvasState();
      }
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    showError(message);
  }
}

main();
