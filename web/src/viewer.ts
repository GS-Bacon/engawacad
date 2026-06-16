import type { BodyMesh } from "./generated/BodyMesh";
import { log } from "./logger";
import {
  AmbientLight,
  AxesHelper,
  Box3,
  Color,
  DirectionalLight,
  Group,
  Mesh,
  MeshBasicMaterial,
  MeshPhongMaterial,
  PerspectiveCamera,
  PlaneGeometry,
  Raycaster,
  Scene,
  Vector2,
  Vector3,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { meshToGeometry } from "./mesh";

export type RefPlaneId = "Front" | "Top" | "Right";

const BODY_COLORS = [0x4a90d9, 0xd94a4a, 0x4ad94a, 0xd9d94a, 0xd94ad9, 0x4ad9d9];

export let selectedFaceId: string | null = null;

export interface ViewerHandle {
  updateBodies(bodies: BodyMesh[]): void;
  getSelectedFaceVertices(): { positions: Float32Array; indices: Uint32Array; faceIds: string[]; faceId: string } | null;
  setView(view: "front" | "top" | "iso"): void;
  dispose(): void;
  onRefPlaneSelected: (id: RefPlaneId | null) => void;
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
  controls.dampingFactor = 0.1;

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

    // Expose raw mesh data for E2E geometric invariant checks (assertMeshHealthy).
    // Not used for rendering; safe to update on every rebuild.
    (window as any).__meshData = pickMeshes.map((m) => {
      const posAttr = m.geometry.getAttribute("position");
      const normAttr = m.geometry.getAttribute("normal");
      return {
        positions: posAttr ? Array.from(posAttr.array as Float32Array) : [],
        normals: normAttr ? Array.from(normAttr.array as Float32Array) : [],
        indices: m.geometry.index
          ? Array.from(m.geometry.index.array as Uint32Array)
          : [],
      };
    });
    // Monotonically-increasing rebuild counter — tests can check that
    // updateBodies() was called by comparing version before/after.
    (window as any).__meshDataVersion =
      ((window as any).__meshDataVersion ?? 0) + 1;
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
    } else {
      // RefPlane (#163) は body がなくても 3 枚表示される (±10 を覆う)。
      // empty scene でも Iso 視点に置いて raycaster が正しく動くようにする。
      controls.target.set(0, 0, 0);
      camera.position.set(15, 15, 30);
      controls.minDistance = Math.max(camera.near * 2, 2);
      axisSize = 10;
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

  // --- RefPlane (#163) ---
  const REFSIZE = 20;
  const refPlaneMeshes: Mesh[] = [];
  let selectedRefPlaneId: RefPlaneId | null = null;
  let onRefPlaneSelected: (id: RefPlaneId | null) => void = () => {};

  function addRefPlanes(targetScene: Scene): void {
    const geo = new PlaneGeometry(REFSIZE, REFSIZE);

    // Front: XY plane (normal +Z), red
    const front = new Mesh(geo, new MeshBasicMaterial({
      color: 0xff4040,
      transparent: true,
      opacity: 0.2,
      side: 2, // DoubleSide
    }));
    front.userData.refPlaneId = "Front";
    targetScene.add(front);
    refPlaneMeshes.push(front);

    // Top: XZ plane (normal +Y), green
    const top = new Mesh(geo, new MeshBasicMaterial({
      color: 0x40ff40,
      transparent: true,
      opacity: 0.2,
      side: 2,
    }));
    top.rotation.x = -Math.PI / 2;
    top.userData.refPlaneId = "Top";
    targetScene.add(top);
    refPlaneMeshes.push(top);

    // Right: YZ plane (normal +X), blue
    const right = new Mesh(geo, new MeshBasicMaterial({
      color: 0x4040ff,
      transparent: true,
      opacity: 0.2,
      side: 2,
    }));
    right.rotation.y = Math.PI / 2;
    right.userData.refPlaneId = "Right";
    targetScene.add(right);
    refPlaneMeshes.push(right);
  }

  function selectRefPlane(id: RefPlaneId | null): void {
    selectedRefPlaneId = id;
    for (const m of refPlaneMeshes) {
      const mat = m.material as MeshBasicMaterial;
      const meshId = m.userData.refPlaneId as RefPlaneId;
      mat.opacity = (meshId === id) ? 0.45 : 0.2;
    }
    onRefPlaneSelected(id);
  }

  addRefPlanes(scene);

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

    // Prioritize face picking over RefPlane (#163)
    if (hits.length > 0 && hits[0].faceIndex != null) {
      const fids: string[] = (hits[0].object as Mesh).geometry.userData.faceIds ?? [];
      setSelection(fids[hits[0].faceIndex] ?? null);
      selectRefPlane(null); // Clear RefPlane selection when face is picked
    } else {
      setSelection(null);
      // Check RefPlane intersection
      const refHits = raycaster.intersectObjects(refPlaneMeshes, false);
      if (refHits.length > 0) {
        const id = (refHits[0].object as Mesh).userData.refPlaneId as RefPlaneId;
        selectRefPlane(id);
      } else {
        selectRefPlane(null);
      }
    }
  });

  let animFrameId: number;
  function animate() {
    animFrameId = requestAnimationFrame(animate);
    controls.update();
    renderer.render(scene, camera);
  }
  (window as any).__viewer = { renderer, camera, controls, scene };

  // ドラッグ/ズーム/パン完了時のみ記録（フレームごとの連続ログは出さない）
  controls.addEventListener("end", () => {
    const p = camera.position;
    const tg = controls.target;
    log("camera_snap", {
      pos: [+p.x.toFixed(3), +p.y.toFixed(3), +p.z.toFixed(3)],
      target: [+tg.x.toFixed(3), +tg.y.toFixed(3), +tg.z.toFixed(3)],
    });
  });

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
    setView(view: "front" | "top" | "iso") {
      const d = camera.position.distanceTo(controls.target);
      const t = controls.target;
      if (view === "front") {
        camera.up.set(0, 1, 0);
        camera.position.set(t.x, t.y, t.z + d);
      } else if (view === "top") {
        // look 方向が (0,-1,0) のため up = (0,1,0) と平行になる → -Z を up に設定
        camera.up.set(0, 0, -1);
        camera.position.set(t.x, t.y + d, t.z);
      } else {
        camera.up.set(0, 1, 0);
        const len = Math.sqrt(1.5);
        camera.position.set(t.x + d * 0.5 / len, t.y + d * 0.5 / len, t.z + d / len);
      }
      camera.lookAt(controls.target);
      camera.updateMatrixWorld();
      controls.update();
      log("view_change", { view });
    },
    get onRefPlaneSelected() {
      return onRefPlaneSelected;
    },
    set onRefPlaneSelected(cb: (id: RefPlaneId | null) => void) {
      onRefPlaneSelected = cb;
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
