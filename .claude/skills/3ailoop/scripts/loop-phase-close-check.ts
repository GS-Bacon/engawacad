#!/usr/bin/env bun
// loop-phase-close-check.ts — Phase 完了条件を検証して必要なら完了処理を実施
//
// 完了条件:
//   1. milestone "Phase N" が存在
//   2. milestone の type:feature Issue がすべて closed
//   3. open Issue 全体で gate:human-feel / gate:adr-review がゼロ
//   4. ROADMAP.md の "## Phase N:" がまだ ✅ 化されていない
//
// 完了処理 (apply):
//   - ROADMAP.md の "## Phase N: ..." を "## ✅ Phase N: ..." に書換 (atomic)
//   - 該当 milestone を gh api PATCH で close
//   - loop-decision-log に phase-transition を append
//
// 使い方:
//   bun loop-phase-close-check.ts check [--phase N]
//   bun loop-phase-close-check.ts apply [--phase N] [--dry-run]

import { existsSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname } from "path";

const ROADMAP_PATH = "ROADMAP.md";

// ADR-013 + #319 Phase E: 3 Phase ごとの Phase audit トリガー
// (旧 Fable 5 監査 → 3 系統独立 adversarial audit に置換、Fable 5 は禁止 memory:fable-5-banned)
// export 名は backward compat のため FABLE5_AUDIT_PHASES を維持、alias で PHASE_AUDIT_PHASES も export。
export const FABLE5_AUDIT_PHASES = new Set([11, 14, 17, 20]);
export const PHASE_AUDIT_PHASES = FABLE5_AUDIT_PHASES;

/** #319 Phase E: 3 系統独立 adversarial audit を invoke (Fable 5 単一 Issue 起票を置換)。
 *  loop-phase-audit.ts が Opus 4.7 (集約役) + Codex 3 persona + GLM 3 persona を並列 dispatch し、
 *  findings 集約後に critical/high/medium を Issue 化する (cap 5)。 */
async function invokePhaseAudit(phase: number): Promise<{ ok: boolean; issues?: number[]; error?: string }> {
  const scriptPath = `${dirname(new URL(import.meta.url).pathname)}/loop-phase-audit.ts`;
  const proc = Bun.spawn(
    ["bun", scriptPath, "--phase", String(phase)],
    { stdout: "pipe", stderr: "pipe" },
  );
  const out = (await new Response(proc.stdout).text()).trim();
  const err = (await new Response(proc.stderr).text()).trim();
  await proc.exited;
  if (proc.exitCode !== 0) {
    return { ok: false, error: err || `loop-phase-audit exit ${proc.exitCode}` };
  }
  // loop-phase-audit.ts の stdout 末尾に JSON 結果を出す想定
  // フォーマット: {"phase": N, "created_issues": [401, 402], ...}
  const lastLine = out.split("\n").filter(l => l.trim().startsWith("{")).pop();
  if (!lastLine) return { ok: true, issues: [] };
  try {
    const parsed = JSON.parse(lastLine);
    return { ok: true, issues: parsed.created_issues ?? [] };
  } catch {
    return { ok: true, issues: [] };
  }
}

function detectCurrentPhase(): number | null {
  if (!existsSync(ROADMAP_PATH)) return null;
  for (const line of readFileSync(ROADMAP_PATH, "utf-8").split("\n")) {
    // コロン必須: "## Phase 9 以降の総括" のような解説見出しを誤検出しないため
    const m = line.match(/^##\s+(?:✅\s+)?Phase\s+(\d+):/);
    if (m && !line.includes("✅")) return parseInt(m[1]);
  }
  return null;
}

async function runGh(args: string[]): Promise<{ stdout: string; exit: number }> {
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return { stdout: out.trim(), exit: proc.exitCode ?? 0 };
}

interface MsLite { number: number; title: string; state: string }
async function fetchMilestone(phase: number): Promise<MsLite | null> {
  const r = await runGh(["api", "repos/{owner}/{repo}/milestones", "--paginate"]);
  if (r.exit !== 0) return null;
  try {
    const arr = JSON.parse(r.stdout) as Array<{ number: number; title: string; state: string }>;
    return arr.find(m => m.title.startsWith(`Phase ${phase}`)) ?? null;
  } catch { return null; }
}

interface IssueLite { number: number; title: string; labels: { name: string }[]; state: string }
async function fetchIssuesByMilestone(milestoneNum: number): Promise<IssueLite[]> {
  const r = await runGh([
    "issue", "list", "--state", "all", "--milestone", String(milestoneNum),
    "--limit", "200",
    "--json", "number,title,labels,state",
  ]);
  if (r.exit !== 0) return [];
  try { return JSON.parse(r.stdout) as IssueLite[]; } catch { return []; }
}

async function fetchOpenIssuesWithLabel(label: string): Promise<IssueLite[]> {
  const r = await runGh([
    "issue", "list", "--state", "open", "--label", label, "--limit", "200",
    "--json", "number,title,labels,state",
  ]);
  if (r.exit !== 0) return [];
  try { return JSON.parse(r.stdout) as IssueLite[]; } catch { return []; }
}

function isFeatureType(labels: { name: string }[]): boolean {
  return labels.some(l => l.name === "type:feature" || l.name === "type: feature");
}

function isRoadmapMarked(phase: number): boolean {
  if (!existsSync(ROADMAP_PATH)) return false;
  const text = readFileSync(ROADMAP_PATH, "utf-8");
  return text.split("\n").some(l => new RegExp(`^##\\s+✅\\s+Phase\\s+${phase}(?!\\d)`).test(l));
}

interface CheckResult {
  ok: boolean;
  phase: number;
  reason: string;
  milestone: { number: number; title: string; state: string } | null;
  features_total: number;
  features_open: number[];
  gate_human_feel: number[];
  gate_adr_review: number[];
  roadmap_already_marked: boolean;
}

/** #271: split parent (blocked-by-split ラベル付き) で全子 closed なら auto-close する。
 *  Phase 完了処理デッドロックの解消に必須 (#194/#195/#206 系)。
 *  返値は close した親 Issue 番号の配列。 */
export async function autoCloseFinishedSplitParents(
  openParents: IssueLite[],
  ghFn: typeof runGh = runGh,
): Promise<number[]> {
  const closed: number[] = [];
  for (const parent of openParents) {
    if (!parent.labels.some(l => l.name === "blocked-by-split")) continue;
    // 子 Issue: parent-blocked-by-split:<N> ラベル付き
    const childLabel = `parent-blocked-by-split:${parent.number}`;
    const r = await ghFn([
      "issue", "list", "--state", "all", "--label", childLabel,
      "--limit", "200", "--json", "number,state",
    ]);
    if (r.exit !== 0) continue;
    let children: Array<{ number: number; state: string }>;
    try {
      children = JSON.parse(r.stdout);
    } catch {
      continue;
    }
    if (children.length === 0) continue;
    const allClosed = children.every(c => c.state.toUpperCase() === "CLOSED");
    if (!allClosed) continue;
    const childList = children.map(c => `#${c.number}`).join(", ");
    const closeR = await ghFn([
      "issue", "close", String(parent.number),
      "--comment", `split children ${childList} all closed — auto-close by phase-close-check (#271)`,
    ]);
    if (closeR.exit === 0) closed.push(parent.number);
  }
  return closed;
}

async function check(phase: number): Promise<CheckResult> {
  const ms = await fetchMilestone(phase);
  if (!ms) {
    return {
      ok: false, phase, reason: `milestone for Phase ${phase} not found`,
      milestone: null, features_total: 0, features_open: [],
      gate_human_feel: [], gate_adr_review: [], roadmap_already_marked: false,
    };
  }
  let issues = await fetchIssuesByMilestone(ms.number);
  // #271: split parent auto-close を check の冒頭で実施 (Phase 完了判定の前)
  const openParents = issues.filter(
    i => i.state.toUpperCase() === "OPEN" && isFeatureType(i.labels),
  );
  const autoClosed = await autoCloseFinishedSplitParents(openParents);
  if (autoClosed.length > 0) {
    // 再 fetch して最新状態を反映
    issues = await fetchIssuesByMilestone(ms.number);
  }
  const features = issues.filter(i => isFeatureType(i.labels));
  const featuresOpen = features.filter(i => i.state.toUpperCase() === "OPEN").map(i => i.number);

  const [gateHF, gateADR] = await Promise.all([
    fetchOpenIssuesWithLabel("gate:human-feel"),
    fetchOpenIssuesWithLabel("gate:adr-review"),
  ]);

  const marked = isRoadmapMarked(phase);

  const reasons: string[] = [];
  // #178 指摘 1 対応: features_total === 0 (空 milestone) は完了対象外
  if (features.length === 0) reasons.push("no type:feature issues in milestone (empty Phase)");
  if (featuresOpen.length > 0) reasons.push(`open type:feature: ${featuresOpen.map(n => `#${n}`).join(", ")}`);
  if (gateHF.length > 0) reasons.push(`gate:human-feel x ${gateHF.length}`);
  if (gateADR.length > 0) reasons.push(`gate:adr-review x ${gateADR.length}`);
  if (marked) reasons.push("ROADMAP already marked ✅ (already applied?)");

  const ok = features.length > 0 && featuresOpen.length === 0 && gateHF.length === 0 && gateADR.length === 0 && !marked;
  return {
    ok, phase,
    reason: ok ? "all complete" : reasons.join("; "),
    milestone: { number: ms.number, title: ms.title, state: ms.state },
    features_total: features.length,
    features_open: featuresOpen,
    gate_human_feel: gateHF.map(i => i.number),
    gate_adr_review: gateADR.map(i => i.number),
    roadmap_already_marked: marked,
  };
}

function atomicRewriteRoadmap(phase: number): void {
  const text = readFileSync(ROADMAP_PATH, "utf-8");
  const newText = text.replace(
    new RegExp(`^(##\\s+)Phase\\s+${phase}(?!\\d)`, "m"),
    `$1✅ Phase ${phase}`,
  );
  if (text === newText) throw new Error("ROADMAP rewrite no-op (already marked or pattern mismatch)");
  const tmp = `${ROADMAP_PATH}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, newText, "utf-8");
  renameSync(tmp, ROADMAP_PATH);
}

async function closeMilestone(num: number): Promise<boolean> {
  const r = await runGh([
    "api", "-X", "PATCH",
    `repos/{owner}/{repo}/milestones/${num}`,
    "-F", "state=closed",
  ]);
  return r.exit === 0;
}

async function recordDecision(message: string): Promise<void> {
  const scriptPath = ".claude/skills/3ailoop/scripts/loop-decision-log.ts";
  if (!existsSync(scriptPath)) return;
  const proc = Bun.spawn(["bun", scriptPath, "append", "--kind", "phase-transition", "--message", message]);
  await proc.exited;
}

async function reopenMilestone(num: number): Promise<boolean> {
  const r = await runGh([
    "api", "-X", "PATCH",
    `repos/{owner}/{repo}/milestones/${num}`,
    "-F", "state=open",
  ]);
  return r.exit === 0;
}

async function apply(phase: number, dryRun: boolean): Promise<{ ok: boolean; actions: string[]; reason: string }> {
  const c = await check(phase);
  if (!c.ok) return { ok: false, actions: [], reason: c.reason };

  const actions: string[] = [];
  if (dryRun) {
    actions.push(`[dry-run] would close milestone #${c.milestone!.number} (${c.milestone!.title})`);
    actions.push(`[dry-run] would rewrite ROADMAP.md: '## Phase ${phase}' → '## ✅ Phase ${phase}'`);
    actions.push(`[dry-run] would append decision-log: phase-transition Phase ${phase} 完了`);
    return { ok: true, actions, reason: "dry-run" };
  }

  // #177 指摘 1 対応: milestone close を先行、失敗時は ROADMAP に触らない
  if (!c.milestone) return { ok: false, actions, reason: "milestone missing in apply phase" };
  const closed = await closeMilestone(c.milestone.number);
  if (!closed) {
    return { ok: false, actions, reason: `failed to close milestone #${c.milestone.number} (ROADMAP untouched, no progression)` };
  }
  actions.push(`closed milestone #${c.milestone.number}`);

  // ROADMAP rewrite。失敗時は milestone を reopen して整合性回復
  try {
    atomicRewriteRoadmap(phase);
    actions.push(`rewrote ROADMAP.md`);
  } catch (e) {
    const reopened = await reopenMilestone(c.milestone.number);
    actions.push(reopened ? `rolled back: reopened milestone #${c.milestone.number}` : `WARNING: reopen failed for milestone #${c.milestone.number}`);
    return { ok: false, actions, reason: `ROADMAP rewrite failed: ${(e as Error).message}` };
  }

  await recordDecision(`Phase ${phase} 完了 (milestone closed, ROADMAP ✅化)`);
  actions.push(`appended decision-log`);

  // #319 Phase E: Phase 11/14/17/20 完了時に 3 系統 adversarial audit を invoke
  // (旧 Fable 5 監査から置換、memory:fable-5-banned)
  if (PHASE_AUDIT_PHASES.has(phase)) {
    const audit = await invokePhaseAudit(phase);
    if (audit.ok) {
      const issueList = (audit.issues ?? []).map(n => `#${n}`).join(", ") || "0 件";
      actions.push(`invoked 3-系統 phase audit → ${issueList}`);
      await recordDecision(`Phase ${phase} 3 系統 adversarial audit 実行 (created: ${issueList})`);
    } else {
      actions.push(`WARNING: phase audit invoke failed: ${audit.error}`);
    }
  }

  return { ok: true, actions, reason: "applied" };
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;
  function arg(name: string): string | undefined {
    const i = rest.indexOf(name);
    return i >= 0 ? rest[i + 1] : undefined;
  }
  function flag(name: string): boolean { return rest.includes(name); }

  const phaseArg = arg("--phase");
  const phase = phaseArg ? parseInt(phaseArg) : detectCurrentPhase();
  if (!phase || isNaN(phase)) {
    console.error("ERROR: --phase が指定されておらず ROADMAP から現 Phase を検出できません");
    process.exit(2);
  }

  if (cmd === "check") {
    const r = await check(phase);
    console.log(JSON.stringify(r, null, 2));
    process.exit(r.ok ? 0 : 1);
  } else if (cmd === "apply") {
    const dryRun = flag("--dry-run");
    const r = await apply(phase, dryRun);
    console.log(JSON.stringify(r, null, 2));
    process.exit(r.ok ? 0 : 1);
  } else {
    console.error("Usage: loop-phase-close-check.ts (check | apply) [--phase N] [--dry-run]");
    process.exit(2);
  }
}
