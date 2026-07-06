#!/usr/bin/env bun
// loop-should-stop.ts — /3ailoop サイクル冒頭の停止判定 (3 段階出力, #253)
//
// batch-select.ts --loop --dry-run を呼んで候補 Issue を取得し、
//   exit 0 (proceed)  — 続行可、上位候補を提示
//   exit 1 (pause)    — 一時停止、watcher は sleep + retry する
//   exit 2 (halt)     — 永遠停止、watcher は exit する (Phase 21 UI 期到達 等)
//
// stdout 形式:
//   proceed: <#N>          — 続行、上位候補
//   pause: <reason>        — 一時停止、retry 候補あり
//   halt: <reason>         — 永遠停止、人間判断要
//
// 使い方:
//   bun loop-should-stop.ts
//
// 関連: plan section "停止条件 = 候補 Issue が gate (gate:human-feel / gate:adr-review)
//       または needs-* のみ" + #253 watcher 不死身化

import { existsSync, readFileSync } from "fs";
import type { BatchPlan } from "../../3ai/scripts/types.ts";

const ROADMAP_PATH = "ROADMAP.md";
const HALT_SWITCH_PATH = "features/.loop/halt";

const LOOP_EXCLUDE_LABELS = new Set([
  "gate:human-feel",
  "gate:adr-review",
  "needs-triage",
  "needs-phase",
  "needs-human",
  "needs-intent-review",
  "needs-review",
  "blocked-by-split",
]);

export interface IssueSummary {
  number: number;
  title: string;
  labels: string[];
}

async function fetchOpenIssues(): Promise<IssueSummary[] | null> {
  const proc = Bun.spawn(
    ["gh", "issue", "list", "--state", "open", "--limit", "200",
     "--json", "number,title,labels"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  if (proc.exitCode !== 0) return null;
  try {
    const raw = JSON.parse(out) as Array<{ number: number; title: string; labels: { name: string }[] }>;
    return raw.map(i => ({
      number: i.number,
      title: i.title,
      labels: i.labels.map(l => l.name),
    }));
  } catch {
    return null;
  }
}

function isLoopActionable(labels: string[]): boolean {
  // gate:* / needs-* / blocked-by-split のいずれも持たない
  for (const l of labels) {
    if (l.startsWith("gate:")) return false;
    if (LOOP_EXCLUDE_LABELS.has(l)) return false;
  }
  return true;
}

/** #253: ROADMAP.md に `## Phase 21:` が ✅ なしで存在するかを判定。
 *  Phase 21 = UI 期 (memory: project_phase8_to_20_autonomous) の到達は
 *  完全自律期の終端であり、watcher は halt して人間判断を仰ぐ。 */
export function detectPhase21Reached(roadmapPath: string = ROADMAP_PATH): boolean {
  if (!existsSync(roadmapPath)) return false;
  for (const line of readFileSync(roadmapPath, "utf-8").split("\n")) {
    const m = line.match(/^##\s+(?:✅\s+)?Phase\s+(\d+):/);
    if (m && parseInt(m[1]) === 21 && !line.includes("✅")) return true;
  }
  return false;
}

/** #253: ユーザーが手動で配置できる kill switch ファイル。存在すれば即 halt。 */
export function detectHaltSwitch(switchPath: string = HALT_SWITCH_PATH): boolean {
  return existsSync(switchPath);
}

/** batch-select --loop --dry-run 結果から候補 Issue が存在するか確認 */
async function runBatchSelectLoop(): Promise<BatchPlan | null> {
  const scriptPath = ".claude/skills/3ai/scripts/batch-select.ts";
  if (!existsSync(scriptPath)) return null;
  const proc = Bun.spawn(
    ["bun", scriptPath, "--loop", "--dry-run"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  if (proc.exitCode !== 0) return null;
  try {
    return JSON.parse(out) as BatchPlan;
  } catch {
    return null;
  }
}

/** 3 段階の停止判定結果。code は process.exit() にそのまま渡す (0=proceed / 1=pause / 2=halt)。 */
export interface StopDecision {
  code: 0 | 1 | 2;
  message: string;
}

/**
 * open Issue 一覧と batch-select プランから停止判定を導く純粋関数 (halt 検知は呼び元)。
 *
 * @param issues fetchOpenIssues() の結果。null = gh 呼び出し失敗。
 * @param plan   runBatchSelectLoop() の結果。null = batch-select 不調 (script 欠落 / exit≠0 / parse 失敗)。
 */
export function decideStopFromCandidates(
  issues: IssueSummary[] | null,
  plan: BatchPlan | null,
): StopDecision {
  if (issues === null) {
    return { code: 1, message: "pause: gh issue list failed (offline or auth error)" };
  }

  if (issues.length === 0) {
    return { code: 1, message: "pause: no open issues" };
  }

  const actionable = issues.filter(i => isLoopActionable(i.labels));

  if (actionable.length === 0) {
    // 全 Issue が gate/needs-* のみ
    const gateCount: Record<string, number> = {};
    const needsCount: Record<string, number> = {};
    for (const i of issues) {
      for (const l of i.labels) {
        if (l.startsWith("gate:")) gateCount[l] = (gateCount[l] ?? 0) + 1;
        else if (LOOP_EXCLUDE_LABELS.has(l)) needsCount[l] = (needsCount[l] ?? 0) + 1;
      }
    }
    const breakdown: string[] = [];
    for (const [k, v] of Object.entries(gateCount)) breakdown.push(`${k}=${v}`);
    for (const [k, v] of Object.entries(needsCount)) breakdown.push(`${k}=${v}`);
    return {
      code: 1,
      message: `pause: all ${issues.length} open issues are gated/needs-* (${breakdown.join(", ")})`,
    };
  }

  if (plan === null) {
    // #310: batch-select 不調のとき、以前は actionable があれば fail-open で proceed していたが、
    // サイクル内の実 batch-select も同条件で失敗/0 件になる公算が高く、Claude 固定費が空回りする。
    // fail-closed (pause) にして watcher の sleep+retry に委ねる。
    return { code: 1, message: `pause: batch-select unavailable (actionable=${actionable.length})` };
  }

  const allCandidates = plan.groups.flatMap(g => g.issues);
  const planIssueCount = allCandidates.length;
  if (planIssueCount === 0) {
    return {
      code: 1,
      message: `pause: batch-select returned 0 candidates (open=${issues.length}, actionable=${actionable.length})`,
    };
  }

  // #310: 全候補が pause_reasons 非空 (needs-review / ambiguous 等) なら、そのサイクルは
  // gate=pause 確定で自律進行できない。Claude を起動する前に front-load して pause する。
  // 1 件でも pause_reasons が空の候補があれば proceed (現行どおり)。
  // plan JSON は無検証信頼のため pause_reasons 欠損は空配列扱い (= pause 理由なし = proceed 候補)。
  // ここで crash すると watcher が想定外 RC でループ全体を停止してしまう。
  if (allCandidates.every(i => (i.pause_reasons ?? []).length > 0)) {
    const reasonCount: Record<string, number> = {};
    for (const i of allCandidates) {
      for (const r of i.pause_reasons ?? []) reasonCount[r] = (reasonCount[r] ?? 0) + 1;
    }
    const breakdown = Object.entries(reasonCount).map(([k, v]) => `${k}=${v}`).join(", ");
    return {
      code: 1,
      message: `pause: all ${planIssueCount} candidates have pause_reasons (${breakdown})`,
    };
  }

  // 続行: 最優先 group の最初の Issue を提示
  const firstIssue = plan.groups[0]?.issues[0];
  if (firstIssue) {
    return {
      code: 0,
      message: `proceed: #${firstIssue.number} (tier=${plan.tier}, group=${plan.groups[0].group}, candidates=${planIssueCount})`,
    };
  }
  return { code: 0, message: `proceed: candidates=${planIssueCount}` };
}

async function main() {
  // #253: halt 条件を先に判定 (Phase 21 到達 / kill switch)
  if (detectHaltSwitch()) {
    console.log(`halt: kill switch file present (${HALT_SWITCH_PATH})`);
    process.exit(2);
  }
  if (detectPhase21Reached()) {
    console.log("halt: Phase 21 (UI 期) 到達 — 人間判断要 (完全自律期終端)");
    process.exit(2);
  }

  const issues = await fetchOpenIssues();
  // batch-select --loop --dry-run で実際の候補集合を確認。
  // actionable が 0 件 (全 gate/needs-*) のときは decide 側で plan を見ずに pause するため、
  // 無駄な spawn を避けて batch-select を呼ばない (deep pause 中の steady state を軽くする)。
  const hasActionable = issues?.some(i => isLoopActionable(i.labels)) ?? false;
  const plan = hasActionable ? await runBatchSelectLoop() : null;

  const decision = decideStopFromCandidates(issues, plan);
  console.log(decision.message);
  process.exit(decision.code);
}

// #253: テストから import される場合は main を起動しない
if (import.meta.main) {
  main().catch(e => {
    console.error(`ERROR: ${(e as Error).message}`);
    process.exit(2);
  });
}
