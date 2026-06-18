#!/usr/bin/env bun
// escalate-glm-adversarial.ts — STEP 6-D 自律 escalation (#225)
//
// 動作:
//   1. ESC_MAX_LOOPS (=3) 到達後の最終ゲート。
//   2. Codex 3 ペルソナ (architect / contrarian / migration) で adversarial review。
//      入力: features/<dir>/ci.log の末尾抜粋 + features/<dir>/debug-spec.md。
//   3. per-Issue tracker (features/.loop/glm-escalation/<issue>.json) に
//      regen_count / token_used を atomic rename で永続化。
//   4. 全員 approved → kind=continue (caller は debug-spec 追記して再 dispatch を続けられる)
//      1 つでも refute → kind=needs_human (reason=refute) + gh で needs-human 付与 + raise-issue-on-failure 起票
//      cumulative token > ESCALATION_TOKEN_CAP → kind=needs_human (reason=token_cap)
//
// 使い方:
//   bun escalate-glm-adversarial.ts --issue <N> --feature-dir features/<N>-<slug> [--dry-run] [--mock-mode pass|refute]
//
// 環境変数:
//   ESC_GLM_ADV_MOCK=pass|refute        テスト用に review 結果を強制 (CLI flag --mock-mode と等価)
//   ESC_TRACKER_ROOT=<path>             tracker 永続化 root を override (テスト用)
//   ESC_RAISE_ISSUE=0                   raise-issue-on-failure 呼び出しを skip (テスト用)
//   ESC_DISABLE_GH=1                    gh CLI 呼び出しを skip (テスト用)
//   CODEX_DRY_RUN=1                     Codex を呼ばず stub (常に approved を返す)
//
// exit code:
//   0 = continue (全 approved、caller は再 dispatch 可能)
//   3 = needs_human (refute or token cap、自動退避完了)
//   1 = 内部エラー

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname } from "path";

export const ESCALATION_TOKEN_CAP = 200_000;
const ESTIMATED_TOKENS_PER_PERSONA = 30_000;
const CI_LOG_TAIL_LINES = 200;

const PERSONAS = [
  {
    key: "architect",
    role: "実装 architect (既存 invariant / API 契約 / B-rep トポロジー保証で refute せよ)",
  },
  {
    key: "contrarian",
    role: "実装 contrarian (採用された修正案を refute し、棄却された代替案の利点を強調せよ)",
  },
  {
    key: "migration",
    role: "実装 migration reviewer (既存テスト互換性 / 後方互換性で refute せよ)",
  },
] as const;

export type PersonaVerdict = {
  persona: string;
  approved: boolean;
  raw_excerpt: string;
};

export type EscalationOutcome =
  | { kind: "continue"; verdicts: PersonaVerdict[]; regen_count: number; token_used: number }
  | {
      kind: "needs_human";
      reason: "refute" | "token_cap";
      details: string[];
      regen_count: number;
      token_used: number;
    };

export type TrackerState = {
  issue: number;
  regen_count: number;
  token_used: number;
  started_at: string;
  last_updated_at: string;
  retired?: { at: string; reason: string };
};

function trackerRoot(): string {
  return process.env.ESC_TRACKER_ROOT ?? "features/.loop/glm-escalation";
}

function trackerFile(issueNum: number): string {
  return `${trackerRoot()}/${issueNum}.json`;
}

function nowIso(): string {
  return new Date().toISOString();
}

function readTracker(issueNum: number): TrackerState | null {
  const path = trackerFile(issueNum);
  if (!existsSync(path)) return null;
  try {
    return JSON.parse(readFileSync(path, "utf-8")) as TrackerState;
  } catch {
    return null;
  }
}

function writeTracker(state: TrackerState): void {
  const path = trackerFile(state.issue);
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp.${process.pid}.${Math.random().toString(36).slice(2, 8)}`;
  writeFileSync(tmp, JSON.stringify(state, null, 2), "utf-8");
  renameSync(tmp, path);
}

export function initTracker(issueNum: number): TrackerState {
  const cur = readTracker(issueNum);
  if (cur) return cur;
  const fresh: TrackerState = {
    issue: issueNum,
    regen_count: 0,
    token_used: 0,
    started_at: nowIso(),
    last_updated_at: nowIso(),
  };
  writeTracker(fresh);
  return fresh;
}

export function incRegen(issueNum: number): TrackerState {
  const cur = readTracker(issueNum) ?? initTracker(issueNum);
  cur.regen_count += 1;
  cur.last_updated_at = nowIso();
  writeTracker(cur);
  return cur;
}

export function addTokens(issueNum: number, n: number): TrackerState {
  if (n < 0) throw new Error("addTokens: n must be >= 0");
  const cur = readTracker(issueNum) ?? initTracker(issueNum);
  cur.token_used += n;
  cur.last_updated_at = nowIso();
  writeTracker(cur);
  return cur;
}

async function runGh(args: string[]): Promise<{ stdout: string; exit: number }> {
  if (process.env.ESC_DISABLE_GH === "1") {
    return { stdout: "", exit: 0 };
  }
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return { stdout: out.trim(), exit: proc.exitCode ?? 0 };
}

function tailLines(text: string, n: number): string {
  const lines = text.split("\n");
  return lines.slice(Math.max(0, lines.length - n)).join("\n");
}

function buildPersonaPrompt(
  persona: typeof PERSONAS[number],
  ciLogExcerpt: string,
  debugSpec: string,
  issueNum: number,
): string {
  return [
    `# Role: ${persona.role}`,
    "",
    "## Task",
    `Issue #${issueNum} の STEP 6-D 自律 escalation。`,
    "GLM 実装が ESC_MAX_LOOPS=3 まで失敗を繰り返した。Claude オーケストレーターが書いた debug-spec.md と最新の ci.log 抜粋を元に、",
    "実装方針 (debug-spec.md の「修正方針」) を adversarial に review してください。",
    "REFUTE を default とし、明確に refute できなければ approved を返してください。",
    "理由が浅い (1 文以下、根拠なし) refute は失敗判定とし approved を返してください。",
    "",
    "## 出力フォーマット (必須)",
    "最終行に必ず以下のいずれかを記載:",
    "  verdict: approved",
    "  verdict: refuted",
    "",
    "refuted の場合、その直前に 200 字以上の refute 理由を記載してください。",
    "",
    "## debug-spec.md (Claude が起こした修正仕様)",
    "",
    debugSpec,
    "",
    "## ci.log 末尾抜粋",
    "",
    "```",
    ciLogExcerpt,
    "```",
  ].join("\n");
}

async function runPersonaReview(
  persona: typeof PERSONAS[number],
  ciLogExcerpt: string,
  debugSpec: string,
  issueNum: number,
  outDir: string,
  mockMode?: "pass" | "refute",
): Promise<PersonaVerdict> {
  const mock = mockMode ?? (process.env.ESC_GLM_ADV_MOCK as "pass" | "refute" | undefined);
  if (mock === "pass") {
    return { persona: persona.key, approved: true, raw_excerpt: "[mock:pass]" };
  }
  if (mock === "refute") {
    return { persona: persona.key, approved: false, raw_excerpt: "[mock:refute]" };
  }
  if (process.env.CODEX_DRY_RUN === "1") {
    return { persona: persona.key, approved: true, raw_excerpt: "[CODEX_DRY_RUN]" };
  }

  mkdirSync(outDir, { recursive: true });
  const instructionFile = `${outDir}/persona-${persona.key}.instruction.md`;
  const resultFile = `${outDir}/persona-${persona.key}.result.md`;
  const prompt = buildPersonaPrompt(persona, ciLogExcerpt, debugSpec, issueNum);
  writeFileSync(instructionFile, prompt, "utf-8");

  const proc = Bun.spawn(
    ["codex", "exec", "-c", "sandbox_mode=read-only", "--output-last-message", resultFile, prompt],
    { stdin: "ignore", stdout: "pipe", stderr: "pipe" },
  );
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  await proc.exited;
  writeFileSync(`${resultFile}.log`, stdout + stderr, "utf-8");

  let text = "";
  try {
    text = readFileSync(resultFile, "utf-8");
  } catch {
    text = stdout;
  }
  const verdictMatch = text.match(/verdict:\s*(approved|refuted)/i);
  const verdict = verdictMatch?.[1].toLowerCase() ?? "unknown";
  return {
    persona: persona.key,
    approved: verdict === "approved",
    raw_excerpt: text.slice(0, 200),
  };
}

async function retireToHuman(
  issueNum: number,
  featureDir: string,
  reason: "refute" | "token_cap",
  state: TrackerState,
  details: string[],
): Promise<void> {
  state.retired = { at: nowIso(), reason };
  state.last_updated_at = nowIso();
  writeTracker(state);

  await runGh(["issue", "edit", String(issueNum), "--add-label", "needs-human"]);

  if (process.env.ESC_RAISE_ISSUE !== "0") {
    const reasonLabel = reason === "refute" ? "Codex adversarial refute" : `token cap (${state.token_used} > ${ESCALATION_TOKEN_CAP})`;
    const summary = `STEP 6-D escalation → needs-human: ${reasonLabel}. ${details.join(" / ").slice(0, 200)}`;
    const proc = Bun.spawn(
      [
        "bun",
        ".claude/skills/3ai/scripts/raise-issue-on-failure.ts",
        "--step",
        "STEP 6-D escalate-glm-adversarial",
        "--feature-dir",
        featureDir,
        "--error-summary",
        summary,
      ],
      { stdin: "ignore", stdout: "pipe", stderr: "pipe" },
    );
    await proc.exited;
  }
}

export async function escalate(opts: {
  issueNum: number;
  featureDir: string;
  dryRun?: boolean;
  mockMode?: "pass" | "refute";
  estimatedReviewTokens?: number;
}): Promise<EscalationOutcome> {
  if (!existsSync(opts.featureDir)) {
    throw new Error(`feature-dir not found: ${opts.featureDir}`);
  }

  const ciLogPath = `${opts.featureDir}/ci.log`;
  const debugSpecPath = `${opts.featureDir}/debug-spec.md`;
  const ciLog = existsSync(ciLogPath) ? tailLines(readFileSync(ciLogPath, "utf-8"), CI_LOG_TAIL_LINES) : "(ci.log not found)";
  const debugSpec = existsSync(debugSpecPath) ? readFileSync(debugSpecPath, "utf-8") : "(debug-spec.md not found)";

  // tracker: token 予算予約 → cap 越えなら即 needs_human
  const estimated = opts.estimatedReviewTokens ?? ESTIMATED_TOKENS_PER_PERSONA * PERSONAS.length;
  let state = incRegen(opts.issueNum);
  state = addTokens(opts.issueNum, estimated);
  if (state.token_used > ESCALATION_TOKEN_CAP) {
    const details = [`token_used=${state.token_used} > cap=${ESCALATION_TOKEN_CAP}`];
    if (!opts.dryRun) {
      await retireToHuman(opts.issueNum, opts.featureDir, "token_cap", state, details);
    }
    return {
      kind: "needs_human",
      reason: "token_cap",
      details,
      regen_count: state.regen_count,
      token_used: state.token_used,
    };
  }

  // dry-run: review skip
  if (opts.dryRun) {
    return {
      kind: "continue",
      verdicts: PERSONAS.map(p => ({ persona: p.key, approved: true, raw_excerpt: "[dry-run]" })),
      regen_count: state.regen_count,
      token_used: state.token_used,
    };
  }

  // Codex 3 ペルソナ並列 review
  const outDir = `${opts.featureDir}/escalation-r${state.regen_count}`;
  mkdirSync(outDir, { recursive: true });
  const verdicts = await Promise.all(
    PERSONAS.map(p => runPersonaReview(p, ciLog, debugSpec, opts.issueNum, outDir, opts.mockMode)),
  );
  writeFileSync(`${outDir}/verdicts.json`, JSON.stringify(verdicts, null, 2), "utf-8");

  const refuted = verdicts.filter(v => !v.approved);
  if (refuted.length > 0) {
    const details = refuted.map(v => `${v.persona}: refuted (${v.raw_excerpt.slice(0, 80)}...)`);
    await retireToHuman(opts.issueNum, opts.featureDir, "refute", state, details);
    return {
      kind: "needs_human",
      reason: "refute",
      details,
      regen_count: state.regen_count,
      token_used: state.token_used,
    };
  }

  return {
    kind: "continue",
    verdicts,
    regen_count: state.regen_count,
    token_used: state.token_used,
  };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  function arg(name: string): string | undefined {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
  }
  function flag(name: string): boolean {
    return argv.includes(name);
  }

  const issueStr = arg("--issue");
  const featureDir = arg("--feature-dir");
  const dryRun = flag("--dry-run");
  const mockArg = arg("--mock-mode") as "pass" | "refute" | undefined;
  const estStr = arg("--estimated-tokens");

  if (!issueStr || !featureDir) {
    console.error("Usage: escalate-glm-adversarial.ts --issue <N> --feature-dir <dir> [--dry-run] [--mock-mode pass|refute] [--estimated-tokens <n>]");
    process.exit(1);
  }
  if (mockArg && mockArg !== "pass" && mockArg !== "refute") {
    console.error(`--mock-mode must be pass|refute, got: ${mockArg}`);
    process.exit(1);
  }

  const issueNum = parseInt(issueStr);
  if (!Number.isFinite(issueNum) || issueNum <= 0) {
    console.error(`--issue must be a positive int, got: ${issueStr}`);
    process.exit(1);
  }

  try {
    const result = await escalate({
      issueNum,
      featureDir,
      dryRun,
      mockMode: mockArg,
      estimatedReviewTokens: estStr ? parseInt(estStr) : undefined,
    });
    console.log(JSON.stringify(result, null, 2));
    process.exit(result.kind === "continue" ? 0 : 3);
  } catch (err) {
    console.error(`escalate-glm-adversarial: ${err instanceof Error ? err.message : String(err)}`);
    process.exit(1);
  }
}
