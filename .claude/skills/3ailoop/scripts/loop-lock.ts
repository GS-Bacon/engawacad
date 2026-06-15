#!/usr/bin/env bun
// loop-lock.ts — 3ailoop / 3ailoop-intake 間の排他制御 (atomic dir + session token, hardened)
//
// 設計:
// - lock は ディレクトリ (`features/.batch/lock/`) で表現し、mkdir の EEXIST atomic semantics
//   で排他取得する (POSIX 保証)。
// - メタ情報 (token / owner / acquired_at / renewed_at / pid) は `features/.batch/lock/meta.json`
//   に temp file + rename で atomic に書き込む。
// - acquire 時に乱数 token (16 byte hex = 32 文字) を発行し stdout に返す。release/renew は
//   token 一致時のみ動作 (同 owner 別セッションの誤介入を防ぐ)。
// - Stale 判定は二段:
//     1. meta.json が読めて値域 OK → acquired_at / renewed_at の最新から 12h 経過で stale
//     2. meta.json 不在 or 破損 → dir mtime から 5min 経過で stale (= mkdir 直後の死亡から復旧)
// - renew は書込直前に再 readMeta() して token 一致確認 (best-effort CAS で race 縮小)。
// - readMeta() は値域厳格検証: token 32hex / owner "loop"|"intake" / 有効 Date / pid number。
//   いずれか不合格なら null 返却 → dir mtime fail-closed 経路で自動回収。
//
// 使い方:
//   bun loop-lock.ts acquire --owner loop|intake          # 成功時 stdout に token、exit 0
//   bun loop-lock.ts release --token <id>                  # token 一致時のみ削除、exit 0/1
//   bun loop-lock.ts release --force                       # 強制解放 (meta 破損時の復旧用)
//   bun loop-lock.ts renew   --token <id>                  # renewed_at を更新、exit 0/1
//   bun loop-lock.ts status                                # meta JSON or "none"
//
// 関連: #167 → #168 → #169 (連鎖、最終 hardening)

import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "fs";
import { dirname, join } from "path";
import { randomBytes } from "crypto";

const LOCK_DIR = "features/.batch/lock";
const META_PATH = join(LOCK_DIR, "meta.json");
const STALE_THRESHOLD_MS = 12 * 60 * 60 * 1000; // 12h: 通常 lock の最大保持
const DIR_MTIME_STALE_MS = 5 * 60 * 1000; // 5min: meta 不在 / 破損時の dir 回収

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

function isValidIsoTimestamp(s: unknown): s is string {
  if (typeof s !== "string") return false;
  const t = new Date(s).getTime();
  return !isNaN(t);
}

function atomicWriteMeta(meta: LockMeta): void {
  mkdirSync(dirname(META_PATH), { recursive: true });
  const tmpPath = `${META_PATH}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmpPath, JSON.stringify(meta, null, 2), "utf-8");
  renameSync(tmpPath, META_PATH);
}

/** 値域厳格検証付き readMeta。不合格なら null。 */
function readMeta(): LockMeta | null {
  if (!existsSync(META_PATH)) return null;
  try {
    const text = readFileSync(META_PATH, "utf-8");
    const obj = JSON.parse(text) as Partial<LockMeta>;
    if (typeof obj.token !== "string" || !/^[0-9a-f]{32}$/.test(obj.token)) return null;
    if (obj.owner !== "loop" && obj.owner !== "intake") return null;
    if (!isValidIsoTimestamp(obj.acquired_at)) return null;
    if (obj.renewed_at !== undefined && !isValidIsoTimestamp(obj.renewed_at)) return null;
    if (typeof obj.pid !== "number" || !Number.isFinite(obj.pid)) return null;
    return obj as LockMeta;
  } catch {
    return null;
  }
}

function isStaleByMeta(meta: LockMeta): { stale: boolean; reason: string } {
  const ref = meta.renewed_at ?? meta.acquired_at;
  const refTime = new Date(ref).getTime();
  const ageMs = Date.now() - refTime;
  if (ageMs > STALE_THRESHOLD_MS) {
    const ageH = (ageMs / 3600000).toFixed(1);
    return { stale: true, reason: `meta age ${ageH}h exceeds ${STALE_THRESHOLD_MS / 3600000}h` };
  }
  return { stale: false, reason: "" };
}

/** meta.json が読めないとき、LOCK_DIR の mtime で stale 判定 (dir 生成のみで死んだケースの復旧)。 */
function isStaleByDirMtime(): { stale: boolean; reason: string } {
  try {
    const stat = statSync(LOCK_DIR);
    const ageMs = Date.now() - stat.mtimeMs;
    if (ageMs > DIR_MTIME_STALE_MS) {
      const ageMin = (ageMs / 60000).toFixed(1);
      return {
        stale: true,
        reason: `dir mtime age ${ageMin}min exceeds ${DIR_MTIME_STALE_MS / 60000}min`,
      };
    }
    return { stale: false, reason: `dir mtime age too recent` };
  } catch {
    return { stale: true, reason: "stat failed" };
  }
}

function tryAcquireDir(): "created" | "exists" {
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
      // meta.json 不在 / 破損 → dir mtime で短期 stale 判定
      const dirStale = isStaleByDirMtime();
      if (dirStale.stale) {
        rmSync(LOCK_DIR, { recursive: true, force: true });
        return acquireLock(owner); // 再試行 (1 段の再帰、新規 mkdir パスへ)
      }
      return {
        ok: false,
        reason: `lock dir exists but meta.json missing/invalid; ${dirStale.reason} (will auto-reclaim after ${DIR_MTIME_STALE_MS / 60000}min). Use 'release --force' to override.`,
      };
    }
    const { stale, reason } = isStaleByMeta(meta);
    if (stale) {
      rmSync(LOCK_DIR, { recursive: true, force: true });
      return acquireLock(owner);
    }
    return {
      ok: false,
      reason: `lock held by owner=${meta.owner} token=${meta.token.slice(0, 8)}... since ${meta.acquired_at}`,
    };
  }

  // created — すぐに meta.json を書く (race window 最小化)
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
    return { ok: false, reason: "meta.json missing or invalid; use --force to release" };
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
    return { ok: false, reason: "no lock to renew (meta missing or invalid)" };
  }
  if (meta.token !== token) {
    return { ok: false, reason: `token mismatch (held by ${meta.token.slice(0, 8)}...)` };
  }
  // Best-effort CAS: 書込直前に再 readMeta して token 一致を再確認
  const recheck = readMeta();
  if (!recheck || recheck.token !== token) {
    return { ok: false, reason: "lock changed during renew (lost lock)" };
  }
  meta.renewed_at = new Date().toISOString();
  try {
    atomicWriteMeta(meta);
  } catch (e) {
    return { ok: false, reason: `write failed: ${(e as Error).message}` };
  }
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
