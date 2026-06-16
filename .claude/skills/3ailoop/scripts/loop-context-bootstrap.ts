#!/usr/bin/env bun
// loop-context-bootstrap.ts — 各 /3ailoop サイクル冒頭の context bootstrap
//
// 新セッション or 長時間自走後の文脈劣化対策として、以下を要点 stdout する:
// - 現 Phase (ROADMAP.md から抽出)
// - MEMORY index (~/.claude/projects/-home-bacon-engawacad/memory/MEMORY.md)
// - 直近 dashboard (features/.dashboard.md、存在すれば)
// - 直近 cycle サマリ (features/.loop/state.json、存在すれば)
// - open Issue 件数 + gate/needs-* 内訳 (gh issue list)
//
// Claude が改めて Read 不要で消化できる構造化 markdown を吐く。
// 使い方:
//   bun loop-context-bootstrap.ts [--memory-index <path>]

import { existsSync, readFileSync } from "fs";
import { homedir } from "os";
import { join } from "path";

const DEFAULT_MEMORY_INDEX = join(
  homedir(),
  ".claude/projects/-home-bacon-engawacad/memory/MEMORY.md",
);
const ROADMAP_PATH = "ROADMAP.md";
const CLAUDE_MD_PATH = "CLAUDE.md";
const DASHBOARD_PATH = "features/.dashboard.md";
const LOOP_STATE_PATH = "features/.loop/state.json";

const LOOP_EXCLUDE_LABELS = new Set([
  "gate:human-feel",
  "gate:adr-review",
  "needs-triage",
  "needs-phase",
  "needs-human",
  "needs-intent-review",
  "blocked-by-split",
]);

function parseArg(name: string): string | undefined {
  const args = process.argv.slice(2);
  const idx = args.indexOf(name);
  return idx >= 0 ? args[idx + 1] : undefined;
}

function safeRead(path: string): string | null {
  if (!existsSync(path)) return null;
  try {
    return readFileSync(path, "utf-8");
  } catch {
    return null;
  }
}

/** ROADMAP.md から現 Phase セクションを抽出 (suffix/prefix どちらの ✅ もスキップ)
 *  #178 指摘 7 対応: 行全体に ✅ が含まれる Phase ヘッダは完了扱い
 */
function extractCurrentPhase(roadmap: string): string {
  const lines = roadmap.split("\n");
  let startIdx = -1;
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(/^##\s+(?:✅\s+)?Phase\s+\d/);
    if (m && !lines[i].includes("✅")) {
      startIdx = i;
      break;
    }
  }
  if (startIdx < 0) return "(現 Phase セクション未検出: 全 Phase が完了済みかも)";
  // 次の ## Phase or --- まで
  let endIdx = lines.length;
  for (let i = startIdx + 1; i < lines.length; i++) {
    if (/^##\s+(?:✅\s+)?Phase\s+\d/.test(lines[i]) || /^---\s*$/.test(lines[i])) {
      endIdx = i;
      break;
    }
  }
  return lines.slice(startIdx, endIdx).join("\n").slice(0, 2000);
}

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

function classifyIssues(issues: IssueSummary[]): {
  total: number;
  gateBreakdown: Record<string, number>;
  needsBreakdown: Record<string, number>;
  loopActionable: number;
} {
  const gateBreakdown: Record<string, number> = {};
  const needsBreakdown: Record<string, number> = {};
  let loopActionable = 0;
  for (const i of issues) {
    let excluded = false;
    for (const l of i.labels) {
      if (l.startsWith("gate:")) {
        gateBreakdown[l] = (gateBreakdown[l] ?? 0) + 1;
        excluded = true;
      } else if (LOOP_EXCLUDE_LABELS.has(l)) {
        needsBreakdown[l] = (needsBreakdown[l] ?? 0) + 1;
        excluded = true;
      }
    }
    if (!excluded) loopActionable += 1;
  }
  return { total: issues.length, gateBreakdown, needsBreakdown, loopActionable };
}

interface LoopState {
  cycle?: number;
  last_cycle_at?: string;
  recent_cycles?: Array<{ cycle: number; closed: number[]; raised: number[]; merged_commits: number; pause_reason?: string }>;
}

async function main() {
  const memoryPath = parseArg("--memory-index") ?? DEFAULT_MEMORY_INDEX;
  const lines: string[] = [];

  lines.push(`# /3ailoop Context Bootstrap`);
  lines.push(`Generated: ${new Date().toISOString()}`);
  lines.push("");

  // 1. Current Phase (ROADMAP)
  lines.push(`## Current Phase (ROADMAP.md)`);
  const roadmap = safeRead(ROADMAP_PATH);
  if (!roadmap) {
    lines.push(`*missing: ${ROADMAP_PATH}*`);
  } else {
    lines.push("```");
    lines.push(extractCurrentPhase(roadmap));
    lines.push("```");
  }
  lines.push("");

  // 2. CLAUDE.md ルールの存在確認 (冒頭抜粋)
  lines.push(`## CLAUDE.md (project rules, first 40 lines)`);
  const claudeMd = safeRead(CLAUDE_MD_PATH);
  if (!claudeMd) {
    lines.push(`*missing: ${CLAUDE_MD_PATH}*`);
  } else {
    lines.push("```");
    lines.push(claudeMd.split("\n").slice(0, 40).join("\n"));
    lines.push("```");
  }
  lines.push("");

  // 3. MEMORY index
  lines.push(`## Memory Index (${memoryPath})`);
  const memory = safeRead(memoryPath);
  if (!memory) {
    lines.push(`*missing: ${memoryPath}*`);
  } else {
    lines.push("```");
    lines.push(memory.split("\n").slice(0, 100).join("\n"));
    lines.push("```");
  }
  lines.push("");

  // 4. Dashboard (直近サマリ)
  lines.push(`## Recent Dashboard (${DASHBOARD_PATH})`);
  const dashboard = safeRead(DASHBOARD_PATH);
  if (!dashboard) {
    lines.push(`*no dashboard yet (initial cycle or post-purge)*`);
  } else {
    lines.push("```");
    lines.push(dashboard.split("\n").slice(0, 60).join("\n"));
    lines.push("```");
  }
  lines.push("");

  // 5. Loop state (直近 3 cycle)
  lines.push(`## Recent Loop State (${LOOP_STATE_PATH})`);
  const stateRaw = safeRead(LOOP_STATE_PATH);
  if (!stateRaw) {
    lines.push(`*no loop state yet*`);
  } else {
    try {
      const state = JSON.parse(stateRaw) as LoopState;
      lines.push(`- Current cycle: ${state.cycle ?? "?"}`);
      lines.push(`- Last cycle at: ${state.last_cycle_at ?? "?"}`);
      const recent = (state.recent_cycles ?? []).slice(-3);
      if (recent.length > 0) {
        lines.push(`- Recent 3 cycles:`);
        for (const c of recent) {
          const closedStr = c.closed?.length ? c.closed.join(", ") : "none";
          const raisedStr = c.raised?.length ? c.raised.join(", ") : "none";
          const pause = c.pause_reason ? ` (paused: ${c.pause_reason})` : "";
          lines.push(`  - Cycle #${c.cycle}: closed=[${closedStr}] raised=[${raisedStr}] merged=${c.merged_commits ?? 0}${pause}`);
        }
      } else {
        lines.push(`- (no recent_cycles recorded)`);
      }
    } catch (e) {
      lines.push(`*corrupt state.json: ${(e as Error).message}*`);
    }
  }
  lines.push("");

  // 6. Open Issues classification
  lines.push(`## Open Issues (gh issue list)`);
  const issues = await fetchOpenIssues();
  if (issues === null) {
    lines.push(`*gh issue list failed (offline?)*`);
  } else {
    const c = classifyIssues(issues);
    lines.push(`- Total open: ${c.total}`);
    lines.push(`- Loop-actionable (no gate / needs-*): **${c.loopActionable}**`);
    if (Object.keys(c.gateBreakdown).length > 0) {
      lines.push(`- Gate breakdown:`);
      for (const [k, v] of Object.entries(c.gateBreakdown)) {
        lines.push(`  - ${k}: ${v}`);
      }
    }
    if (Object.keys(c.needsBreakdown).length > 0) {
      lines.push(`- Needs/blocked breakdown:`);
      for (const [k, v] of Object.entries(c.needsBreakdown)) {
        lines.push(`  - ${k}: ${v}`);
      }
    }
    if (c.loopActionable === 0) {
      lines.push("");
      lines.push(`> **NOTE**: loop-actionable Issue が 0 件です。loop-should-stop が pause を返すはず`);
    }
  }
  lines.push("");
  lines.push(`---`);
  lines.push(`(end of context bootstrap)`);

  console.log(lines.join("\n"));
}

main().catch(e => {
  console.error(`ERROR: ${(e as Error).message}`);
  process.exit(1);
});
