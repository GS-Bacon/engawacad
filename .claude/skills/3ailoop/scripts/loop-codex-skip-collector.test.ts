// loop-codex-skip-collector.test.ts — Codex gate スキップ後払いレビュー回収のテスト
//
// 実 Codex / git / gh は呼ばず、CollectorDeps を mock 注入して回収ロジックのみ検証する。

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { appendFileSync, mkdtempSync, rmSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  collectCodexSkips,
  type CollectorDeps,
} from "./loop-codex-skip-collector.ts";
import {
  appendSkip,
  buildSkipEntry,
  readLedger,
  type SkipEntry,
} from "../../3ai/scripts/record-codex-skip.ts";

let workDir: string;
let ledger: string;

beforeEach(() => {
  workDir = mkdtempSync(join(tmpdir(), "codex-skip-collector-"));
  ledger = join(workDir, "codex-skips.jsonl");
});

afterEach(() => {
  rmSync(workDir, { recursive: true, force: true });
});

interface Calls {
  findSha: number[];
  review: number[];
  raise: number[];
  removeLabel: number[];
}

function makeDeps(over: Partial<CollectorDeps> = {}): CollectorDeps & { calls: Calls } {
  const calls: Calls = { findSha: [], review: [], raise: [], removeLabel: [] };
  const base: CollectorDeps = {
    async findMergeSha(issue) {
      calls.findSha.push(issue);
      return `sha-${issue}`;
    },
    async getCommitDiff(sha) {
      return `diff for ${sha}`;
    },
    async getIssueMeta(issue) {
      return { title: `title ${issue}`, body: `body ${issue}`, batch: "batch:kernel" };
    },
    async reviewCodex({ entry }) {
      calls.review.push(entry.issue);
      return { ok: true, usageLimit: false, blocking: 0, findings: "clean" };
    },
    async raiseBlockingIssue({ entry }) {
      calls.raise.push(entry.issue);
      return 9000 + entry.issue;
    },
    async removeDeferredLabel(issue) {
      calls.removeLabel.push(issue);
    },
    now: () => new Date("2026-07-06T00:00:00.000Z"),
  };
  return Object.assign({ calls }, base, over);
}

function seed(issue: number, step: "3.5" | "7.5", isoDay: string): SkipEntry {
  const e = buildSkipEntry(issue, `slug-${issue}`, step, "usage limit", new Date(isoDay));
  appendSkip(e, ledger);
  return e;
}

describe("loop-codex-skip-collector", () => {
  test("append → unresolved 抽出 → resolved 化の一連", async () => {
    seed(101, "3.5", "2026-07-01T00:00:00Z");
    const deps = makeDeps();
    const res = await collectCodexSkips({ ledgerPath: ledger, deps });

    expect(res.processed).toBe(1);
    expect(res.resolved).toBe(1);
    expect(res.raised).toBe(0);
    expect(res.aborted).toBe(false);
    expect(res.remaining).toBe(0);

    const after = readLedger(ledger);
    expect(after[0].resolved_at).not.toBeNull();
    expect(after[0].resolution).toBe("reviewed-clean");
    expect(deps.calls.review).toEqual([101]);
    expect(deps.calls.removeLabel).toEqual([101]);
  });

  test("2 件/サイクル上限 (古い順に 2 件だけ処理)", async () => {
    seed(101, "3.5", "2026-07-01T00:00:00Z");
    seed(102, "3.5", "2026-07-02T00:00:00Z");
    seed(103, "7.5", "2026-07-03T00:00:00Z");
    const deps = makeDeps();
    const res = await collectCodexSkips({ ledgerPath: ledger, deps });

    expect(res.processed).toBe(2);
    expect(res.resolved).toBe(2);
    expect(res.remaining).toBe(1);
    // 古い 101/102 が処理され、最新 103 が残る
    expect(deps.calls.review).toEqual([101, 102]);
    const after = readLedger(ledger);
    const unresolved = after.filter((e) => e.resolved_at == null);
    expect(unresolved.map((e) => e.issue)).toEqual([103]);
  });

  test("--max で 1 件/サイクルに絞れる", async () => {
    seed(101, "3.5", "2026-07-01T00:00:00Z");
    seed(102, "3.5", "2026-07-02T00:00:00Z");
    const deps = makeDeps();
    const res = await collectCodexSkips({ ledgerPath: ledger, maxPerCycle: 1, deps });
    expect(res.processed).toBe(1);
    expect(res.remaining).toBe(1);
  });

  test("usage-limit sentinel で中断 (以降のエントリは次サイクルへ)", async () => {
    seed(101, "3.5", "2026-07-01T00:00:00Z");
    seed(102, "3.5", "2026-07-02T00:00:00Z");
    const deps = makeDeps({
      async reviewCodex() {
        return { ok: false, usageLimit: true, blocking: 0, findings: "" };
      },
    });
    const res = await collectCodexSkips({ ledgerPath: ledger, deps });

    expect(res.aborted).toBe(true);
    expect(res.resolved).toBe(0);
    expect(res.processed).toBe(1); // 1 件目で中断
    expect(res.remaining).toBe(2); // 何も resolved しない
    const after = readLedger(ledger);
    expect(after.every((e) => e.resolved_at == null)).toBe(true);
  });

  test("blocking findings で bug Issue 起票 + resolved 化", async () => {
    seed(101, "7.5", "2026-07-01T00:00:00Z");
    const deps = makeDeps({
      async reviewCodex({ entry }) {
        return { ok: true, usageLimit: false, blocking: 2, findings: "verdict: fail\nseverity: critical" };
      },
    });
    const res = await collectCodexSkips({ ledgerPath: ledger, deps });

    expect(res.raised).toBe(1);
    expect(res.resolved).toBe(1);
    expect(deps.calls.raise).toEqual([101]);
    const after = readLedger(ledger);
    expect(after[0].resolution).toMatch(/^blocking-raised:#9101$/);
    expect(after[0].resolved_at).not.toBeNull();
  });

  test("blocking 起票失敗 (throw) 時は resolved にせず次サイクルへ持ち越し", async () => {
    seed(101, "7.5", "2026-07-01T00:00:00Z");
    const deps = makeDeps({
      async reviewCodex() {
        return { ok: true, usageLimit: false, blocking: 1, findings: "bad" };
      },
      async raiseBlockingIssue() {
        throw new Error("gh down");
      },
    });
    const res = await collectCodexSkips({ ledgerPath: ledger, deps });
    expect(res.raised).toBe(0);
    expect(res.resolved).toBe(0);
    expect(res.remaining).toBe(1);
    const after = readLedger(ledger);
    expect(after[0].resolved_at).toBeNull();
  });

  test("非 usage-limit エラーは 1 件失敗で全体を止めない", async () => {
    seed(101, "3.5", "2026-07-01T00:00:00Z");
    seed(102, "7.5", "2026-07-02T00:00:00Z");
    const deps = makeDeps({
      async reviewCodex({ entry }) {
        if (entry.issue === 101)
          return { ok: false, usageLimit: false, blocking: 0, findings: "", error: "codex exit=1" };
        return { ok: true, usageLimit: false, blocking: 0, findings: "clean" };
      },
    });
    const res = await collectCodexSkips({ ledgerPath: ledger, deps });

    expect(res.processed).toBe(2);
    expect(res.resolved).toBe(1); // 102 のみ
    const after = readLedger(ledger);
    const byIssue = Object.fromEntries(after.map((e) => [e.issue, e]));
    expect(byIssue[101].resolved_at).toBeNull(); // 101 は次サイクル再試行
    expect(byIssue[102].resolved_at).not.toBeNull();
  });

  test("破損 JSONL 行を読み飛ばして有効行だけ処理", async () => {
    seed(101, "3.5", "2026-07-01T00:00:00Z");
    appendFileSync(ledger, "{ this is not valid json\n");
    seed(102, "7.5", "2026-07-02T00:00:00Z");

    // readLedger 単体でも破損行を除外する
    expect(readLedger(ledger).length).toBe(2);

    const deps = makeDeps();
    const res = await collectCodexSkips({ ledgerPath: ledger, deps });
    expect(res.processed).toBe(2);
    expect(res.resolved).toBe(2);
  });

  test("no-merge-commit は Codex を呼ばず close 扱い", async () => {
    seed(101, "3.5", "2026-07-01T00:00:00Z");
    const deps = makeDeps({
      async findMergeSha() {
        return null;
      },
    });
    const res = await collectCodexSkips({ ledgerPath: ledger, deps });

    expect(res.resolved).toBe(1);
    expect(res.raised).toBe(0);
    expect(deps.calls.review.length).toBe(0); // レビューは走らない
    const after = readLedger(ledger);
    expect(after[0].resolution).toBe("no-merge-commit");
    expect(after[0].resolved_at).not.toBeNull();
  });

  test("既に resolved のエントリは再処理しない", async () => {
    const done = buildSkipEntry(100, "slug-100", "3.5", "r", new Date("2026-06-30T00:00:00Z"));
    done.resolved_at = "2026-07-01T00:00:00.000Z";
    done.resolution = "reviewed-clean";
    appendSkip(done, ledger);
    seed(101, "3.5", "2026-07-02T00:00:00Z");

    const deps = makeDeps();
    const res = await collectCodexSkips({ ledgerPath: ledger, deps });
    expect(res.processed).toBe(1);
    expect(deps.calls.review).toEqual([101]);
  });

  test("空 / 不在 ledger でも例外を出さず no-op", async () => {
    const deps = makeDeps();
    const res = await collectCodexSkips({ ledgerPath: ledger, deps });
    expect(res.processed).toBe(0);
    expect(res.resolved).toBe(0);
    expect(res.remaining).toBe(0);
  });
});
