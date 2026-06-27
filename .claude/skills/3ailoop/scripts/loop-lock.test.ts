// loop-lock.test.ts — #282 pid liveness check の動作検証
//
// テスト戦略:
// - env LOOP_LOCK_DIR で lock dir を tmpdir に逃がす (本物の features/.batch/lock を汚さない)
// - meta.json を直接書き込んで「外部 pid 所有」状態を作る
// - dead pid は spawnSync で子プロセス起動 → 即終了 → reaped pid を使う

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { acquireLock, releaseLock, statusLock } from "./loop-lock";

let workDir: string;
let lockDir: string;
let metaPath: string;
let origLockDirEnv: string | undefined;

beforeEach(() => {
  workDir = mkdtempSync(join(tmpdir(), "loop-lock-test-"));
  lockDir = join(workDir, "lock");
  metaPath = join(lockDir, "meta.json");
  origLockDirEnv = process.env.LOOP_LOCK_DIR;
  process.env.LOOP_LOCK_DIR = lockDir;
});

afterEach(() => {
  if (origLockDirEnv === undefined) {
    delete process.env.LOOP_LOCK_DIR;
  } else {
    process.env.LOOP_LOCK_DIR = origLockDirEnv;
  }
  if (existsSync(workDir)) rmSync(workDir, { recursive: true, force: true });
});

/** 外部 pid 所有の lock を作成。acquire を経由せず meta.json を直接書く。 */
function setupExternalLock(pid: number): string {
  const token = "0".repeat(31) + "f"; // 32 hex chars, distinct from generateToken
  mkdirSync(lockDir, { recursive: true });
  writeFileSync(
    metaPath,
    JSON.stringify({
      token,
      pid,
      owner: "loop",
      acquired_at: new Date().toISOString(),
    }),
    "utf-8",
  );
  return token;
}

/**
 * ESRCH 確定の pid を作る — async spawn + SIGKILL + await exited で waitpid 済の pid を得る。
 * 直後の pid recycling race は極小 (kernel が次の fork までに同 pid 再割り当てする確率は実質 0)。
 */
async function getDeadPid(): Promise<number> {
  const proc = Bun.spawn(["/bin/sleep", "60"], { stdout: "ignore", stderr: "ignore" });
  const pid = proc.pid;
  if (typeof pid !== "number" || pid <= 0) {
    throw new Error(`Bun.spawn did not return a valid pid (got ${pid})`);
  }
  proc.kill("SIGKILL");
  await proc.exited;
  // sanity: kill(0) で ESRCH を確認 (環境差吸収)。Bun は code 未設定なので errno でも判定。
  try {
    process.kill(pid, 0);
    throw new Error(`pid ${pid} is still alive after SIGKILL+waitpid; pid recycling?`);
  } catch (e: unknown) {
    const err = e as NodeJS.ErrnoException & { errno?: number };
    const isESRCH = err.code === "ESRCH" || err.errno === 3;
    if (!isESRCH) {
      throw new Error(
        `unexpected error checking dead pid ${pid}: code=${err.code} errno=${err.errno} msg=${err.message}`,
      );
    }
  }
  return pid;
}

describe("acquireLock pid liveness check (#282)", () => {
  test("T01 alive owner: 別 owner の acquire は LOCKED", () => {
    // 自プロセス (= 確実に alive) の pid で外部 lock を立てる
    const externalToken = setupExternalLock(process.pid);
    expect(existsSync(metaPath)).toBe(true);

    const result = acquireLock("loop");
    expect(result.ok).toBe(false);
    expect(result.reason).toMatch(/lock held by/);
    expect(result.reason).toMatch(/alive/);

    // meta.json は外部 token のまま (takeover されてない)
    const meta = JSON.parse(readFileSync(metaPath, "utf-8")) as { token: string };
    expect(meta.token).toBe(externalToken);
  });

  test("T02 dead owner: acquire が takeover して新 token 発行", async () => {
    const deadPid = await getDeadPid();
    const externalToken = setupExternalLock(deadPid);

    const result = acquireLock("loop");
    expect(result.ok).toBe(true);
    expect(result.token).toBeDefined();
    expect(result.token).not.toBe(externalToken);
    expect(result.token).toMatch(/^[0-9a-f]{32}$/);

    // 新しい meta が書き込まれている
    const meta = JSON.parse(readFileSync(metaPath, "utf-8")) as {
      token: string;
      pid: number;
    };
    expect(meta.token).toBe(result.token!);
    expect(meta.pid).toBe(process.pid);
  });

  test("T03 dead owner reason 詳細: status は新 owner の pid を返す", async () => {
    const deadPid = await getDeadPid();
    setupExternalLock(deadPid);

    const result = acquireLock("loop");
    expect(result.ok).toBe(true);

    const status = statusLock();
    expect(status).not.toBeNull();
    expect(status!.pid).toBe(process.pid);
    expect(status!.owner).toBe("loop");
  });

  test("T04_boundary 既存 dir-mtime 経路は変更なし: meta 不在 + 新規 dir → LOCKED", () => {
    // mkdir のみで meta.json なし (= 既存 fallback 経路)
    mkdirSync(lockDir, { recursive: true });

    const result = acquireLock("loop");
    expect(result.ok).toBe(false);
    expect(result.reason).toMatch(/meta\.json missing\/invalid/);
    expect(result.reason).toMatch(/dir mtime age/);
  });

  test("T05_degen_takeover_round_trip: dead 回収 → release → 再 acquire まで通る", async () => {
    const deadPid = await getDeadPid();
    setupExternalLock(deadPid);

    // 1. takeover
    const first = acquireLock("loop");
    expect(first.ok).toBe(true);
    const token1 = first.token!;

    // 2. release
    const released = releaseLock(token1);
    expect(released.ok).toBe(true);
    expect(existsSync(lockDir)).toBe(false);

    // 3. 新規 acquire
    const second = acquireLock("intake");
    expect(second.ok).toBe(true);
    expect(second.token).not.toBe(token1);

    // cleanup
    releaseLock(second.token!);
  });
});
