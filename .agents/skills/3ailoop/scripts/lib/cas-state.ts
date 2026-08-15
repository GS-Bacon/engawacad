#!/usr/bin/env bun
// cas-state.ts — Issue 割当の locked compare-and-set state machine
//
// これが new runtime の安全性の核。Open defect #333 (stale plan が closed Issue を再割当)
// を構造的に防ぐ。すべての遷移は generation check つき CAS (compare-and-set) で行う。
//
// 設計:
// - state file = features/.loop/assign-state.json (1 file, 全 Issue の状態)
// - 各 Issue は generation を持つ。assign は「read 時の generation と一致する時のみ」成功。
//   これにより、古い plan が読んだ generation で write しようとしても弾かれる (#333)。
// - Issue が closed の場合、assign は不可 (closed_at / closed フラグで判定)。
// - worker が terminal (completed/failed) に達したら即座に idle に戻し、
//   busy のまま滞留しない (#333 の「stuck busy after terminal outcome」)。
// - 全 write は withFileLockSync + tmpfile+rename で atomic。
//
// storage layout (features/.loop/assign-state.json):
//   {
//     "issues": {
//       "295": {
//         "generation": 3,
//         "status": "assigned",          // unassigned | assigned | verifying | landing | closed
//         "worker_id": "worker-1",
//         "assigned_at": "...",
//         "closed": false,
//         "closed_at": null,
//         "last_outcome": "completed" | "needs_supervisor" | "failed" | "rate_limited" | null
//       },
//       ...
//     }
//   }
//
// 関連: contract §"State, receipts, Git, and landing" #1, #333

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { withFileLockSync } from "./file-lock.ts";

const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
const DEFAULT_STATE_PATH = resolve(REPO_ROOT, "features/.loop/assign-state.json");

// ---------- Types ----------

export type IssueStatus =
  | "unassigned"
  | "assigned"
  | "verifying"
  | "landing"
  | "closed";

export type TerminalOutcome =
  | "completed"
  | "needs_supervisor"
  | "failed"
  | "rate_limited";

export interface IssueState {
  generation: number;
  status: IssueStatus;
  worker_id: string | null;
  assigned_at: string | null;
  closed: boolean;
  closed_at: string | null;
  last_outcome: TerminalOutcome | null;
}

export interface AssignState {
  issues: Record<string, IssueState>;
}

// ---------- Errors ----------

/** Thrown when a CAS write's expected generation does not match the current value.
 *  Caller must re-read and re-decide (#333 stale-writer defense). */
export class StaleGenerationError extends Error {
  constructor(
    public readonly issue: number,
    public readonly expected: number,
    public readonly actual: number,
  ) {
    super(
      `StaleGeneration: issue #${issue} expected gen=${expected} but actual gen=${actual} (re-read and re-decide)`,
    );
    this.name = "StaleGenerationError";
  }
}

/** Thrown when an operation targets an Issue that is closed (#333). */
export class ClosedIssueError extends Error {
  constructor(public readonly issue: number) {
    super(`ClosedIssue: issue #${issue} is closed; assignment forbidden`);
    this.name = "ClosedIssueError";
  }
}

/** Thrown when an Issue is already assigned to a different worker (double-assign defense). */
export class AlreadyAssignedError extends Error {
  constructor(public readonly issue: number, public readonly worker: string) {
    super(`AlreadyAssigned: issue #${issue} already assigned to ${worker}`);
    this.name = "AlreadyAssignedError";
  }
}

export class InvalidTransitionError extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "InvalidTransitionError";
  }
}

// ---------- Defaults / IO ----------

function emptyState(): AssignState {
  return { issues: {} };
}

function defaultIssueState(): IssueState {
  return {
    generation: 0,
    status: "unassigned",
    worker_id: null,
    assigned_at: null,
    closed: false,
    closed_at: null,
    last_outcome: null,
  };
}

/** 値域検証つき readState。ファイル不在 or 破損なら empty state を返す (fail-safe)。 */
export function readState(path: string = DEFAULT_STATE_PATH): AssignState {
  if (!existsSync(path)) return emptyState();
  try {
    const text = readFileSync(path, "utf-8");
    const obj = JSON.parse(text) as Partial<AssignState>;
    const issues: Record<string, IssueState> = {};
    if (obj.issues && typeof obj.issues === "object") {
      for (const [key, raw] of Object.entries(obj.issues)) {
        const n = Number(key);
        if (!Number.isFinite(n) || n <= 0) continue;
        const r = raw as Partial<IssueState>;
        const status = validStatus(r.status);
        const lastOutcome = r.last_outcome === null || r.last_outcome === undefined
          ? null
          : validOutcome(r.last_outcome);
        issues[String(n)] = {
          generation: typeof r.generation === "number" && Number.isFinite(r.generation) && r.generation >= 0
            ? Math.floor(r.generation)
            : 0,
          status,
          worker_id: typeof r.worker_id === "string" ? r.worker_id : null,
          assigned_at: typeof r.assigned_at === "string" ? r.assigned_at : null,
          closed: typeof r.closed === "boolean" ? r.closed : status === "closed",
          closed_at: typeof r.closed_at === "string" ? r.closed_at : null,
          last_outcome: lastOutcome,
        };
      }
    }
    return { issues };
  } catch {
    return emptyState();
  }
}

function validStatus(s: unknown): IssueStatus {
  switch (s) {
    case "unassigned":
    case "assigned":
    case "verifying":
    case "landing":
    case "closed":
      return s;
    default:
      return "unassigned";
  }
}

function validOutcome(s: unknown): TerminalOutcome | null {
  switch (s) {
    case "completed":
    case "needs_supervisor":
    case "failed":
    case "rate_limited":
      return s;
    default:
      return null;
  }
}

/** atomic write: tmpfile → rename。ディレクトリは自動作成。 */
export function writeState(state: AssignState, path: string = DEFAULT_STATE_PATH): void {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, JSON.stringify(state, null, 2), "utf-8");
  renameSync(tmp, path);
}

export function getIssue(state: AssignState, issue: number): IssueState {
  return state.issues[String(issue)] ?? defaultIssueState();
}

// ---------- Pure transition logic (test target) ----------

export interface AssignRequest {
  issue: number;
  workerId: string;
  /** generation the caller read before deciding. Must match current or StaleGenerationError. */
  expectedGeneration: number;
  /** If true, refuse when the Issue is closed (#333). Default true. */
  refuseIfClosed?: boolean;
  /** If the issue is already assigned to THIS worker, treat as idempotent success. */
  now: () => Date;
}

export interface AssignOk {
  ok: true;
  state: AssignState;
  newGeneration: number;
}
export interface AssignNo {
  ok: false;
  error: "stale-generation" | "closed" | "already-assigned";
  message: string;
}
export type AssignResult = AssignOk | AssignNo;

/**
 * Pure assign transition. Caller supplies the full current state and the generation
 * it observed when deciding. Returns new state (immutable copy) or a structured failure.
 *
 * #333 defense:
 *   - closed Issue → ClosedIssueError-shaped failure (refuseIfClosed)
 *   - expectedGeneration mismatch → stale-generation failure (caller re-reads)
 *   - already assigned to another worker → already-assigned failure
 *   - generation always increments on successful (re)assignment
 */
export function tryAssign(state: AssignState, req: AssignRequest): AssignResult {
  const key = String(req.issue);
  const cur = getIssue(state, req.issue);
  const refuseIfClosed = req.refuseIfClosed ?? true;

  if (refuseIfClosed && cur.closed) {
    return {
      ok: false,
      error: "closed",
      message: `issue #${req.issue} is closed (closed_at=${cur.closed_at ?? "unknown"})`,
    };
  }

  if (cur.generation !== req.expectedGeneration) {
    return {
      ok: false,
      error: "stale-generation",
      message: `expected gen=${req.expectedGeneration} actual=${cur.generation}`,
    };
  }

  // Idempotent re-assign to the SAME worker at the same generation is allowed
  // (e.g. watcher retry). It still bumps generation.
  if (cur.worker_id !== null && cur.worker_id !== req.workerId && cur.status !== "unassigned") {
    return {
      ok: false,
      error: "already-assigned",
      message: `issue #${req.issue} assigned to ${cur.worker_id} (status=${cur.status})`,
    };
  }

  const nextIssue: IssueState = {
    ...cur,
    generation: cur.generation + 1,
    status: "assigned",
    worker_id: req.workerId,
    assigned_at: req.now().toISOString(),
    last_outcome: null,
  };

  const next: AssignState = {
    issues: { ...state.issues, [key]: nextIssue },
  };
  return { ok: true, state: next, newGeneration: nextIssue.generation };
}

/** Mark an Issue closed. Future assigns are refused (#333). Bumps generation. */
export function markClosed(state: AssignState, issue: number, now: () => Date): AssignState {
  const key = String(issue);
  const cur = getIssue(state, issue);
  const nextIssue: IssueState = {
    ...cur,
    closed: true,
    closed_at: now().toISOString(),
    status: "closed",
    generation: cur.generation + 1,
    worker_id: null,
  };
  return { issues: { ...state.issues, [key]: nextIssue } };
}

/** Transition an assigned issue to verifying (receipt arrived, before landing). */
export function moveToVerifying(
  state: AssignState,
  issue: number,
  workerId: string,
  expectedGeneration: number,
): AssignResult {
  const key = String(issue);
  const cur = getIssue(state, issue);
  if (cur.generation !== expectedGeneration) {
    return {
      ok: false,
      error: "stale-generation",
      message: `expected gen=${expectedGeneration} actual=${cur.generation}`,
    };
  }
  if (cur.worker_id !== workerId) {
    return {
      ok: false,
      error: "already-assigned",
      message: `issue #${issue} owned by ${cur.worker_id ?? "null"}, not ${workerId}`,
    };
  }
  const nextIssue: IssueState = { ...cur, status: "verifying", generation: cur.generation + 1 };
  return { ok: true, state: { issues: { ...state.issues, [key]: nextIssue } }, newGeneration: nextIssue.generation };
}

/**
 * Record a terminal outcome for a worker's issue and return the worker to idle.
 * This is the "stuck busy after terminal outcome" defense (#333): a terminal
 * receipt MUST free the slot. last_outcome is persisted on the issue.
 *
 * The worker-side registry uses its own releaseWorker; here we only transition
 * the issue. If outcome is completed/needs_supervisor we keep status; on
 * failed/rate_limited we return to unassigned so a future cycle can re-pick.
 */
export function recordTerminal(
  state: AssignState,
  issue: number,
  workerId: string,
  outcome: TerminalOutcome,
  expectedGeneration: number,
  now: () => Date,
): AssignResult {
  const key = String(issue);
  const cur = getIssue(state, issue);
  if (cur.generation !== expectedGeneration) {
    return {
      ok: false,
      error: "stale-generation",
      message: `expected gen=${expectedGeneration} actual=${cur.generation}`,
    };
  }
  if (cur.worker_id !== workerId) {
    return {
      ok: false,
      error: "already-assigned",
      message: `issue #${issue} owned by ${cur.worker_id ?? "null"}, not ${workerId}`,
    };
  }
  // completed / needs_supervisor → status stays (caller drives landing/consultation).
  // failed / rate_limited → unassign so the next cycle can re-pick (no silent double-assign).
  const toUnassign = outcome === "failed" || outcome === "rate_limited";
  const nextIssue: IssueState = {
    ...cur,
    last_outcome: outcome,
    generation: cur.generation + 1,
    status: toUnassign ? "unassigned" : cur.status,
    worker_id: toUnassign ? null : cur.worker_id,
    assigned_at: toUnassign ? null : cur.assigned_at,
  };
  void now; // reserved for future heartbeat bookkeeping
  return { ok: true, state: { issues: { ...state.issues, [key]: nextIssue } }, newGeneration: nextIssue.generation };
}

// ---------- Side-effectful RMW under file lock ----------

/** Read-modify-write the state file under a file lock. `fn` returns either the new
 *  state (accepted) or throws to abort. Returns the accepted state. */
export function mutateState<T extends AssignState>(
  path: string,
  fn: (cur: AssignState) => T,
): T {
  return withFileLockSync(path, () => {
    const cur = readState(path);
    const next = fn(cur);
    writeState(next, path);
    return next;
  });
}

export const STATE_PATHS = {
  DEFAULT_STATE_PATH,
};
