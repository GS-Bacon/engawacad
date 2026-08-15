#!/usr/bin/env bun
// worker-registry.ts — isolated worker 台帳 (new runtime)
//
// 旧 .claude/.../loop-worker-registry.ts と同等の lifecycle を持つが、ここでは
// 「terminal outcome 後の worker cleanup」を明示的なメソッドで提供する (#333 の
// "stuck busy after terminal outcome" 防御)。worker は completed/failed に達したら
// 必ず idle に戻り、次の Issue を受けられる。busy/merging のまま永久滞留しない。
//
// 設計:
// - features/.loop/worker-registry.json を単一 source of truth
// - state 遷移: idle → busy (assign) → merging (mark-merging) → idle (release)
//   busy/merging → error (mark-error)
//   busy/merging → idle (releaseTerminal, outcome に関わらず強制 idle)
// - 全 write は withFileLockSync + tmpfile+rename で atomic
// - register は max_workers を超える追加を拒否
// - 冪等: idle worker への release は no-op
//
// storage layout:
//   features/.loop/worker-registry.json
//   {
//     "workers": {
//       "worker-1": { "pane_id", "worktree", "current_issue", "state", "assigned_at", "model_id" }
//     },
//     "max_workers": 3
//   }

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { withFileLockSync } from "./file-lock.ts";

const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
const DEFAULT_REGISTRY_PATH = resolve(REPO_ROOT, "features/.loop/worker-registry.json");
const DEFAULT_MAX_WORKERS = parseInt(process.env.LOOP_MAX_WORKERS ?? "3", 10);

export type WorkerState = "idle" | "busy" | "merging" | "error";

export interface WorkerEntry {
  pane_id: string;
  worktree: string;
  current_issue: number | null;
  state: WorkerState;
  assigned_at: string | null;
  error_reason?: string;
  /** Free-model identity pinned to this worker (provider/model). Required so a
   *  worker never silently runs an unqualified/paid model. */
  model_id?: string | null;
}

export interface Registry {
  workers: Record<string, WorkerEntry>;
  max_workers: number;
}

function emptyRegistry(maxWorkers: number = DEFAULT_MAX_WORKERS): Registry {
  return { workers: {}, max_workers: maxWorkers };
}

export function readRegistry(path: string = DEFAULT_REGISTRY_PATH): Registry {
  if (!existsSync(path)) return emptyRegistry();
  try {
    const text = readFileSync(path, "utf-8");
    const obj = JSON.parse(text) as Partial<Registry>;
    const maxWorkers = typeof obj.max_workers === "number" && obj.max_workers > 0
      ? obj.max_workers
      : DEFAULT_MAX_WORKERS;
    const workers: Record<string, WorkerEntry> = {};
    if (obj.workers && typeof obj.workers === "object") {
      for (const [id, raw] of Object.entries(obj.workers)) {
        const w = raw as Partial<WorkerEntry>;
        if (typeof w?.pane_id !== "string") continue;
        if (typeof w?.worktree !== "string") continue;
        const state: WorkerState = ((): WorkerState => {
          switch (w.state) {
            case "idle":
            case "busy":
            case "merging":
            case "error":
              return w.state;
            default:
              return "idle";
          }
        })();
        workers[id] = {
          pane_id: w.pane_id,
          worktree: w.worktree,
          current_issue: typeof w.current_issue === "number" ? w.current_issue : null,
          state,
          assigned_at: typeof w.assigned_at === "string" ? w.assigned_at : null,
          error_reason: typeof w.error_reason === "string" ? w.error_reason : undefined,
          model_id: typeof w.model_id === "string" ? w.model_id : null,
        };
      }
    }
    return { workers, max_workers: maxWorkers };
  } catch {
    return emptyRegistry();
  }
}

export function writeRegistry(reg: Registry, path: string = DEFAULT_REGISTRY_PATH): void {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, JSON.stringify(reg, null, 2), "utf-8");
  renameSync(tmp, path);
}

// --- pure logic ---

export function registerWorker(
  reg: Registry,
  workerId: string,
  paneId: string,
  worktree: string,
  modelId?: string,
): Registry {
  const next: Registry = { workers: { ...reg.workers }, max_workers: reg.max_workers };
  if (!next.workers[workerId] && Object.keys(next.workers).length >= next.max_workers) {
    throw new Error(
      `registerWorker: max_workers=${next.max_workers} 到達 (現 ${Object.keys(next.workers).length} 人)`,
    );
  }
  next.workers[workerId] = {
    pane_id: paneId,
    worktree,
    current_issue: null,
    state: "idle",
    assigned_at: null,
    model_id: modelId ?? null,
  };
  return next;
}

export function assignIssue(reg: Registry, workerId: string, issue: number): Registry {
  const w = reg.workers[workerId];
  if (!w) throw new Error(`assignIssue: worker ${workerId} not registered`);
  if (w.state !== "idle") {
    throw new Error(
      `assignIssue: worker ${workerId} not idle (state=${w.state}, current_issue=${w.current_issue ?? "null"})`,
    );
  }
  const next: Registry = { workers: { ...reg.workers }, max_workers: reg.max_workers };
  next.workers[workerId] = {
    ...w,
    current_issue: issue,
    state: "busy",
    assigned_at: new Date().toISOString(),
    error_reason: undefined,
  };
  return next;
}

export function markMerging(reg: Registry, workerId: string): Registry {
  const w = reg.workers[workerId];
  if (!w) throw new Error(`markMerging: worker ${workerId} not registered`);
  if (w.state !== "busy") {
    throw new Error(`markMerging: worker ${workerId} not busy (state=${w.state})`);
  }
  const next: Registry = { workers: { ...reg.workers }, max_workers: reg.max_workers };
  next.workers[workerId] = { ...w, state: "merging" };
  return next;
}

/** Idempotent: release of an idle worker is a no-op. error workers can also be
 *  released back to idle (manual recovery path). */
export function releaseWorker(reg: Registry, workerId: string): Registry {
  const w = reg.workers[workerId];
  if (!w) throw new Error(`releaseWorker: worker ${workerId} not registered`);
  if (w.state === "idle" && w.current_issue === null) return reg;
  const next: Registry = { workers: { ...reg.workers }, max_workers: reg.max_workers };
  next.workers[workerId] = {
    ...w,
    current_issue: null,
    state: "idle",
    assigned_at: null,
    error_reason: undefined,
  };
  return next;
}

/**
 * #333 "stuck busy after terminal outcome": force a worker back to idle regardless
 * of outcome. Called whenever a worker emits a terminal receipt (completed /
 * needs_supervisor / failed / rate_limited). This guarantees a terminal worker
 * never stays busy/merging forever even if the release path errored.
 *
 * Unlike releaseWorker (no-op on idle), this always normalizes — useful for the
 * safety sweep that reconciles registry against CAS state.
 */
export function releaseTerminal(reg: Registry, workerId: string): Registry {
  const w = reg.workers[workerId];
  if (!w) throw new Error(`releaseTerminal: worker ${workerId} not registered`);
  if (w.state === "idle" && w.current_issue === null) return reg;
  const next: Registry = { workers: { ...reg.workers }, max_workers: reg.max_workers };
  next.workers[workerId] = {
    ...w,
    current_issue: null,
    state: "idle",
    assigned_at: null,
    error_reason: undefined,
  };
  return next;
}

export function markError(reg: Registry, workerId: string, reason: string): Registry {
  const w = reg.workers[workerId];
  if (!w) throw new Error(`markError: worker ${workerId} not registered`);
  const next: Registry = { workers: { ...reg.workers }, max_workers: reg.max_workers };
  next.workers[workerId] = { ...w, state: "error", error_reason: reason };
  return next;
}

export function setWorkerModel(reg: Registry, workerId: string, modelId: string): Registry {
  const w = reg.workers[workerId];
  if (!w) throw new Error(`setWorkerModel: worker ${workerId} not registered`);
  const next: Registry = { workers: { ...reg.workers }, max_workers: reg.max_workers };
  next.workers[workerId] = { ...w, model_id: modelId };
  return next;
}

export function unregisterWorker(reg: Registry, workerId: string): Registry {
  if (!reg.workers[workerId]) return reg;
  const next: Registry = { workers: { ...reg.workers }, max_workers: reg.max_workers };
  delete next.workers[workerId];
  return next;
}

/** First idle worker name (deterministic ascending order), or null if all busy. */
export function firstIdle(reg: Registry): string | null {
  const ids = Object.keys(reg.workers).sort();
  for (const id of ids) {
    if (reg.workers[id].state === "idle") return id;
  }
  return null;
}

/**
 * Reconcile the registry against CAS issue state: any worker whose current_issue
 * has reached a terminal status (closed / unassigned-after-failure) is force-released.
 * This is the periodic safety net that mops up workers the terminal-release path
 * missed (process crash mid-receipt, etc.). Pure function; caller persists.
 */
export function reconcileBusy(reg: Registry, terminalIssues: Set<number>): Registry {
  let changed = false;
  const workers: Record<string, WorkerEntry> = {};
  for (const [id, w] of Object.entries(reg.workers)) {
    if (
      (w.state === "busy" || w.state === "merging") &&
      w.current_issue !== null &&
      terminalIssues.has(w.current_issue)
    ) {
      changed = true;
      workers[id] = {
        ...w,
        current_issue: null,
        state: "idle",
        assigned_at: null,
        error_reason: undefined,
      };
    } else {
      workers[id] = w;
    }
  }
  return changed ? { workers, max_workers: reg.max_workers } : reg;
}

// --- side-effectful RMW ---

function mutate(path: string, fn: (reg: Registry) => Registry): Registry {
  return withFileLockSync(path, () => {
    const current = readRegistry(path);
    const next = fn(current);
    writeRegistry(next, path);
    return next;
  });
}

export function mutateRegistry(path: string, fn: (reg: Registry) => Registry): Registry {
  return mutate(path, fn);
}

export const REGISTRY_DEFAULTS = {
  DEFAULT_REGISTRY_PATH,
  DEFAULT_MAX_WORKERS,
};
