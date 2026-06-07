import { fetchBodies, postFeature } from "./api";
import { initViewer } from "./viewer";
import { planeForFaceId, buildExtrudeFeatures, buildExtrudeCutFeatures } from "./extrude";

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

async function main(): Promise<void> {
  try {
    let currentBodies = await fetchBodies();
    if (currentBodies.length === 0) {
      showError("No bodies found in document");
      return;
    }

    // Tracks ALL feature IDs ever used (bodies + sketches), grows monotonically.
    // Prevents duplicate IDs across consecutive extrude / extrude-cut operations.
    const usedFeatureIds = new Set(currentBodies.map((b) => b.feature_id));

    const handle = initViewer(app, currentBodies, { onSelectionChange: onSelect });

    const panel = document.querySelector<HTMLElement>('[data-testid="extrude-panel"]')!;
    const depthInput = document.querySelector<HTMLInputElement>('[data-testid="extrude-depth"]')!;
    const btn = document.querySelector<HTMLButtonElement>('[data-testid="btn-extrude"]')!;

    function onSelect(faceId: string | null): void {
      panel.style.display = faceId && planeForFaceId(faceId) ? "block" : "none";
    }

    btn.addEventListener("click", async () => {
      const sel = handle.getSelectedFaceVertices();
      const depth = Number(depthInput.value);
      if (!sel || !Number.isFinite(depth) || depth <= 0) return;
      const built = buildExtrudeFeatures(sel.faceId, sel.positions, sel.indices, sel.faceIds, depth, usedFeatureIds);
      if (!built) return;
      // Reserve IDs before any POST so partial failures don't cause reuse on retry.
      usedFeatureIds.add(built.sketch.id);
      usedFeatureIds.add(built.extrude.id);
      try {
        await postFeature(built.sketch);
        const updated = await postFeature(built.extrude);
        currentBodies = updated;
        handle.updateBodies(updated);
      } catch (err) {
        showError(err instanceof Error ? err.message : String(err));
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
      // Reserve IDs before any POST so partial failures don't cause reuse on retry.
      usedFeatureIds.add(built.sketch.id);
      usedFeatureIds.add(built.extrudeCut.id);
      try {
        await postFeature(built.sketch);
        const updated = await postFeature(built.extrudeCut);
        currentBodies = updated;
        handle.updateBodies(updated);
      } catch (err) {
        showError(err instanceof Error ? err.message : String(err));
      }
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    showError(message);
  }
}

main();
