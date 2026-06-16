import { fetchAllFeatureIds, fetchBodies, postFeature } from "./api";
import { initViewer, type RefPlaneId } from "./viewer";
import { planeForFaceId, planeForFaceNormal, buildExtrudeFeatures, buildExtrudeCutFeatures } from "./extrude";
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

    const handle = initViewer(app, currentBodies, { onSelectionChange: onSelect });
    log("init", { bodies: currentBodies.length });

    // RefPlane selection (#163)
    const selectedRefPlaneEl = document.querySelector<HTMLElement>('[data-testid="selected-refplane"]')!;
    handle.onRefPlaneSelected = (id: RefPlaneId | null) => {
      selectedRefPlaneEl.textContent = id ?? "";
      selectedRefPlaneEl.style.display = id ? "block" : "none";
    };

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
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    showError(message);
  }
}

main();
