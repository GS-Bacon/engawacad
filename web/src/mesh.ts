import type { TriangleMesh } from "./generated/TriangleMesh";
import { BufferAttribute, BufferGeometry } from "three";

export class MeshValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "MeshValidationError";
  }
}

export function validateMesh(mesh: TriangleMesh): void {
  const { positions, normals, indices } = mesh;

  if (normals.length !== positions.length) {
    throw new MeshValidationError(
      `normals length (${normals.length}) != positions length (${positions.length})`,
    );
  }

  for (let i = 0; i < positions.length; i++) {
    const [x, y, z] = positions[i];
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) {
      throw new MeshValidationError(
        `non-finite position at vertex ${i}: [${x}, ${y}, ${z}]`,
      );
    }
  }

  for (let i = 0; i < normals.length; i++) {
    const [nx, ny, nz] = normals[i];
    if (!Number.isFinite(nx) || !Number.isFinite(ny) || !Number.isFinite(nz)) {
      throw new MeshValidationError(
        `non-finite normal at vertex ${i}: [${nx}, ${ny}, ${nz}]`,
      );
    }
  }

  for (let i = 0; i < indices.length; i++) {
    const idx = indices[i];
    if (!Number.isInteger(idx) || idx < 0 || idx >= positions.length) {
      throw new MeshValidationError(
        `index ${idx} at position ${i} out of range [0, ${positions.length})`,
      );
    }
  }

  for (let i = 0; i + 2 < indices.length; i += 3) {
    const i0 = indices[i], i1 = indices[i + 1], i2 = indices[i + 2];
    const [x0, y0, z0] = positions[i0];
    const [x1, y1, z1] = positions[i1];
    const [x2, y2, z2] = positions[i2];
    const ex = x1 - x0, ey = y1 - y0, ez = z1 - z0;
    const fx = x2 - x0, fy = y2 - y0, fz = z2 - z0;
    const cx = ey * fz - ez * fy;
    const cy = ez * fx - ex * fz;
    const cz = ex * fy - ey * fx;
    if (cx === 0 && cy === 0 && cz === 0) {
      throw new MeshValidationError(
        `degenerate triangle at indices [${i0}, ${i1}, ${i2}]: zero area`,
      );
    }
  }
}

export function meshToGeometry(mesh: TriangleMesh): BufferGeometry {
  validateMesh(mesh);

  const positionData = new Float32Array(mesh.positions.length * 3);
  for (let i = 0; i < mesh.positions.length; i++) {
    positionData[i * 3] = mesh.positions[i][0];
    positionData[i * 3 + 1] = mesh.positions[i][1];
    positionData[i * 3 + 2] = mesh.positions[i][2];
  }

  const normalData = new Float32Array(mesh.normals.length * 3);
  for (let i = 0; i < mesh.normals.length; i++) {
    normalData[i * 3] = mesh.normals[i][0];
    normalData[i * 3 + 1] = mesh.normals[i][1];
    normalData[i * 3 + 2] = mesh.normals[i][2];
  }

  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new BufferAttribute(positionData, 3));
  geometry.setAttribute("normal", new BufferAttribute(normalData, 3));
  if (mesh.indices.length > 0) {
    geometry.setIndex(Array.from(mesh.indices));
  }

  const faceIds = mesh.face_ids ?? [];
  geometry.userData.faceIds = faceIds;

  // Same face_id triangles are contiguous in tessellation output.
  // Create one group per contiguous run for material-index switching.
  const groupFaceIds: string[] = [];
  let runStart = 0;
  for (let t = 1; t <= faceIds.length; t++) {
    if (t === faceIds.length || faceIds[t] !== faceIds[runStart]) {
      geometry.addGroup(runStart * 3, (t - runStart) * 3, 0);
      groupFaceIds.push(faceIds[runStart]);
      runStart = t;
    }
  }
  geometry.userData.groupFaceIds = groupFaceIds;

  return geometry;
}
