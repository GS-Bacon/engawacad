#!/usr/bin/env bun
// verify-identity.ts — bind a worker receipt to the assigned job's lifecycle
//
// contract §"State, receipts, Git, and landing" #2 + §"Orchestration and authority":
//   A worker receipt must carry provider/model/session identity. Verification must not
//   accept a structurally valid receipt for the WRONG issue/model/session — the receipt
//   must be bound to the assigned worker lifecycle (queued job / qualified-model registry).
//
// This pure module compares a parsed receipt against the expected identity the caller
// derived from the queued job descriptor (run-worker.ts marker) and the qualified-model
// registry. Any supplied expectation that does not match is a mismatch. Missing
// expectations are skipped (caller decides how much to bind), so this composes with tests
// that only check a subset.
//
// 関連: contract §"State, receipts, Git, and landing", lib/receipt.ts, run-worker.ts

import type { Receipt } from "./receipt.ts";

/** The identity a receipt is expected to carry. Any field left `undefined` is not checked. */
export interface IdentityExpectation {
  /** Issue number the job was assigned to. */
  issue?: number;
  /** Worker id the job was enqueued for. */
  workerId?: string;
  /** Canonical "provider/model" identity pinned by model-select / the qualified registry. */
  model?: string;
  /** Opaque session id from the worker harness / delegation marker. */
  sessionId?: string;
}

export interface IdentityCheck {
  /** true when every supplied expectation matched (or none were supplied). */
  ok: boolean;
  /** human-readable mismatch reasons (empty when ok). */
  mismatches: string[];
}

/** Compare a receipt's identity against the expected identity. Pure. */
export function matchIdentity(receipt: Receipt, expected: IdentityExpectation): IdentityCheck {
  const mismatches: string[] = [];

  if (expected.issue !== undefined && receipt.issue !== expected.issue) {
    mismatches.push(`issue: receipt=${receipt.issue} expected=${expected.issue}`);
  }
  if (expected.workerId !== undefined && receipt.worker_id !== expected.workerId) {
    mismatches.push(`worker_id: receipt=${receipt.worker_id} expected=${expected.workerId}`);
  }
  if (expected.model !== undefined) {
    const actual = `${receipt.model.provider}/${receipt.model.model}`;
    if (actual !== expected.model) {
      mismatches.push(`model: receipt=${actual} expected=${expected.model}`);
    }
  }
  if (expected.sessionId !== undefined && receipt.session_id !== expected.sessionId) {
    mismatches.push(`session_id: receipt=${receipt.session_id ?? "(none)"} expected=${expected.sessionId}`);
  }

  return { ok: mismatches.length === 0, mismatches };
}
