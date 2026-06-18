// loop-decision-log.ts: JSONL 併記 + stats subcommand テスト (#232)

import { describe, expect, test, beforeEach, afterAll } from "bun:test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  appendEntry,
  computeStats,
  readJsonlRows,
  VALID_KINDS,
  type DecisionRow,
} from "./loop-decision-log.ts";

const TMP_BASE = join(tmpdir(), `loop-decision-log-test-${process.pid}`);

function freshTmpRoot(label: string): string {
  const dir = join(TMP_BASE, label);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  process.env.DECISION_LOG_DIR = dir;
  return dir;
}

afterAll(() => {
  rmSync(TMP_BASE, { recursive: true, force: true });
  delete process.env.DECISION_LOG_DIR;
});

describe("loop-decision-log appendEntry", () => {
  test("T01 append basic: kind + message のみ → md と jsonl 両方に記録", () => {
    const dir = freshTmpRoot("t01");
    appendEntry({ kind: "adr-draft", message: "ADR-099 draft" });
    expect(existsSync(`${dir}/decisions.log.md`)).toBe(true);
    expect(existsSync(`${dir}/decisions.log.jsonl`)).toBe(true);
    const md = readFileSync(`${dir}/decisions.log.md`, "utf-8");
    expect(md).toContain("[adr-draft] ADR-099 draft");
    const rows = readJsonlRows();
    expect(rows).toHaveLength(1);
    expect(rows[0].kind).toBe("adr-draft");
    expect(rows[0].message).toBe("ADR-099 draft");
    expect(rows[0].cycle).toBe(null);
    expect(rows[0].root_issue).toBe(null);
    expect(rows[0].time_min).toBe(null);
    expect(rows[0].token_delta).toBe(null);
    expect(rows[0].blocked_on).toEqual([]);
    expect(typeof rows[0].at).toBe("string");
  });

  test("T02 append full: 全 field 渡し → jsonl 行に全 field 反映", () => {
    freshTmpRoot("t02");
    appendEntry({
      kind: "failure-escape",
      message: "#220 needs-human 退避",
      cycle: 26,
      rootIssue: 220,
      timeMin: 45,
      tokenDelta: { claude: 1000, glm: 50000, codex: 3000 },
      blockedOn: ["220", "gate:adr-review"],
    });
    const rows = readJsonlRows();
    expect(rows[0]).toMatchObject({
      kind: "failure-escape",
      message: "#220 needs-human 退避",
      cycle: 26,
      root_issue: 220,
      time_min: 45,
      token_delta: { claude: 1000, glm: 50000, codex: 3000 },
      blocked_on: ["220", "gate:adr-review"],
    });
  });

  test("T03 backward-compat: 旧 kind (needs-human, phase-transition) → エラーなし", () => {
    freshTmpRoot("t03");
    appendEntry({ kind: "needs-human", message: "退避" });
    appendEntry({ kind: "phase-transition", message: "Phase 8 完了" });
    const rows = readJsonlRows();
    expect(rows).toHaveLength(2);
    expect(rows.map(r => r.kind).sort()).toEqual(["needs-human", "phase-transition"]);
  });

  test("T04 invalid kind: 未知 kind → throw", () => {
    freshTmpRoot("t04");
    expect(() => appendEntry({ kind: "totally-invalid", message: "x" })).toThrow(/invalid kind/);
  });
});

describe("loop-decision-log computeStats", () => {
  function row(overrides: Partial<DecisionRow>): DecisionRow {
    return {
      at: "2026-06-18T00:00:00.000Z",
      cycle: null,
      kind: "other",
      root_issue: null,
      time_min: null,
      token_delta: null,
      blocked_on: [],
      message: "x",
      ...overrides,
    };
  }

  test("T05 kind distribution: 異なる kind 5 件 → 集計が正しい", () => {
    const rows = [
      row({ kind: "failure-escape" }),
      row({ kind: "failure-escape" }),
      row({ kind: "adr-draft" }),
      row({ kind: "other" }),
      row({ kind: "other" }),
    ];
    const stats = computeStats(rows);
    expect(stats.total_rows).toBe(5);
    const dist = Object.fromEntries(stats.kind_distribution.map(k => [k.kind, k.count]));
    expect(dist).toEqual({ "failure-escape": 2, "adr-draft": 1, other: 2 });
    expect(stats.kind_distribution[0].count).toBeGreaterThanOrEqual(stats.kind_distribution[stats.kind_distribution.length - 1].count);
  });

  test("T06 time avg: time_min 付き 3 件 (10, 20, 30) → avg=20, sum=60", () => {
    const rows = [
      row({ time_min: 10 }),
      row({ time_min: 20 }),
      row({ time_min: 30 }),
      row({ time_min: null }),
    ];
    const stats = computeStats(rows);
    expect(stats.time_min.avg).toBe(20);
    expect(stats.time_min.sum).toBe(60);
    expect(stats.time_min.reported_count).toBe(3);
  });

  test("T07 root cause top: #220×3, #206×2 → 正しい順序", () => {
    const rows = [
      row({ root_issue: 220 }),
      row({ root_issue: 220 }),
      row({ root_issue: 220 }),
      row({ root_issue: 206 }),
      row({ root_issue: 206 }),
      row({ root_issue: null }),
    ];
    const stats = computeStats(rows);
    expect(stats.pause_root_cause_top).toEqual([
      { root_issue: 220, count: 3 },
      { root_issue: 206, count: 2 },
    ]);
  });

  test("T08 token sum: token_delta 3 件 → claude/glm/codex の合算", () => {
    const rows = [
      row({ token_delta: { claude: 100, glm: 1000, codex: 50 } }),
      row({ token_delta: { claude: 200, glm: 2000, codex: 0 } }),
      row({ token_delta: { claude: 0, glm: 0, codex: 100 } }),
    ];
    const stats = computeStats(rows);
    expect(stats.token_delta_sum).toEqual({ claude: 300, glm: 3000, codex: 150 });
  });

  test("T09_degen_empty: jsonl 不在 → total_rows=0", () => {
    freshTmpRoot("t09");
    const rows = readJsonlRows();
    expect(rows).toEqual([]);
    const stats = computeStats(rows);
    expect(stats.total_rows).toBe(0);
  });

  test("T10_boundary_last_cycles: cycle 1-5 のうち --last-cycles=2 → cycle 4-5 のみ", () => {
    const rows = [
      row({ cycle: 1, kind: "other" }),
      row({ cycle: 2, kind: "other" }),
      row({ cycle: 3, kind: "adr-draft" }),
      row({ cycle: 4, kind: "failure-escape" }),
      row({ cycle: 5, kind: "failure-escape" }),
    ];
    const stats = computeStats(rows, 2);
    expect(stats.total_rows).toBe(2);
    const dist = Object.fromEntries(stats.kind_distribution.map(k => [k.kind, k.count]));
    expect(dist).toEqual({ "failure-escape": 2 });
  });
});

describe("loop-decision-log VALID_KINDS coverage", () => {
  test("T11: Issue spec の 6 kind + 既存 2 kind = 8 が VALID_KINDS に含まれる", () => {
    const required = [
      "adr-draft",
      "issue-split",
      "failure-escape",
      "phase-close",
      "policy-update",
      "other",
      "needs-human",
      "phase-transition",
    ];
    for (const k of required) {
      expect(VALID_KINDS.has(k)).toBe(true);
    }
    expect(VALID_KINDS.size).toBe(required.length);
  });
});
