#!/usr/bin/env bun
// loop-phase-retrospective.ts — Phase N 完了後に loop 運用メトリクスを集計して
// retrospective Issue を起票する。3-系統 Fable5 監査 (Phase 11/14/17/20) を補完し、
// 「Phase ごと」に運用ドリフトの早期発見材料を残す。
//
// 使い方:
//   bun loop-phase-retrospective.ts --phase <N> [--dry-run]
//                                   [--journal <path>] [--skips <path>]
//                                   [--decisions <path>] [--roadmap <path>]
//
// 出力:
//   1) 集計した metrics JSON (stderr)
//   2) 起票した (または dry-run で起票予定の) Issue draft 一覧 (stdout)
//   3) features/.loop/phase-retro-log.jsonl に append (dry-run 時は skip)

import { appendFileSync, existsSync, mkdirSync, readFileSync } from "fs";
import { dirname } from "path";

const DEFAULT_JOURNAL_PATH = "features/.loop/cycle-journal.log";
const DEFAULT_SKIPS_PATH = "features/.loop/codex-skips.jsonl";
const DEFAULT_DECISIONS_PATH = "features/.loop/decisions.log.jsonl";
const DEFAULT_ROADMAP_PATH = "ROADMAP.md";
const RETRO_LOG_PATH = "features/.loop/phase-retro-log.jsonl";

// --- 型定義 -----------------------------------------------------------------

export interface Cycle {
  cycle: number;
  started_at: string;
  ended_at: string;
  closed: number[];
  raised: number[];
  merged_commits: number;
  adr_drafts?: string[];
  pause_reason?: string;
  tokens?: { claude: number; glm: number; codex: number };
}

export interface SkipEntry {
  issue: number;
  slug: string;
  step: string;
  reason: string;
  recorded_at: string;
  resolved_at: string | null;
}

export interface PhaseRange {
  startTime: string | null;
  endTime: string | null;
}

export interface PauseMetric {
  total: number;
  paused: number;
  rate: number;
  top_reasons: string[];
}

export interface NeedsHumanMetric {
  count: number;
  issues: number[];
}

export interface CodexSkipMetric {
  created: number;
  resolved: number;
  pending: number;
}

export interface SkillImprovementMetric {
  skill_changes: number;
  mean_cycle_before_ms: number;
  mean_cycle_after_ms: number;
  delta_ratio: number;
  first_skill_change_at: string | null;
}

export interface AllMetrics {
  phase: number;
  range: PhaseRange;
  cycle_count: number;
  pause: PauseMetric;
  needs_human: NeedsHumanMetric;
  codex_skip: CodexSkipMetric;
  skill_improvement: SkillImprovementMetric;
}

export interface IssueDraft {
  title: string;
  body: string;
  labels: string[];
  topic: "baseline" | "pause" | "needs-human" | "codex-skip" | "skill-improvement";
}

// --- Phase range extraction -------------------------------------------------

/**
 * decisions.log.jsonl を walk して Phase N の [startTime, endTime] を決定する。
 *
 *   - endTime = "Phase N 完了" の phase-transition entry (無ければ null = in-progress)
 *   - startTime = "Phase (N-1) 完了" の phase-transition entry (無ければ null = 記録開始前)
 *
 * roadmapText を渡した場合、Phase N が ROADMAP.md に存在しない (= 未定義 phase) と
 * 判断できたら { startTime: null, endTime: null } を返す。呼び元は filter 後の 0 件を
 * 検出して exit 2 する運用のため、この関数自体は phase 未定義を強くは表現しない。
 */
export function parsePhaseRange(
  decisionsLog: string,
  phase: number,
  roadmapText?: string,
): PhaseRange {
  if (roadmapText && !isPhaseInRoadmap(roadmapText, phase)) {
    return { startTime: null, endTime: null };
  }
  const lines = decisionsLog.split("\n").filter(l => l.trim().length > 0);
  let startTime: string | null = null;
  let endTime: string | null = null;
  for (const line of lines) {
    let obj: { at?: string; kind?: string; message?: string };
    try {
      obj = JSON.parse(line);
    } catch {
      continue;
    }
    if (obj.kind !== "phase-transition") continue;
    if (typeof obj.message !== "string" || typeof obj.at !== "string") continue;
    const m = obj.message.match(/Phase\s+(\d+)\s*完了/);
    if (!m) continue;
    const n = parseInt(m[1], 10);
    if (n === phase) endTime = obj.at;
    else if (n === phase - 1) startTime = obj.at;
  }
  return { startTime, endTime };
}

export function isPhaseInRoadmap(roadmapText: string, phase: number): boolean {
  const re = new RegExp(`^##\\s+(?:✅\\s+)?Phase\\s+${phase}(?!\\d)`, "m");
  return re.test(roadmapText);
}

/** started_at/ended_at ではなく ended_at を使う (cycle が完了した時刻で判定)。
 *  startTime が null = "beginning"、endTime が null = "now" として扱う。 */
export function filterCyclesByPhase(cycles: Cycle[], range: PhaseRange): Cycle[] {
  return cycles.filter(c => {
    if (range.startTime && c.ended_at <= range.startTime) return false;
    if (range.endTime && c.ended_at > range.endTime) return false;
    return true;
  });
}

// --- Metric computation -----------------------------------------------------

/** pause_reason に含まれる keyword を粗く分類。順序は「よく出るもの」順。 */
const PAUSE_REASON_KEYWORDS: Array<{ label: string; patterns: RegExp[] }> = [
  { label: "codex-usage-limit", patterns: [/codex.*usage[ _-]?limit/i, /codex.*empty/i] },
  { label: "adr-not-finalized", patterns: [/adr[- ]?(review|regen|not[- ]?finalized)/i, /ADR-\d+/i] },
  { label: "batch-select-empty", patterns: [/batch-select.*0/i, /actionable\s*=?\s*0/i] },
  { label: "intent-aligned-no", patterns: [/aligned:?\s*no/i, /intent.*(check|guard)/i] },
  { label: "needs-human-blocked", patterns: [/needs[- ]human/i, /parent.*needs-human/i] },
  { label: "token-threshold", patterns: [/token.*threshold/i, /token.*limit/i] },
  { label: "scope-cut", patterns: [/scope[ -]?(cut|defer|creep)/i] },
];

export function computePauseMetric(cycles: Cycle[]): PauseMetric {
  const total = cycles.length;
  const paused = cycles.filter(c => c.pause_reason).length;
  const rate = total === 0 ? 0 : paused / total;

  const bucket = new Map<string, number>();
  for (const c of cycles) {
    if (!c.pause_reason) continue;
    let labeled = false;
    for (const { label, patterns } of PAUSE_REASON_KEYWORDS) {
      if (patterns.some(p => p.test(c.pause_reason!))) {
        bucket.set(label, (bucket.get(label) ?? 0) + 1);
        labeled = true;
        break;
      }
    }
    if (!labeled) {
      bucket.set("other", (bucket.get("other") ?? 0) + 1);
    }
  }
  const top_reasons = [...bucket.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([label, count]) => `${label}(${count})`);
  return { total, paused, rate, top_reasons };
}

/** raisedIssues[] の内、labels map で needs-human ラベルが付いている Issue を数える。 */
export function computeNeedsHumanMetric(
  raisedIssues: number[],
  labels: Map<number, string[]>,
): NeedsHumanMetric {
  const hit: number[] = [];
  for (const n of raisedIssues) {
    const lbls = labels.get(n) ?? [];
    if (lbls.includes("needs-human")) hit.push(n);
  }
  return { count: hit.length, issues: hit };
}

/** recorded_at が phase range 内の skip entry のみを対象に、resolved / pending を数える。 */
export function computeCodexSkipMetric(
  skips: SkipEntry[],
  range: PhaseRange,
): CodexSkipMetric {
  const inRange = skips.filter(s => {
    if (range.startTime && s.recorded_at <= range.startTime) return false;
    if (range.endTime && s.recorded_at > range.endTime) return false;
    return true;
  });
  const resolved = inRange.filter(s => s.resolved_at !== null).length;
  const pending = inRange.length - resolved;
  return { created: inRange.length, resolved, pending };
}

/** git log 形式 (--pretty=format:"%H %ct" + --name-only) から skill 改修コミットを
 *  時系列で抽出し、cycles を before/after に分けて平均 cycle 時間を比較する。
 *
 *  gitLog 想定形式 (改行区切り、コミットごと):
 *     <sha> <unix_seconds>
 *     path/to/file
 *     path/to/file
 *     (空行)
 *     <sha> <unix_seconds>
 *     ...
 *
 *  cycle mean time = ended_at - started_at (ms) の平均。
 *  first_skill_change_at より前 = before、以後 = after で分割する。
 *  before/after いずれかが 0 件なら delta_ratio = 0 (比較不能)。 */
export function computeSkillImprovementMetric(
  gitLog: string,
  cycles: Cycle[],
): SkillImprovementMetric {
  const commits = parseGitLogSkillChanges(gitLog);
  const skill_changes = commits.length;
  if (skill_changes === 0 || cycles.length === 0) {
    return {
      skill_changes,
      mean_cycle_before_ms: 0,
      mean_cycle_after_ms: 0,
      delta_ratio: 0,
      first_skill_change_at: null,
    };
  }
  // 最初の skill 改修コミットを boundary とする
  const firstAt = commits[0].at;
  const before = cycles.filter(c => c.ended_at < firstAt);
  const after = cycles.filter(c => c.ended_at >= firstAt);
  const meanMs = (arr: Cycle[]): number => {
    if (arr.length === 0) return 0;
    const total = arr.reduce((s, c) => {
      const d = new Date(c.ended_at).getTime() - new Date(c.started_at).getTime();
      return s + Math.max(0, d);
    }, 0);
    return Math.round(total / arr.length);
  };
  const meanBefore = meanMs(before);
  const meanAfter = meanMs(after);
  const delta = meanBefore === 0 || meanAfter === 0
    ? 0
    : (meanAfter - meanBefore) / meanBefore;
  return {
    skill_changes,
    mean_cycle_before_ms: meanBefore,
    mean_cycle_after_ms: meanAfter,
    delta_ratio: delta,
    first_skill_change_at: firstAt,
  };
}

interface SkillCommit { sha: string; at: string; files: string[] }

function parseGitLogSkillChanges(gitLog: string): SkillCommit[] {
  const commits: SkillCommit[] = [];
  const chunks = gitLog.split(/\n\s*\n/);
  for (const chunk of chunks) {
    const lines = chunk.split("\n").filter(l => l.length > 0);
    if (lines.length === 0) continue;
    const header = lines[0].match(/^([0-9a-f]+)\s+(\d+)$/);
    if (!header) continue;
    const sha = header[1];
    const at = new Date(parseInt(header[2], 10) * 1000).toISOString();
    const files = lines.slice(1).filter(f => f.startsWith(".claude/skills/"));
    if (files.length === 0) continue;
    commits.push({ sha, at, files });
  }
  return commits.sort((a, b) => a.at.localeCompare(b.at));
}

// --- Issue draft rendering --------------------------------------------------

const PAUSE_HIGH_THRESHOLD = 0.30;
const NEEDS_HUMAN_THRESHOLD = 3;
const CODEX_SKIP_PENDING_THRESHOLD = 5;
const ISSUE_CAP = 5;

export function renderRetroIssues(metrics: AllMetrics, phase: number): IssueDraft[] {
  const drafts: IssueDraft[] = [];

  // 常に「概要」Issue を先頭に (baseline record)
  drafts.push({
    topic: "baseline",
    title: `retro(phase${phase}): 概要`,
    body: renderBaselineBody(metrics, phase),
    labels: ["type: foundation", "batch:skill"],
  });

  if (metrics.pause.rate > PAUSE_HIGH_THRESHOLD) {
    drafts.push({
      topic: "pause",
      title: `retro(phase${phase}): pause 率が高い (${Math.round(metrics.pause.rate * 100)}%)`,
      body: renderPauseBody(metrics, phase),
      labels: ["type: foundation", "batch:skill"],
    });
  }

  if (metrics.needs_human.count >= NEEDS_HUMAN_THRESHOLD) {
    drafts.push({
      topic: "needs-human",
      title: `retro(phase${phase}): needs-human 退避が多発 (${metrics.needs_human.count} 件)`,
      body: renderNeedsHumanBody(metrics, phase),
      labels: ["type: foundation", "batch:skill"],
    });
  }

  if (metrics.codex_skip.pending >= CODEX_SKIP_PENDING_THRESHOLD) {
    drafts.push({
      topic: "codex-skip",
      title: `retro(phase${phase}): Codex skip 後払いが滞留 (${metrics.codex_skip.pending} 件)`,
      body: renderCodexSkipBody(metrics, phase),
      labels: ["type: foundation", "batch:skill"],
    });
  }

  if (
    metrics.skill_improvement.skill_changes > 0
    && metrics.skill_improvement.mean_cycle_before_ms > 0
    && metrics.skill_improvement.mean_cycle_after_ms > metrics.skill_improvement.mean_cycle_before_ms
  ) {
    drafts.push({
      topic: "skill-improvement",
      title: `retro(phase${phase}): Skill 改修後に cycle 時間が悪化`,
      body: renderSkillBody(metrics, phase),
      labels: ["type: foundation", "batch:skill"],
    });
  }

  return drafts.slice(0, ISSUE_CAP);
}

function renderBaselineBody(m: AllMetrics, phase: number): string {
  return [
    `## Phase ${phase} retrospective (baseline record)`,
    "",
    "loop 運用メトリクスを Phase 完了時点で記録する。ADR-013 Fable5 監査 (11/14/17/20) を",
    "補完する定点観測。閾値を跨いだ metric は別 Issue が起票される。",
    "",
    "## Range",
    "",
    `- start: ${m.range.startTime ?? "(記録開始前)"}`,
    `- end:   ${m.range.endTime ?? "(in-progress)"}`,
    `- cycles: ${m.cycle_count}`,
    "",
    "## Metrics",
    "",
    "| Metric | Value |",
    "|---|---|",
    `| pause rate | ${(m.pause.rate * 100).toFixed(1)}% (${m.pause.paused}/${m.pause.total}) |`,
    `| pause top reasons | ${m.pause.top_reasons.join(", ") || "(none)"} |`,
    `| needs-human count | ${m.needs_human.count} |`,
    `| codex skip created | ${m.codex_skip.created} |`,
    `| codex skip resolved | ${m.codex_skip.resolved} |`,
    `| codex skip pending | ${m.codex_skip.pending} |`,
    `| skill changes | ${m.skill_improvement.skill_changes} |`,
    `| mean cycle before skill change | ${formatMs(m.skill_improvement.mean_cycle_before_ms)} |`,
    `| mean cycle after skill change | ${formatMs(m.skill_improvement.mean_cycle_after_ms)} |`,
    `| delta ratio | ${(m.skill_improvement.delta_ratio * 100).toFixed(1)}% |`,
    "",
    "## 関連",
    "",
    "- ADR-013 (Fable5 監査トリガー)",
    "- memory: `project-3ailoop-policy`",
  ].join("\n");
}

function renderPauseBody(m: AllMetrics, phase: number): string {
  return [
    `## 現象`,
    "",
    `Phase ${phase} で pause 率が ${Math.round(m.pause.rate * 100)}% (${m.pause.paused}/${m.pause.total} cycle)`,
    `に達し、閾値 ${Math.round(PAUSE_HIGH_THRESHOLD * 100)}% を超過。`,
    "",
    "## 主な pause 理由 (top 5)",
    "",
    ...(m.pause.top_reasons.length > 0
      ? m.pause.top_reasons.map(r => `- ${r}`)
      : ["- (分類できず)"]),
    "",
    "## 期待するアクション",
    "",
    "- 上位理由が infra 系 (codex-usage-limit 等) → 回復戦略の見直し",
    "- 上位理由が構造系 (adr-not-finalized / batch-select-empty) → skill / policy の見直し",
    "- 何も変更不要と判断したら close",
  ].join("\n");
}

function renderNeedsHumanBody(m: AllMetrics, phase: number): string {
  const list = m.needs_human.issues.slice(0, 20).map(n => `- #${n}`).join("\n");
  return [
    `## 現象`,
    "",
    `Phase ${phase} で ${m.needs_human.count} 件の Issue が needs-human に退避された`,
    `(閾値 ${NEEDS_HUMAN_THRESHOLD} 件超過)。`,
    "",
    "## 該当 Issue",
    "",
    list || "- (none)",
    "",
    "## 期待するアクション",
    "",
    "- 退避理由に共通パターンがあるか確認 (同じ ADR 依存 / 同じツールの usage limit 等)",
    "- 必要ならば skill or /3ai ルート改修 Issue を別途起票",
    "- 個々の needs-human は本 retro とは独立に人間が処理する",
  ].join("\n");
}

function renderCodexSkipBody(m: AllMetrics, phase: number): string {
  return [
    `## 現象`,
    "",
    `Phase ${phase} で Codex 後払いレビュー台帳 (\`features/.loop/codex-skips.jsonl\`) の`,
    `未解決エントリが ${m.codex_skip.pending} 件 (作成 ${m.codex_skip.created} / 解決 ${m.codex_skip.resolved}) となり、`,
    `閾値 ${CODEX_SKIP_PENDING_THRESHOLD} 件を超過した。`,
    "",
    "## 期待するアクション",
    "",
    "- Codex usage 復旧タイミングで `loop-codex-skip-collector.ts` を集中実行",
    "- 恒常的に滞留するなら STEP 3.5/7.5 skip 判定基準の緩和 or 別 review 経路の検討",
  ].join("\n");
}

function renderSkillBody(m: AllMetrics, phase: number): string {
  return [
    `## 現象`,
    "",
    `Phase ${phase} で \`.claude/skills/**\` に ${m.skill_improvement.skill_changes} 件のコミットが入り、`,
    `最初の改修 (${m.skill_improvement.first_skill_change_at}) の前後で cycle 平均時間が`,
    `${formatMs(m.skill_improvement.mean_cycle_before_ms)} → ${formatMs(m.skill_improvement.mean_cycle_after_ms)}`,
    `(delta ${(m.skill_improvement.delta_ratio * 100).toFixed(1)}%) と悪化した。`,
    "",
    "## 期待するアクション",
    "",
    "- 該当 skill 改修コミットが原因か切り分け (改修後の pause 率も併せて確認)",
    "- 誤検知の可能性 (Phase 後半に難易度の高い Issue が集中しただけ、等) を排除",
    "- 実害があれば revert or 追加改修 Issue を起票",
  ].join("\n");
}

function formatMs(ms: number): string {
  if (ms === 0) return "n/a";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  if (ms < 3_600_000) return `${(ms / 60_000).toFixed(1)}min`;
  return `${(ms / 3_600_000).toFixed(1)}h`;
}

// --- I/O helpers ------------------------------------------------------------

function readJsonlSafely<T>(path: string): T[] {
  if (!existsSync(path)) return [];
  const raw = readFileSync(path, "utf-8");
  const out: T[] = [];
  for (const line of raw.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      out.push(JSON.parse(trimmed) as T);
    } catch {
      /* skip corrupt line */
    }
  }
  return out;
}

function readTextSafely(path: string): string {
  if (!existsSync(path)) return "";
  return readFileSync(path, "utf-8");
}

async function runGitLogForSkills(sinceIso: string | null, untilIso: string | null): Promise<string> {
  const args = ["log", "--pretty=format:%H %ct", "--name-only", "--", ".claude/skills/"];
  if (sinceIso) args.push(`--since=${sinceIso}`);
  if (untilIso) args.push(`--until=${untilIso}`);
  const proc = Bun.spawn(["git", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return out;
}

async function fetchIssueLabelsFor(issueNumbers: number[]): Promise<Map<number, string[]>> {
  const map = new Map<number, string[]>();
  if (issueNumbers.length === 0) return map;
  // 1 回のクエリで済ませたいので範囲検索: N 番号を含む Issue の label を取る
  // 数が少なければ per-issue で view しても OK。ここでは 100 件上限で
  // `gh issue list --state all --search "in:number ..."` は使えないので、
  // 個別に view API を呼ぶ (retrospective は phase 単位で高々数十件)。
  await Promise.all(issueNumbers.map(async n => {
    const proc = Bun.spawn(
      ["gh", "issue", "view", String(n), "--json", "labels"],
      { stdout: "pipe", stderr: "pipe" },
    );
    const out = await new Response(proc.stdout).text();
    await proc.exited;
    if (proc.exitCode !== 0) {
      map.set(n, []);
      return;
    }
    try {
      const obj = JSON.parse(out) as { labels?: Array<{ name: string }> };
      map.set(n, (obj.labels ?? []).map(l => l.name));
    } catch {
      map.set(n, []);
    }
  }));
  return map;
}

async function createIssue(draft: IssueDraft): Promise<{ ok: boolean; number?: number; error?: string }> {
  const proc = Bun.spawn(
    [
      "gh", "issue", "create",
      "--title", draft.title,
      "--body", draft.body,
      "--label", draft.labels.join(","),
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
  return { ok: true, number: m ? parseInt(m[1], 10) : undefined };
}

function appendRetroLog(entry: {
  phase: number;
  timestamp: string;
  metrics: AllMetrics;
  created_issues: number[];
}): void {
  try {
    mkdirSync(dirname(RETRO_LOG_PATH), { recursive: true });
    appendFileSync(RETRO_LOG_PATH, JSON.stringify(entry) + "\n", "utf-8");
  } catch (e) {
    process.stderr.write(`WARN: retro-log append failed: ${(e as Error).message}\n`);
  }
}

// --- Main -------------------------------------------------------------------

interface CliOpts {
  phase: number;
  dryRun: boolean;
  journalPath: string;
  skipsPath: string;
  decisionsPath: string;
  roadmapPath: string;
}

function parseArgs(argv: string[]): CliOpts | null {
  const rest = argv.slice(2);
  const arg = (name: string): string | undefined => {
    const i = rest.indexOf(name);
    return i >= 0 ? rest[i + 1] : undefined;
  };
  const flag = (name: string): boolean => rest.includes(name);

  const phaseStr = arg("--phase");
  if (!phaseStr) return null;
  const phase = parseInt(phaseStr, 10);
  if (!Number.isFinite(phase) || phase <= 0) return null;

  return {
    phase,
    dryRun: flag("--dry-run"),
    journalPath: arg("--journal") ?? DEFAULT_JOURNAL_PATH,
    skipsPath: arg("--skips") ?? DEFAULT_SKIPS_PATH,
    decisionsPath: arg("--decisions") ?? DEFAULT_DECISIONS_PATH,
    roadmapPath: arg("--roadmap") ?? DEFAULT_ROADMAP_PATH,
  };
}

export async function runRetrospective(opts: CliOpts): Promise<number> {
  const decisionsText = readTextSafely(opts.decisionsPath);
  const roadmapText = readTextSafely(opts.roadmapPath);

  // ROADMAP を読めた場合のみ phase 存在チェックする (テスト時の空 ROADMAP 相当は skip)。
  // ROADMAP 記載外 phase は無効扱いで exit 2 (未来 phase の誤集計を防ぐ)。
  if (roadmapText && !isPhaseInRoadmap(roadmapText, opts.phase)) {
    process.stderr.write(
      `no cycles for phase ${opts.phase} (Phase ${opts.phase} not found in ${opts.roadmapPath})\n`,
    );
    return 2;
  }

  const range = parsePhaseRange(decisionsText, opts.phase, roadmapText || undefined);

  const allCycles = readJsonlSafely<Cycle>(opts.journalPath);
  const cycles = filterCyclesByPhase(allCycles, range);

  if (cycles.length === 0) {
    process.stderr.write(
      `no cycles for phase ${opts.phase} (range: start=${range.startTime ?? "(null)"}, end=${range.endTime ?? "(null)"})\n`,
    );
    return 2;
  }

  const skips = readJsonlSafely<SkipEntry>(opts.skipsPath);

  // needs-human: gh を叩くのは実運用のみ。dry-run では skip して空 Map で近似する。
  const raisedIssues = [...new Set(cycles.flatMap(c => c.raised))];
  const labels = opts.dryRun
    ? new Map<number, string[]>()
    : await fetchIssueLabelsFor(raisedIssues);

  // skill 改修は git log を叩く。範囲は startTime〜endTime (null なら未指定 = 全期間)。
  const gitLog = await runGitLogForSkills(range.startTime, range.endTime);

  const metrics: AllMetrics = {
    phase: opts.phase,
    range,
    cycle_count: cycles.length,
    pause: computePauseMetric(cycles),
    needs_human: computeNeedsHumanMetric(raisedIssues, labels),
    codex_skip: computeCodexSkipMetric(skips, range),
    skill_improvement: computeSkillImprovementMetric(gitLog, cycles),
  };

  process.stderr.write(`# metrics\n${JSON.stringify(metrics, null, 2)}\n`);

  const drafts = renderRetroIssues(metrics, opts.phase);

  const createdNumbers: number[] = [];
  for (const d of drafts) {
    if (opts.dryRun) {
      console.log(`--- [DRAFT] ${d.title}`);
      console.log(`labels: ${d.labels.join(",")}`);
      console.log(d.body);
      console.log("");
      continue;
    }
    const r = await createIssue(d);
    if (r.ok) {
      console.log(`created #${r.number ?? "?"}: ${d.title}`);
      if (typeof r.number === "number") createdNumbers.push(r.number);
    } else {
      console.error(`FAILED: ${d.title} — ${r.error}`);
    }
  }

  if (!opts.dryRun) {
    appendRetroLog({
      phase: opts.phase,
      timestamp: new Date().toISOString(),
      metrics,
      created_issues: createdNumbers,
    });
  }

  return 0;
}

if (import.meta.main) {
  const opts = parseArgs(process.argv);
  if (!opts) {
    console.error("Usage: loop-phase-retrospective.ts --phase <N> [--dry-run] [--journal <path>] [--skips <path>] [--decisions <path>] [--roadmap <path>]");
    process.exit(2);
  }
  const rc = await runRetrospective(opts);
  process.exit(rc);
}
