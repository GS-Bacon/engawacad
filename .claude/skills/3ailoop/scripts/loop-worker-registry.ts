#!/usr/bin/env bun
// loop-worker-registry.ts — /3ailoop tmux N=3 worker pane の状態台帳 (Phase D-1)
//
// 設計:
// - features/.loop/worker-registry.json を単一の source of truth とする
// - state 遷移: idle → busy (assign) → merging (mark-merging) → idle (release)
//   or busy/merging → error (mark-error)
// - 全 write は withFileLockSync + tmpfile+rename で atomic
// - register は max_workers を超える追加を拒否 (=既存が entry 済みの N を超えて増やさない)
// - 冪等系: release-when-idle は no-op (エラーにしない)
//
// storage layout:
//   features/.loop/worker-registry.json
//   {
//     "workers": {
//       "worker-1": {
//         "pane_id": "%12",
//         "worktree": "/home/bacon/worktrees/w1",
//         "current_issue": 295,
//         "state": "busy",
//         "assigned_at": "2026-07-24T11:00:00Z"
//       },
//       ...
//     },
//     "max_workers": 3
//   }
//
// CLI:
//   register     --worker-id N --pane-id X --worktree /path
//   assign       --worker-id N --issue M
//   mark-merging --worker-id N
//   release      --worker-id N
//   mark-error   --worker-id N --reason "..."
//   list         [--json | --pretty]
//   next-free
//   unregister   --worker-id N
//
// 関連: Phase D-1 plan parsed-wiggling-newt.md, ADR-012 (tmux ランタイム)

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { withFileLockSync } from "./loop-file-lock.ts";

// REPO_ROOT は .claude/skills/3ailoop/scripts/ から 4 階層上
const REPO_ROOT = resolve(import.meta.dir, "../../../..");
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
}

export interface Registry {
  workers: Record<string, WorkerEntry>;
  max_workers: number;
}

function emptyRegistry(maxWorkers: number = DEFAULT_MAX_WORKERS): Registry {
  return { workers: {}, max_workers: maxWorkers };
}

/** 値域検証つき readRegistry。ファイル不在 or 破損なら empty registry を返す。 */
export function readRegistry(path: string): Registry {
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
        };
      }
    }
    return { workers, max_workers: maxWorkers };
  } catch {
    return emptyRegistry();
  }
}

/** atomic write: tmpfile → rename。ディレクトリは自動作成。 */
export function writeRegistry(path: string, reg: Registry): void {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, JSON.stringify(reg, null, 2), "utf-8");
  renameSync(tmp, path);
}

// --- pure logic (bun test 対象) ---

export function registerWorker(
  reg: Registry,
  workerId: string,
  paneId: string,
  worktree: string,
): Registry {
  const next: Registry = { workers: { ...reg.workers }, max_workers: reg.max_workers };
  const existed = Boolean(next.workers[workerId]);
  if (!existed && Object.keys(next.workers).length >= next.max_workers) {
    throw new Error(
      `registerWorker: max_workers=${next.max_workers} 到達 (現 ${Object.keys(next.workers).length} 人)`,
    );
  }
  // 既存 entry の update は state=idle にリセット (再登録は「掃除」の意味)
  next.workers[workerId] = {
    pane_id: paneId,
    worktree,
    current_issue: null,
    state: "idle",
    assigned_at: null,
  };
  return next;
}

export function assignIssue(
  reg: Registry,
  workerId: string,
  issue: number,
): Registry {
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

/** 冪等: idle worker への release は no-op。error 状態も release で idle に戻せる (人手回収後の再投入)。 */
export function releaseWorker(reg: Registry, workerId: string): Registry {
  const w = reg.workers[workerId];
  if (!w) throw new Error(`releaseWorker: worker ${workerId} not registered`);
  if (w.state === "idle" && w.current_issue === null) return reg; // no-op
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

export function unregisterWorker(reg: Registry, workerId: string): Registry {
  if (!reg.workers[workerId]) return reg;
  const next: Registry = { workers: { ...reg.workers }, max_workers: reg.max_workers };
  delete next.workers[workerId];
  return next;
}

/** 最初の idle worker 名。全 busy なら null。順序は worker id の昇順で決定的。 */
export function firstIdle(reg: Registry): string | null {
  const ids = Object.keys(reg.workers).sort();
  for (const id of ids) {
    if (reg.workers[id].state === "idle") return id;
  }
  return null;
}

// --- side-effectful helpers (RMW under file lock) ---

function mutate(path: string, fn: (reg: Registry) => Registry): Registry {
  return withFileLockSync(path, () => {
    const current = readRegistry(path);
    const next = fn(current);
    writeRegistry(path, next);
    return next;
  });
}

// --- CLI ---

function arg(args: string[], name: string): string | undefined {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
}
function hasFlag(args: string[], name: string): boolean {
  return args.includes(name);
}

function fmtPretty(reg: Registry): string {
  const rows = Object.entries(reg.workers).sort(([a], [b]) => a.localeCompare(b));
  if (rows.length === 0) return `(no workers; max_workers=${reg.max_workers})`;
  const lines = [
    `max_workers=${reg.max_workers}`,
    ...rows.map(([id, w]) => {
      const issue = w.current_issue == null ? "-" : `#${w.current_issue}`;
      const at = w.assigned_at ?? "-";
      const err = w.error_reason ? ` reason=${JSON.stringify(w.error_reason)}` : "";
      return `  ${id}  pane=${w.pane_id}  state=${w.state}  issue=${issue}  since=${at}${err}`;
    }),
  ];
  return lines.join("\n");
}

async function cli(): Promise<void> {
  const [, , cmd, ...rest] = process.argv;
  const path = arg(rest, "--registry-path") ?? DEFAULT_REGISTRY_PATH;

  const workerId = arg(rest, "--worker-id");
  switch (cmd) {
    case "register": {
      const paneId = arg(rest, "--pane-id");
      const worktree = arg(rest, "--worktree");
      if (!workerId || !paneId || !worktree) {
        console.error("Usage: register --worker-id N --pane-id X --worktree /path");
        process.exit(2);
      }
      try {
        const next = mutate(path, r => registerWorker(r, workerId, paneId, worktree));
        console.log(JSON.stringify(next.workers[workerId], null, 2));
        process.exit(0);
      } catch (e) {
        console.error(`FAIL: ${(e as Error).message}`);
        process.exit(1);
      }
      break;
    }
    case "assign": {
      const issueStr = arg(rest, "--issue");
      const issue = issueStr ? parseInt(issueStr, 10) : NaN;
      if (!workerId || !Number.isFinite(issue)) {
        console.error("Usage: assign --worker-id N --issue M");
        process.exit(2);
      }
      try {
        const next = mutate(path, r => assignIssue(r, workerId, issue));
        console.log(JSON.stringify(next.workers[workerId], null, 2));
        process.exit(0);
      } catch (e) {
        console.error(`FAIL: ${(e as Error).message}`);
        process.exit(1);
      }
      break;
    }
    case "mark-merging": {
      if (!workerId) {
        console.error("Usage: mark-merging --worker-id N");
        process.exit(2);
      }
      try {
        const next = mutate(path, r => markMerging(r, workerId));
        console.log(JSON.stringify(next.workers[workerId], null, 2));
        process.exit(0);
      } catch (e) {
        console.error(`FAIL: ${(e as Error).message}`);
        process.exit(1);
      }
      break;
    }
    case "release": {
      if (!workerId) {
        console.error("Usage: release --worker-id N");
        process.exit(2);
      }
      try {
        const next = mutate(path, r => releaseWorker(r, workerId));
        console.log(JSON.stringify(next.workers[workerId], null, 2));
        process.exit(0);
      } catch (e) {
        console.error(`FAIL: ${(e as Error).message}`);
        process.exit(1);
      }
      break;
    }
    case "mark-error": {
      const reason = arg(rest, "--reason") ?? "unspecified";
      if (!workerId) {
        console.error("Usage: mark-error --worker-id N --reason \"...\"");
        process.exit(2);
      }
      try {
        const next = mutate(path, r => markError(r, workerId, reason));
        console.log(JSON.stringify(next.workers[workerId], null, 2));
        process.exit(0);
      } catch (e) {
        console.error(`FAIL: ${(e as Error).message}`);
        process.exit(1);
      }
      break;
    }
    case "unregister": {
      if (!workerId) {
        console.error("Usage: unregister --worker-id N");
        process.exit(2);
      }
      mutate(path, r => unregisterWorker(r, workerId));
      process.exit(0);
      break;
    }
    case "list": {
      const reg = readRegistry(path);
      if (hasFlag(rest, "--pretty")) {
        console.log(fmtPretty(reg));
      } else {
        console.log(JSON.stringify(reg, null, 2));
      }
      process.exit(0);
      break;
    }
    case "next-free": {
      const reg = readRegistry(path);
      const id = firstIdle(reg);
      if (id) {
        console.log(id);
        process.exit(0);
      } else {
        process.stderr.write("no idle worker\n");
        process.exit(1);
      }
      break;
    }
    default:
      console.error(
        "Usage: loop-worker-registry.ts (register | assign | mark-merging | release | mark-error | unregister | list | next-free) [args]",
      );
      process.exit(2);
  }
}

if (import.meta.main) {
  await cli();
}
