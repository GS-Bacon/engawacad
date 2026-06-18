// loop-file-lock.test.ts — #226 RMW serialize util の動作検証

import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { withFileLock, withFileLockSync } from "./loop-file-lock";

let workDir: string;
let target: string;

beforeEach(() => {
  workDir = mkdtempSync(join(tmpdir(), "loop-file-lock-test-"));
  target = join(workDir, "counter.json");
  writeFileSync(target, JSON.stringify({ count: 0 }), "utf-8");
});

afterEach(() => {
  if (existsSync(workDir)) rmSync(workDir, { recursive: true, force: true });
});

function rmwIncrement(): number {
  const obj = JSON.parse(readFileSync(target, "utf-8")) as { count: number };
  // simulate non-atomic read-modify-write with a small delay to widen the race window
  for (let i = 0; i < 1000; i++) Math.sqrt(i);
  obj.count += 1;
  writeFileSync(target, JSON.stringify(obj), "utf-8");
  return obj.count;
}

describe("withFileLock (sync)", () => {
  test("RMW under lock produces correct count with N concurrent in-process increments", async () => {
    // Run 10 increments via Promise.all wrapping the sync withFileLockSync.
    // Without the lock, race condition would cause lost updates and count < 10.
    const N = 10;
    const results = await Promise.all(
      Array.from({ length: N }, () =>
        Promise.resolve().then(() => withFileLockSync(target, rmwIncrement)),
      ),
    );
    expect(results.length).toBe(N);
    const final = JSON.parse(readFileSync(target, "utf-8")) as { count: number };
    expect(final.count).toBe(N);
    // each returned count must be unique and in [1..N]
    expect(new Set(results).size).toBe(N);
  });

  test("releases lock even when fn throws", () => {
    expect(() =>
      withFileLockSync(target, () => {
        throw new Error("boom");
      }),
    ).toThrow("boom");
    // The lock dir must be removed so the next call can acquire immediately.
    expect(withFileLockSync(target, () => 42)).toBe(42);
  });

  test("timeout when lock is held by another holder", () => {
    // Manually create the lock dir to simulate another process holding it.
    const lockPath = `${target}.lock`;
    require("fs").mkdirSync(lockPath);
    try {
      expect(() =>
        withFileLockSync(target, () => 1, { timeoutMs: 100, pollMs: 20 }),
      ).toThrow(/timed out waiting/);
    } finally {
      require("fs").rmdirSync(lockPath);
    }
  });
});

describe("withFileLock (async)", () => {
  test("RMW under async lock produces correct count", async () => {
    const N = 5;
    const results = await Promise.all(
      Array.from({ length: N }, () =>
        withFileLock(target, async () => {
          const obj = JSON.parse(readFileSync(target, "utf-8")) as { count: number };
          // yield to widen race window
          await new Promise(r => setTimeout(r, 1));
          obj.count += 1;
          writeFileSync(target, JSON.stringify(obj), "utf-8");
          return obj.count;
        }),
      ),
    );
    expect(results.length).toBe(N);
    const final = JSON.parse(readFileSync(target, "utf-8")) as { count: number };
    expect(final.count).toBe(N);
    expect(new Set(results).size).toBe(N);
  });

  test("releases async lock even when fn throws", async () => {
    await expect(
      withFileLock(target, async () => {
        throw new Error("boom-async");
      }),
    ).rejects.toThrow("boom-async");
    const r = await withFileLock(target, async () => 99);
    expect(r).toBe(99);
  });
});
