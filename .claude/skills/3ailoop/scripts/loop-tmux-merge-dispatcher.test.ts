// loop-tmux-merge-dispatcher.test.ts — Phase D-1 直列マージ処理の単体テスト
//
// 検証観点:
// - serial: 2 件を run-once ×2 で処理して両方通る
// - rebase-conflict: rebase 失敗時に worker が error、abortRebase が呼ばれ、CI/push は呼ばれない
// - ci-red: CI 赤時に retry label が付き、push が呼ばれず、worker が error
// - push-success: 全通過で push + close + release が呼ばれる
// - enqueue → parseQueueLine の round-trip
// - empty queue: --run-once 相当 (runOnce) が null 即返し
//
// 副作用の隔離: real git/gh/tmux を叩かず、MergeDeps を record 型 mock で全て差し替える。

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  enqueueMerge,
  parseQueueLine,
  processEntry,
  readQueue,
  runOnce,
  type MergeDeps,
  type MergeQueueEntry,
  type MergeResult,
} from "./loop-tmux-merge-dispatcher";

let workDir: string;
let queuePath: string;

beforeEach(() => {
  workDir = mkdtempSync(join(tmpdir(), "loop-merge-test-"));
  queuePath = join(workDir, "merge-queue.jsonl");
});

afterEach(() => {
  if (existsSync(workDir)) rmSync(workDir, { recursive: true, force: true });
});

interface Call {
  fn: string;
  args: unknown[];
}

/** 全 dep 呼び出しを配列に record する mock ビルダー。 */
function buildDeps(overrides: Partial<MergeDeps> = {}): { deps: MergeDeps; calls: Call[]; nowRef: { t: number } } {
  const calls: Call[] = [];
  const nowRef = { t: 1_700_000_000_000 };
  const base: MergeDeps = {
    rebase: async (worktree) => { calls.push({ fn: "rebase", args: [worktree] }); return { ok: true, stderr: "" }; },
    runCi: async (worktree) => { calls.push({ fn: "runCi", args: [worktree] }); return { ok: true, stderr: "" }; },
    push: async (worktree) => { calls.push({ fn: "push", args: [worktree] }); return { ok: true, stderr: "" }; },
    headSha: async (worktree) => { calls.push({ fn: "headSha", args: [worktree] }); return "abc123"; },
    closeIssueIfOpen: async (issue) => { calls.push({ fn: "closeIssueIfOpen", args: [issue] }); return true; },
    addRetryLabel: async (issue) => { calls.push({ fn: "addRetryLabel", args: [issue] }); },
    releaseWorker: async (id) => { calls.push({ fn: "releaseWorker", args: [id] }); },
    markWorkerError: async (id, reason) => { calls.push({ fn: "markWorkerError", args: [id, reason] }); },
    notify: async (kind, text) => { calls.push({ fn: "notify", args: [kind, text] }); },
    abortRebase: async (worktree) => { calls.push({ fn: "abortRebase", args: [worktree] }); },
    now: () => { nowRef.t += 10; return nowRef.t; },
  };
  return { deps: { ...base, ...overrides }, calls, nowRef };
}

function mkEntry(overrides: Partial<MergeQueueEntry> = {}): MergeQueueEntry {
  return {
    issue: 295,
    worker_id: "worker-1",
    worktree: "/tmp/w1",
    commit_sha: "cafef00d",
    closes_issue: true,
    enqueued_at: "2026-07-24T11:00:00Z",
    ...overrides,
  };
}

// -----------------------------------------------------------------------------
// T04_merge_dispatcher_serial
// -----------------------------------------------------------------------------

describe("T04_merge_dispatcher_serial", () => {
  test("2 entry を enqueue → runOnce ×2 で両方が順に success で処理される", async () => {
    enqueueMerge(mkEntry({ issue: 295, worker_id: "worker-1" }), queuePath);
    enqueueMerge(mkEntry({ issue: 296, worker_id: "worker-2", worktree: "/tmp/w2" }), queuePath);

    const results: MergeResult[] = [];
    const { deps: deps1 } = buildDeps();
    const r1 = await runOnce(queuePath, deps1);
    results.push(r1!);
    expect(r1?.outcome).toBe("success");
    expect(r1?.issue).toBe(295); // fifo 順

    // queue から先頭が popped
    expect(readQueue(queuePath).map(e => e.issue)).toEqual([296]);

    const { deps: deps2 } = buildDeps();
    const r2 = await runOnce(queuePath, deps2);
    results.push(r2!);
    expect(r2?.outcome).toBe("success");
    expect(r2?.issue).toBe(296);

    // 全部処理して空
    expect(readQueue(queuePath)).toEqual([]);
  });
});

// -----------------------------------------------------------------------------
// T05_merge_dispatcher_rebase_conflict
// -----------------------------------------------------------------------------

describe("T05_merge_dispatcher_rebase_conflict", () => {
  test("rebase 失敗 → outcome=rebase-conflict、abortRebase / markError / notify が呼ばれ、CI/push はスキップ", async () => {
    const { deps, calls } = buildDeps({
      rebase: async () => ({ ok: false, stderr: "CONFLICT (content): Merge conflict in src/foo.rs" }),
    });
    const r = await processEntry(mkEntry(), deps);

    expect(r.outcome).toBe("rebase-conflict");
    expect(r.detail).toMatch(/CONFLICT/);

    const fns = calls.map(c => c.fn);
    expect(fns).toContain("abortRebase");
    expect(fns).toContain("markWorkerError");
    expect(fns).toContain("notify");
    // CI / push は呼ばれていない
    expect(fns).not.toContain("runCi");
    expect(fns).not.toContain("push");
    expect(fns).not.toContain("closeIssueIfOpen");
    expect(fns).not.toContain("releaseWorker");
  });

  test("rebase 衝突後は runOnce レベルでも queue から pop されて同一 run 内で再試行されない", async () => {
    const { deps } = buildDeps({
      rebase: async () => ({ ok: false, stderr: "conflict" }),
    });
    enqueueMerge(mkEntry({ issue: 400 }), queuePath);
    const r = await runOnce(queuePath, deps);
    expect(r?.outcome).toBe("rebase-conflict");
    expect(readQueue(queuePath)).toEqual([]); // popped, not retried
  });
});

// -----------------------------------------------------------------------------
// T_bonus_ci_red
// -----------------------------------------------------------------------------

describe("T_bonus_ci_red", () => {
  test("CI 赤 → retry:rebase-conflict label が付与され、push はスキップ、worker は error", async () => {
    const { deps, calls } = buildDeps({
      runCi: async () => ({ ok: false, stderr: "test failed: some assertion" }),
    });
    const r = await processEntry(mkEntry({ issue: 501 }), deps);

    expect(r.outcome).toBe("ci-red");
    const labelCalls = calls.filter(c => c.fn === "addRetryLabel");
    expect(labelCalls.length).toBe(1);
    expect(labelCalls[0].args[0]).toBe(501);

    const fns = calls.map(c => c.fn);
    expect(fns).not.toContain("push");
    expect(fns).toContain("markWorkerError");
  });
});

// -----------------------------------------------------------------------------
// T_bonus_push_success
// -----------------------------------------------------------------------------

describe("T_bonus_push_success", () => {
  test("全 green → rebase → CI → push → close → release の順で呼ばれる", async () => {
    let step = 0;
    const seq: string[] = [];
    const { deps, calls } = buildDeps({
      rebase: async () => { seq.push(`rebase:${++step}`); return { ok: true, stderr: "" }; },
      runCi: async () => { seq.push(`runCi:${++step}`); return { ok: true, stderr: "" }; },
      push: async () => { seq.push(`push:${++step}`); return { ok: true, stderr: "" }; },
      closeIssueIfOpen: async () => { seq.push(`close:${++step}`); return true; },
      releaseWorker: async () => { seq.push(`release:${++step}`); },
    });
    const r = await processEntry(mkEntry({ issue: 700, closes_issue: true }), deps);

    expect(r.outcome).toBe("success");
    // 順序検証
    expect(seq[0]).toMatch(/^rebase:/);
    expect(seq[1]).toMatch(/^runCi:/);
    expect(seq[2]).toMatch(/^push:/);
    expect(seq[3]).toMatch(/^close:/);
    expect(seq[4]).toMatch(/^release:/);

    // markError / addRetryLabel / abortRebase は一切呼ばれない
    const fns = calls.map(c => c.fn);
    expect(fns).not.toContain("markWorkerError");
    expect(fns).not.toContain("addRetryLabel");
    expect(fns).not.toContain("abortRebase");
  });

  test("closes_issue=false なら closeIssueIfOpen は呼ばれない", async () => {
    const { deps, calls } = buildDeps();
    const r = await processEntry(mkEntry({ closes_issue: false }), deps);
    expect(r.outcome).toBe("success");
    const fns = calls.map(c => c.fn);
    expect(fns).not.toContain("closeIssueIfOpen");
    expect(fns).toContain("releaseWorker"); // release はそれでも呼ぶ
  });
});

// -----------------------------------------------------------------------------
// T_bonus_enqueue (round-trip)
// -----------------------------------------------------------------------------

describe("T_bonus_enqueue", () => {
  test("enqueueMerge → readQueue で往復同一", () => {
    const e1 = mkEntry({ issue: 800 });
    const e2 = mkEntry({ issue: 801, worker_id: "worker-2", closes_issue: false });
    enqueueMerge(e1, queuePath);
    enqueueMerge(e2, queuePath);

    const loaded = readQueue(queuePath);
    expect(loaded.length).toBe(2);
    expect(loaded[0]).toEqual(e1);
    expect(loaded[1]).toEqual(e2);
  });

  test("parseQueueLine は 1 行 JSON を正しく復元する", () => {
    const e = mkEntry({ issue: 999 });
    const raw = JSON.stringify(e);
    const parsed = parseQueueLine(raw);
    expect(parsed).toEqual(e);
  });

  test("parseQueueLine は必須フィールド欠如で throw", () => {
    expect(() => parseQueueLine(`{"issue":1}`)).toThrow(/worker_id/);
    expect(() => parseQueueLine(`{"issue":"not-a-number","worker_id":"x","worktree":"y","commit_sha":"z","closes_issue":true,"enqueued_at":"now"}`))
      .toThrow(/issue/);
    expect(() => parseQueueLine(`{"issue":1,"worker_id":"x","worktree":"y","commit_sha":"z","closes_issue":"yes","enqueued_at":"now"}`))
      .toThrow(/closes_issue/);
  });

  test("破損行を含む queue は skip して valid 行だけ返る", () => {
    // 1 行目 valid、2 行目 broken、3 行目 valid
    writeFileSync(queuePath, [
      JSON.stringify(mkEntry({ issue: 1 })),
      "{ not valid",
      JSON.stringify(mkEntry({ issue: 2 })),
      "",
    ].join("\n"), "utf-8");
    const loaded = readQueue(queuePath);
    expect(loaded.map(e => e.issue)).toEqual([1, 2]);
  });
});

// -----------------------------------------------------------------------------
// T_bonus_empty_queue
// -----------------------------------------------------------------------------

describe("T_bonus_empty_queue", () => {
  test("queue 不在 → runOnce は null 即返し", async () => {
    const { deps, calls } = buildDeps();
    const r = await runOnce(queuePath, deps);
    expect(r).toBeNull();
    expect(calls).toEqual([]); // どの副作用も走らない
  });

  test("queue が空ファイル → runOnce は null", async () => {
    writeFileSync(queuePath, "", "utf-8");
    const { deps } = buildDeps();
    const r = await runOnce(queuePath, deps);
    expect(r).toBeNull();
  });
});

// -----------------------------------------------------------------------------
// T_bonus_headSha_failure (worktree 破損)
// -----------------------------------------------------------------------------

describe("T_bonus_headSha_failure", () => {
  test("headSha が throw → outcome=internal-error、rebase/CI/push は呼ばれない", async () => {
    const { deps, calls } = buildDeps({
      headSha: async () => { throw new Error("fatal: not a git repo"); },
    });
    const r = await processEntry(mkEntry(), deps);
    expect(r.outcome).toBe("internal-error");
    const fns = calls.map(c => c.fn);
    expect(fns).not.toContain("rebase");
    expect(fns).not.toContain("runCi");
    expect(fns).toContain("markWorkerError");
    expect(fns).toContain("notify");
  });
});
