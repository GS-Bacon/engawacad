import { fetchBodies, postFeature } from "./api";
import { initViewer } from "./viewer";
import { planeForFaceId, buildExtrudeFeatures } from "./extrude";

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
      const existing = new Set(currentBodies.map((b) => b.feature_id));
      const built = buildExtrudeFeatures(sel.faceId, sel.positions, sel.indices, sel.faceIds, depth, existing);
      if (!built) return;
      try {
        await postFeature(built.sketch);
        const updated = await postFeature(built.extrude);
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
