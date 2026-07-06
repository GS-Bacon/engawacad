// #253: loop-should-stop の halt 検知ロジックを単体テスト
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  detectPhase21Reached,
  detectHaltSwitch,
  decideStopFromCandidates,
  type IssueSummary,
} from "./loop-should-stop";
import type { BatchIssue, BatchPlan } from "../../3ai/scripts/types.ts";

// --- #310: decideStopFromCandidates 用フィクスチャ ---

function makeCandidate(overrides: Partial<BatchIssue> = {}): BatchIssue {
  return {
    number: 295,
    slug: "sketch-offset",
    title: "feat(kernel): Sketch Offset",
    labels: ["type: feature", "batch:kernel"],
    deliverable: "code",
    flow: "full",
    gate: "auto",
    pause_reasons: [],
    ambiguous: false,
    intent_check_required: true,
    keep_codex_gate: true,
    deps: [],
    raw_refs: [],
    ...overrides,
  };
}

function makePlan(candidates: BatchIssue[], tier: BatchPlan["tier"] = "split-batch"): BatchPlan {
  return {
    generated_at: "2026-07-06T00:00:00.000Z",
    batch_start_sha: "deadbeef",
    tier,
    batch_arg: null,
    current_phase: 10,
    groups: candidates.length > 0
      ? [{ group: "batch:kernel", order: 0, issues: candidates }]
      : [],
    warnings: [],
  };
}

const actionableIssue: IssueSummary = {
  number: 295,
  title: "feat(kernel): Sketch Offset",
  labels: ["type: feature", "batch:kernel"],
};

describe("detectPhase21Reached (#253)", () => {
  let workDir: string;

  beforeEach(() => {
    workDir = mkdtempSync(join(tmpdir(), "should-stop-test-"));
  });

  afterEach(() => {
    rmSync(workDir, { recursive: true, force: true });
  });

  test("ROADMAP 不在 → false", () => {
    expect(detectPhase21Reached(join(workDir, "no-such.md"))).toBe(false);
  });

  test("Phase 21 セクションが ✅ なしで存在 → true (UI 期到達)", () => {
    const p = join(workDir, "ROADMAP.md");
    writeFileSync(p, "## Phase 20: Quality Pass ✅\n## Phase 21: UI 期\n", "utf-8");
    expect(detectPhase21Reached(p)).toBe(true);
  });

  test("Phase 21 セクションが ✅ 付き → false (既に完了 = 次が来る)", () => {
    const p = join(workDir, "ROADMAP.md");
    writeFileSync(p, "## ✅ Phase 21: UI 期\n## Phase 22: ...\n", "utf-8");
    expect(detectPhase21Reached(p)).toBe(false);
  });

  test("Phase 20 までしかなければ false", () => {
    const p = join(workDir, "ROADMAP.md");
    writeFileSync(p, "## Phase 9: foo\n## Phase 20: Quality Pass\n", "utf-8");
    expect(detectPhase21Reached(p)).toBe(false);
  });

  test("Phase 21 の見出しに `:` がなければ拾わない (regex 仕様)", () => {
    const p = join(workDir, "ROADMAP.md");
    writeFileSync(p, "## Phase 21 以降の話 — 注意書きセクション\n", "utf-8");
    expect(detectPhase21Reached(p)).toBe(false);
  });

  test("複数の Phase 21 行があっても 1 つでも ✅ なしなら true", () => {
    const p = join(workDir, "ROADMAP.md");
    writeFileSync(p, "## ✅ Phase 21: 完了済\n## Phase 21: 別軸 (未完)\n", "utf-8");
    expect(detectPhase21Reached(p)).toBe(true);
  });
});

describe("detectHaltSwitch (#253)", () => {
  let workDir: string;

  beforeEach(() => {
    workDir = mkdtempSync(join(tmpdir(), "halt-switch-test-"));
  });

  afterEach(() => {
    rmSync(workDir, { recursive: true, force: true });
  });

  test("switch ファイル不在 → false", () => {
    expect(detectHaltSwitch(join(workDir, "halt"))).toBe(false);
  });

  test("switch ファイル存在 → true", () => {
    const p = join(workDir, "halt");
    writeFileSync(p, "halt by user", "utf-8");
    expect(detectHaltSwitch(p)).toBe(true);
  });

  test("空ファイルでも存在さえすれば true", () => {
    const p = join(workDir, "halt");
    writeFileSync(p, "", "utf-8");
    expect(detectHaltSwitch(p)).toBe(true);
  });
});

describe("decideStopFromCandidates (#310)", () => {
  // --- 経路 1: fail-open → fail-closed ---
  test("batch-select 不調 (plan=null) + actionable あり → pause (旧: proceed 空回り)", () => {
    const d = decideStopFromCandidates([actionableIssue], null);
    expect(d.code).toBe(1);
    expect(d.message).toContain("batch-select unavailable");
    expect(d.message).toContain("actionable=1");
  });

  // --- 経路 2: 全候補 pause_reasons 非空 → front-load pause ---
  test("全候補 pause_reasons 非空 → pause (旧: proceed 空回り)", () => {
    const plan = makePlan([
      makeCandidate({ number: 295, pause_reasons: ["ambiguous"], ambiguous: true }),
      makeCandidate({ number: 296, pause_reasons: ["ambiguous"], ambiguous: true }),
    ]);
    const d = decideStopFromCandidates([actionableIssue], plan);
    expect(d.code).toBe(1);
    expect(d.message).toContain("all 2 candidates have pause_reasons");
    expect(d.message).toContain("ambiguous=2");
  });

  test("1 件でも pause_reasons が空なら proceed (現行どおり)", () => {
    const plan = makePlan([
      makeCandidate({ number: 295, pause_reasons: ["ambiguous"], ambiguous: true }),
      makeCandidate({ number: 296, pause_reasons: [] }),
    ]);
    const d = decideStopFromCandidates([actionableIssue], plan);
    expect(d.code).toBe(0);
    expect(d.message).toContain("proceed: #295");
  });

  // --- 回帰: 正常系・既存 pause 系が維持されること ---
  test("正常系: 候補あり (pause_reasons 空) → proceed #295", () => {
    const plan = makePlan([makeCandidate({ number: 295 })]);
    const d = decideStopFromCandidates([actionableIssue], plan);
    expect(d.code).toBe(0);
    expect(d.message).toContain("proceed: #295");
    expect(d.message).toContain("tier=split-batch");
  });

  test("gh 失敗 (issues=null) → pause", () => {
    const d = decideStopFromCandidates(null, null);
    expect(d.code).toBe(1);
    expect(d.message).toContain("gh issue list failed");
  });

  test("open issue 0 件 → pause", () => {
    const d = decideStopFromCandidates([], null);
    expect(d.code).toBe(1);
    expect(d.message).toContain("no open issues");
  });

  test("全 Issue が gate/needs-* のみ → pause", () => {
    const gated: IssueSummary = { number: 1, title: "x", labels: ["gate:adr-review"] };
    const d = decideStopFromCandidates([gated], null);
    expect(d.code).toBe(1);
    expect(d.message).toContain("gated/needs-*");
  });

  test("actionable あるが batch-select 0 候補 → pause", () => {
    const d = decideStopFromCandidates([actionableIssue], makePlan([]));
    expect(d.code).toBe(1);
    expect(d.message).toContain("returned 0 candidates");
  });

  // --- 堅牢性: plan JSON は無検証信頼のため、将来の batch-select 変更や部分的な JSON で
  // pause_reasons が欠けても crash してはいけない (crash → watcher が想定外 RC で全停止する)。
  // 欠損 = pause 理由なし = proceed 候補、が安全方向。
  test("pause_reasons フィールド欠損の候補が混ざっても crash せず proceed", () => {
    const broken = makeCandidate({ number: 296 });
    delete (broken as Partial<BatchIssue>).pause_reasons;
    const plan = makePlan([
      makeCandidate({ number: 295, pause_reasons: ["ambiguous"], ambiguous: true }),
      broken,
    ]);
    const d = decideStopFromCandidates([actionableIssue], plan);
    expect(d.code).toBe(0);
    expect(d.message).toContain("proceed: #295");
  });
});
