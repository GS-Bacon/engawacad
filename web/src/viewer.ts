import type { BodyMesh } from "./generated/BodyMesh";
import {
  AmbientLight,
  AxesHelper,
  Box3,
  Color,
  DirectionalLight,
  Group,
  Mesh,
  MeshPhongMaterial,
  PerspectiveCamera,
  Raycaster,
  Scene,
  Vector2,
  Vector3,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { meshToGeometry } from "./mesh";

const BODY_COLORS = [0x4a90d9, 0xd94a4a, 0x4ad94a, 0xd9d94a, 0xd94ad9, 0x4ad9d9];

export let selectedFaceId: string | null = null;

export interface ViewerHandle {
  updateBodies(bodies: BodyMesh[]): void;
  getSelectedFaceVertices(): { positions: Float32Array; indices: Uint32Array; faceIds: string[]; faceId: string } | null;
  dispose(): void;
}

export function initViewer(
  container: HTMLElement,
  bodies: BodyMesh[],
  opts?: { onSelectionChange?: (faceId: string | null) => void },
): ViewerHandle {
  const width = container.clientWidth;
  const height = container.clientHeight;

  const scene = new Scene();
  scene.background = new Color(0xf0f0f0);

  const camera = new PerspectiveCamera(50, width / height, 1.0, 10000);

  const renderer = new WebGLRenderer({ antialias: true });
  renderer.setSize(width, height);
  renderer.setPixelRatio(window.devicePixelRatio);
  container.appendChild(renderer.domElement);

  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;

  const group = new Group();
  const pickMeshes: Mesh[] = [];
  const highlightMaterial = new MeshPhongMaterial({
    color: 0xffaa00,
    specular: 0x333333,
    shininess: 30,
  });

  function buildScene(bodies: BodyMesh[]): void {
    // Clear existing
    while (group.children.length > 0) {
      const child = group.children[0] as Mesh;
      child.geometry.dispose();
      group.remove(child);
    }
    pickMeshes.length = 0;

    for (let i = 0; i < bodies.length; i++) {
      const geometry = meshToGeometry(bodies[i].mesh);
      geometry.computeBoundingBox();
      const color = BODY_COLORS[i % BODY_COLORS.length];
      const baseMaterial = new MeshPhongMaterial({
        color,
        specular: 0x333333,
        shininess: 30,
      });
      const hasFaceIds = (geometry.userData.faceIds as unknown[])?.length > 0;
      const threeMesh = hasFaceIds
        ? new Mesh(geometry, [baseMaterial, highlightMaterial])
        : new Mesh(geometry, baseMaterial);
      group.add(threeMesh);
      pickMeshes.push(threeMesh);
    }
  }

  buildScene(bodies);
  scene.add(group);

  function fitCamera(): void {
    const bbox = new Box3().setFromObject(group);
    const size = new Vector3();
    let axisSize = 10;
    if (!bbox.isEmpty()) {
      const center = new Vector3();
      bbox.getCenter(center);
      controls.target.copy(center);
      bbox.getSize(size);
      const maxDim = Math.max(size.x, size.y, size.z);
      axisSize = Math.max(maxDim * 0.5, 1);
      const distance = maxDim * 2;
      camera.position.set(
        center.x + distance * 0.5,
        center.y + distance * 0.5,
        center.z + distance,
      );
      controls.minDistance = Math.max(camera.near * 2, maxDim * 0.1);
    }
    // Remove old axis helper if present
    const oldAxis = scene.children.find((c) => c instanceof AxesHelper);
    if (oldAxis) scene.remove(oldAxis);
    scene.add(new AxesHelper(axisSize));
    camera.lookAt(controls.target);
  }

  fitCamera();

  scene.add(new AmbientLight(0xffffff, 0.6));
  const dirLight = new DirectionalLight(0xffffff, 0.8);
  dirLight.position.set(5, 10, 7);
  scene.add(dirLight);

  controls.update();

  function onResize() {
    const w = container.clientWidth;
    const h = container.clientHeight;
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    renderer.setSize(w, h);
  }
  window.addEventListener("resize", onResize);

  // --- face picking (#94) ---
  const raycaster = new Raycaster();
  const ndc = new Vector2();
  const selectedEl = document.querySelector<HTMLElement>(
    '[data-testid="selected-face-id"]',
  );

  function setSelection(faceId: string | null): void {
    selectedFaceId = faceId;
    if (selectedEl) {
      selectedEl.textContent = faceId ?? "";
      selectedEl.style.display = faceId ? "block" : "none";
    }
    for (const m of pickMeshes) {
      const gfids: string[] = m.geometry.userData.groupFaceIds ?? [];
      const groups = m.geometry.groups;
      for (let k = 0; k < groups.length; k++) {
        groups[k].materialIndex =
          faceId !== null && faceId !== "" && gfids[k] === faceId ? 1 : 0;
      }
    }
    opts?.onSelectionChange?.(faceId);
  }

  let downX = 0,
    downY = 0;
  const DRAG_THRESHOLD = 5;
  renderer.domElement.addEventListener("pointerdown", (e) => {
    downX = e.clientX;
    downY = e.clientY;
  });
  renderer.domElement.addEventListener("pointerup", (e) => {
    if (e.button !== 0) return;
    if (Math.hypot(e.clientX - downX, e.clientY - downY) > DRAG_THRESHOLD)
      return;
    const rect = renderer.domElement.getBoundingClientRect();
    ndc.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    ndc.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
    raycaster.setFromCamera(ndc, camera);
    const hits = raycaster.intersectObjects(pickMeshes, false);
    if (hits.length > 0 && hits[0].faceIndex != null) {
      const fids: string[] = (hits[0].object as Mesh).geometry.userData.faceIds ?? [];
      setSelection(fids[hits[0].faceIndex] ?? null);
    } else {
      setSelection(null);
    }
  });

  let animFrameId: number;
  function animate() {
    animFrameId = requestAnimationFrame(animate);
    controls.update();
    renderer.render(scene, camera);
  }
  animate();

  return {
    updateBodies(newBodies: BodyMesh[]) {
      buildScene(newBodies);
      fitCamera();
      setSelection(null);
    },
    getSelectedFaceVertices() {
      if (!selectedFaceId) return null;
      for (const m of pickMeshes) {
        const fids: string[] = m.geometry.userData.faceIds ?? [];
        if (fids.includes(selectedFaceId)) {
          const posAttr = m.geometry.getAttribute("position");
          const idxAttr = m.geometry.index;
          const indices = idxAttr
            ? new Uint32Array(idxAttr.array as Uint32Array)
            : new Uint32Array(posAttr.count).map((_, i) => i);
          return {
            positions: new Float32Array(posAttr.array as Float32Array),
            indices,
            faceIds: fids,
            faceId: selectedFaceId,
          };
        }
      }
      return null;
    },
    dispose() {
      cancelAnimationFrame(animFrameId);
      window.removeEventListener("resize", onResize);
      renderer.dispose();
      for (const m of pickMeshes) {
        m.geometry.dispose();
      }
    },
  };
}
