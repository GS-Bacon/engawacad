import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, rmSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { REGEN_CAP, TOKEN_CAP, addTokens, getState, incRegen } from "./loop-adr-regen-tracker";

let workDir: string;
let originalCwd: string;

beforeEach(() => {
  originalCwd = process.cwd();
  workDir = mkdtempSync(join(tmpdir(), "adr-regen-test-"));
  process.chdir(workDir);
});

afterEach(() => {
  process.chdir(originalCwd);
  rmSync(workDir, { recursive: true, force: true });
});

describe("loop-adr-regen-tracker", () => {
  test("初回 incRegen は count=1, capped=false", () => {
    const r = incRegen("test-adr");
    expect(r.state.regen_count).toBe(1);
    expect(r.capped).toBe(false);
    expect(getState("test-adr")?.regen_count).toBe(1);
  });

  test("REGEN_CAP+1 回目で capped=true (reason=regen_cap)", () => {
    let last;
    for (let i = 0; i < REGEN_CAP + 1; i++) {
      last = incRegen("test-adr");
    }
    expect(last!.state.regen_count).toBe(REGEN_CAP + 1);
    expect(last!.capped).toBe(true);
    expect(last!.reason).toBe("regen_cap");
  });

  test("addTokens 累積で TOKEN_CAP 越えると capped=true (reason=token_cap)", () => {
    const half = Math.ceil(TOKEN_CAP / 2);
    const r1 = addTokens("test-adr", half);
    expect(r1.capped).toBe(false);
    const r2 = addTokens("test-adr", half + 100);
    expect(r2.capped).toBe(true);
    expect(r2.reason).toBe("token_cap");
    expect(r2.state.token_used).toBe(half * 2 + 100);
  });

  test("getState 未作成は null", () => {
    expect(getState("unknown")).toBeNull();
  });

  test("addTokens 負数で throw", () => {
    expect(() => addTokens("test-adr", -1)).toThrow();
  });

  test("incRegen と addTokens は独立カウント (同じ slug でも干渉しない)", () => {
    addTokens("test-adr", 50_000);
    incRegen("test-adr");
    const s = getState("test-adr");
    expect(s?.regen_count).toBe(1);
    expect(s?.token_used).toBe(50_000);
  });
});
