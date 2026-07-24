// loop-phase-retrospective.test.ts — Issue #315
import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  type AllMetrics,
  type Cycle,
  type PhaseRange,
  type SkipEntry,
  computeCodexSkipMetric,
  computeNeedsHumanMetric,
  computePauseMetric,
  computeSkillImprovementMetric,
  filterCyclesByPhase,
  parsePhaseRange,
  renderRetroIssues,
} from "./loop-phase-retrospective.ts";

const SCRIPT = join(import.meta.dir, "loop-phase-retrospective.ts");

// -------- helpers ------------------------------------------------------------

function makeCycle(n: number, opts: {
  startedAt?: string;
  endedAt?: string;
  paused?: boolean;
  pauseReason?: string;
  closed?: number[];
  raised?: number[];
  merged?: number;
} = {}): Cycle {
  const started = opts.startedAt ?? `2026-06-${String(n + 15).padStart(2, "0")}T00:00:00.000Z`;
  const ended = opts.endedAt ?? `2026-06-${String(n + 15).padStart(2, "0")}T01:00:00.000Z`;
  const c: Cycle = {
    cycle: n,
    started_at: started,
    ended_at: ended,
    closed: opts.closed ?? [],
    raised: opts.raised ?? [],
    merged_commits: opts.merged ?? 0,
    adr_drafts: [],
  };
  if (opts.paused || opts.pauseReason) {
    c.pause_reason = opts.pauseReason ?? "some pause";
  }
  return c;
}

function writeTempFiles(files: Record<string, string>): string {
  const dir = mkdtempSync(join(tmpdir(), "retro-test-"));
  for (const [rel, content] of Object.entries(files)) {
    const full = join(dir, rel);
    mkdirSync(join(dir, rel, ".."), { recursive: true });
    writeFileSync(full, content, "utf-8");
  }
  return dir;
}

function baselineMetrics(overrides: Partial<AllMetrics> = {}): AllMetrics {
  const base: AllMetrics = {
    phase: 10,
    range: { startTime: null, endTime: null },
    cycle_count: 10,
    pause: { total: 10, paused: 0, rate: 0, top_reasons: [] },
    needs_human: { count: 0, issues: [] },
    codex_skip: { created: 0, resolved: 0, pending: 0 },
    skill_improvement: {
      skill_changes: 0,
      mean_cycle_before_ms: 0,
      mean_cycle_after_ms: 0,
      delta_ratio: 0,
      first_skill_change_at: null,
    },
  };
  return { ...base, ...overrides };
}

// -------- parsePhaseRange / filterCyclesByPhase ------------------------------

describe("parsePhaseRange", () => {
  test("Phase N と N-1 の phase-transition entry を拾う", () => {
    const log = [
      JSON.stringify({ at: "2026-06-10T00:00:00.000Z", kind: "phase-transition", message: "Phase 8 完了" }),
      JSON.stringify({ at: "2026-06-22T00:00:00.000Z", kind: "phase-transition", message: "Phase 9 完了 (milestone ...)" }),
      JSON.stringify({ at: "2026-07-30T00:00:00.000Z", kind: "phase-transition", message: "Phase 10 完了" }),
    ].join("\n");
    const range = parsePhaseRange(log, 10);
    expect(range.startTime).toBe("2026-06-22T00:00:00.000Z");
    expect(range.endTime).toBe("2026-07-30T00:00:00.000Z");
  });

  test("phase 完了 entry が無い場合 endTime=null (in-progress)", () => {
    const log = JSON.stringify({
      at: "2026-06-22T00:00:00.000Z", kind: "phase-transition", message: "Phase 9 完了",
    });
    const range = parsePhaseRange(log, 10);
    expect(range.startTime).toBe("2026-06-22T00:00:00.000Z");
    expect(range.endTime).toBeNull();
  });

  test("空 log は startTime/endTime ともに null", () => {
    const range = parsePhaseRange("", 10);
    expect(range).toEqual({ startTime: null, endTime: null });
  });

  test("ROADMAP に Phase N が無ければ range={null,null}", () => {
    const log = JSON.stringify({ at: "2026-06-22T00:00:00.000Z", kind: "phase-transition", message: "Phase 9 完了" });
    const roadmap = "## Phase 9: foo ✅\n## Phase 10: bar\n";
    const range = parsePhaseRange(log, 99, roadmap);
    expect(range).toEqual({ startTime: null, endTime: null });
  });
});

describe("filterCyclesByPhase", () => {
  const cycles: Cycle[] = [
    makeCycle(1, { endedAt: "2026-06-20T00:00:00.000Z" }),
    makeCycle(2, { endedAt: "2026-06-25T00:00:00.000Z" }),
    makeCycle(3, { endedAt: "2026-07-05T00:00:00.000Z" }),
    makeCycle(4, { endedAt: "2026-08-01T00:00:00.000Z" }),
  ];
  test("startTime/endTime で境界 filter", () => {
    const range: PhaseRange = { startTime: "2026-06-22T00:00:00.000Z", endTime: "2026-07-30T00:00:00.000Z" };
    const out = filterCyclesByPhase(cycles, range);
    expect(out.map(c => c.cycle)).toEqual([2, 3]);
  });
  test("startTime=null → 上限のみで filter", () => {
    const out = filterCyclesByPhase(cycles, { startTime: null, endTime: "2026-07-01T00:00:00.000Z" });
    expect(out.map(c => c.cycle)).toEqual([1, 2]);
  });
  test("endTime=null → 下限のみで filter (in-progress phase)", () => {
    const out = filterCyclesByPhase(cycles, { startTime: "2026-06-22T00:00:00.000Z", endTime: null });
    expect(out.map(c => c.cycle)).toEqual([2, 3, 4]);
  });
});

// -------- computeXxxMetric ---------------------------------------------------

describe("computePauseMetric", () => {
  test("空 → total=0, rate=0", () => {
    const m = computePauseMetric([]);
    expect(m).toEqual({ total: 0, paused: 0, rate: 0, top_reasons: [] });
  });
  test("categorization: codex-usage-limit と adr-not-finalized", () => {
    const cycles: Cycle[] = [
      makeCycle(1, { pauseReason: "cycle 32: codex usage limit で empty" }),
      makeCycle(2, { pauseReason: "cycle 33: codex usage limit again" }),
      makeCycle(3, { pauseReason: "ADR-017 gate:adr-review 未 finalize" }),
      makeCycle(4),
    ];
    const m = computePauseMetric(cycles);
    expect(m.total).toBe(4);
    expect(m.paused).toBe(3);
    expect(m.rate).toBeCloseTo(0.75, 2);
    expect(m.top_reasons[0]).toContain("codex-usage-limit(2)");
  });
});

describe("computeNeedsHumanMetric", () => {
  test("labels map から needs-human を持つ Issue だけ数える", () => {
    const raised = [100, 101, 102, 103];
    const labels = new Map<number, string[]>([
      [100, ["type: feature"]],
      [101, ["needs-human", "type: foundation"]],
      [102, ["needs-human"]],
      [103, []],
    ]);
    const m = computeNeedsHumanMetric(raised, labels);
    expect(m.count).toBe(2);
    expect(m.issues).toEqual([101, 102]);
  });
});

describe("computeCodexSkipMetric", () => {
  test("recorded_at が range 内のものだけ数え、resolved / pending を分ける", () => {
    const skips: SkipEntry[] = [
      { issue: 1, slug: "a", step: "7.5", reason: "x", recorded_at: "2026-05-01T00:00:00.000Z", resolved_at: null },
      { issue: 2, slug: "b", step: "7.5", reason: "x", recorded_at: "2026-06-25T00:00:00.000Z", resolved_at: null },
      { issue: 3, slug: "c", step: "7.5", reason: "x", recorded_at: "2026-06-26T00:00:00.000Z", resolved_at: "2026-06-27T00:00:00.000Z" },
      { issue: 4, slug: "d", step: "7.5", reason: "x", recorded_at: "2026-08-01T00:00:00.000Z", resolved_at: null },
    ];
    const range: PhaseRange = { startTime: "2026-06-22T00:00:00.000Z", endTime: "2026-07-30T00:00:00.000Z" };
    const m = computeCodexSkipMetric(skips, range);
    expect(m.created).toBe(2);
    expect(m.resolved).toBe(1);
    expect(m.pending).toBe(1);
  });
});

describe("computeSkillImprovementMetric", () => {
  test("skill 改修コミット 0 件 → skill_changes=0, delta_ratio=0", () => {
    const cycles: Cycle[] = [makeCycle(1)];
    const m = computeSkillImprovementMetric("", cycles);
    expect(m.skill_changes).toBe(0);
    expect(m.delta_ratio).toBe(0);
  });

  test("改修コミット前後で cycle 平均時間を比較 (worse → delta > 0)", () => {
    // 改修コミット at 2026-06-25 (unix秒 = 1780358400)
    const cycles: Cycle[] = [
      makeCycle(1, {
        startedAt: "2026-06-20T00:00:00.000Z", endedAt: "2026-06-20T01:00:00.000Z", // 1h
      }),
      makeCycle(2, {
        startedAt: "2026-06-21T00:00:00.000Z", endedAt: "2026-06-21T01:00:00.000Z", // 1h
      }),
      makeCycle(3, {
        startedAt: "2026-06-26T00:00:00.000Z", endedAt: "2026-06-26T03:00:00.000Z", // 3h
      }),
      makeCycle(4, {
        startedAt: "2026-06-27T00:00:00.000Z", endedAt: "2026-06-27T03:00:00.000Z", // 3h
      }),
    ];
    const boundary = Math.floor(new Date("2026-06-25T00:00:00.000Z").getTime() / 1000);
    const gitLog = `abc123 ${boundary}\n.claude/skills/3ailoop/scripts/loop-cycle-record.ts\n`;
    const m = computeSkillImprovementMetric(gitLog, cycles);
    expect(m.skill_changes).toBe(1);
    expect(m.mean_cycle_before_ms).toBe(3600 * 1000);
    expect(m.mean_cycle_after_ms).toBe(3 * 3600 * 1000);
    expect(m.delta_ratio).toBeCloseTo(2.0, 2);
  });
});

// -------- renderRetroIssues ---------------------------------------------------

describe("renderRetroIssues", () => {
  test("T_bonus_baseline: 全 metric clean でも「概要」Issue は必ず 1 件出る", () => {
    const m = baselineMetrics();
    const drafts = renderRetroIssues(m, 10);
    expect(drafts.length).toBe(1);
    expect(drafts[0].topic).toBe("baseline");
    expect(drafts[0].title).toBe("retro(phase10): 概要");
    expect(drafts[0].labels).toContain("type: foundation");
    expect(drafts[0].labels).toContain("batch:skill");
  });

  test("T_bonus_pause_low: 10 cycles, 1 paused → pause Issue は出ない", () => {
    const m = baselineMetrics({
      pause: { total: 10, paused: 1, rate: 0.1, top_reasons: ["codex-usage-limit(1)"] },
    });
    const drafts = renderRetroIssues(m, 10);
    expect(drafts.length).toBe(1);
    expect(drafts.some(d => d.topic === "pause")).toBe(false);
  });

  test("T_bonus_pause_high: 10 cycles, 5 paused → pause Issue が出る", () => {
    const m = baselineMetrics({
      pause: { total: 10, paused: 5, rate: 0.5, top_reasons: ["codex-usage-limit(3)", "adr-not-finalized(2)"] },
    });
    const drafts = renderRetroIssues(m, 10);
    expect(drafts.length).toBe(2);
    const pauseDraft = drafts.find(d => d.topic === "pause");
    expect(pauseDraft).toBeDefined();
    expect(pauseDraft!.title).toContain("pause 率が高い");
    expect(pauseDraft!.title).toContain("50%");
  });

  test("T_bonus_needs_human_batch: needs-human 3 件以上 → Issue が出る", () => {
    const m = baselineMetrics({
      needs_human: { count: 3, issues: [201, 202, 203] },
    });
    const drafts = renderRetroIssues(m, 10);
    const nh = drafts.find(d => d.topic === "needs-human");
    expect(nh).toBeDefined();
    expect(nh!.body).toContain("#201");
    expect(nh!.body).toContain("#202");
    expect(nh!.body).toContain("#203");
  });

  test("T_bonus_skill_improvement: mean before < after → Issue が出る", () => {
    const m = baselineMetrics({
      skill_improvement: {
        skill_changes: 2,
        mean_cycle_before_ms: 60_000,
        mean_cycle_after_ms: 120_000,
        delta_ratio: 1.0,
        first_skill_change_at: "2026-06-25T00:00:00.000Z",
      },
    });
    const drafts = renderRetroIssues(m, 10);
    const si = drafts.find(d => d.topic === "skill-improvement");
    expect(si).toBeDefined();
    expect(si!.title).toContain("Skill 改修後に cycle 時間が悪化");
  });

  test("T_bonus_skill_improvement: mean before >= after → Issue は出ない (改善 or 同等)", () => {
    const m = baselineMetrics({
      skill_improvement: {
        skill_changes: 2,
        mean_cycle_before_ms: 120_000,
        mean_cycle_after_ms: 60_000,
        delta_ratio: -0.5,
        first_skill_change_at: "2026-06-25T00:00:00.000Z",
      },
    });
    const drafts = renderRetroIssues(m, 10);
    expect(drafts.some(d => d.topic === "skill-improvement")).toBe(false);
  });

  test("T_bonus_cap_5: 4 red flags 全部 trigger でも 概要 + 4 topics で 5 件 (cap 遵守)", () => {
    const m = baselineMetrics({
      pause: { total: 10, paused: 8, rate: 0.8, top_reasons: ["codex-usage-limit(8)"] },
      needs_human: { count: 5, issues: [1, 2, 3, 4, 5] },
      codex_skip: { created: 10, resolved: 2, pending: 8 },
      skill_improvement: {
        skill_changes: 3,
        mean_cycle_before_ms: 60_000,
        mean_cycle_after_ms: 180_000,
        delta_ratio: 2.0,
        first_skill_change_at: "2026-06-25T00:00:00.000Z",
      },
    });
    const drafts = renderRetroIssues(m, 10);
    expect(drafts.length).toBe(5);
    const topics = drafts.map(d => d.topic).sort();
    expect(topics).toEqual(["baseline", "codex-skip", "needs-human", "pause", "skill-improvement"].sort());
  });

  test("codex-skip pending < threshold → Issue は出ない", () => {
    const m = baselineMetrics({
      codex_skip: { created: 5, resolved: 2, pending: 3 },
    });
    const drafts = renderRetroIssues(m, 10);
    expect(drafts.some(d => d.topic === "codex-skip")).toBe(false);
  });
});

// -------- CLI end-to-end (subprocess) ---------------------------------------

describe("CLI", () => {
  test("T04_retro_dry_run: 10 cycles, 3 paused → dry-run で drafts が stdout に出る", () => {
    const journal = [
      { cycle: 1, started_at: "2026-06-23T00:00:00.000Z", ended_at: "2026-06-23T01:00:00.000Z", closed: [301], raised: [], merged_commits: 1, adr_drafts: [] },
      { cycle: 2, started_at: "2026-06-23T02:00:00.000Z", ended_at: "2026-06-23T03:00:00.000Z", closed: [], raised: [], merged_commits: 0, adr_drafts: [], pause_reason: "codex usage limit" },
      { cycle: 3, started_at: "2026-06-23T04:00:00.000Z", ended_at: "2026-06-23T05:00:00.000Z", closed: [302], raised: [], merged_commits: 2, adr_drafts: [] },
      { cycle: 4, started_at: "2026-06-24T00:00:00.000Z", ended_at: "2026-06-24T01:00:00.000Z", closed: [], raised: [], merged_commits: 0, adr_drafts: [], pause_reason: "batch-select returned 0 candidates" },
      { cycle: 5, started_at: "2026-06-24T02:00:00.000Z", ended_at: "2026-06-24T03:00:00.000Z", closed: [303], raised: [], merged_commits: 1, adr_drafts: [] },
      { cycle: 6, started_at: "2026-06-24T04:00:00.000Z", ended_at: "2026-06-24T05:00:00.000Z", closed: [304], raised: [], merged_commits: 1, adr_drafts: [] },
      { cycle: 7, started_at: "2026-06-24T06:00:00.000Z", ended_at: "2026-06-24T07:00:00.000Z", closed: [305], raised: [], merged_commits: 1, adr_drafts: [] },
      { cycle: 8, started_at: "2026-06-24T08:00:00.000Z", ended_at: "2026-06-24T09:00:00.000Z", closed: [], raised: [], merged_commits: 0, adr_drafts: [], pause_reason: "ADR-017 not finalized" },
      { cycle: 9, started_at: "2026-06-24T10:00:00.000Z", ended_at: "2026-06-24T11:00:00.000Z", closed: [306], raised: [], merged_commits: 1, adr_drafts: [] },
      { cycle: 10, started_at: "2026-06-24T12:00:00.000Z", ended_at: "2026-06-24T13:00:00.000Z", closed: [307], raised: [], merged_commits: 1, adr_drafts: [] },
    ].map(o => JSON.stringify(o)).join("\n");
    const decisions = [
      { at: "2026-06-22T00:00:00.000Z", kind: "phase-transition", message: "Phase 9 完了" },
      { at: "2026-06-30T00:00:00.000Z", kind: "phase-transition", message: "Phase 10 完了" },
    ].map(o => JSON.stringify(o)).join("\n");
    const dir = writeTempFiles({
      "journal.log": journal,
      "decisions.jsonl": decisions,
      "skips.jsonl": "",
      "ROADMAP.md": "## Phase 9: foo ✅\n## Phase 10: bar\n",
    });

    const r = Bun.spawnSync(
      [
        "bun", SCRIPT,
        "--phase", "10",
        "--dry-run",
        "--journal", join(dir, "journal.log"),
        "--decisions", join(dir, "decisions.jsonl"),
        "--skips", join(dir, "skips.jsonl"),
        "--roadmap", join(dir, "ROADMAP.md"),
      ],
      { stdout: "pipe", stderr: "pipe" },
    );
    expect(r.exitCode).toBe(0);
    const out = new TextDecoder().decode(r.stdout);
    // 概要 Issue は必ず出る
    expect(out).toContain("[DRAFT] retro(phase10): 概要");
    // pause は 3/10 = 30% (境界上、threshold は > 0.30 なので pause Issue は出ない)
    expect(out).not.toContain("pause 率が高い");
  });

  test("T05_retro_empty: 空 journal + 空 decisions log → exit 2", () => {
    const dir = writeTempFiles({
      "journal.log": "",
      "decisions.jsonl": "",
      "skips.jsonl": "",
    });
    const r = Bun.spawnSync(
      [
        "bun", SCRIPT,
        "--phase", "10",
        "--dry-run",
        "--journal", join(dir, "journal.log"),
        "--decisions", join(dir, "decisions.jsonl"),
        "--skips", join(dir, "skips.jsonl"),
      ],
      { stdout: "pipe", stderr: "pipe" },
    );
    expect(r.exitCode).toBe(2);
    const err = new TextDecoder().decode(r.stderr);
    expect(err).toContain("no cycles for phase");
  });

  test("--phase 未指定 → exit 2 with Usage", () => {
    const r = Bun.spawnSync(["bun", SCRIPT, "--dry-run"], { stdout: "pipe", stderr: "pipe" });
    expect(r.exitCode).toBe(2);
    const err = new TextDecoder().decode(r.stderr);
    expect(err).toContain("Usage");
  });
});
