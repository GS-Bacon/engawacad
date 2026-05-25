import type { TriangleMesh } from "./generated/TriangleMesh";
import type { ErrorResponse } from "./generated/ErrorResponse";

export async function fetchMesh(file: string): Promise<TriangleMesh> {
  const url = `/api/v0/mesh?file=${encodeURIComponent(file)}`;
  const res = await fetch(url);
  if (!res.ok) {
    let message = `HTTP ${res.status}`;
    try {
      const body = (await res.json()) as ErrorResponse;
      if (body.error) message = body.error;
    } catch {
      // keep default message
    }
    throw new Error(message);
  }
  return (await res.json()) as TriangleMesh;
}
