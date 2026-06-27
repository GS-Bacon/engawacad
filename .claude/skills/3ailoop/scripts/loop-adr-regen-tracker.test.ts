import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, rmSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { mkdirSync, writeFileSync } from "fs";
import { REGEN_CAP, TOKEN_CAP, addTokens, clearRetiredIfHumanReleased, getState, incRegen } from "./loop-adr-regen-tracker";

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

  // ADR-013 運用: retired ADR を再評価ルートに戻す clearRetiredIfHumanReleased
  function seedRetiredState(slug: string, regen: number = REGEN_CAP + 1) {
    mkdirSync("features/.loop/adr-regen-count", { recursive: true });
    writeFileSync(
      `features/.loop/adr-regen-count/${slug}.json`,
      JSON.stringify({
        adr: slug,
        regen_count: regen,
        token_used: 0,
        started_at: "2026-06-20T00:00:00.000Z",
        last_updated_at: "2026-06-22T00:00:00.000Z",
        retired: { at: "2026-06-22T00:00:00.000Z", reason: "regen_cap" },
      }),
    );
  }

  test("clearRetiredIfHumanReleased: retired あり + needs-human 無し → cleared:true, regen_count=0", async () => {
    seedRetiredState("test-adr");
    const ghMock = async () => ({ stdout: JSON.stringify({ labels: [{ name: "docs" }] }), exit: 0 });
    const r = await clearRetiredIfHumanReleased("test-adr", 123, ghMock);
    expect(r.cleared).toBe(true);
    const s = getState("test-adr");
    expect(s?.regen_count).toBe(0);
    expect(s?.retired).toBeUndefined();
  });

  test("clearRetiredIfHumanReleased: retired あり + needs-human あり → cleared:false (needs_human_still_present)", async () => {
    seedRetiredState("test-adr");
    const ghMock = async () => ({
      stdout: JSON.stringify({ labels: [{ name: "docs" }, { name: "needs-human" }] }),
      exit: 0,
    });
    const r = await clearRetiredIfHumanReleased("test-adr", 123, ghMock);
    expect(r.cleared).toBe(false);
    expect(r.reason).toBe("needs_human_still_present");
    const s = getState("test-adr");
    expect(s?.regen_count).toBe(REGEN_CAP + 1);
    expect(s?.retired).toBeDefined();
  });

  test("clearRetiredIfHumanReleased: tracker file 無い → cleared:false (no_tracker_file)", async () => {
    const ghMock = async () => ({ stdout: "", exit: 0 });
    const r = await clearRetiredIfHumanReleased("unknown-adr", 123, ghMock);
    expect(r.cleared).toBe(false);
    expect(r.reason).toBe("no_tracker_file");
  });

  test("clearRetiredIfHumanReleased: retired 無し state → cleared:false (not_retired)", async () => {
    incRegen("test-adr");
    const ghMock = async () => ({ stdout: JSON.stringify({ labels: [] }), exit: 0 });
    const r = await clearRetiredIfHumanReleased("test-adr", 123, ghMock);
    expect(r.cleared).toBe(false);
    expect(r.reason).toBe("not_retired");
  });

  test("clearRetiredIfHumanReleased: gh exit≠0 → cleared:false (gh_error)", async () => {
    seedRetiredState("test-adr");
    const ghMock = async () => ({ stdout: "", exit: 1 });
    const r = await clearRetiredIfHumanReleased("test-adr", 123, ghMock);
    expect(r.cleared).toBe(false);
    expect(r.reason).toBe("gh_error");
    const s = getState("test-adr");
    expect(s?.retired).toBeDefined();
  });

  test("clearRetiredIfHumanReleased: gh stdout 不正 JSON → cleared:false (gh_error)", async () => {
    seedRetiredState("test-adr");
    const ghMock = async () => ({ stdout: "not-json", exit: 0 });
    const r = await clearRetiredIfHumanReleased("test-adr", 123, ghMock);
    expect(r.cleared).toBe(false);
    expect(r.reason).toBe("gh_error");
  });
});
