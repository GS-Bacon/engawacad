import type { BodyMesh } from "./generated/BodyMesh";
import type { ErrorResponse } from "./generated/ErrorResponse";

export async function fetchBodies(): Promise<BodyMesh[]> {
  const res = await fetch("/api/v0/mesh");
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
  return (await res.json()) as BodyMesh[];
}
