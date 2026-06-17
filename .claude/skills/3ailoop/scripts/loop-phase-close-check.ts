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

const ROADMAP_PATH = "ROADMAP.md";

// ADR-013: 3 Phase ごとの Fable 5 全体監査トリガー
export const FABLE5_AUDIT_PHASES = new Set([11, 14, 17, 20]);

async function raiseFable5AuditIssue(phase: number): Promise<{ ok: boolean; issue?: number; error?: string }> {
  const title = `audit(fable5): Phase ${phase} 完了監査 — 直近 3 Phase の ADR + 実装乖離 + 過去 ADR 矛盾`;
  const body = [
    `## 概要`,
    "",
    `ADR-013 の規定により、Phase ${phase} 完了時に Fable 5 で全体監査を実施する。`,
    "",
    `## 監査対象`,
    "",
    `直近 3 Phase (Phase ${phase - 2}〜${phase}) の以下を Fable 5 で審査:`,
    "",
    `- 新規 / 改訂 ADR の整合性 (過去 ADR との矛盾、採用前提崩壊 trigger の発火可能性)`,
    `- 実装と ADR の乖離 (ADR で決めた性質を破る実装)`,
    `- 過去 ADR の前提崩壊 (Phase 進行で前提が変わった可能性)`,
    `- ADR 自動 accept フロー (ADR-013) の判定品質 (Phase ${phase - 2}〜${phase} で accept された ADR の事後妥当性)`,
    "",
    `## 出力`,
    "",
    `- critical: 即座に修正 Issue 起票 (loop が消化)`,
    `- high / medium: 別 Issue 起票して Phase ${phase + 1} 中に消化`,
    `- low: ADR-013 採用前提崩壊 trigger の発火判定材料として記録`,
    "",
    `## 関連`,
    "",
    `- ADR-013 (ADR 自動 accept フロー)`,
    `- memory: feedback_fable5_strategy`,
  ].join("\n");

  const proc = Bun.spawn(
    [
      "gh", "issue", "create",
      "--title", title,
      "--body", body,
      "--label", "type: foundation,batch:kernel",
    ],
    { stdout: "pipe", stderr: "pipe" },
  );
  const out = (await new Response(proc.stdout).text()).trim();
  const err = (await new Response(proc.stderr).text()).trim();
  await proc.exited;
  if (proc.exitCode !== 0) {
    return { ok: false, error: err || `gh exit ${proc.exitCode}` };
  }
  const m = out.match(/\/issues\/(\d+)$/);
  return { ok: true, issue: m ? parseInt(m[1]) : undefined };
}

function detectCurrentPhase(): number | null {
  if (!existsSync(ROADMAP_PATH)) return null;
  for (const line of readFileSync(ROADMAP_PATH, "utf-8").split("\n")) {
    const m = line.match(/^##\s+(?:✅\s+)?Phase\s+(\d+)/);
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

async function check(phase: number): Promise<CheckResult> {
  const ms = await fetchMilestone(phase);
  if (!ms) {
    return {
      ok: false, phase, reason: `milestone for Phase ${phase} not found`,
      milestone: null, features_total: 0, features_open: [],
      gate_human_feel: [], gate_adr_review: [], roadmap_already_marked: false,
    };
  }
  const issues = await fetchIssuesByMilestone(ms.number);
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

  // ADR-013: Phase 11/14/17/20 完了時に Fable 5 監査 Issue を起票
  if (FABLE5_AUDIT_PHASES.has(phase)) {
    const audit = await raiseFable5AuditIssue(phase);
    if (audit.ok) {
      actions.push(`raised Fable 5 audit issue #${audit.issue ?? "?"}`);
      await recordDecision(`Phase ${phase} Fable 5 監査 Issue #${audit.issue ?? "?"} を起票 (ADR-013)`);
    } else {
      actions.push(`WARNING: Fable 5 audit issue create failed: ${audit.error}`);
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
