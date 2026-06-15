#!/usr/bin/env bun
// loop-lock.ts — 3ailoop / 3ailoop-intake 間の排他制御 (atomic dir + session token)
//
// 設計:
// - lock は ディレクトリ (`features/.batch/lock/`) で表現し、mkdir の EEXIST atomic semantics
//   で排他取得する (POSIX 保証)。
// - メタ情報 (token / owner / acquired_at / renewed_at / pid) は `features/.batch/lock/meta.json`
//   に temp file + rename で atomic に書き込む。
// - acquire 時に乱数 token (16 byte hex) を発行し stdout に返す。release/renew は token 一致
//   時のみ動作 (同 owner 別セッションの誤介入を防ぐ)。
// - stale 判定は acquired_at または renewed_at (最新のもの) から STALE_THRESHOLD_MS (12h)
//   を超えた場合のみ。
// - meta.json が parse 失敗 (= 不完全な書き込み or 破損) のとき fail-closed (lock 取得失敗
//   扱い) し、勝手な解放はしない。明示的に `release --force` した場合のみ強制解放可。
//
// 使い方:
//   bun loop-lock.ts acquire --owner loop|intake          # 成功時 stdout に token、exit 0
//   bun loop-lock.ts release --token <id>                  # token 一致時のみ削除、exit 0/1
//   bun loop-lock.ts release --force                       # 強制解放 (meta 破損時の復旧用)
//   bun loop-lock.ts renew   --token <id>                  # renewed_at を更新、exit 0/1
//   bun loop-lock.ts status                                # meta JSON or "none"
//
// 関連:
// - Issue #167 (初版実装、PID 判定撤廃)
// - Issue #168 (Codex review critical x2 / high x2 への対応、本ファイルの atomic 化)

import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "fs";
import { join, dirname } from "path";
import { randomBytes } from "crypto";

const LOCK_DIR = "features/.batch/lock";
const META_PATH = join(LOCK_DIR, "meta.json");
const STALE_THRESHOLD_MS = 12 * 60 * 60 * 1000; // 12h

type Owner = "loop" | "intake";

interface LockMeta {
  token: string;
  pid: number;
  owner: Owner;
  acquired_at: string; // ISO8601
  renewed_at?: string; // ISO8601, optional
}

function generateToken(): string {
  return randomBytes(16).toString("hex");
}

function atomicWriteMeta(meta: LockMeta): void {
  mkdirSync(dirname(META_PATH), { recursive: true });
  const tmpPath = `${META_PATH}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmpPath, JSON.stringify(meta, null, 2), "utf-8");
  renameSync(tmpPath, META_PATH);
}

function readMeta(): LockMeta | null {
  if (!existsSync(META_PATH)) return null;
  try {
    const text = readFileSync(META_PATH, "utf-8");
    const obj = JSON.parse(text) as LockMeta;
    if (typeof obj.token !== "string" || typeof obj.owner !== "string") {
      return null;
    }
    return obj;
  } catch {
    return null;
  }
}

function isStale(meta: LockMeta): { stale: boolean; reason: string } {
  const ref = meta.renewed_at ?? meta.acquired_at;
  const refTime = new Date(ref).getTime();
  if (isNaN(refTime)) {
    return { stale: true, reason: `invalid timestamp: ${ref}` };
  }
  const ageMs = Date.now() - refTime;
  if (ageMs > STALE_THRESHOLD_MS) {
    const ageH = (ageMs / 1000 / 60 / 60).toFixed(1);
    return { stale: true, reason: `age ${ageH}h exceeds ${STALE_THRESHOLD_MS / 3600000}h` };
  }
  return { stale: false, reason: "" };
}

/** mkdir で原子的に LOCK_DIR を取得。EEXIST なら既存 lock あり扱い。 */
function tryAcquireDir(): "created" | "exists" {
  // 親ディレクトリは事前に確保
  mkdirSync(dirname(LOCK_DIR), { recursive: true });
  try {
    mkdirSync(LOCK_DIR, { recursive: false });
    return "created";
  } catch (e: unknown) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === "EEXIST") return "exists";
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
      // meta.json が無い or 破損 → fail-closed
      return {
        ok: false,
        reason: "lock dir exists but meta.json is missing or corrupt; use 'release --force' to recover",
      };
    }
    // Stale 判定
    const { stale, reason } = isStale(meta);
    if (stale) {
      rmSync(LOCK_DIR, { recursive: true, force: true });
      // Recover acquire
      const retry = tryAcquireDir();
      if (retry === "exists") {
        return { ok: false, reason: "race detected during stale recovery; retry later" };
      }
      const token = generateToken();
      const newMeta: LockMeta = {
        token,
        pid: process.pid,
        owner,
        acquired_at: new Date().toISOString(),
      };
      atomicWriteMeta(newMeta);
      return { ok: true, token, reason: `stale lock released (${reason}); new lock acquired` };
    }
    return {
      ok: false,
      reason: `lock held by owner=${meta.owner} token=${meta.token.slice(0, 8)}... since ${meta.acquired_at}`,
    };
  }

  // created
  const token = generateToken();
  const meta: LockMeta = {
    token,
    pid: process.pid,
    owner,
    acquired_at: new Date().toISOString(),
  };
  atomicWriteMeta(meta);
  return { ok: true, token, reason: "acquired" };
}

export interface SimpleResult {
  ok: boolean;
  reason: string;
}

export function releaseLock(token: string, force = false): SimpleResult {
  if (!existsSync(LOCK_DIR)) {
    return { ok: true, reason: "no lock to release" };
  }
  if (force) {
    rmSync(LOCK_DIR, { recursive: true, force: true });
    return { ok: true, reason: "force released" };
  }
  const meta = readMeta();
  if (!meta) {
    return { ok: false, reason: "meta.json missing or corrupt; use --force to release" };
  }
  if (meta.token !== token) {
    return { ok: false, reason: `token mismatch (held by ${meta.token.slice(0, 8)}...)` };
  }
  rmSync(LOCK_DIR, { recursive: true, force: true });
  return { ok: true, reason: "released" };
}

export function renewLock(token: string): SimpleResult {
  const meta = readMeta();
  if (!meta) {
    return { ok: false, reason: "no lock to renew (meta.json missing or corrupt)" };
  }
  if (meta.token !== token) {
    return { ok: false, reason: `token mismatch (held by ${meta.token.slice(0, 8)}...)` };
  }
  meta.renewed_at = new Date().toISOString();
  atomicWriteMeta(meta);
  return { ok: true, reason: "renewed" };
}

export function statusLock(): LockMeta | null {
  return readMeta();
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;

  function parseArg(args: string[], name: string): string | undefined {
    const idx = args.indexOf(name);
    return idx >= 0 ? args[idx + 1] : undefined;
  }
  function hasFlag(args: string[], name: string): boolean {
    return args.includes(name);
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
        process.stdout.write(`${result.token}\n`);
        process.stderr.write(`OK: ${result.reason}\n`);
        process.exit(0);
      } else {
        process.stderr.write(`LOCKED: ${result.reason}\n`);
        process.exit(1);
      }
      break;
    }
    case "release": {
      const tokenArg = parseArg(rest, "--token") ?? "";
      const force = hasFlag(rest, "--force");
      if (!tokenArg && !force) {
        console.error("Usage: loop-lock.ts release (--token <id> | --force)");
        process.exit(2);
      }
      const result = releaseLock(tokenArg, force);
      if (result.ok) {
        process.stderr.write(`OK: ${result.reason}\n`);
        process.exit(0);
      } else {
        process.stderr.write(`FAIL: ${result.reason}\n`);
        process.exit(1);
      }
      break;
    }
    case "renew": {
      const tokenArg = parseArg(rest, "--token");
      if (!tokenArg) {
        console.error("Usage: loop-lock.ts renew --token <id>");
        process.exit(2);
      }
      const result = renewLock(tokenArg);
      if (result.ok) {
        process.stderr.write(`OK: ${result.reason}\n`);
        process.exit(0);
      } else {
        process.stderr.write(`FAIL: ${result.reason}\n`);
        process.exit(1);
      }
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
      console.error(
        "Usage: loop-lock.ts (acquire --owner loop|intake | release --token <id>|--force | renew --token <id> | status)",
      );
      process.exit(2);
  }
}
