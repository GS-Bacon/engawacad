#!/usr/bin/env bun
// loop-dashboard.ts — features/.dashboard.md 累積俯瞰型を上書き生成
//
// plan の dashboard 構造に沿う:
//   - Header (last updated, cycle, loop start)
//   - Current Status (state / phase / pause reason / resume action)
//   - 24h Activity
//   - Pending Gates
//   - Needs-Human Backlog
//   - Cumulative Stats
//   - Recent Activity (直近 20 件)
//   - Decision Log (loop-decision-log の最新 N 件)
//
// 使い方:
//   bun loop-dashboard.ts [--state-only] [--state <path>]

import { existsSync, readFileSync, writeFileSync } from "fs";

const DASHBOARD_PATH = "features/.dashboard.md";
const STATE_PATH = "features/.loop/state.json";
const DECISIONS_PATH = "features/.loop/decisions.log.md";

interface RecentCycle {
  cycle: number;
  started_at: string;
  ended_at: string;
  closed: number[];
  raised: number[];
  merged_commits: number;
  adr_drafts: string[];
  pause_reason?: string;
}
interface LoopState {
  loop_start: string;
  cycle: number;
  last_cycle_at: string;
  recent_cycles: RecentCycle[];
  cumulative: {
    cycles_total: number;
    issues_closed: number;
    raised_resolved: number;
    adr_total: number;
    token_claude: number;
    token_glm: number;
    token_codex: number;
  };
}

function safeRead(path: string): string | null {
  if (!existsSync(path)) return null;
  try { return readFileSync(path, "utf-8"); } catch { return null; }
}

/** state.json を読む。未存在は null、破損は throw (#177 指摘 2: fail-closed) */
function readState(): LoopState | null {
  const raw = safeRead(STATE_PATH);
  if (!raw) return null;
  try { return JSON.parse(raw) as LoopState; } catch (e) {
    throw new Error(`state.json parse failed: ${(e as Error).message}`);
  }
}

interface IssueSummary { number: number; title: string; labels: string[] }
async function fetchOpenIssues(): Promise<IssueSummary[]> {
  const proc = Bun.spawn(
    ["gh", "issue", "list", "--state", "open", "--limit", "200",
     "--json", "number,title,labels"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  if (proc.exitCode !== 0) return [];
  try {
    const raw = JSON.parse(out) as Array<{ number: number; title: string; labels: { name: string }[] }>;
    return raw.map(i => ({ number: i.number, title: i.title, labels: i.labels.map(l => l.name) }));
  } catch { return []; }
}

function sectionHeader(state: LoopState | null): string {
  const last = state?.last_cycle_at ?? new Date().toISOString();
  const cycle = state?.cycle ?? 0;
  const start = state?.loop_start ?? "(initial)";
  return `# /3ailoop Dashboard\n\nLast updated: ${last} | Cycle #${cycle} | Loop start: ${start}\n`;
}

function sectionCurrentStatus(state: LoopState | null, openIssues: IssueSummary[]): string {
  const lines: string[] = ["## Current Status"];
  const last = state?.recent_cycles?.at(-1);
  const stateLabel = last?.pause_reason ? "paused" : (state ? "running" : "not-started");
  lines.push(`**State**: ${stateLabel}`);

  // Current Phase (ROADMAP から抽出)
  const roadmap = safeRead("ROADMAP.md");
  if (roadmap) {
    const m = roadmap.match(/^##\s+Phase\s+(\d+):?\s*([^\n]*)/m);
    if (m) lines.push(`**Current Phase**: Phase ${m[1]} ${m[2].trim() ? "— " + m[2].trim() : ""}`);
  }

  if (last?.pause_reason) {
    lines.push(`**Pause reason**: ${last.pause_reason}`);
    lines.push(`**Resume action**: 視覚承認 / ADR レビュー / 失敗退避レビュー / token 閾値リセット`);
  }

  const gateCount = openIssues.filter(i => i.labels.some(l => l.startsWith("gate:"))).length;
  const needsCount = openIssues.filter(i => i.labels.some(l =>
    ["needs-triage", "needs-phase", "needs-human", "needs-intent-review", "needs-review", "blocked-by-split"].includes(l))).length;
  lines.push(`**Loop-actionable open issues**: ${openIssues.length - gateCount - needsCount} (gate=${gateCount}, needs-*=${needsCount})`);
  return lines.join("\n") + "\n";
}

function section24hActivity(state: LoopState | null): string {
  const lines: string[] = ["## 24h Activity"];
  if (!state) { lines.push("*no state yet*"); return lines.join("\n") + "\n"; }
  const cutoff = Date.now() - 24 * 3600 * 1000;
  const recent24h = state.recent_cycles.filter(c => new Date(c.ended_at).getTime() >= cutoff);
  const closed = recent24h.flatMap(c => c.closed);
  const raised = recent24h.flatMap(c => c.raised);
  const merged = recent24h.reduce((s, c) => s + c.merged_commits, 0);
  const adr = recent24h.flatMap(c => c.adr_drafts);
  lines.push(`- Closed Issues: ${closed.length}${closed.length ? ` (${closed.map(n => `#${n}`).join(", ")})` : ""}`);
  lines.push(`- Merged commits: ${merged}`);
  lines.push(`- Auto-raised Issues: ${raised.length}${raised.length ? ` (${raised.map(n => `#${n}`).join(", ")})` : ""}`);
  lines.push(`- New ADR drafts: ${adr.length}${adr.length ? ` (${adr.join(", ")})` : ""}`);
  lines.push(`- Token (24h tracked): claude+glm+codex collected by loop-token-meter`);
  return lines.join("\n") + "\n";
}

function sectionPendingGates(openIssues: IssueSummary[]): string {
  const lines: string[] = ["## Pending Gates (要人間対応)"];
  const gated = openIssues.filter(i => i.labels.some(l => l.startsWith("gate:")));
  if (gated.length === 0) { lines.push("*none*"); return lines.join("\n") + "\n"; }
  lines.push("| # | Title | Gate |");
  lines.push("|---|---|---|");
  for (const i of gated) {
    const gate = i.labels.find(l => l.startsWith("gate:")) ?? "";
    lines.push(`| #${i.number} | ${i.title.replace(/\|/g, "\\|")} | ${gate} |`);
  }
  return lines.join("\n") + "\n";
}

function sectionNeedsHuman(openIssues: IssueSummary[]): string {
  const lines: string[] = ["## Needs-Human Backlog (失敗退避)"];
  const needs = openIssues.filter(i => i.labels.includes("needs-human"));
  if (needs.length === 0) { lines.push("*none*"); return lines.join("\n") + "\n"; }
  for (const i of needs) {
    let failures = 0;
    const fpath = `features/.loop/failure-streak/${i.number}.json`;
    if (existsSync(fpath)) {
      try {
        const obj = JSON.parse(readFileSync(fpath, "utf-8"));
        failures = obj.count ?? 0;
      } catch { /* ignore */ }
    }
    lines.push(`- #${i.number} ${i.title} (failures: ${failures})`);
  }
  return lines.join("\n") + "\n";
}

function sectionCumulative(state: LoopState | null): string {
  const lines: string[] = ["## Cumulative Stats (loop start から)"];
  if (!state) { lines.push("*no state yet*"); return lines.join("\n") + "\n"; }
  const c = state.cumulative;
  lines.push(`- Cycles run: ${c.cycles_total}`);
  lines.push(`- Issues closed: ${c.issues_closed}`);
  lines.push(`- Auto-raised resolved: ${c.raised_resolved}`);
  lines.push(`- ADR drafts generated: ${c.adr_total}`);
  lines.push(`- Total token: claude ${c.token_claude} / glm ${c.token_glm} / codex ${c.token_codex} (累積 100M で auto-pause)`);
  return lines.join("\n") + "\n";
}

function sectionRecentActivity(state: LoopState | null): string {
  const lines: string[] = ["## Recent Activity (直近 20 件)"];
  if (!state || state.recent_cycles.length === 0) {
    lines.push("*no activity yet*"); return lines.join("\n") + "\n";
  }
  const reversed = [...state.recent_cycles].reverse();
  for (const c of reversed.slice(0, 20)) {
    const closed = c.closed.length ? `closed [${c.closed.map(n => `#${n}`).join(", ")}]` : "";
    const raised = c.raised.length ? `raised [${c.raised.map(n => `#${n}`).join(", ")}]` : "";
    const pause = c.pause_reason ? ` (paused: ${c.pause_reason})` : "";
    const merged = c.merged_commits ? `merged ${c.merged_commits}` : "";
    const parts = [closed, raised, merged].filter(Boolean).join(", ");
    lines.push(`- [${c.ended_at}] Cycle #${c.cycle}: ${parts || "no changes"}${pause}`);
  }
  return lines.join("\n") + "\n";
}

function sectionDecisionLog(): string {
  const lines: string[] = ["## Decision Log (重要判断、累積)"];
  const raw = safeRead(DECISIONS_PATH);
  if (!raw) { lines.push("*no decisions yet*"); return lines.join("\n") + "\n"; }
  const allLines = raw.split("\n").filter(l => l.startsWith("- "));
  const recent = allLines.slice(-20).reverse();
  if (recent.length === 0) { lines.push("*no decisions yet*"); return lines.join("\n") + "\n"; }
  lines.push(...recent);
  return lines.join("\n") + "\n";
}

async function main() {
  // #177 指摘 2: state.json 破損なら dashboard.md を触らず exit 1
  let state: LoopState | null;
  try {
    state = readState();
  } catch (e) {
    process.stderr.write(`FAIL-CLOSED: ${(e as Error).message}; dashboard.md left untouched\n`);
    process.exit(1);
  }
  const openIssues = await fetchOpenIssues();

  const sections = [
    sectionHeader(state),
    sectionCurrentStatus(state, openIssues),
    section24hActivity(state),
    sectionPendingGates(openIssues),
    sectionNeedsHuman(openIssues),
    sectionCumulative(state),
    sectionRecentActivity(state),
    sectionDecisionLog(),
  ];

  const body = sections.join("\n");
  writeFileSync(DASHBOARD_PATH, body, "utf-8");
  console.log(`Wrote ${DASHBOARD_PATH} (${body.length} bytes)`);
}

main().catch(e => {
  console.error(`ERROR: ${(e as Error).message}`);
  process.exit(1);
});
