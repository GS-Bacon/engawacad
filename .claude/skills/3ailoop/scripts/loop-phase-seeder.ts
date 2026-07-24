#!/usr/bin/env bun
// loop-phase-seeder.ts — Phase 促進時に ROADMAP 完了条件を起点 Issue に展開する (L-1.7)
//
// #315 Phase B: current_phase が bump したとき、新 Phase の milestone に actionable な
// 起点 Issue が 1 件もない状態を回避する。放置すると /3ailoop L-2 で actionable=0 →
// pause 連続 → 実観測 5 日停滞 (2026-07-01 → 07-06 Phase 10 移行時) に至る。
//
// 使い方:
//   bun loop-phase-seeder.ts --phase N [--dry-run] [--milestone-title "Phase N: <title>"]
//
// 動作:
//   1. ROADMAP.md から `## Phase N: <title>` を探す (✅ 付きは拒否 = 完了済み Phase の seed 禁止)
//   2. `**完了条件**:` ブロックの top-level `-` を条件として抽出
//   3. 各条件を Issue draft に純関数変換 (LLM 呼び出しなし。billing 影響ゼロ、決定的)
//   4. check-issue-granularity で粒度検証、no + split_proposal なら 1 回だけ children 展開
//   5. lint-issue-labels でラベル検証
//   6. dry-run: stdout に計画出力 / 通常: gh issue create + phase-seeder-log.jsonl append
//
// 設計上の注意:
//   - LLM を呼ばない (memory: feedback_3ai_billing "Anthropic claude -p の spawn は NG")
//     Phase seeder は Issue 起票の meta-work であり /3ai 実装サイクルの中核ではないが、
//     billing 疑義を回避しシンプルさ優先で純関数実装 (Phase C+ で必要なら Sonnet subagent 化)
//   - milestone は事前に存在している前提 (loop-phase-close-check.ts or 手動セットアップ)
//   - dry-run では gh は一切叩かない (テスト容易性 + 安全)

import { appendFileSync, existsSync, mkdirSync, readFileSync } from "fs";
import { dirname } from "path";
import {
  checkGranularity,
  type CheckResult,
  type IssueMeta,
  type SplitChild,
} from "../../3ai/scripts/check-issue-granularity";
import { lintLabels } from "../../3ai/scripts/lint-issue-labels";

const ROADMAP_PATH = "ROADMAP.md";
const SEEDER_LOG_PATH = "features/.loop/phase-seeder-log.jsonl";

export interface PhaseContext {
  phase: number;
  title: string;
  visibleOutcome: string | null;
  adrRefs: string[];
}

export interface ParsedPhase {
  found: boolean;
  completed: boolean;
  title: string;
  visibleOutcome: string | null;
  adrRefs: string[];
  conditions: string[];
}

/** ROADMAP テキストから指定 Phase の完了条件セクションを抜き出す。
 *  top-level `-` (行頭) のみを 1 条件として扱う。indent した子 bullet は同一条件の
 *  一部として無視する (次の top-level に到達するまで前の条件が続く扱いだが、条件
 *  本文自体は 1 行目のみを採用 = 短くする)。
 */
export function parseCompletionConditions(roadmapText: string, phase: number): ParsedPhase {
  const lines = roadmapText.split("\n");
  const headerRe = new RegExp(`^##\\s+(✅\\s+)?Phase\\s+${phase}(?!\\d)\\s*:?\\s*(.*)$`);

  let start = -1;
  let headerLine = "";
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(headerRe);
    if (m) {
      start = i;
      headerLine = lines[i];
      break;
    }
  }
  if (start < 0) {
    return {
      found: false,
      completed: false,
      title: "",
      visibleOutcome: null,
      adrRefs: [],
      conditions: [],
    };
  }

  const completed = /✅/.test(headerLine);
  const titleMatch = headerLine.match(headerRe);
  const title = (titleMatch?.[2] ?? "").trim();

  // section end: 次の "## " (Phase 見出しか総括見出し)、または "---" separator、または EOF
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i++) {
    const l = lines[i];
    if (/^##\s+/.test(l)) { end = i; break; }
    if (/^---\s*$/.test(l)) { end = i; break; }
  }

  const section = lines.slice(start + 1, end);

  // visibleOutcome: `**外から見た成果**: <text>` (単一行)
  let visibleOutcome: string | null = null;
  for (const l of section) {
    const m = l.match(/^\*\*外から見た成果\*\*\s*[::]\s*(.+)$/);
    if (m) { visibleOutcome = m[1].trim(); break; }
  }

  // adrRefs: `**前提 ADR**: ...` の行から ADR-NNN or docs/decisions/NNN-... を集める
  const adrRefs: string[] = [];
  for (const l of section) {
    if (!/^\*\*前提 ADR\*\*/.test(l)) continue;
    const adrMatches = l.match(/ADR-\d+/g) ?? [];
    for (const a of adrMatches) if (!adrRefs.includes(a)) adrRefs.push(a);
    const docMatches = l.match(/docs\/decisions\/\d+-[a-z0-9-]+\.md/g) ?? [];
    for (const d of docMatches) if (!adrRefs.includes(d)) adrRefs.push(d);
  }

  // conditions: `**完了条件**:` の直後の block、top-level `-` bullet だけを採用
  const condIdx = section.findIndex(l => /^\*\*完了条件\*\*\s*[::]/.test(l));
  const conditions: string[] = [];
  if (condIdx >= 0) {
    for (let i = condIdx + 1; i < section.length; i++) {
      const l = section[i];
      // 次の bold marker で完了条件ブロック終了
      if (/^\*\*[^*]+\*\*/.test(l)) break;
      // top-level bullet
      const m = l.match(/^-\s+(.+?)\s*$/);
      if (m) {
        conditions.push(m[1].trim());
        continue;
      }
      // indent した bullet (`  -` `    -`) は無視 (親条件の一部)
      // 空行や他の内容も skip
    }
  }

  return {
    found: true,
    completed,
    title,
    visibleOutcome,
    adrRefs,
    conditions,
  };
}

/** 条件テキストから batch:* ラベルを推定する。
 *  優先順: skill (最も限定的な語) → data → viewer → kernel (default)
 */
export function guessBatchLabel(conditionText: string): "kernel" | "data" | "viewer" | "skill" {
  const t = conditionText;

  // skill: /3ai, 3ailoop, loop-X.ts, .claude/skills, dispatch-
  if (/\/3ai(loop|\b)|3ailoop|loop-[a-z]|\.claude\/skills|dispatch-/i.test(t)) return "skill";

  // data: .engawa, engawa-format, スキーマ, schema, YAML, フォーマット
  if (/\.engawa|engawa-format|スキーマ|schema|YAML|フォーマット/i.test(t)) return "data";

  // viewer: ブラウザ, viewer, UI, HTTP, Three\.js, フロント, engawa view
  if (/ブラウザ|viewer|\bUI\b|HTTP|Three\.js|フロント|engawa view/i.test(t)) return "viewer";

  return "kernel";
}

export interface IssueDraft {
  title: string;
  body: string;
  labels: string[];
  /** 分割後の origin: parent draft の condition text (log 用) */
  sourceCondition: string;
}

/** タイトル用に conditionText を短縮する。角括弧内の列挙は削除、末尾の "が動作" 等は保持。 */
function shortenConditionForTitle(text: string, maxLen: number): string {
  // 括弧内の英語 alias 列挙を除去 (例: "幾何拘束 (Horizontal / Vertical / ...) が動作")
  // 半角 () と全角 （） の両方に対応
  const stripped = text
    .replace(/\([^)]*\)/g, "")
    .replace(/（[^）]*）/g, "")
    .replace(/\s+/g, " ")
    .trim();
  if (stripped.length <= maxLen) return stripped;
  return stripped.slice(0, maxLen).trimEnd() + "…";
}

export function renderIssueDraft(condition: string, phaseCtx: PhaseContext): IssueDraft {
  const batch = guessBatchLabel(condition);
  const shortTitle = shortenConditionForTitle(condition, 50);
  const title = `feat(phase${phaseCtx.phase}): ${shortTitle}`;

  const adrLine = phaseCtx.adrRefs.length > 0
    ? `- 前提 ADR: ${phaseCtx.adrRefs.join(", ")}`
    : "";

  const outcomeLine = phaseCtx.visibleOutcome
    ? `\n**Phase ${phaseCtx.phase} 外から見た成果**: ${phaseCtx.visibleOutcome}\n`
    : "";

  const bodyLines = [
    "## 概要",
    "",
    `Phase ${phaseCtx.phase} (${phaseCtx.title}) の完了条件の 1 つ:`,
    "",
    `> ${condition}`,
    outcomeLine,
    "## In-Scope / Out-of-Scope",
    "",
    "| 項目 | Scope |",
    "|---|---|",
    `| ${condition} を engawa-kernel / engawa-format / engawa-build に実装 | In |`,
    `| 他の Phase ${phaseCtx.phase} 完了条件 | Out (別 Issue) |`,
    "",
    "## Non-Goals",
    "",
    "- 該当なし (詳細化は plan.md で行う)",
    "",
    "## 実装対象",
    "",
    "TBD (Claude が /3ai STEP 2 の plan.md で詳細化)",
    "",
    "## テスト計画",
    "",
    "TBD (Claude が /3ai STEP 2 の plan.md で詳細化)",
    "",
    "## 関連",
    "",
    "- 起点: /3ailoop L-1.7 Phase seeder (#315)",
    `- Phase: ROADMAP.md#phase-${phaseCtx.phase}`,
  ];
  if (adrLine) bodyLines.push(adrLine);

  return {
    title,
    body: bodyLines.filter(l => l !== null && l !== undefined).join("\n"),
    labels: ["type: feature", `batch:${batch}`],
    sourceCondition: condition,
  };
}

/** SplitChild -> IssueDraft (親 draft の phaseCtx を継承)。粒度チェックが no+split を
 *  返したときの子展開に使う。SplitChild の labels は check-issue-granularity が
 *  親のラベルから複写して返してくるので、そのまま採用 (足りなければ再 lint で弾かれる)。 */
function splitChildToDraft(child: SplitChild, sourceCondition: string): IssueDraft {
  return {
    title: child.title,
    body: child.body,
    labels: child.labels.length > 0 ? child.labels : ["type: feature", "batch:kernel"],
    sourceCondition,
  };
}

export type Checker = (meta: IssueMeta) => CheckResult;

export interface ExpandResult {
  finalDrafts: IssueDraft[];
  needsHuman: Array<{ sourceCondition: string; reason: string }>;
  splitCount: number;
}

/** 各 draft を粒度チェックし、no+split_proposal なら 1 度だけ children に展開する。
 *  子でまだ no なら needs-human に退避 (再帰なし)。 */
export function expandDrafts(drafts: IssueDraft[], checker: Checker): ExpandResult {
  const finalDrafts: IssueDraft[] = [];
  const needsHuman: Array<{ sourceCondition: string; reason: string }> = [];
  let splitCount = 0;

  for (const draft of drafts) {
    const meta: IssueMeta = { title: draft.title, body: draft.body, labels: draft.labels };
    const result = checker(meta);
    if (result.aligned === "yes" || result.aligned === "skip") {
      finalDrafts.push(draft);
      continue;
    }
    // aligned = no
    if (!result.split_proposal || result.split_proposal.length === 0) {
      needsHuman.push({
        sourceCondition: draft.sourceCondition,
        reason: `granularity check failed without split_proposal: ${result.reason ?? "(no reason)"}`,
      });
      continue;
    }
    splitCount += 1;
    for (const child of result.split_proposal) {
      const childDraft = splitChildToDraft(child, draft.sourceCondition);
      const childMeta: IssueMeta = {
        title: childDraft.title,
        body: childDraft.body,
        labels: childDraft.labels,
      };
      const childResult = checker(childMeta);
      if (childResult.aligned === "yes" || childResult.aligned === "skip") {
        finalDrafts.push(childDraft);
      } else {
        needsHuman.push({
          sourceCondition: draft.sourceCondition,
          reason: `child draft "${childDraft.title}" still aligned:no after 1 split (${childResult.reason ?? "no reason"})`,
        });
      }
    }
  }

  return { finalDrafts, needsHuman, splitCount };
}

export interface RunOpts {
  phase: number;
  dryRun: boolean;
  milestoneTitle?: string;
  roadmapPath?: string;
  logPath?: string;
}

export interface RunDeps {
  checker?: Checker;
  createIssue?: (title: string, body: string, labels: string[], milestone: string) => Promise<{ ok: boolean; number?: number; error?: string }>;
  now?: () => string;
  writeLog?: (path: string, line: string) => void;
}

export interface RunResult {
  exitCode: number;
  message: string;
  parsed?: ParsedPhase;
  plannedDrafts?: IssueDraft[];
  createdIssues?: number[];
  needsHuman?: Array<{ sourceCondition: string; reason: string }>;
  splitCount?: number;
}

async function defaultCreateIssue(
  title: string,
  body: string,
  labels: string[],
  milestone: string,
): Promise<{ ok: boolean; number?: number; error?: string }> {
  const proc = Bun.spawn(
    [
      "gh", "issue", "create",
      "--title", title,
      "--body", body,
      "--label", labels.join(","),
      "--milestone", milestone,
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
  return { ok: true, number: m ? parseInt(m[1]) : undefined };
}

function defaultWriteLog(path: string, line: string): void {
  const dir = dirname(path);
  if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
  appendFileSync(path, line + "\n", "utf-8");
}

export async function runSeeder(opts: RunOpts, deps: RunDeps = {}): Promise<RunResult> {
  const roadmapPath = opts.roadmapPath ?? ROADMAP_PATH;
  const logPath = opts.logPath ?? SEEDER_LOG_PATH;
  const checker = deps.checker ?? checkGranularity;
  const createIssue = deps.createIssue ?? defaultCreateIssue;
  const writeLog = deps.writeLog ?? defaultWriteLog;
  const now = deps.now ?? (() => new Date().toISOString());

  if (!existsSync(roadmapPath)) {
    return { exitCode: 2, message: `ROADMAP not found: ${roadmapPath}` };
  }
  const text = readFileSync(roadmapPath, "utf-8");
  const parsed = parseCompletionConditions(text, opts.phase);
  if (!parsed.found) {
    return {
      exitCode: 2,
      message: `Phase ${opts.phase} section not found in ${roadmapPath}`,
      parsed,
    };
  }
  if (parsed.completed) {
    return {
      exitCode: 2,
      message: `Phase ${opts.phase} is already marked ✅ (refusing to seed a completed phase)`,
      parsed,
    };
  }
  if (parsed.conditions.length === 0) {
    return {
      exitCode: 0,
      message: `no seed: Phase ${opts.phase} has empty 完了条件 block`,
      parsed,
      plannedDrafts: [],
      createdIssues: [],
      needsHuman: [],
      splitCount: 0,
    };
  }

  const phaseCtx: PhaseContext = {
    phase: opts.phase,
    title: parsed.title,
    visibleOutcome: parsed.visibleOutcome,
    adrRefs: parsed.adrRefs,
  };

  // 1) 各条件を draft に変換
  const rawDrafts = parsed.conditions.map(c => renderIssueDraft(c, phaseCtx));

  // 2) 粒度チェック + 1 回のみ split 展開
  const { finalDrafts, needsHuman, splitCount } = expandDrafts(rawDrafts, checker);

  // 3) ラベル検証 (lint に落ちる draft は needs-human に退避)
  const passingDrafts: IssueDraft[] = [];
  for (const d of finalDrafts) {
    const lr = lintLabels(d.labels);
    if (lr.ok) {
      passingDrafts.push(d);
    } else {
      needsHuman.push({
        sourceCondition: d.sourceCondition,
        reason: `label lint failed: ${lr.errors.join(" / ")}`,
      });
    }
  }

  const milestoneTitle = opts.milestoneTitle ?? `Phase ${opts.phase}: ${parsed.title}`;

  if (opts.dryRun) {
    const lines: string[] = [];
    lines.push(`[dry-run] Phase ${opts.phase}: ${parsed.title}`);
    lines.push(`[dry-run] milestone: "${milestoneTitle}"`);
    lines.push(`[dry-run] conditions: ${parsed.conditions.length}, final drafts: ${passingDrafts.length}, split: ${splitCount}, needs-human: ${needsHuman.length}`);
    lines.push("");
    passingDrafts.forEach((d, i) => {
      lines.push(`--- draft ${i + 1}/${passingDrafts.length} ---`);
      lines.push(`title:  ${d.title}`);
      lines.push(`labels: ${d.labels.join(", ")}`);
      lines.push(`body (first 20 lines):`);
      for (const bl of d.body.split("\n").slice(0, 20)) {
        lines.push(`  ${bl}`);
      }
      lines.push("");
    });
    if (needsHuman.length > 0) {
      lines.push("[dry-run] needs-human entries:");
      for (const nh of needsHuman) {
        lines.push(`  - ${nh.sourceCondition}: ${nh.reason}`);
      }
    }
    process.stdout.write(lines.join("\n") + "\n");
    return {
      exitCode: 0,
      message: `dry-run: ${passingDrafts.length} drafts planned`,
      parsed,
      plannedDrafts: passingDrafts,
      createdIssues: [],
      needsHuman,
      splitCount,
    };
  }

  // 4) 実起票
  const createdIssues: number[] = [];
  const failures: string[] = [];
  for (const d of passingDrafts) {
    const r = await createIssue(d.title, d.body, d.labels, milestoneTitle);
    if (r.ok && r.number) {
      createdIssues.push(r.number);
      process.stdout.write(`created #${r.number}: ${d.title}\n`);
    } else {
      failures.push(`${d.title}: ${r.error ?? "unknown"}`);
      process.stderr.write(`FAILED to create "${d.title}": ${r.error ?? "unknown"}\n`);
    }
  }

  // 5) log append
  const logEntry = {
    phase: opts.phase,
    timestamp: now(),
    created_issues: createdIssues,
    split_count: splitCount,
    needs_human_count: needsHuman.length,
    failures_count: failures.length,
  };
  try {
    writeLog(logPath, JSON.stringify(logEntry));
  } catch (e) {
    process.stderr.write(`WARN: failed to write seeder log: ${(e as Error).message}\n`);
  }

  return {
    exitCode: failures.length === 0 ? 0 : 1,
    message: `created ${createdIssues.length}/${passingDrafts.length} issues`,
    parsed,
    plannedDrafts: passingDrafts,
    createdIssues,
    needsHuman,
    splitCount,
  };
}

// --- CLI ---
if (import.meta.main) {
  const args = process.argv.slice(2);
  let phase = NaN;
  let dryRun = false;
  let milestoneTitle: string | undefined;

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--phase":
        phase = parseInt(args[++i]);
        break;
      case "--dry-run":
        dryRun = true;
        break;
      case "--milestone-title":
        milestoneTitle = args[++i];
        break;
      default:
        process.stderr.write(`Unknown arg: ${args[i]}\n`);
        process.exit(2);
    }
  }

  if (!phase || isNaN(phase)) {
    process.stderr.write(
      "Usage: loop-phase-seeder.ts --phase <N> [--dry-run] [--milestone-title \"Phase N: <title>\"]\n",
    );
    process.exit(2);
  }

  const r = await runSeeder({ phase, dryRun, milestoneTitle });
  if (!r.parsed?.found) {
    process.stderr.write(r.message + "\n");
  } else {
    process.stdout.write(r.message + "\n");
  }
  process.exit(r.exitCode);
}
