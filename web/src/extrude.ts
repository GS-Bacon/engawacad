/**
 * Pure functions for building extrude features from a selected face.
 * #95: Viewer-side logic for the Extrude UI.
 */
import type { Feature } from "./generated/Feature";
import type { SketchPlane } from "./generated/SketchPlane";

const EPSILON_GUARD = 1e-9;

/**
 * Map face_id axis role to the sketch plane that lies on that axis pair.
 * face_id format: `N(<fid>;face:f_<axis>_<sign>)` → axis determines the plane:
 *   Z → xy / Y → xz / X → yz
 * Returns null for empty string or unsupported roles.
 */
export function planeForFaceId(faceId: string): SketchPlane | null {
  if (!faceId) return null;
  const match = faceId.match(/f_([xyz])_[a-z]+/);
  if (!match) return null;
  switch (match[1]) {
    case "z":
      return "xy";
    case "y":
      return "xz";
    case "x":
      return "yz";
    default:
      return null;
  }
}

/**
 * Determine sketch plane from the geometric normal of a selected face.
 * Takes the first triangle matching `faceId`, computes its cross-product normal,
 * and maps the dominant axis to a sketch plane (same mapping as planeForFaceId).
 * Returns null for degenerate (zero-area) triangles or missing faceId.
 */
export function planeForFaceNormal(
  positions: ArrayLike<number>,
  indices: ArrayLike<number>,
  faceIds: string[],
  faceId: string,
): SketchPlane | null {
  // Find the first triangle matching the given faceId
  let triIdx = -1;
  for (let t = 0; t < faceIds.length; t++) {
    if (faceIds[t] === faceId) {
      triIdx = t;
      break;
    }
  }
  if (triIdx < 0) return null;

  const i0 = indices[triIdx * 3];
  const i1 = indices[triIdx * 3 + 1];
  const i2 = indices[triIdx * 3 + 2];
  const ax = positions[i0 * 3];
  const ay = positions[i0 * 3 + 1];
  const az = positions[i0 * 3 + 2];
  const bx = positions[i1 * 3];
  const by = positions[i1 * 3 + 1];
  const bz = positions[i1 * 3 + 2];
  const cx = positions[i2 * 3];
  const cy = positions[i2 * 3 + 1];
  const cz = positions[i2 * 3 + 2];

  // edge1 = B - A, edge2 = C - A
  const e1x = bx - ax;
  const e1y = by - ay;
  const e1z = bz - az;
  const e2x = cx - ax;
  const e2y = cy - ay;
  const e2z = cz - az;

  // cross product: normal = edge1 × edge2
  const nx = e1y * e2z - e1z * e2y;
  const ny = e1z * e2x - e1x * e2z;
  const nz = e1x * e2y - e1y * e2x;

  const len = Math.sqrt(nx * nx + ny * ny + nz * nz);
  if (len <= EPSILON_GUARD) return null;

  const anx = Math.abs(nx / len);
  const any_ = Math.abs(ny / len);
  const anz = Math.abs(nz / len);

  if (anz >= any_ && anz >= anx) return "xy";
  if (any_ >= anz && any_ >= anx) return "xz";
  return "yz";
}

type SketchSegment = { id: string; from: [number, number]; to: [number, number] };

/**
 * Project triangles of the selected face onto the given plane and return
 * a bounding rectangle as 4 closed sketch segments (CCW).
 *
 * @param positions Flat vertex positions (3 floats per vertex: x,y,z)
 * @param indices Triangle index buffer (3 indices per triangle)
 * @param faceIds Per-triangle face id array
 * @param faceId The selected face id to filter
 * @param plane The sketch plane for (u,v) projection
 * @returns 4 closed CCW segments or null on degenerate/empty input
 */
export function footprintProfile(
  positions: ArrayLike<number>,
  indices: ArrayLike<number>,
  faceIds: string[],
  faceId: string,
  plane: SketchPlane,
): SketchSegment[] | null {
  // Collect unique vertex indices from matching triangles
  const vertSet = new Set<number>();
  for (let t = 0; t < faceIds.length; t++) {
    if (faceIds[t] === faceId) {
      vertSet.add(indices[t * 3]);
      vertSet.add(indices[t * 3 + 1]);
      vertSet.add(indices[t * 3 + 2]);
    }
  }
  if (vertSet.size === 0) return null;

  // Project to (u,v) on the given plane
  const projections: [number, number][] = [];
  for (const vi of vertSet) {
    const offset = vi * 3;
    const x = positions[offset];
    const y = positions[offset + 1];
    const z = positions[offset + 2];
    switch (plane) {
      case "xy":
        projections.push([x, y]);
        break;
      case "xz":
        projections.push([x, z]);
        break;
      case "yz":
        projections.push([y, z]);
        break;
    }
  }

  let minU = projections[0][0];
  let maxU = projections[0][0];
  let minV = projections[0][1];
  let maxV = projections[0][1];
  for (let i = 1; i < projections.length; i++) {
    const [u, v] = projections[i];
    if (u < minU) minU = u;
    if (u > maxU) maxU = u;
    if (v < minV) minV = v;
    if (v > maxV) maxV = v;
  }

  if (maxU - minU <= EPSILON_GUARD || maxV - minV <= EPSILON_GUARD) return null;

  // CCW bounding rectangle: BL → BR → TR → TL → (back to BL)
  return [
    { id: "seg_0", from: [minU, minV], to: [maxU, minV] },
    { id: "seg_1", from: [maxU, minV], to: [maxU, maxV] },
    { id: "seg_2", from: [maxU, maxV], to: [minU, maxV] },
    { id: "seg_3", from: [minU, maxV], to: [minU, minV] },
  ];
}

/**
 * Compute the signed offset of a face from the canonical plane origin along the plane's normal axis.
 * Uses the first triangle vertex matching `faceId` for deterministic calculation.
 * Returns 0.0 if no matching face triangle is found.
 */
export function faceOffsetFromPlane(
  positions: ArrayLike<number>,
  indices: ArrayLike<number>,
  faceIds: string[],
  faceId: string,
  plane: SketchPlane,
): number {
  for (let t = 0; t < faceIds.length; t++) {
    if (faceIds[t] === faceId) {
      const i0 = indices[t * 3];
      const x = positions[i0 * 3];
      const y = positions[i0 * 3 + 1];
      const z = positions[i0 * 3 + 2];
      switch (plane) {
        case "yz":
          return x;
        case "xz":
          return y;
        case "xy":
          return z;
      }
    }
  }
  return 0.0;
}

/**
 * Build create_sketch + extrude Feature pair for the given selection.
 * IDs are deterministic: smallest non-colliding `sketch_<n>` / `extrude_<n>`.
 * Returns null if the face cannot be resolved to a plane or the profile is degenerate.
 */
export function buildExtrudeFeatures(
  faceId: string,
  positions: ArrayLike<number>,
  indices: ArrayLike<number>,
  faceIds: string[],
  depth: number,
  existingFeatureIds: Set<string>,
  targetBody?: string,
): { sketch: Feature; extrude: Feature } | null {
  const plane = planeForFaceId(faceId);
  if (!plane) return null;

  const profile = footprintProfile(positions, indices, faceIds, faceId, plane);
  if (!profile) return null;

  if (!Number.isFinite(depth) || depth <= 0) return null;

  const offset = faceOffsetFromPlane(positions, indices, faceIds, faceId, plane);

  const sketchId = nextId("sketch_", existingFeatureIds);
  const extrudeId = nextId("extrude_", existingFeatureIds);

  const sketch: Feature = {
    type: "create_sketch",
    id: sketchId,
    plane,
    offset,
    profile,
  };
  const extrude: Feature = {
    type: "extrude",
    id: extrudeId,
    sketch: sketchId,
    depth,
    ...(targetBody ? { fuse_target: targetBody } : {}),
  };
  return { sketch, extrude };
}

function nextId(prefix: string, existing: Set<string>): string {
  let n = 0;
  while (existing.has(`${prefix}${n}`)) n++;
  return `${prefix}${n}`;
}

/**
 * Inset a bounding rectangle toward its centroid by the given ratio (0 < ratio < 0.5).
 * Returns null if the resulting extent is <= EPSILON_GUARD in either axis.
 */
export function insetRect(
  rect: SketchSegment[],
  ratio: number,
): SketchSegment[] | null {
  // Extract corners from segments (CCW: BL→BR→TR→TL)
  const corners: [number, number][] = rect.map((s) => s.from);
  let minU = corners[0][0];
  let maxU = corners[0][0];
  let minV = corners[0][1];
  let maxV = corners[0][1];
  for (const [u, v] of corners) {
    if (u < minU) minU = u;
    if (u > maxU) maxU = u;
    if (v < minV) minV = v;
    if (v > maxV) maxV = v;
  }
  const extentU = maxU - minU;
  const extentV = maxV - minV;
  const shrinkU = extentU * ratio;
  const shrinkV = extentV * ratio;
  const newMinU = minU + shrinkU;
  const newMaxU = maxU - shrinkU;
  const newMinV = minV + shrinkV;
  const newMaxV = maxV - shrinkV;
  if (newMaxU - newMinU <= EPSILON_GUARD || newMaxV - newMinV <= EPSILON_GUARD) {
    return null;
  }
  return [
    { id: "seg_0", from: [newMinU, newMinV], to: [newMaxU, newMinV] },
    { id: "seg_1", from: [newMaxU, newMinV], to: [newMaxU, newMaxV] },
    { id: "seg_2", from: [newMaxU, newMaxV], to: [newMinU, newMaxV] },
    { id: "seg_3", from: [newMinU, newMaxV], to: [newMinU, newMinV] },
  ];
}

export const CUT_INSET_RATIO = 0.25;

/**
 * Returns the distance from the selected face to the canonical plane origin along the
 * plane's normal axis. Used to clamp ExtrudeCut depth to avoid coplanar face errors.
 * Returns Infinity if no matching face triangle is found.
 */
export function faceToCanonicalPlaneDistance(
  positions: ArrayLike<number>,
  indices: ArrayLike<number>,
  faceIds: string[],
  faceId: string,
  plane: SketchPlane,
): number {
  for (let t = 0; t < faceIds.length; t++) {
    if (faceIds[t] === faceId) {
      const i0 = indices[t * 3];
      const x = positions[i0 * 3];
      const y = positions[i0 * 3 + 1];
      const z = positions[i0 * 3 + 2];
      switch (plane) {
        case "yz":
          return Math.abs(x);
        case "xz":
          return Math.abs(y);
        case "xy":
          return Math.abs(z);
      }
    }
  }
  return Infinity;
}

/**
 * Build create_sketch + extrude_cut Feature pair for the given selection.
 * The tool profile is inset inward from the face footprint to avoid coplanar faces.
 * depth is clamped to (face-to-canonical-plane distance - ε) to prevent manifold errors.
 * Returns null if the face cannot be resolved, profile is degenerate, or inset collapses.
 */
export function buildExtrudeCutFeatures(
  faceId: string,
  positions: ArrayLike<number>,
  indices: ArrayLike<number>,
  faceIds: string[],
  depth: number,
  target: string,
  existingFeatureIds: Set<string>,
): { sketch: Feature; extrudeCut: Feature } | null {
  const plane = planeForFaceId(faceId);
  if (!plane) return null;
  if (!target) return null;
  if (!Number.isFinite(depth) || depth <= 0) return null;

  const maxSafeDepth =
    faceToCanonicalPlaneDistance(positions, indices, faceIds, faceId, plane) - EPSILON_GUARD;
  const effectiveDepth = Math.min(depth, maxSafeDepth);
  if (effectiveDepth <= 0) return null;

  const rect = footprintProfile(positions, indices, faceIds, faceId, plane);
  if (!rect) return null;

  const inset = insetRect(rect, CUT_INSET_RATIO);
  if (!inset) return null;

  const sketchId = nextId("sketch_", existingFeatureIds);
  const cutId = nextId("extrude_cut_", existingFeatureIds);

  const sketch: Feature = {
    type: "create_sketch",
    id: sketchId,
    plane,
    profile: inset,
  };
  const extrudeCut: Feature = {
    type: "extrude_cut",
    id: cutId,
    sketch: sketchId,
    depth: effectiveDepth,
    target,
  };
  return { sketch, extrudeCut };
}
