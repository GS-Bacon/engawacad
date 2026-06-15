#!/usr/bin/env bun
// loop-lock.ts — 3ailoop / 3ailoop-intake 間の排他制御
//
// features/.batch/lock を介した OS file lock。intake と loop の同時実行を防ぐ。
//
// 使い方:
//   bun loop-lock.ts acquire --owner loop|intake   # 成功 exit 0、失敗 exit 1
//   bun loop-lock.ts release                       # 常に exit 0
//   bun loop-lock.ts status                        # stdout に lock 状態 JSON、未取得なら "none"
//
// 仕様:
// - lock 内容: { pid, owner, acquired_at } を JSON で記録 (pid はデバッグ参照用)
// - acquire 時に既存 lock の acquired_at から STALE_THRESHOLD_MS (2h) 以上経過なら自動解放
// - PID 判定は採用しない: /3ailoop / /3ailoop-intake は subprocess を逐次呼ぶ運用で、各
//   acquire コマンドが exit すると bun プロセスが終了するため「PID 死亡 = stale」では
//   排他制御として成立しない。代わりに acquired_at の絶対時刻のみで stale 判定する
// - 同 owner からの再 acquire は idempotent (失敗扱いにせず exit 0)

import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "fs";
import { dirname } from "path";

const LOCK_PATH = "features/.batch/lock";
const STALE_THRESHOLD_MS = 2 * 60 * 60 * 1000; // 2h

type Owner = "loop" | "intake";

interface LockData {
  pid: number;
  owner: Owner;
  acquired_at: string; // ISO8601
}

function readLock(): LockData | null {
  if (!existsSync(LOCK_PATH)) return null;
  try {
    return JSON.parse(readFileSync(LOCK_PATH, "utf-8")) as LockData;
  } catch {
    return null;
  }
}

function writeLock(data: LockData): void {
  mkdirSync(dirname(LOCK_PATH), { recursive: true });
  writeFileSync(LOCK_PATH, JSON.stringify(data, null, 2), "utf-8");
}

function deleteLock(): void {
  if (existsSync(LOCK_PATH)) rmSync(LOCK_PATH);
}

function isStale(lock: LockData): { stale: boolean; reason: string } {
  const acquiredAt = new Date(lock.acquired_at).getTime();
  if (isNaN(acquiredAt)) {
    return { stale: true, reason: `invalid acquired_at: ${lock.acquired_at}` };
  }
  const ageMs = Date.now() - acquiredAt;
  if (ageMs > STALE_THRESHOLD_MS) {
    const ageH = (ageMs / 1000 / 60 / 60).toFixed(1);
    return { stale: true, reason: `lock age ${ageH}h exceeds ${STALE_THRESHOLD_MS / 1000 / 60 / 60}h` };
  }
  return { stale: false, reason: "" };
}

export function acquireLock(owner: Owner): { ok: boolean; reason: string } {
  const existing = readLock();
  if (existing) {
    // Idempotent: 同 owner の lock がある場合は再取得扱い (PID は判定に使わない)
    // 別 owner の lock がある場合のみ block / stale 判定
    if (existing.owner === owner) {
      return { ok: true, reason: "idempotent re-acquire (same owner)" };
    }
    // 別 owner の lock。Stale ならば解放、まだ生きていれば失敗
    const { stale, reason } = isStale(existing);
    if (stale) {
      deleteLock();
      const data: LockData = {
        pid: process.pid,
        owner,
        acquired_at: new Date().toISOString(),
      };
      writeLock(data);
      return { ok: true, reason: `stale lock released (${reason}); new lock acquired` };
    }
    return {
      ok: false,
      reason: `lock held by owner=${existing.owner} pid=${existing.pid} since ${existing.acquired_at}`,
    };
  }
  const data: LockData = {
    pid: process.pid,
    owner,
    acquired_at: new Date().toISOString(),
  };
  writeLock(data);
  return { ok: true, reason: "acquired" };
}

export function releaseLock(): void {
  deleteLock();
}

export function statusLock(): LockData | null {
  return readLock();
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;

  function parseArg(args: string[], name: string): string | undefined {
    const idx = args.indexOf(name);
    return idx >= 0 ? args[idx + 1] : undefined;
  }

  switch (cmd) {
    case "acquire": {
      const ownerArg = parseArg(rest, "--owner");
      if (ownerArg !== "loop" && ownerArg !== "intake") {
        console.error("Usage: loop-lock.ts acquire --owner loop|intake");
        process.exit(2);
      }
      const result = acquireLock(ownerArg as Owner);
      if (result.ok) {
        console.log(`OK: ${result.reason}`);
        process.exit(0);
      } else {
        console.error(`LOCKED: ${result.reason}`);
        process.exit(1);
      }
      break;
    }
    case "release": {
      releaseLock();
      console.log("OK: released");
      process.exit(0);
      break;
    }
    case "status": {
      const s = statusLock();
      if (s) {
        console.log(JSON.stringify(s, null, 2));
      } else {
        console.log("none");
      }
      process.exit(0);
      break;
    }
    default:
      console.error("Usage: loop-lock.ts (acquire --owner loop|intake | release | status)");
      process.exit(2);
  }
}
