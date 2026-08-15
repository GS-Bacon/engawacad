#!/usr/bin/env bun
// loop-lock.ts — 3ailoop 排他制御 CLI (atomic dir + session token, hardened)
//
// 旧 .claude/skills/3ailoop/scripts/loop-lock.ts と同等の設計を新パスで独立実装。
// - lock は ディレクトリ (features/.batch/lock/) で表現し、mkdir の EEXIST atomic semantics で排他取得。
// - meta (token/owner/acquired_at/renewed_at/pid) を meta.json に temp+rename atomic で書く。
// - Stale 判定: meta の acquired/renewed から 12h、または meta 不在/破損時は dir mtime から 5min。
//
// 使い方:
//   bun loop-lock.ts acquire --owner loop|intake     # 成功時 stdout に token、exit 0
//   bun loop-lock.ts release --token <id> [--force]   # token 一致時のみ削除
//   bun loop-lock.ts renew   --token <id>
//   bun loop-lock.ts status

import {
  existsSync, mkdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync,
} from "fs";
import { dirname, join } from "path";
import { randomBytes } from "crypto";

function getLockDir(): string {
  return process.env.LOOP_LOCK_DIR ?? "features/.batch/lock";
}
function getMetaPath(): string {
  return join(getLockDir(), "meta.json");
}
const STALE_THRESHOLD_MS = 12 * 60 * 60 * 1000;
const DIR_MTIME_STALE_MS = 5 * 60 * 1000;

type Owner = "loop" | "intake";

interface LockMeta {
  token: string;
  pid: number;
  owner: Owner;
  acquired_at: string;
  renewed_at?: string;
}

function generateToken(): string {
  return randomBytes(16).toString("hex");
}

function isValidIso(s: unknown): s is string {
  if (typeof s !== "string") return false;
  return !isNaN(new Date(s).getTime());
}

function atomicWriteMeta(meta: LockMeta): void {
  const metaPath = getMetaPath();
  mkdirSync(dirname(metaPath), { recursive: true });
  const tmp = `${metaPath}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, JSON.stringify(meta, null, 2), "utf-8");
  renameSync(tmp, metaPath);
}

function readMeta(): LockMeta | null {
  const metaPath = getMetaPath();
  if (!existsSync(metaPath)) return null;
  try {
    const obj = JSON.parse(readFileSync(metaPath, "utf-8")) as Partial<LockMeta>;
    if (typeof obj.token !== "string" || !/^[0-9a-f]{32}$/.test(obj.token)) return null;
    if (obj.owner !== "loop" && obj.owner !== "intake") return null;
    if (!isValidIso(obj.acquired_at)) return null;
    if (obj.renewed_at !== undefined && !isValidIso(obj.renewed_at)) return null;
    if (typeof obj.pid !== "number" || !Number.isFinite(obj.pid)) return null;
    return obj as LockMeta;
  } catch {
    return null;
  }
}

function isOwnerDead(meta: LockMeta): boolean {
  if (typeof meta.pid !== "number" || !Number.isFinite(meta.pid)) return false;
  try {
    process.kill(meta.pid, 0);
    return false;
  } catch (e) {
    const err = e as NodeJS.ErrnoException & { errno?: number };
    return err.code === "ESRCH" || err.errno === 3;
  }
}

function isStaleByMeta(meta: LockMeta): boolean {
  const ref = meta.renewed_at ?? meta.acquired_at;
  return Date.now() - new Date(ref).getTime() > STALE_THRESHOLD_MS;
}

function isStaleByDirMtime(): boolean {
  try {
    return Date.now() - statSync(getLockDir()).mtimeMs > DIR_MTIME_STALE_MS;
  } catch {
    return true;
  }
}

function tryAcquireDir(): "created" | "exists" {
  const lockDir = getLockDir();
  mkdirSync(dirname(lockDir), { recursive: true });
  try {
    mkdirSync(lockDir, { recursive: false });
    return "created";
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "EEXIST") return "exists";
    throw e;
  }
}

export interface AcquireResult {
  ok: boolean;
  token?: string;
  reason: string;
}

export function acquireLock(owner: Owner): AcquireResult {
  const result = tryAcquireDir();
  if (result === "exists") {
    const meta = readMeta();
    if (!meta) {
      if (isStaleByDirMtime()) {
        rmSync(getLockDir(), { recursive: true, force: true });
        return acquireLock(owner);
      }
      return { ok: false, reason: `lock dir exists but meta missing/invalid (auto-reclaim after ${DIR_MTIME_STALE_MS / 60000}min)` };
    }
    if (isOwnerDead(meta) || isStaleByMeta(meta)) {
      rmSync(getLockDir(), { recursive: true, force: true });
      return acquireLock(owner);
    }
    return { ok: false, reason: `held by owner=${meta.owner} pid=${meta.pid} token=${meta.token.slice(0, 8)}...` };
  }
  const token = generateToken();
  atomicWriteMeta({ token, pid: process.pid, owner, acquired_at: new Date().toISOString() });
  return { ok: true, token, reason: "acquired" };
}

export function releaseLock(token: string, force = false): { ok: boolean; reason: string } {
  const lockDir = getLockDir();
  if (!existsSync(lockDir)) return { ok: true, reason: "no lock to release" };
  if (force) {
    rmSync(lockDir, { recursive: true, force: true });
    return { ok: true, reason: "force released" };
  }
  const meta = readMeta();
  if (!meta) return { ok: false, reason: "meta missing/invalid; use --force" };
  if (meta.token !== token) return { ok: false, reason: `token mismatch (held by ${meta.token.slice(0, 8)}...)` };
  rmSync(lockDir, { recursive: true, force: true });
  return { ok: true, reason: "released" };
}

export function renewLock(token: string): { ok: boolean; reason: string } {
  const meta = readMeta();
  if (!meta) return { ok: false, reason: "no lock to renew" };
  if (meta.token !== token) return { ok: false, reason: "token mismatch" };
  const recheck = readMeta();
  if (!recheck || recheck.token !== token) return { ok: false, reason: "lock changed during renew" };
  meta.renewed_at = new Date().toISOString();
  try {
    atomicWriteMeta(meta);
  } catch (e) {
    return { ok: false, reason: `write failed: ${(e as Error).message}` };
  }
  return { ok: true, reason: "renewed" };
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;
  const argN = (n: string) => { const i = rest.indexOf(n); return i >= 0 ? rest[i + 1] : undefined; };
  const has = (n: string) => rest.includes(n);

  switch (cmd) {
    case "acquire": {
      const owner = argN("--owner");
      if (owner !== "loop" && owner !== "intake") {
        console.error("Usage: loop-lock.ts acquire --owner loop|intake");
        process.exit(2);
      }
      const r = acquireLock(owner);
      if (r.ok) {
        process.stdout.write(`${r.token}\n`);
        process.stderr.write(`OK: ${r.reason}\n`);
        process.exit(0);
      } else {
        process.stderr.write(`LOCKED: ${r.reason}\n`);
        process.exit(1);
      }
    }
    case "release": {
      const token = argN("--token") ?? "";
      const force = has("--force");
      if (!token && !force) {
        console.error("Usage: loop-lock.ts release (--token <id> | --force)");
        process.exit(2);
      }
      const r = releaseLock(token, force);
      process.stderr.write(`${r.ok ? "OK" : "FAIL"}: ${r.reason}\n`);
      process.exit(r.ok ? 0 : 1);
    }
    case "renew": {
      const token = argN("--token");
      if (!token) { console.error("Usage: loop-lock.ts renew --token <id>"); process.exit(2); }
      const r = renewLock(token);
      process.stderr.write(`${r.ok ? "OK" : "FAIL"}: ${r.reason}\n`);
      process.exit(r.ok ? 0 : 1);
    }
    case "status": {
      const s = readMeta();
      console.log(s ? JSON.stringify(s, null, 2) : "none");
      process.exit(0);
    }
    default:
      console.error("Usage: loop-lock.ts (acquire --owner loop|intake | release --token <id>|--force | renew --token <id> | status)");
      process.exit(2);
  }
}
