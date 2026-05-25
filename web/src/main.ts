import { fetchMesh } from "./api";
import { initViewer } from "./viewer";

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
  const params = new URLSearchParams(window.location.search);
  const file = params.get("file");

  if (!file) {
    showInfo("Usage: ?file=<absolute-path-to-.mycad>");
    return;
  }

  try {
    const mesh = await fetchMesh(file);
    initViewer(app, mesh);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    showError(message);
  }
}

main();
