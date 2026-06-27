import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, rmSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { mkdirSync, writeFileSync } from "fs";
import { REGEN_CAP, TOKEN_CAP, addTokens, clearRetiredIfHumanReleased, getState, incRegen, retire } from "./loop-adr-regen-tracker";

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

  // --- #283 欠陥 A 再現テスト: ADR retire 時の子 Issue 伝播 ---
  // ADR Issue #N を retire したとき、parent-adr:N ラベル付きの子 Issue
  // 全部に blocked-by-adr-retired を一括付与する。これがないと batch-select
  // の split-batch tier で子が pick され続け、intent-check aligned=no で
  // 連続 pause する (cycle 49-65 観測)。

  function makeGhRecorder(opts: {
    listResult?: Array<{ number: number }>;
    listExit?: number;
    editExit?: number;
  } = {}) {
    const calls: string[][] = [];
    const list = opts.listResult ?? [];
    const ghFn = async (args: string[]) => {
      calls.push([...args]);
      if (args[0] === "issue" && args[1] === "list") {
        return { stdout: JSON.stringify(list), exit: opts.listExit ?? 0 };
      }
      return { stdout: "", exit: opts.editExit ?? 0 };
    };
    return { calls, ghFn };
  }

  test("retire: parent-adr:N 子 Issue 全部に blocked-by-adr-retired を一括付与する", async () => {
    const { calls, ghFn } = makeGhRecorder({ listResult: [{ number: 274 }, { number: 275 }, { number: 276 }] });
    const r = await retire("test-adr", 123, "token_cap", ghFn);
    expect(r.ok).toBe(true);
    expect(r.propagated).toEqual([274, 275, 276]);

    // 親 ADR Issue は gate:adr-review 削除 + needs-human 付与
    expect(calls).toContainEqual(["issue", "edit", "123", "--remove-label", "gate:adr-review"]);
    expect(calls).toContainEqual(["issue", "edit", "123", "--add-label", "needs-human"]);
    // 子 Issue 一括付与
    expect(calls).toContainEqual(["issue", "edit", "274", "--add-label", "blocked-by-adr-retired"]);
    expect(calls).toContainEqual(["issue", "edit", "275", "--add-label", "blocked-by-adr-retired"]);
    expect(calls).toContainEqual(["issue", "edit", "276", "--add-label", "blocked-by-adr-retired"]);
    // gh issue list 子検索 (parent-adr:<ADR Issue 番号>)
    const listCalls = calls.filter(c => c[0] === "issue" && c[1] === "list");
    expect(listCalls.length).toBeGreaterThan(0);
    expect(listCalls[0]).toContain("--label");
    expect(listCalls[0]).toContain("parent-adr:123");
  });

  test("retire: parent-adr:N 子が 0 件でも親 retire は成功", async () => {
    const { ghFn } = makeGhRecorder({ listResult: [] });
    const r = await retire("test-adr", 123, "token_cap", ghFn);
    expect(r.ok).toBe(true);
    expect(r.propagated).toEqual([]);
  });

  test("retire: gh list が失敗しても親 retire は完遂、子伝播は []", async () => {
    const { ghFn } = makeGhRecorder({ listExit: 1 });
    const r = await retire("test-adr", 123, "token_cap", ghFn);
    expect(r.ok).toBe(true);
    expect(r.propagated).toEqual([]);
  });

  test("clearRetiredIfHumanReleased: cleared 時に子から blocked-by-adr-retired を一括削除", async () => {
    seedRetiredState("test-adr");
    const calls: string[][] = [];
    const ghFn = async (args: string[]) => {
      calls.push([...args]);
      if (args[0] === "issue" && args[1] === "view") {
        // 親 Issue: needs-human 無し → 解除可
        return { stdout: JSON.stringify({ labels: [{ name: "docs" }] }), exit: 0 };
      }
      if (args[0] === "issue" && args[1] === "list") {
        return { stdout: JSON.stringify([{ number: 274 }, { number: 275 }]), exit: 0 };
      }
      return { stdout: "", exit: 0 };
    };
    const r = await clearRetiredIfHumanReleased("test-adr", 123, ghFn);
    expect(r.cleared).toBe(true);
    expect(r.released).toEqual([274, 275]);

    // 子 Issue から blocked-by-adr-retired を削除
    expect(calls).toContainEqual(["issue", "edit", "274", "--remove-label", "blocked-by-adr-retired"]);
    expect(calls).toContainEqual(["issue", "edit", "275", "--remove-label", "blocked-by-adr-retired"]);

    // tracker state は reset
    const s = getState("test-adr");
    expect(s?.regen_count).toBe(0);
    expect(s?.retired).toBeUndefined();
  });

  test("clearRetiredIfHumanReleased: cleared:false なら子の release は呼ばれない", async () => {
    seedRetiredState("test-adr");
    const calls: string[][] = [];
    const ghFn = async (args: string[]) => {
      calls.push([...args]);
      if (args[0] === "issue" && args[1] === "view") {
        // 親 Issue にまだ needs-human がある → 解除不可
        return { stdout: JSON.stringify({ labels: [{ name: "needs-human" }] }), exit: 0 };
      }
      return { stdout: "", exit: 0 };
    };
    const r = await clearRetiredIfHumanReleased("test-adr", 123, ghFn);
    expect(r.cleared).toBe(false);
    expect(r.reason).toBe("needs_human_still_present");
    // 子検索 list call すら無い
    expect(calls.filter(c => c[0] === "issue" && c[1] === "list")).toEqual([]);
  });
});
