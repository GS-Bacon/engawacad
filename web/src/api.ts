import type { BodyMesh } from "./generated/BodyMesh";
import type { ErrorResponse } from "./generated/ErrorResponse";
import type { Feature } from "./generated/Feature";

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

export async function fetchAllFeatureIds(): Promise<string[]> {
  const res = await fetch("/api/v0/features");
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
  return (await res.json()) as string[];
}

export async function postFeature(feature: Feature): Promise<BodyMesh[]> {
  const res = await fetch("/api/v0/features", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(feature),
  });
  if (!res.ok) {
    let message = `HTTP ${res.status}`;
    try {
      const b = (await res.json()) as ErrorResponse;
      if (b.error) message = b.error;
    } catch {
      // keep default message
    }
    throw new Error(message);
  }
  return (await res.json()) as BodyMesh[];
}
