#!/usr/bin/env bun
// cutover.ts — old → new runtime の one-shot cutover (idempotent / fail-safe)
//
// contract §"Compatibility and cutover" #2:
//   - まず old watchers / locks / registries / queues / worktrees を検出して stop/reconcile し、
//     その後 new runtime を有効化する。
//   - idempotent or fail-safe。絶対に両 orchestrator を同時に active にしない。
//   contract §"Compatibility and cutover" #3:
//   - 自律 loop の起動や live cutover を automated test / delivery verification で行わない。
//   (本 CLI 自体は dry 検出 + reconcile まで。live 起動は Codex/ユーザー明示時のみ)
//
// 使い方:
//   bun cutover.ts --check                 # 状態を報告のみ (idempotent, 変更しない)
//   bun cutover.ts --reconcile             # old を停止/正規化し new を有効化 (idempotent)
//   bun cutover.ts --reconcile --force     # 判定不能 stale old を強制停止
//
// 出力 (stdout JSON): { old_active, reconciled, new_enabled, actions[] }
// exit: 0 / 1=両方が active で安全に解決できなかった (--force が必要) / 2=usage

import { existsSync, readFileSync, readdirSync, rmSync, writeFileSync, mkdirSync } from "fs";
import { resolve } from "path";
import { arg, emitJson, emitStatus, runCmd, REPO_ROOT } from "./cli/helpers.ts";
import { planReconcile, oldProcessActive, type OldRuntimeProbe } from "./lib/cutover-reconcile.ts";

const LOOP_DIR = resolve(REPO_ROOT, "features/.loop");
const OLD_LOCK_DIR = process.env.LOOP_LOCK_DIR
  ? resolve(process.env.LOOP_LOCK_DIR)
  : resolve(REPO_ROOT, "features/.batch/lock");
const OLD_WATCHER_PIDFILE = resolve(LOOP_DIR, "tmux", "watcher.pid");
const NEW_FLAG = process.env.LOOP_NEW_FLAG
  ? resolve(process.env.LOOP_NEW_FLAG)
  : resolve(LOOP_DIR, "new-runtime.enabled");
const OLD_WATCHER_HINT = resolve(LOOP_DIR, "tmux");

async function probeOld(): Promise<OldRuntimeProbe> {
  const oldLockHeld = existsSync(OLD_LOCK_DIR);
  const watcherPidFilePresent = existsSync(OLD_WATCHER_PIDFILE);
  let watcherAlive: boolean | null = null;
  if (watcherPidFilePresent) {
    try {
      const pid = parseInt(readFileSync(OLD_WATCHER_PIDFILE, "utf-8").trim(), 10);
      if (Number.isFinite(pid)) {
        try {
          process.kill(pid, 0);
          watcherAlive = true;
        } catch (e) {
          watcherAlive = (e as NodeJS.ErrnoException).code !== "ESRCH" ? true : false;
        }
      }
    } catch {
      watcherAlive = false;
    }
  }
  // tmux session named "3ailoop" (old runtime convention)
  let tmuxSessionPresent = false;
  const tmux = await runCmd(["tmux", "has-session", "-t", "3ailoop"]);
  if (tmux.exitCode === 0) tmuxSessionPresent = true;

  let mergeQueueNonEmpty = false;
  const mq = resolve(LOOP_DIR, "merge-queue.jsonl");
  if (existsSync(mq) && readFileSync(mq, "utf-8").trim().length > 0) {
    mergeQueueNonEmpty = true;
  }

  // --- durable old-runtime state: registries / queues / worktrees (F4 reconciliation) ---
  const oldBusyWorkers = readOldBusyWorkers();
  const oldModelRegistryPresent = readOldModelRegistryPresent();
  const oldAssignedIssues = readOldAssignedIssues();
  const oldQueueDepth = readOldQueueDepth();
  const oldWorktrees = readOldWorktrees();

  return {
    oldLockHeld, watcherPidFilePresent, watcherAlive, tmuxSessionPresent, mergeQueueNonEmpty,
    oldBusyWorkers, oldModelRegistryPresent, oldAssignedIssues, oldQueueDepth, oldWorktrees,
  };
}

/** Old worker-registry: workers that are NOT idle (busy/merging/error) with a current issue. */
function readOldBusyWorkers(): OldRuntimeProbe["oldBusyWorkers"] {
  const p = resolve(LOOP_DIR, "worker-registry.json");
  if (!existsSync(p)) return [];
  try {
    const obj = JSON.parse(readFileSync(p, "utf-8")) as { workers?: Record<string, { state?: string; current_issue?: number | null }> };
    const out: { worker_id: string; state: string; current_issue: number | null }[] = [];
    for (const [id, w] of Object.entries(obj.workers ?? {})) {
      if (w.state && w.state !== "idle") {
        out.push({ worker_id: id, state: w.state, current_issue: w.current_issue ?? null });
      }
    }
    return out;
  } catch {
    return [];
  }
}

/** Old model-registry present + has any non-retired entry. */
function readOldModelRegistryPresent(): boolean {
  const p = resolve(LOOP_DIR, "model-registry.json");
  if (!existsSync(p)) return false;
  try {
    const obj = JSON.parse(readFileSync(p, "utf-8")) as { models?: Record<string, { status?: string }> };
    return Object.values(obj.models ?? {}).some(m => m.status && m.status !== "retired");
  } catch {
    return false;
  }
}

/** Old assign-state: issues that are assigned/verifying/landing and not closed. */
function readOldAssignedIssues(): number[] {
  const p = resolve(LOOP_DIR, "assign-state.json");
  if (!existsSync(p)) return [];
  try {
    const obj = JSON.parse(readFileSync(p, "utf-8")) as { issues?: Record<string, { status?: string; closed?: boolean }> };
    const out: number[] = [];
    for (const [k, s] of Object.entries(obj.issues ?? {})) {
      const n = Number(k);
      if (!Number.isFinite(n)) continue;
      if (s.closed) continue;
      if (s.status === "assigned" || s.status === "verifying" || s.status === "landing") out.push(n);
    }
    return out.sort((a, b) => a - b);
  } catch {
    return [];
  }
}

/** Old worker-queue depth (pending job descriptors). */
function readOldQueueDepth(): number {
  const q = resolve(LOOP_DIR, "worker-queue");
  if (!existsSync(q)) return 0;
  try {
    return readdirSync(q).filter(f => f.endsWith(".json")).length;
  } catch {
    return 0;
  }
}

/** Old runtime worktrees: directories under the tmux/worktrees hint that look like worktrees. */
function readOldWorktrees(): string[] {
  const hint = resolve(LOOP_DIR, "tmux", "worktrees");
  if (!existsSync(hint)) return [];
  try {
    return readdirSync(hint, { withFileTypes: true })
      .filter(d => d.isDirectory())
      .map(d => resolve(hint, d.name));
  } catch {
    return [];
  }
}

/** Re-probe ONLY the live-process signals after a stop attempt, to confirm death (F4). */
async function reprobeProcesses(): Promise<{ watcherAlive: boolean | null; tmuxSessionPresent: boolean }> {
  let watcherAlive: boolean | null = null;
  if (existsSync(OLD_WATCHER_PIDFILE)) {
    try {
      const pid = parseInt(readFileSync(OLD_WATCHER_PIDFILE, "utf-8").trim(), 10);
      if (Number.isFinite(pid)) {
        try {
          process.kill(pid, 0);
          watcherAlive = true;
        } catch (e) {
          watcherAlive = (e as NodeJS.ErrnoException).code !== "ESRCH" ? true : false;
        }
      } else {
        watcherAlive = false;
      }
    } catch {
      watcherAlive = false;
    }
  } else {
    watcherAlive = false; // pidfile removed → process no longer tracked as alive
  }
  let tmuxSessionPresent = false;
  const tmux = await runCmd(["tmux", "has-session", "-t", "3ailoop"]);
  if (tmux.exitCode === 0) tmuxSessionPresent = true;
  return { watcherAlive, tmuxSessionPresent };
}

function oldActive(probe: OldRuntimeProbe): boolean {
  return oldProcessActive(probe);
}

async function reconcile(probe: OldRuntimeProbe, force: boolean): Promise<string[]> {
  const actions: string[] = [];

  // 1. stop old watcher if alive
  if (probe.watcherAlive === true) {
    if (!force) {
      throw new Error("old watcher is alive; refusing to stop without --force");
    }
    try {
      const pid = parseInt(readFileSync(OLD_WATCHER_PIDFILE, "utf-8").trim(), 10);
      process.kill(pid, "SIGTERM");
      actions.push(`SIGTERM old watcher pid=${pid}`);
    } catch (e) {
      actions.push(`warn: could not stop watcher: ${(e as Error).message}`);
    }
  }

  // 2. stop old tmux session
  if (probe.tmuxSessionPresent) {
    if (!force) {
      throw new Error("old tmux session present; refusing to kill without --force");
    }
    await runCmd(["tmux", "kill-session", "-t", "3ailoop"]);
    actions.push("kill-session old tmux '3ailoop'");
  }

  // 3. RE-PROBE to confirm the processes actually stopped before touching durable state or
  //    enabling the new runtime (F4). Recording an attempt as an action is not enough: a
  //    still-alive watcher/tmux would leave both orchestrators active.
  const reprobe = await reprobeProcesses();
  const plan = planReconcile(probe, reprobe);

  for (const s of plan.stopped) actions.push(s);
  if (plan.stillAlive.length > 0) {
    // fail-safe: never enable the new runtime while an old process survived the stop attempt
    throw new Error(`cutover aborts (never run both): ${plan.stillAlive.join("; ")}`);
  }

  // 4. now that processes are confirmed gone: remove pidfile + release old lock
  if (probe.watcherPidFilePresent) {
    rmSync(OLD_WATCHER_PIDFILE, { force: true });
    actions.push("removed old watcher pidfile");
  }
  if (probe.oldLockHeld) {
    rmSync(OLD_LOCK_DIR, { recursive: true, force: true });
    actions.push("released old features/.batch/lock");
  }

  // 5. record reconciled durable state (registries/queues/worktrees) for operator review.
  //    The new runtime keeps its own CAS/registry; nothing old is auto-landed or auto-carried.
  for (const r of plan.reconciledState) {
    actions.push(`reconciled ${r.kind}: ${r.detail}`);
  }

  // 6. enable new runtime flag — only reached when safeToEnable (no still-alive process).
  mkdirSync(LOOP_DIR, { recursive: true });
  writeFileSync(NEW_FLAG, JSON.stringify({ enabled_at: new Date().toISOString(), by: "cutover.ts" }, null, 2), "utf-8");
  actions.push("enabled new runtime flag (features/.loop/new-runtime.enabled)");

  return actions;
}

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const check = args.includes("--check");
  const reconcileFlag = args.includes("--reconcile");
  const force = args.includes("--force");
  const statePath = arg(args, "--state-path");

  if (!check && !reconcileFlag) {
    emitStatus("Usage: cutover.ts (--check | --reconcile [--force])");
    return 2;
  }
  void statePath;

  const probe = await probeOld();
  const active = oldActive(probe);

  if (check) {
    emitJson({
      probe,
      old_active: active,
      new_enabled: existsSync(NEW_FLAG),
    });
    return 0;
  }

  // reconcile
  if (active && !force) {
    // refuse to run both at once without explicit force
    emitJson({
      old_active: active,
      reconciled: false,
      new_enabled: existsSync(NEW_FLAG),
      actions: [],
      error: "old runtime active; pass --force to stop it (never run both)",
    });
    return 1;
  }

  try {
    const actions = await reconcile(probe, force);
    emitStatus(`cutover: reconciled. actions=${actions.length}`);
    emitJson({
      probe,
      old_active: false,
      reconciled: true,
      new_enabled: true,
      actions,
    });
    return 0;
  } catch (e) {
    emitJson({
      old_active: active,
      reconciled: false,
      new_enabled: existsSync(NEW_FLAG),
      actions: [],
      error: (e as Error).message,
    });
    return 1;
  }
}

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}

void OLD_WATCHER_HINT;
