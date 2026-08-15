#!/usr/bin/env bun
// cutover-reconcile.ts — pure old→new runtime reconciliation logic for cutover.ts
//
// contract §"Compatibility and cutover" #2:
//   - まず old watchers / locks / registries / queues / worktrees を検出して stop/reconcile し、
//     その後 new runtime を有効化する。
//   - idempotent or fail-safe。絶対に両 orchestrator を同時に active にしない。
//
// The cutover CLI probes the OLD runtime's durable state (locks, watcher PID, tmux session,
// worker/model registries, assign-state, worker-queue, worktrees). This module classifies the
// probe into:
//   - processes that must be stopped (watcher, tmux) — and whether a re-probe confirms they died
//   - durable state that must be reconciled (busy/assigned workers, non-empty queue, orphaned
//     worktrees) — surfaced explicitly so the operator knows what was NOT silently carried over
//
// The key safety property: if a process we tried to stop is STILL alive on re-probe, enabling the
// new runtime is forbidden (both orchestrators must never run at once). Pure; the CLI performs IO.
//
// 関連: contract §"Compatibility and cutover", cutover.ts

/** Snapshot of OLD runtime durable state + live processes. */
export interface OldRuntimeProbe {
  oldLockHeld: boolean;
  watcherPidFilePresent: boolean;
  watcherAlive: boolean | null;
  tmuxSessionPresent: boolean;
  mergeQueueNonEmpty: boolean;
  /** old worker registry: any worker not idle (busy/merging/error) with a current issue. */
  oldBusyWorkers: { worker_id: string; state: string; current_issue: number | null }[];
  /** old model registry present + has any non-retired entry. */
  oldModelRegistryPresent: boolean;
  /** old assign-state present + has any non-closed assigned/verifying/landing issue. */
  oldAssignedIssues: number[];
  /** old worker-queue: number of pending job descriptors. */
  oldQueueDepth: number;
  /** orphaned worktrees the old runtime left behind (paths). */
  oldWorktrees: string[];
}

/** Result of classifying a probe + a post-stop re-probe. Pure. */
export interface ReconcilePlan {
  /** true only when it is SAFE to enable the new runtime: no old process remains alive. */
  safeToEnable: boolean;
  /** watcher/tmux processes confirmed dead on re-probe (recorded as stopped). */
  stopped: string[];
  /** processes we attempted to stop but are STILL alive on re-probe (blocker). */
  stillAlive: string[];
  /** durable old state that was reconciled / recorded for operator review. */
  reconciledState: { kind: string; detail: string }[];
  /** true if the probe found no old-runtime footprint at all (idempotent no-op cutover). */
  cleanTree: boolean;
}

/** Decide whether the new runtime may be enabled given a post-stop re-probe.
 *  The watcher/tmux must be confirmed dead. Durable state (registries/queues/worktrees) is
 *  recorded for reconciliation but does NOT block enabling on its own — landing/assignment
 *  state is reconciled by the new runtime's own CAS/registry, and the contract forbids
 *  auto-landing during cutover. What MUST block is a live old orchestrator process. Pure. */
export function planReconcile(
  before: OldRuntimeProbe,
  reprobe: { watcherAlive: boolean | null; tmuxSessionPresent: boolean },
): ReconcilePlan {
  const stopped: string[] = [];
  const stillAlive: string[] = [];
  const reconciledState: { kind: string; detail: string }[] = [];

  // --- processes: re-probe confirms death ---
  if (before.watcherAlive === true) {
    if (reprobe.watcherAlive === true) {
      stillAlive.push("old watcher still alive after SIGTERM");
    } else {
      stopped.push("old watcher confirmed stopped");
    }
  } else if (before.watcherPidFilePresent) {
    stopped.push("old watcher pidfile present but process already gone");
  }
  if (before.tmuxSessionPresent) {
    if (reprobe.tmuxSessionPresent) {
      stillAlive.push("old '3ailoop' tmux session still present after kill-session");
    } else {
      stopped.push("old '3ailoop' tmux session confirmed gone");
    }
  }
  if (before.oldLockHeld) {
    reconciledState.push({ kind: "old-lock", detail: "old features/.batch/lock released" });
  }

  // --- durable state: recorded for operator review, NOT auto-landed (contract) ---
  if (before.mergeQueueNonEmpty) {
    reconciledState.push({ kind: "merge-queue", detail: "old merge-queue.jsonl non-empty; entries preserved for manual review (not auto-landed)" });
  }
  if (before.oldBusyWorkers.length > 0) {
    reconciledState.push({
      kind: "busy-workers",
      detail: `${before.oldBusyWorkers.length} old worker(s) not idle: ${before.oldBusyWorkers.map(w => `${w.worker_id}(${w.state}#${w.current_issue ?? "?"})`).join(", ")} — must be reconciled to a terminal outcome before reassigning`,
    });
  }
  if (before.oldAssignedIssues.length > 0) {
    reconciledState.push({
      kind: "assigned-issues",
      detail: `${before.oldAssignedIssues.length} old assigned/verifying/landing issue(s): #${before.oldAssignedIssues.join(", #")} — new runtime CAS starts fresh; verify none are silently dropped`,
    });
  }
  if (before.oldModelRegistryPresent) {
    reconciledState.push({ kind: "model-registry", detail: "old model registry present; new runtime keeps its own features/.loop/model-registry.json" });
  }
  if (before.oldQueueDepth > 0) {
    reconciledState.push({ kind: "worker-queue", detail: `${before.oldQueueDepth} old worker-queue job(s); not auto-launched by the new runtime` });
  }
  if (before.oldWorktrees.length > 0) {
    reconciledState.push({ kind: "worktrees", detail: `${before.oldWorktrees.length} old worktree(s): ${before.oldWorktrees.join(", ")} — preserved, not deleted` });
  }

  const cleanTree =
    !before.oldLockHeld &&
    before.watcherAlive !== true &&
    !before.tmuxSessionPresent &&
    !before.mergeQueueNonEmpty &&
    before.oldBusyWorkers.length === 0 &&
    before.oldAssignedIssues.length === 0 &&
    before.oldQueueDepth === 0 &&
    before.oldWorktrees.length === 0;

  return {
    safeToEnable: stillAlive.length === 0,
    stopped,
    stillAlive,
    reconciledState,
    cleanTree,
  };
}

/** True when the probe indicates an old orchestrator is actively running (must stop first). */
export function oldProcessActive(probe: OldRuntimeProbe): boolean {
  return Boolean(probe.watcherAlive === true || probe.tmuxSessionPresent);
}
