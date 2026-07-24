#!/usr/bin/env bun
// loop-phase-audit.ts — Phase 11/14/17/20 完了時の 3 系統独立 adversarial audit
//
// Fable 5 全体監査 (feedback_fable_5_banned で禁止) の置換。相関的な blind spot を
// 崩すため、Anthropic (Opus 4.7) + OpenAI (Codex) + Z.AI (GLM) の 3 系統から
// 独立に監査を走らせ、findings を集約する。
//
// MVP (この初期版) の重要な注意:
//   - 実 LLM dispatch は wire していない。Stage 2 は pure-script simulation。
//   - 各系統は空 findings を返す骨格のみ。Phase 11 が近づいたら wire する。
//   - script の shape (aggregate / categorize / render) はすでに全部 testable な
//     形で切り出してあるため、実 dispatch を差し込むだけで済むよう設計してある。
//
// CLI:
//   bun loop-phase-audit.ts --phase <N> [--dry-run] [--depth <M>]
//
// exit code:
//   0 = audit 完了 (dry-run は必ず 0、実行時は Issue 起票が最後まで通れば 0)
//   1 = 内部エラー (I/O 失敗等)
//   2 = --phase が FABLE5_AUDIT_PHASES に無い

import { appendFileSync, existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname } from "path";
import { FABLE5_AUDIT_PHASES } from "./loop-phase-close-check.ts";

const DEFAULT_DEPTH = 3;
const AUDIT_LOG_PATH = "features/.loop/phase-audit-log.jsonl";
const AUDIT_INPUT_DIR = "features/.loop"; // subdir per phase appended below
const ISSUE_CAP = 5;

// --- 型 ---------------------------------------------------------------------

export type Severity = "critical" | "high" | "medium" | "low";

export interface Finding {
  /** 系統名: opus / codex-architect / codex-contrarian / codex-migration / glm-scope 等 */
  source: string;
  /** severity */
  severity: Severity;
  /** file:line 形式 (dedupe key の一部) */
  location: string;
  /** 1 行サマリ (dedupe key の一部) */
  summary: string;
  /** 詳細 (Issue body に埋め込む) */
  detail?: string;
}

export interface AdrChange {
  sha: string;
  path: string;
  message: string;
}

export interface ImplChange {
  sha: string;
  message: string;
}

export interface ScopeRange {
  fromPhase: number;
  toPhase: number;
}

export interface AggregatedFindings {
  /** dedupe / merge 後の findings (source は "|" で連結される) */
  findings: Finding[];
  /** 統計 (dedupe 前の source 別件数) */
  counts_raw: {
    opus: number;
    codex: number; // 3 persona 合算
    glm: number; // 3 persona 合算
  };
}

export interface CategorizedFindings {
  critical: Finding[];
  high: Finding[];
  medium: Finding[];
  low: Finding[];
}

export interface IssueDraft {
  title: string;
  body: string;
  labels: string[];
  severity: Severity;
}

export interface AuditResult {
  phase: number;
  scope: ScopeRange;
  aggregated: AggregatedFindings;
  categorized: CategorizedFindings;
  drafts: IssueDraft[];
  created_issues: number[];
  input_file: string;
}

// --- Stage 1: scope + input 準備 --------------------------------------------

/** Phase N を含む直近 depth Phase (default 3) の範囲を返す。
 *  depth=3, N=11 → { from: 9, to: 11 }。from が 1 未満なら 1 で clamp。 */
export function computeScopeRange(phase: number, depth: number = DEFAULT_DEPTH): ScopeRange {
  const from = Math.max(1, phase - depth + 1);
  return { fromPhase: from, toPhase: phase };
}

/** git log --pretty=format:"%h %s" --name-only -- docs/decisions/ の出力を parse する。
 *
 *  想定形式:
 *    <sha> <subject>
 *    docs/decisions/001-foo.md
 *    docs/decisions/002-bar.md
 *
 *    <sha> <subject>
 *    docs/decisions/003-baz.md
 *
 *  scope は現状 filter に使わない (git log 側で --since / --until が絞る前提)。
 *  引数として受け取っておくことで、後で「commit 内から phase を推定して filter」
 *  したくなったときに breaking change なしに拡張できる。 */
export function parseAdrChanges(gitLogOutput: string, _scope: ScopeRange): AdrChange[] {
  const changes: AdrChange[] = [];
  const chunks = gitLogOutput.split(/\n\s*\n/);
  for (const chunk of chunks) {
    const lines = chunk.split("\n").filter(l => l.length > 0);
    if (lines.length === 0) continue;
    const header = lines[0].match(/^([0-9a-f]+)\s+(.*)$/);
    if (!header) continue;
    const sha = header[1];
    const message = header[2];
    for (const path of lines.slice(1)) {
      if (path.startsWith("docs/decisions/")) {
        changes.push({ sha, path, message });
      }
    }
  }
  return changes;
}

/** git log --pretty=format:"%h %s" (--name-only なし) を parse。impl diff の
 *  代表 commit 一覧を得る。 */
export function parseImplDiff(gitLogOutput: string, _scope: ScopeRange): ImplChange[] {
  const changes: ImplChange[] = [];
  for (const line of gitLogOutput.split("\n")) {
    const m = line.match(/^([0-9a-f]+)\s+(.*)$/);
    if (!m) continue;
    changes.push({ sha: m[1], message: m[2] });
  }
  return changes;
}

/** Stage 1 で作る input.md の中身を組み立てる。実 LLM dispatch 時にこの文字列を
 *  そのまま prompt に流し込む想定。 */
export function buildInputMarkdown(
  phase: number,
  scope: ScopeRange,
  adrChanges: AdrChange[],
  implChanges: ImplChange[],
  journalExcerpt: string,
): string {
  const lines: string[] = [];
  lines.push(`# Phase ${phase} 監査入力 (直近 Phase ${scope.fromPhase}〜${scope.toPhase})`);
  lines.push("");
  lines.push("## ADR 変更 (docs/decisions/)");
  lines.push("");
  if (adrChanges.length === 0) {
    lines.push("- (該当なし)");
  } else {
    for (const c of adrChanges) {
      lines.push(`- ${c.sha} \`${c.path}\` — ${c.message}`);
    }
  }
  lines.push("");
  lines.push("## 実装 commit (代表)");
  lines.push("");
  if (implChanges.length === 0) {
    lines.push("- (該当なし)");
  } else {
    // 上限で切る (prompt 肥大化回避)
    const shown = implChanges.slice(0, 200);
    for (const c of shown) {
      lines.push(`- ${c.sha} ${c.message}`);
    }
    if (implChanges.length > shown.length) {
      lines.push(`- ... (${implChanges.length - shown.length} more)`);
    }
  }
  lines.push("");
  lines.push("## Loop 運用メトリクス (cycle-journal.log 抜粋)");
  lines.push("");
  lines.push("```");
  lines.push(journalExcerpt || "(該当なし)");
  lines.push("```");
  return lines.join("\n");
}

// --- Stage 2: 3 系統 dispatch -----------------------------------------------

export type DispatchFn = (input: string) => Promise<Finding[]>;

/** DI 用の dispatch 集合。runAudit で override 可能。default は pure-script stub。 */
export interface DispatchDeps {
  opus: DispatchFn;
  codex: {
    architect: DispatchFn;
    contrarian: DispatchFn;
    migration: DispatchFn;
  };
  glm: {
    architect: DispatchFn;
    contrarian: DispatchFn;
    migration: DispatchFn;
  };
}

/** MVP 用 stub: 空 findings。実 LLM wire は Phase 11 が近づいたら実施。
 *  TODO(Phase 11 直前): 実 LLM dispatch を wire する:
 *    - opus: `claude --model claude-opus-4-7 --dangerously-skip-permissions -p <prompt>`
 *      cf. feedback_3ai_billing は per-Issue impl 用の制約であり、per-Phase の
 *      meta audit (最大 4 回) は別枠。1 回 / audit の cap は本 script 側で担保。
 *    - codex: dispatch-codex.ts を Bun.spawn で 3 persona 並列起動
 *    - glm: dispatch-glm-review.ts を Bun.spawn で 3 persona 並列起動 */
export function defaultDispatchDeps(): DispatchDeps {
  const empty: DispatchFn = async () => [];
  return {
    opus: empty,
    codex: { architect: empty, contrarian: empty, migration: empty },
    glm: { architect: empty, contrarian: empty, migration: empty },
  };
}

/** 3 系統を Promise.all で並列 dispatch。返値は persona 単位の Finding[] リスト。 */
export async function runAllDispatches(
  input: string,
  deps: DispatchDeps,
): Promise<{ opus: Finding[]; codex: Finding[][]; glm: Finding[][] }> {
  const [opus, codexA, codexC, codexM, glmA, glmC, glmM] = await Promise.all([
    deps.opus(input),
    deps.codex.architect(input),
    deps.codex.contrarian(input),
    deps.codex.migration(input),
    deps.glm.architect(input),
    deps.glm.contrarian(input),
    deps.glm.migration(input),
  ]);
  return {
    opus,
    codex: [codexA, codexC, codexM],
    glm: [glmA, glmC, glmM],
  };
}

// --- Stage 3: 集約 ----------------------------------------------------------

/** location + summary をキーに dedupe。同じ finding が複数系統から来た場合、
 *  source を "|" で連結して 1 件に merge。severity は最大 (critical > ... > low)。 */
export function aggregateFindings(
  opus: Finding[],
  codex: Finding[][],
  glm: Finding[][],
): AggregatedFindings {
  const codexFlat = codex.flat();
  const glmFlat = glm.flat();

  const map = new Map<string, Finding>();
  const push = (f: Finding): void => {
    const key = `${f.location}::${f.summary}`;
    const cur = map.get(key);
    if (!cur) {
      map.set(key, { ...f });
      return;
    }
    // merge: source を連結、severity は max、detail は先着優先
    const merged: Finding = {
      source: cur.source.includes(f.source) ? cur.source : `${cur.source}|${f.source}`,
      severity: maxSeverity(cur.severity, f.severity),
      location: cur.location,
      summary: cur.summary,
      detail: cur.detail ?? f.detail,
    };
    map.set(key, merged);
  };

  for (const f of opus) push(f);
  for (const f of codexFlat) push(f);
  for (const f of glmFlat) push(f);

  return {
    findings: [...map.values()],
    counts_raw: {
      opus: opus.length,
      codex: codexFlat.length,
      glm: glmFlat.length,
    },
  };
}

const SEVERITY_ORDER: Record<Severity, number> = { critical: 3, high: 2, medium: 1, low: 0 };

function maxSeverity(a: Severity, b: Severity): Severity {
  return SEVERITY_ORDER[a] >= SEVERITY_ORDER[b] ? a : b;
}

export function categorizeFindings(agg: AggregatedFindings): CategorizedFindings {
  const out: CategorizedFindings = { critical: [], high: [], medium: [], low: [] };
  for (const f of agg.findings) {
    out[f.severity].push(f);
  }
  return out;
}

// --- Stage 3: Issue draft rendering -----------------------------------------

/** 1 finding = 1 Issue で render。critical → high → medium の順で並べる。
 *  low は Issue 化しない (log のみ)。全体 ISSUE_CAP 件で切る。
 *  critical は `type: foundation, batch:kernel` (即修正)、
 *  high/medium は `type: foundation, batch:kernel` + `defer:phase-<next>` (Phase N+1 に消化)。 */
export function renderAuditIssues(
  categorized: CategorizedFindings,
  phase: number,
  scope: ScopeRange,
): IssueDraft[] {
  const drafts: IssueDraft[] = [];
  const nextPhase = phase + 1;

  const addAll = (findings: Finding[], severity: Severity, deferLabel: string | null): void => {
    for (const f of findings) {
      const labels = ["type: foundation", "batch:kernel"];
      if (deferLabel) labels.push(deferLabel);
      drafts.push({
        title: renderTitle(severity, phase, f),
        body: renderBody(severity, phase, scope, f),
        labels,
        severity,
      });
    }
  };

  // 順序: critical → high → medium。cap は最後に適用する。
  addAll(categorized.critical, "critical", null);
  addAll(categorized.high, "high", `defer:phase-${nextPhase}`);
  addAll(categorized.medium, "medium", `defer:phase-${nextPhase}`);

  return drafts.slice(0, ISSUE_CAP);
}

function renderTitle(severity: Severity, phase: number, f: Finding): string {
  const trimmed = f.summary.length > 60 ? f.summary.slice(0, 57) + "..." : f.summary;
  return `audit(phase${phase},${severity}): ${trimmed}`;
}

function renderBody(severity: Severity, phase: number, scope: ScopeRange, f: Finding): string {
  return [
    `## 概要`,
    "",
    `Phase ${phase} 完了時 3 系統独立 adversarial audit (直近 Phase ${scope.fromPhase}〜${scope.toPhase}) で検出された ${severity} finding。`,
    "",
    `## 検出系統`,
    "",
    `- ${f.source}`,
    "",
    `## 場所`,
    "",
    `- ${f.location}`,
    "",
    `## サマリ`,
    "",
    f.summary,
    "",
    ...(f.detail ? [`## 詳細`, "", f.detail, ""] : []),
    `## 関連`,
    "",
    `- ADR-013 (Phase 監査自動起票)`,
    `- memory: fable-5-banned (旧 Fable 5 監査からの移行)`,
  ].join("\n");
}

// --- CLI + I/O --------------------------------------------------------------

export interface RunOpts {
  phase: number;
  dryRun: boolean;
  depth: number;
}

export interface RunDeps {
  dispatch: DispatchDeps;
  /** git log --pretty=format:"%h %s" --name-only -- docs/decisions/ 相当 */
  runGitAdrLog: () => Promise<string>;
  /** git log --pretty=format:"%h %s" 相当 (broad range) */
  runGitImplLog: () => Promise<string>;
  /** cycle-journal.log 抜粋 */
  readJournalExcerpt: () => Promise<string>;
  /** gh issue create wrapper。成功時は Issue 番号を返す */
  createIssue: (draft: IssueDraft) => Promise<number | null>;
  /** phase-audit-log.jsonl 用 append (test 時は差し替えられる) */
  appendLog: (entry: Record<string, unknown>) => void;
  /** input.md 書き出し用 */
  writeInputFile: (path: string, body: string) => void;
}

export function defaultRunDeps(): RunDeps {
  return {
    dispatch: defaultDispatchDeps(),
    runGitAdrLog: async () => runGitLog(["--pretty=format:%h %s", "--name-only", "--", "docs/decisions/"]),
    runGitImplLog: async () => runGitLog(["--pretty=format:%h %s", "-n", "500"]),
    readJournalExcerpt: async () => {
      const p = "features/.loop/cycle-journal.log";
      if (!existsSync(p)) return "";
      const raw = await Bun.file(p).text();
      const lines = raw.split("\n");
      return lines.slice(-200).join("\n");
    },
    createIssue: async (draft) => {
      const proc = Bun.spawn(
        ["gh", "issue", "create", "--title", draft.title, "--body", draft.body, "--label", draft.labels.join(",")],
        { stdout: "pipe", stderr: "pipe" },
      );
      const out = (await new Response(proc.stdout).text()).trim();
      await proc.exited;
      if (proc.exitCode !== 0) return null;
      const m = out.match(/\/issues\/(\d+)$/);
      return m ? parseInt(m[1], 10) : null;
    },
    appendLog: (entry) => {
      try {
        mkdirSync(dirname(AUDIT_LOG_PATH), { recursive: true });
        appendFileSync(AUDIT_LOG_PATH, JSON.stringify(entry) + "\n", "utf-8");
      } catch (e) {
        process.stderr.write(`WARN: phase-audit-log append failed: ${(e as Error).message}\n`);
      }
    },
    writeInputFile: (path, body) => {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, body, "utf-8");
    },
  };
}

async function runGitLog(args: string[]): Promise<string> {
  const proc = Bun.spawn(["git", "log", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return out;
}

export async function runAudit(opts: RunOpts, deps: RunDeps): Promise<AuditResult> {
  const scope = computeScopeRange(opts.phase, opts.depth);

  // Stage 1: input 収集
  const [adrLog, implLog, journal] = await Promise.all([
    deps.runGitAdrLog(),
    deps.runGitImplLog(),
    deps.readJournalExcerpt(),
  ]);
  const adrChanges = parseAdrChanges(adrLog, scope);
  const implChanges = parseImplDiff(implLog, scope);
  const inputMd = buildInputMarkdown(opts.phase, scope, adrChanges, implChanges, journal);
  const inputPath = `${AUDIT_INPUT_DIR}/phase-audit-${opts.phase}/input.md`;
  deps.writeInputFile(inputPath, inputMd);

  // Stage 2: 3 系統 dispatch (MVP は stub)
  const dispatched = await runAllDispatches(inputMd, deps.dispatch);

  // Stage 3: 集約
  const aggregated = aggregateFindings(dispatched.opus, dispatched.codex, dispatched.glm);
  const categorized = categorizeFindings(aggregated);
  const drafts = renderAuditIssues(categorized, opts.phase, scope);

  // Stage 3: Issue 起票 (dry-run 時は skip)
  const createdIssues: number[] = [];
  if (!opts.dryRun) {
    for (const draft of drafts) {
      const n = await deps.createIssue(draft);
      if (n !== null) createdIssues.push(n);
    }
  }

  // Stage 4: log
  const entry = {
    phase: opts.phase,
    timestamp: new Date().toISOString(),
    scope: `Phase ${scope.fromPhase}-${scope.toPhase}`,
    findings: {
      critical: categorized.critical.length,
      high: categorized.high.length,
      medium: categorized.medium.length,
      low: categorized.low.length,
    },
    created_issues: createdIssues,
    dry_run: opts.dryRun,
  };
  if (!opts.dryRun) deps.appendLog(entry);

  return {
    phase: opts.phase,
    scope,
    aggregated,
    categorized,
    drafts,
    created_issues: createdIssues,
    input_file: inputPath,
  };
}

// --- CLI entry --------------------------------------------------------------

interface CliOpts {
  phase: number;
  dryRun: boolean;
  depth: number;
}

function parseArgs(argv: string[]): CliOpts | { error: string } {
  const rest = argv.slice(2);
  const arg = (name: string): string | undefined => {
    const i = rest.indexOf(name);
    return i >= 0 ? rest[i + 1] : undefined;
  };
  const flag = (name: string): boolean => rest.includes(name);

  const phaseStr = arg("--phase");
  if (!phaseStr) return { error: "--phase is required" };
  const phase = parseInt(phaseStr, 10);
  if (!Number.isFinite(phase) || phase <= 0) return { error: `--phase must be a positive int, got: ${phaseStr}` };

  const depthStr = arg("--depth");
  const depth = depthStr ? parseInt(depthStr, 10) : DEFAULT_DEPTH;
  if (!Number.isFinite(depth) || depth <= 0) return { error: `--depth must be a positive int, got: ${depthStr}` };

  return { phase, dryRun: flag("--dry-run"), depth };
}

async function main(): Promise<number> {
  const parsed = parseArgs(process.argv);
  if ("error" in parsed) {
    console.error(`ERROR: ${parsed.error}`);
    console.error("Usage: loop-phase-audit.ts --phase <N> [--dry-run] [--depth <M>]");
    return 2;
  }

  if (!FABLE5_AUDIT_PHASES.has(parsed.phase)) {
    console.error(
      `ERROR: --phase ${parsed.phase} is not in FABLE5_AUDIT_PHASES (${[...FABLE5_AUDIT_PHASES].join(", ")})`,
    );
    return 2;
  }

  const deps = defaultRunDeps();
  const result = await runAudit(parsed, deps);

  // stdout: 人間向け summary + drafts のプレビュー (dry-run 時は起票せず)
  console.log(`# phase-audit result (phase=${result.phase}, scope=Phase ${result.scope.fromPhase}-${result.scope.toPhase})`);
  console.log(`  input:   ${result.input_file}`);
  console.log(`  opus:    ${result.aggregated.counts_raw.opus} findings`);
  console.log(`  codex:   ${result.aggregated.counts_raw.codex} findings (3 persona 合算)`);
  console.log(`  glm:     ${result.aggregated.counts_raw.glm} findings (3 persona 合算)`);
  console.log(
    `  aggregated: critical=${result.categorized.critical.length} high=${result.categorized.high.length} medium=${result.categorized.medium.length} low=${result.categorized.low.length}`,
  );
  console.log(`  planned issues: ${result.drafts.length} (cap ${ISSUE_CAP})`);
  console.log("");

  if (parsed.dryRun) {
    console.log("=== [DRY-RUN] planned dispatch (3 系統) ===");
    console.log("  - Opus 4.7 (Anthropic) — 集約役兼任 (MVP: pure-script stub)");
    console.log("  - Codex 3 persona (OpenAI): architect / contrarian / migration (MVP: pure-script stub)");
    console.log("  - GLM 3 persona (Z.AI): architect / contrarian / migration (MVP: pure-script stub)");
    console.log("");
    console.log("=== [DRY-RUN] input.md preview (先頭 40 行) ===");
    try {
      const inputText = await Bun.file(result.input_file).text();
      const head = inputText.split("\n").slice(0, 40).join("\n");
      console.log(head);
    } catch {
      console.log("(input.md 読み込み失敗)");
    }
    console.log("");
  }

  console.log("=== planned issue drafts ===");
  if (result.drafts.length === 0) {
    console.log("  (none)");
  } else {
    for (const d of result.drafts) {
      console.log(`  [${d.severity}] ${d.title}`);
      console.log(`    labels: ${d.labels.join(",")}`);
    }
  }

  if (!parsed.dryRun) {
    console.log("");
    console.log(`=== created issues: ${result.created_issues.map(n => `#${n}`).join(", ") || "(none)"} ===`);
  }

  return 0;
}

if (import.meta.main) {
  const rc = await main();
  process.exit(rc);
}
