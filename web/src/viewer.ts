import type { BodyMesh } from "./generated/BodyMesh";
import {
  AmbientLight,
  Box3,
  Color,
  DirectionalLight,
  Group,
  Mesh,
  MeshPhongMaterial,
  PerspectiveCamera,
  Scene,
  Vector3,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { meshToGeometry } from "./mesh";

const BODY_COLORS = [0x4a90d9, 0xd94a4a, 0x4ad94a, 0xd9d94a, 0xd94ad9, 0x4ad9d9];

export function initViewer(container: HTMLElement, bodies: BodyMesh[]): void {
  const width = container.clientWidth;
  const height = container.clientHeight;

  const scene = new Scene();
  scene.background = new Color(0xf0f0f0);

  const camera = new PerspectiveCamera(50, width / height, 0.01, 10000);

  const renderer = new WebGLRenderer({ antialias: true });
  renderer.setSize(width, height);
  renderer.setPixelRatio(window.devicePixelRatio);
  container.appendChild(renderer.domElement);

  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;

  const group = new Group();

  for (let i = 0; i < bodies.length; i++) {
    const geometry = meshToGeometry(bodies[i].mesh);
    geometry.computeBoundingBox();
    const color = BODY_COLORS[i % BODY_COLORS.length];
    const material = new MeshPhongMaterial({
      color,
      specular: 0x333333,
      shininess: 30,
    });
    const threeMesh = new Mesh(geometry, material);
    group.add(threeMesh);
  }

  scene.add(group);

  const bbox = new Box3().setFromObject(group);
  if (!bbox.isEmpty()) {
    const center = new Vector3();
    bbox.getCenter(center);
    controls.target.copy(center);

    const size = new Vector3();
    bbox.getSize(size);
    const maxDim = Math.max(size.x, size.y, size.z);
    const distance = maxDim * 2;
    camera.position.set(
      center.x + distance * 0.5,
      center.y + distance * 0.5,
      center.z + distance,
    );
  }

  camera.lookAt(controls.target);

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

  function animate() {
    requestAnimationFrame(animate);
    controls.update();
    renderer.render(scene, camera);
  }
  animate();
}
