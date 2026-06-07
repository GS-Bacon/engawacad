import { describe, it, expect } from "vitest";
import {
  validateMesh,
  meshToGeometry,
  MeshValidationError,
} from "./mesh";
import type { TriangleMesh } from "./generated/TriangleMesh";

function makeMesh(
  overrides: Partial<TriangleMesh> = {},
): TriangleMesh {
  return {
    positions: [
      [0, 0, 0],
      [1, 0, 0],
      [0, 1, 0],
    ],
    normals: [
      [0, 0, 1],
      [0, 0, 1],
      [0, 0, 1],
    ],
    indices: [0, 1, 2],
    face_ids: [""],
    ...overrides,
  };
}

// --- Core tests (T07, T08) ---

describe("validateMesh", () => {
  it("accepts a valid mesh", () => {
    expect(() => validateMesh(makeMesh())).not.toThrow();
  });

  it("accepts empty indices (degenerate but not error)", () => {
    expect(() => validateMesh(makeMesh({ indices: [] }))).not.toThrow();
  });

  it("rejects normals length mismatch", () => {
    expect(() =>
      validateMesh(
        makeMesh({ normals: [[0, 0, 1]] }),
      ),
    ).toThrow(MeshValidationError);
  });

  it("rejects index out of range", () => {
    expect(() =>
      validateMesh(makeMesh({ indices: [0, 1, 99] })),
    ).toThrow(MeshValidationError);
  });

  it("rejects negative index", () => {
    expect(() =>
      validateMesh(makeMesh({ indices: [0, 1, -1] })),
    ).toThrow(MeshValidationError);
  });

  it("rejects NaN in positions", () => {
    expect(() =>
      validateMesh(
        makeMesh({
          positions: [
            [0, 0, 0],
            [NaN, 0, 0],
            [0, 1, 0],
          ],
        }),
      ),
    ).toThrow(MeshValidationError);
  });

  it("rejects Infinity in normals", () => {
    expect(() =>
      validateMesh(
        makeMesh({
          normals: [
            [0, 0, 1],
            [Infinity, 0, 0],
            [0, 1, 0],
          ],
        }),
      ),
    ).toThrow(MeshValidationError);
  });

  it("rejects -Infinity in positions", () => {
    expect(() =>
      validateMesh(
        makeMesh({
          positions: [
            [0, 0, 0],
            [-Infinity, 0, 0],
            [0, 1, 0],
          ],
        }),
      ),
    ).toThrow(MeshValidationError);
  });
});

describe("meshToGeometry", () => {
  it("produces correct attribute lengths", () => {
    const mesh = makeMesh();
    const geo = meshToGeometry(mesh);

    expect(geo.getAttribute("position").count).toBe(3);
    expect(geo.getAttribute("normal").count).toBe(3);
    expect(geo.getIndex()!.count).toBe(3);
  });

  it("produces correct position data", () => {
    const mesh = makeMesh();
    const geo = meshToGeometry(mesh);
    const pos = geo.getAttribute("position");

    expect(pos.getX(0)).toBeCloseTo(0);
    expect(pos.getY(0)).toBeCloseTo(0);
    expect(pos.getZ(0)).toBeCloseTo(0);
    expect(pos.getX(1)).toBeCloseTo(1);
    expect(pos.getY(1)).toBeCloseTo(0);
    expect(pos.getZ(1)).toBeCloseTo(0);
  });

  it("handles empty indices", () => {
    const mesh = makeMesh({ indices: [] });
    const geo = meshToGeometry(mesh);
    expect(geo.getIndex()).toBeNull();
  });

  it("throws on invalid mesh", () => {
    expect(() =>
      meshToGeometry(makeMesh({ normals: [[0, 0, 1]] })),
    ).toThrow(MeshValidationError);
  });
});

// --- Determinism test (T07) ---

describe("determinism", () => {
  it("produces identical geometry on repeated conversion", () => {
    const mesh = makeMesh();
    const results = Array.from({ length: 10 }, () => meshToGeometry(mesh));

    for (let i = 1; i < results.length; i++) {
      const a = results[0];
      const b = results[i];

      const posA = a.getAttribute("position");
      const posB = b.getAttribute("position");
      expect(posA.count).toBe(posB.count);

      for (let j = 0; j < posA.count * 3; j++) {
        const arrA = posA.array as Float32Array;
        const arrB = posB.array as Float32Array;
        expect(arrA[j]).toBeCloseTo(arrB[j]);
      }

      if (a.getIndex() && b.getIndex()) {
        const idxA = Array.from(a.getIndex()!.array);
        const idxB = Array.from(b.getIndex()!.array);
        expect(idxA).toEqual(idxB);
      }
    }
  });
});

// --- Edge case tests (adversarial persona) ---

describe("edge cases", () => {
  it("rejects degenerate triangle (all vertices at same point)", () => {
    const mesh = makeMesh({
      positions: [
        [1, 1, 1],
        [1, 1, 1],
        [1, 1, 1],
      ],
      normals: [
        [0, 0, 1],
        [0, 0, 1],
        [0, 0, 1],
      ],
      indices: [0, 1, 2],
    });
    expect(() => validateMesh(mesh)).toThrow(MeshValidationError);
  });

  it("rejects degenerate triangle (collinear vertices)", () => {
    expect(() =>
      validateMesh(
        makeMesh({ positions: [[0, 0, 0], [1, 1, 1], [2, 2, 2]] }),
      ),
    ).toThrow(MeshValidationError);
  });

  it("handles large coordinate values", () => {
    const big = 1e6;
    const mesh = makeMesh({
      positions: [
        [0, 0, 0],
        [big, 0, 0],
        [0, big, 0],
      ],
    });
    expect(() => validateMesh(mesh)).not.toThrow();
    const geo = meshToGeometry(mesh);
    expect(geo.getAttribute("position").getX(1)).toBeCloseTo(big);
  });

  it("handles very small (subnormal) coordinate values", () => {
    // 5e-324 is the smallest subnormal double; (5e-324)^2 underflows to 0,
    // so positions [[0,0,0],[tiny,0,0],[0,tiny,0]] would form a zero-area triangle
    // in float64. Use a non-degenerate triangle that still exercises subnormal values.
    const tiny = 5e-324;
    const mesh = makeMesh({
      positions: [
        [0, 0, 0],
        [1, 0, 0],
        [0, 0, tiny],
      ],
    });
    expect(() => validateMesh(mesh)).not.toThrow();
  });

  it("rejects -0.0 in positions? (no: -0 is finite, should pass)", () => {
    const mesh = makeMesh({
      positions: [
        [-0, -0, -0],
        [1, 0, 0],
        [0, 1, 0],
      ],
    });
    expect(() => validateMesh(mesh)).not.toThrow();
  });

  it("rejects NaN in normals", () => {
    expect(() =>
      validateMesh(
        makeMesh({
          normals: [
            [0, 0, 1],
            [NaN, 0, 0],
            [0, 1, 0],
          ],
        }),
      ),
    ).toThrow(MeshValidationError);
  });

  it("handles many vertices", () => {
    const n = 1000;
    const positions: [number, number, number][] = [];
    const normals: [number, number, number][] = [];
    const indices: number[] = [];
    for (let i = 0; i < n; i++) {
      // zig-zag in y so consecutive triples are never collinear
      positions.push([i, i % 2, 0]);
      normals.push([0, 0, 1]);
    }
    for (let i = 0; i < n - 2; i++) {
      indices.push(i, i + 1, i + 2);
    }
    const mesh = makeMesh({ positions, normals, indices });
    expect(() => validateMesh(mesh)).not.toThrow();
    const geo = meshToGeometry(mesh);
    expect(geo.getAttribute("position").count).toBe(n);
  });

  it("handles indices at exact boundary (last valid index)", () => {
    const mesh = makeMesh({
      positions: [
        [0, 0, 0],
        [1, 0, 0],
        [0, 1, 0],
      ],
      normals: [
        [0, 0, 1],
        [0, 0, 1],
        [0, 0, 1],
      ],
      indices: [0, 1, 2],
    });
    expect(() => validateMesh(mesh)).not.toThrow();
  });

  it("rejects index equal to positions.length", () => {
    expect(() =>
      validateMesh(makeMesh({ indices: [0, 1, 3] })),
    ).toThrow(MeshValidationError);
  });

  it("handles zero vertices with empty data", () => {
    const mesh: TriangleMesh = {
      positions: [],
      normals: [],
      indices: [],
      face_ids: [],
    };
    expect(() => validateMesh(mesh)).not.toThrow();
    const geo = meshToGeometry(mesh);
    expect(geo.getAttribute("position").count).toBe(0);
  });

  it("rejects float as index", () => {
    expect(() =>
      validateMesh(makeMesh({ indices: [0, 1.5, 2] })),
    ).toThrow(MeshValidationError);
  });
});
