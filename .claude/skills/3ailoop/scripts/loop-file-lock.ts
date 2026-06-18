#!/usr/bin/env bun
// loop-file-lock.ts — RMW (read-modify-write) を file-path-scoped で直列化する共通 util (#226)
//
// 設計:
// - state.json / failure-streak/*.json / intent-history.json の atomic write はあるが、
//   read → modify → write の間に race window があり lost update が起きる
// - directory-based lock (mkdir は POSIX atomic) で RMW ブロックを cross-process 排他
// - lock dir は <target>.lock。timeout/poll で待機、finally で必ず rmdir
//
// 使い方:
//   import { withFileLockSync } from "./loop-file-lock";
//   const newCount = withFileLockSync("features/.loop/state.json", () => {
//     const s = readState();
//     s.cumulative.cycles_total += 1;
//     atomicWriteState(s);
//     return s.cumulative.cycles_total;
//   });
//
// 関連: #226, memory project-3ailoop-known-races

import { mkdirSync, rmdirSync } from "fs";
import { resolve } from "path";

export interface LockOptions {
  /** Max wait time in ms before throwing. Default 5000. */
  timeoutMs?: number;
  /** Polling interval in ms while waiting. Default 50. */
  pollMs?: number;
}

const DEFAULT_TIMEOUT_MS = 5000;
const DEFAULT_POLL_MS = 50;

function lockPathFor(target: string): string {
  return resolve(`${target}.lock`);
}

/** Sync sleep using Atomics.wait. Returns immediately if ms <= 0. */
function sleepSyncMs(ms: number): void {
  if (ms <= 0) return;
  const sab = new SharedArrayBuffer(4);
  const view = new Int32Array(sab);
  Atomics.wait(view, 0, 0, ms);
}

function tryAcquireSync(lockPath: string): boolean {
  try {
    mkdirSync(lockPath);
    return true;
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === "EEXIST") return false;
    throw e;
  }
}

function releaseSync(lockPath: string): void {
  try {
    rmdirSync(lockPath);
  } catch {
    // best effort — if removed by another process or by stale cleanup, ignore
  }
}

/** Run `fn` under an exclusive lock for `target`. Sync version for the current
 *  RMW callsites which all use sync I/O.
 *  Throws if the lock cannot be acquired within `opts.timeoutMs`. */
export function withFileLockSync<T>(
  target: string,
  fn: () => T,
  opts: LockOptions = {},
): T {
  const lockPath = lockPathFor(target);
  const timeoutMs = opts.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const pollMs = opts.pollMs ?? DEFAULT_POLL_MS;
  const deadline = Date.now() + timeoutMs;

  while (!tryAcquireSync(lockPath)) {
    if (Date.now() >= deadline) {
      throw new Error(
        `withFileLockSync: timed out waiting for ${lockPath} after ${timeoutMs}ms`,
      );
    }
    sleepSyncMs(pollMs);
  }

  try {
    return fn();
  } finally {
    releaseSync(lockPath);
  }
}

/** Async version for callsites with `await` in the RMW block. */
export async function withFileLock<T>(
  target: string,
  fn: () => Promise<T>,
  opts: LockOptions = {},
): Promise<T> {
  const lockPath = lockPathFor(target);
  const timeoutMs = opts.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const pollMs = opts.pollMs ?? DEFAULT_POLL_MS;
  const deadline = Date.now() + timeoutMs;

  while (!tryAcquireSync(lockPath)) {
    if (Date.now() >= deadline) {
      throw new Error(
        `withFileLock: timed out waiting for ${lockPath} after ${timeoutMs}ms`,
      );
    }
    await new Promise(r => setTimeout(r, pollMs));
  }

  try {
    return await fn();
  } finally {
    releaseSync(lockPath);
  }
}
