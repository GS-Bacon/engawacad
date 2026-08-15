#!/usr/bin/env bun
// file-lock.ts — RMW (read-modify-write) を file-path-scoped で直列化する共通 util
//
// この new runtime は旧 .claude/skills/3ailoop/scripts/loop-file-lock.ts の設計を引き継ぐが、
// 新パス (`.agents/`) 配下で独立して動作する (旧実装への依存を切るため)。
//
// 設計:
// - read → modify → write の間の race window で lost update が起きるのを防ぐ。
// - directory-based lock (mkdir は POSIX atomic) で RMW ブロックを cross-process 排他。
// - lock dir は `<target>.lock`。timeout/poll で待機、finally で必ず rmdir。
// - 単一プロセス内の再入は直列化 (sync 版は同期的なのでそもそも再入しない)。
//
// 使い方:
//   import { withFileLockSync } from "./file-lock";
//   const next = withFileLockSync("features/.loop/state.json", () => {
//     const s = readState();
//     s.generation += 1;
//     atomicWrite(s);
//     return s.generation;
//   });

import { mkdirSync, rmdirSync } from "fs";
import { dirname, resolve } from "path";

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

/** Run `fn` under an exclusive lock for `target`. Sync version for RMW callsites
 *  which all use sync I/O.
 *  Throws if the lock cannot be acquired within `opts.timeoutMs`. */
export function withFileLockSync<T>(
  target: string,
  fn: () => T,
  opts: LockOptions = {},
): T {
  const lockPath = lockPathFor(target);
  mkdirSync(dirname(lockPath), { recursive: true });
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
  mkdirSync(dirname(lockPath), { recursive: true });
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
