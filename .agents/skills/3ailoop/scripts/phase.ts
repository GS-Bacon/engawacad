#!/usr/bin/env bun
// phase.ts — ADR/Phase/retrospective/dashboard state maintenance CLI
//
// contract §"Compatibility and cutover" #1 + §"Skill surface" #2:
//   preserve Phase close/seed, retrospective, dashboard のユーザー可視機能。
//   これらは旧 .claude/skills/3ailoop/scripts/loop-phase-*.ts の純粋ロジックを
//   引き継ぐが、ClaudeCode/GLM に依存しない。
//
// サブコマンド:
//   close-check  [--phase N] [--apply]     — Phase 完了条件検証 + (apply 時) ROADMAP 更新指示
//   seed         [--phase N]               — Phase 昇格後の起点 Issue 草案生成
//   retrospective [--phase N] [--dry-run]  — Phase 運用メトリクス集計 + Issue 起票草案
//   dashboard                             — features/.dashboard.md 累積俯瞰生成
//
// 出力 (stdout JSON): 各サブコマンドの結果
// 注: 本 CLI は「草案と判定」まで。GitHub への起票/push は行わない (delivery worker は write 禁止)。
//
// 関連: contract §"Compatibility and cutover"

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { arg, emitJson, emitStatus, REPO_ROOT } from "./cli/helpers.ts";

const ROADMAP = resolve(REPO_ROOT, "ROADMAP.md");

interface PhaseCondition {
  phase: number;
  title: string;
  completed: boolean; // ROADMAP 上 ✅ か
  conditions: string[]; // 抽出した完了条件行
}

export interface PhaseIssueSnapshot { number: number; state: string; labels: string[]; milestone?: string | null }
export interface MilestoneSnapshot { title: string; state: string }

/** ROADMAP.md から Phase N セクションの完了条件を抽出する (pure)。 */
export function extractPhaseConditions(roadmapText: string, phase: number): PhaseCondition | null {
  // セクション見出し: "## ✅ Phase N: title" or "## 🚧 Phase N: title" or "## Phase N: title"
  const re = new RegExp(`^##\\s+(?:✅\\s+|🚧\\s+)?Phase\\s+${phase}\\s*:\\s*(.+)$`, "m");
  const m = roadmapText.match(re);
  if (!m) return null;
  const title = m[1].trim();
  const completed = roadmapText.slice(0, m.index).includes("✅") && /##\s+✅\s+Phase/.test(m[0]);
  // 次の ## までを行として取り出す
  const after = roadmapText.slice(m.index! + m[0].length);
  const nextSec = after.search(/^##\s/m);
  const body = nextSec >= 0 ? after.slice(0, nextSec) : after;
  // 完了条件らしき行: "- [ ]" / "- [x]" / "完了条件" など。ここではリスト項目を条件とみなす。
  const conditions = body
    .split(/\r?\n/)
    .map(l => l.trim())
    .filter(l => /^-\s+(?:\[[ xX]\]\s+)?/.test(l) && !/^---/.test(l))
    .map(l => l.replace(/^-\s+(?:\[[ xX]\]\s+)?/, "").trim())
    .filter(Boolean);
  return { phase, title, completed, conditions };
}

export function evaluatePhaseClose(
  phase: number,
  condition: PhaseCondition,
  issues: PhaseIssueSnapshot[],
  milestones: MilestoneSnapshot[],
) {
  const milestone = milestones.find(m => m.title.startsWith(`Phase ${phase}`)) ?? null;
  const phaseIssues = issues.filter(i => (i.milestone ?? "").startsWith(`Phase ${phase}`));
  const featureOpen = phaseIssues.filter(i => i.state.toLowerCase() !== "closed" && i.labels.some(l => l === "type: feature" || l === "type:feature"));
  const gates = issues.filter(i => i.state.toLowerCase() !== "closed" && i.labels.some(l => l === "gate:human-feel" || l === "gate:adr-review"));
  const eligible = Boolean(milestone) && milestone?.state === "open" && featureOpen.length === 0 && gates.length === 0;
  return {
    phase, found: true, title: condition.title, eligible,
    milestone, feature_total: phaseIssues.filter(i => i.labels.some(l => l === "type: feature" || l === "type:feature")).length,
    feature_open: featureOpen.map(i => i.number), blocking_gates: gates.map(i => i.number),
    roadmap_marked_completed: condition.completed, conditions: condition.conditions,
  };
}

function readSnapshot<T>(path: string | undefined, fallback: T): T {
  if (!path) return fallback;
  return JSON.parse(readFileSync(path, "utf-8")) as T;
}

export function buildDashboard(state: any, decisionsText: string): string {
  const cycles = Array.isArray(state?.recent_cycles) ? state.recent_cycles : [];
  const workers = state?.workers ?? {};
  const lines = [
    "# EngawaCAD loop dashboard", "",
    `- Current cycle: ${state?.cycle ?? state?.current_cycle ?? "unknown"}`,
    `- Paused: ${state?.paused ? "yes" : "no"}`,
    `- Workers: ${Object.keys(workers).length}`,
    "", "## Recent Activity", "",
  ];
  for (const cycle of cycles.slice(-20).reverse()) {
    lines.push(`- Cycle #${cycle.cycle ?? "?"}: closed ${(cycle.closed ?? []).map((n: number) => `#${n}`).join(", ") || "none"}${cycle.pause_reason ? `; paused: ${cycle.pause_reason}` : ""}`);
  }
  lines.push("", "## Decision Log", "");
  const decisions = decisionsText.split("\n").filter(Boolean).slice(-20).reverse();
  lines.push(...(decisions.length ? decisions.map(x => `- ${x.replace(/^-\s*/, "")}`) : ["*no decisions yet*"]));
  return `${lines.join("\n")}\n`;
}

async function main(): Promise<number> {
  const [, , cmd, ...rest] = process.argv;
  const roadmapText = existsSync(ROADMAP) ? readFileSync(ROADMAP, "utf-8") : "";

  switch (cmd) {
    case "close-check": {
      const phaseStr = arg(rest, "--phase");
      const apply = rest.includes("--apply");
      if (!phaseStr) { emitStatus("Usage: phase close-check --phase N [--apply]"); return 2; }
      const phase = parseInt(phaseStr, 10);
      const cond = extractPhaseConditions(roadmapText, phase);
      if (!cond) {
        emitJson({ phase, found: false });
        return apply ? 0 : 0;
      }
      const issues = readSnapshot<PhaseIssueSnapshot[]>(arg(rest, "--issues-json"), []);
      const milestones = readSnapshot<MilestoneSnapshot[]>(arg(rest, "--milestones-json"), []);
      const result = evaluatePhaseClose(phase, cond, issues, milestones);
      emitJson({ ...result, action: apply && result.eligible ? "Codex may apply ADR-002 ROADMAP+milestone close through the authorized path" : "report-only" });
      return result.eligible || cond.completed ? 0 : 1;
    }
    case "seed": {
      const phaseStr = arg(rest, "--phase");
      if (!phaseStr) { emitStatus("Usage: phase seed --phase N"); return 2; }
      const phase = parseInt(phaseStr, 10);
      const cond = extractPhaseConditions(roadmapText, phase);
      const drafts = (cond?.conditions ?? []).map((c, i) => ({
        title: c.slice(0, 80),
        body: `Auto-seeded from ROADMAP Phase ${phase} condition #${i + 1}: ${c}`,
        labels: ["type: feature", "batch:kernel"],
        milestone: `Phase ${phase}: ${cond?.title ?? ""}`.trim(),
        granularity_check_required: true,
      }));
      emitJson({ phase, seeded_drafts: drafts, note: "drafts only; Codex/Skill creates issues via user-authorized path" });
      return 0;
    }
    case "retrospective": {
      const phaseStr = arg(rest, "--phase");
      const dryRun = rest.includes("--dry-run");
      if (!phaseStr) { emitStatus("Usage: phase retrospective --phase N [--dry-run]"); return 2; }
      const phase = parseInt(phaseStr, 10);
      const journal = resolve(REPO_ROOT, "features/.loop/cycle-journal.log");
      const cycles = existsSync(journal) ? readFileSync(journal, "utf-8").split("\n").filter(Boolean).flatMap(line => { try { return [JSON.parse(line)]; } catch { return []; } }) : [];
      const paused = cycles.filter((c: any) => Boolean(c.pause_reason));
      const needsHuman = new Set<number>();
      for (const c of cycles) for (const n of c.needs_human ?? []) needsHuman.add(Number(n));
      const skipPath = resolve(REPO_ROOT, "features/.loop/codex-skips.jsonl");
      const skips = existsSync(skipPath) ? readFileSync(skipPath, "utf-8").split("\n").filter(Boolean).flatMap(line => { try { return [JSON.parse(line)]; } catch { return []; } }) : [];
      emitJson({
        phase,
        dry_run: dryRun,
        metrics: {
          cycles: cycles.length,
          pauses: paused.length,
          pause_rate: cycles.length ? paused.length / cycles.length : 0,
          needs_human: [...needsHuman],
          codex_skips: { created: skips.length, resolved: skips.filter((s: any) => s.resolved_at).length, pending: skips.filter((s: any) => !s.resolved_at).length },
        },
        issue_drafts: [
          { title: `retro(phase${phase}): overview`, labels: ["type:foundation", "batch:skill"] },
        ],
      });
      return 0;
    }
    case "dashboard": {
      const dash = resolve(REPO_ROOT, "features/.dashboard.md");
      const statePath = resolve(REPO_ROOT, "features/.loop/state.json");
      let state: any = {};
      if (existsSync(statePath)) {
        try { state = JSON.parse(readFileSync(statePath, "utf-8")); }
        catch (e) { emitJson({ dashboard: dash, written: false, error: `invalid state: ${(e as Error).message}` }); return 1; }
      }
      const decisionsPath = resolve(REPO_ROOT, "features/.loop/decisions.log.md");
      const body = buildDashboard(state, existsSync(decisionsPath) ? readFileSync(decisionsPath, "utf-8") : "");
      emitJson({ dashboard: dash, written: !rest.includes("--dry-run"), bytes: body.length });
      if (!rest.includes("--dry-run")) {
        mkdirSync(dirname(dash), { recursive: true }); writeFileSync(dash, body, "utf-8");
      }
      return 0;
    }
    default:
      emitStatus("Usage: phase.ts (close-check | seed | retrospective | dashboard) [args]");
      return 2;
  }
}

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}
