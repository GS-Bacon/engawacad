#!/usr/bin/env bun
// loop-should-stop.ts — /3ailoop サイクル冒頭の停止判定
//
// batch-select.ts --loop --dry-run を呼んで候補 Issue を取得し、
// 候補が gate/needs-* のみ or ゼロなら exit 1 (pause)、続行可なら exit 0 を返す。
//
// stdout 形式:
//   proceed: <#N>          — 続行、上位候補
//   stop: <reason>         — pause、理由
//
// 使い方:
//   bun loop-should-stop.ts
//
// 関連: plan section "停止条件 = 候補 Issue が gate (gate:human-feel / gate:adr-review)
//       または needs-* のみ"

import { existsSync } from "fs";
import type { BatchPlan } from "../../3ai/scripts/types.ts";

const LOOP_EXCLUDE_LABELS = new Set([
  "gate:human-feel",
  "gate:adr-review",
  "needs-triage",
  "needs-phase",
  "needs-human",
  "needs-intent-review",
  "blocked-by-split",
]);

interface IssueSummary {
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

async function main() {
  const issues = await fetchOpenIssues();
  if (issues === null) {
    console.log("stop: gh issue list failed (offline or auth error)");
    process.exit(1);
  }

  if (issues.length === 0) {
    console.log("stop: no open issues");
    process.exit(1);
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
    console.log(`stop: all ${issues.length} open issues are gated/needs-* (${breakdown.join(", ")})`);
    process.exit(1);
  }

  // batch-select --loop --dry-run で実際の候補集合を確認
  const plan = await runBatchSelectLoop();
  if (plan === null) {
    // batch-select 不調の場合は actionable があれば fail-open で続行
    console.log(`proceed: actionable=${actionable.length} (batch-select unavailable)`);
    process.exit(0);
  }

  const planIssueCount = plan.groups.reduce((acc, g) => acc + g.issues.length, 0);
  if (planIssueCount === 0) {
    console.log(`stop: batch-select returned 0 candidates (open=${issues.length}, actionable=${actionable.length})`);
    process.exit(1);
  }

  // 続行: 最優先 group の最初の Issue を提示
  const firstIssue = plan.groups[0]?.issues[0];
  if (firstIssue) {
    console.log(`proceed: #${firstIssue.number} (tier=${plan.tier}, group=${plan.groups[0].group}, candidates=${planIssueCount})`);
  } else {
    console.log(`proceed: candidates=${planIssueCount}`);
  }
  process.exit(0);
}

main().catch(e => {
  console.error(`ERROR: ${(e as Error).message}`);
  process.exit(2);
});
