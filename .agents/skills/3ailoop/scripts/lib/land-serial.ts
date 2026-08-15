#!/usr/bin/env bun
// land-serial.ts — serialized landing with full revalidation (new runtime)
//
// contract §"State, receipts, Git, and landing" #4:
//   - Landing は直列。
//   - default-branch mutation の前に receipt / generation / Issue state / scope /
//     complete diff / tests を再検証。
//   - conflict や CI/test 失敗は bounded な明示的 state に戻す (破壊的 recovery しない)。
//
// 設計:
// - landLock (file lock on a dedicated target) で直列化。同時に 1 件だけ landing。
// - revalidate() が全項目をチェックし、FailureReason を返す。1 つでも NG なら land しない。
// - 実際の git/cargo/gh 呼び出しは DI で注入 (test は fake adapters)。
// - outcome は success | rebase-conflict | ci-red | push-failed | scope-drift |
//   stale-generation | receipt-invalid | closed-issue | completeness-fail | internal-error。
//
// 関連: contract §"State, receipts, Git, and landing" #4

import { withFileLock } from "./file-lock.ts";
import { resolve } from "path";
import { mkdirSync, writeFileSync, existsSync } from "fs";

const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
const DEFAULT_LAND_LOCK_TARGET = resolve(REPO_ROOT, "features/.loop/land-serial.lock-target");

export type LandOutcome =
  | "success"
  | "rebase-conflict"
  | "ci-red"
  | "push-failed"
  | "scope-drift"
  | "stale-generation"
  | "receipt-invalid"
  | "closed-issue"
  | "completeness-fail"
  | "internal-error"
  | "state-unreconciled";

export interface LandRequest {
  issue: number;
  workerId: string;
  worktree: string;
  /** generation the caller read from CAS state before deciding to land. */
  expectedGeneration: number;
  /** the receipt to revalidate. */
  receipt: {
    status: string;
    model: { provider: string; model: string };
    commit_sha?: string | null;
    changed_files?: string[];
  };
}

export interface LandDeps {
  // CAS state revalidation
  /** read current CAS state for the issue: returns {generation, closed, worker_id}. */
  readCas: (issue: number) => Promise<{ generation: number; closed: boolean; worker_id: string | null }>;
  // scope re-check
  /** verify the worktree changes stay within Issue scope. ok=false → scope-drift. */
  checkScope: (issue: number, worktree: string) => Promise<{ ok: boolean; detail: string }>;
  // completeness
  /** compute the complete diff (tracked + staged + committed + untracked). */
  completeDiff: (worktree: string) => Promise<{ all: string[] }>;
  // git/cargo/gh
  rebase: (worktree: string) => Promise<{ ok: boolean; stderr: string }>;
  abortRebase: (worktree: string) => Promise<void>;
  runCi: (worktree: string) => Promise<{ ok: boolean; stderr: string }>;
  push: (worktree: string) => Promise<{ ok: boolean; stderr: string }>;
  /** mark CAS issue closed after a successful land. */
  markClosed: (issue: number) => Promise<void>;
  releaseWorker: (workerId: string) => Promise<void>;
  now: () => number;
}

export interface LandResult {
  issue: number;
  workerId: string;
  outcome: LandOutcome;
  detail: string;
  durationMs: number;
}

/** Revalidate everything that must hold BEFORE any default-branch mutation.
 *  Returns the first failure reason, or null if all checks pass. Pure-ish (calls deps). */
export async function revalidate(req: LandRequest, deps: LandDeps): Promise<LandOutcome | null> {
  // 1. receipt status must be completed
  if (req.receipt.status !== "completed") {
    return "receipt-invalid";
  }
  if (!req.receipt.commit_sha) {
    return "receipt-invalid";
  }

  // 2. CAS generation + closed + worker ownership
  const cas = await deps.readCas(req.issue);
  if (cas.closed) return "closed-issue";
  if (cas.generation !== req.expectedGeneration) return "stale-generation";
  // worker ownership: the issue must currently be owned by THIS worker. A null owner
  // (never assigned / already released) is NOT a license to land — landing a structurally
  // valid receipt for an issue with no prior assignment must be refused (F2).
  if (cas.worker_id !== req.workerId) return "stale-generation";

  // 3. scope
  const scope = await deps.checkScope(req.issue, req.worktree);
  if (!scope.ok) return "scope-drift";

  // 4. completeness: receipt.changed_files must exactly cover the complete diff (#330)
  const diff = await deps.completeDiff(req.worktree);
  const claimed = new Set(req.receipt.changed_files ?? []);
  const actual = new Set(diff.all);
  const missing = Array.from(actual).filter(f => !claimed.has(f));
  const extra = Array.from(claimed).filter(f => !actual.has(f));
  if (missing.length > 0 || extra.length > 0) {
    return "completeness-fail";
  }

  return null;
}

/** Process one landing entry, serialized by the land lock. */
export async function processLand(req: LandRequest, deps: LandDeps): Promise<LandResult> {
  const started = deps.now();

  const failure = await revalidate(req, deps);
  if (failure) {
    // bounded explicit state — no destructive recovery
    if (failure === "rebase-conflict") {
      // not yet attempted, fall through
    } else {
      return { issue: req.issue, workerId: req.workerId, outcome: failure, detail: failure, durationMs: deps.now() - started };
    }
  }

  // rebase
  const rebased = await deps.rebase(req.worktree);
  if (!rebased.ok) {
    await deps.abortRebase(req.worktree);
    return {
      issue: req.issue,
      workerId: req.workerId,
      outcome: "rebase-conflict",
      detail: rebased.stderr.slice(0, 500),
      durationMs: deps.now() - started,
    };
  }

  // CI
  const ci = await deps.runCi(req.worktree);
  if (!ci.ok) {
    return {
      issue: req.issue,
      workerId: req.workerId,
      outcome: "ci-red",
      detail: ci.stderr.slice(0, 500),
      durationMs: deps.now() - started,
    };
  }

  // push
  const pushed = await deps.push(req.worktree);
  if (!pushed.ok) {
    return {
      issue: req.issue,
      workerId: req.workerId,
      outcome: "push-failed",
      detail: pushed.stderr.slice(0, 500),
      durationMs: deps.now() - started,
    };
  }

  // success: close issue in CAS + release worker. These durable-state mutations are part
  // of the landing contract — a failure here means the push succeeded but the lifecycle
  // record was NOT reconciled (issue could be reselected / worker stuck busy). That is a
  // bounded, non-success outcome so the caller re-runs state reconciliation rather than
  // treating already-landed work as fresh (F2). The push itself is not rolled back.
  let stateError: string | null = null;
  try {
    await deps.markClosed(req.issue);
  } catch (e) {
    stateError = `markClosed: ${(e as Error).message}`;
  }
  try {
    await deps.releaseWorker(req.workerId);
  } catch (e) {
    stateError = stateError ? `${stateError}; releaseWorker: ${(e as Error).message}` : `releaseWorker: ${(e as Error).message}`;
  }

  if (stateError) {
    return {
      issue: req.issue,
      workerId: req.workerId,
      outcome: "state-unreconciled",
      detail: `push succeeded but lifecycle state not reconciled: ${stateError}`,
      durationMs: deps.now() - started,
    };
  }

  return {
    issue: req.issue,
    workerId: req.workerId,
    outcome: "success",
    detail: "landed",
    durationMs: deps.now() - started,
  };
}

/** Ensure the lock-target sentinel exists (file-lock needs a path). */
function ensureLockTarget(target: string = DEFAULT_LAND_LOCK_TARGET): void {
  mkdirSync(resolve(REPO_ROOT, "features/.loop"), { recursive: true });
  if (!existsSync(target)) {
    writeFileSync(target, "land-serial lock target\n", "utf-8");
  }
}

/** Land exactly one entry, serialized. Other landers wait on the file lock. */
export async function landOnce(
  req: LandRequest,
  deps: LandDeps,
  lockTarget: string = DEFAULT_LAND_LOCK_TARGET,
): Promise<LandResult> {
  ensureLockTarget(lockTarget);
  return withFileLock(
    lockTarget,
    async () => processLand(req, deps),
    { timeoutMs: 60_000 },
  );
}

export const LAND_PATHS = {
  DEFAULT_LAND_LOCK_TARGET,
};
