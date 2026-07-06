#!/usr/bin/env bun
// dispatch-glm-review.ts — GLM (Z.AI) レビュアー単一ペルソナ起動スクリプト
// STEP 3 (設計レビュー) / STEP 7 (最終レビュー) で Claude がペルソナごとに呼ぶ。
// 複数ペルソナの並列起動は SKILL.md の指示に従い Bash run_in_background で行う。
//
// 使い方 (設計レビュー):
//   bun dispatch-glm-review.ts \
//     --persona scope|invariant|ambig|numeric \
//     --issue N --round R \
//     --input features/N-SLUG/plan.md \
//     --feature-dir features/N-SLUG \
//     --result features/N-SLUG/review-scope-r1.yaml \
//     [--adr-context features/N-SLUG/adr-context.md]
//     [--rejection features/N-SLUG/rejection.md]
//     [--judgment-summary features/N-SLUG/judgment-summary.md]
//     [--plan-snapshot-dir features/N-SLUG/plan-snapshots]
//
// 使い方 (最終レビュー):
//   bun dispatch-glm-review.ts \
//     --persona final \
//     --issue N \
//     --feature-dir features/N-SLUG \
//     --result features/N-SLUG/review-final.yaml \
//     [--test-summary features/N-SLUG/test-summary.json]

import { readFileSync, writeFileSync, existsSync, mkdirSync, unlinkSync } from "fs";
import { resolveGlmModel, buildAnthropicModelEnv } from "./zai-model.ts";

const PERSONA_AGENT: Record<string, string> = {
  scope:     ".claude/skills/3ai/agents/glm-reviewer-scope.md",
  invariant: ".claude/skills/3ai/agents/glm-reviewer-invariant.md",
  ambig:     ".claude/skills/3ai/agents/glm-reviewer-ambig.md",
  numeric:   ".claude/skills/3ai/agents/glm-reviewer-numeric.md",
  assembly:  ".claude/skills/3ai/agents/glm-reviewer-assembly.md",
  final:     ".claude/skills/3ai/agents/glm-reviewer-final.md",
};

function parseEnvFile(path: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const line of readFileSync(path, "utf-8").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eqIdx = trimmed.indexOf("=");
    if (eqIdx === -1) continue;
    const key = trimmed.slice(0, eqIdx).trim();
    let val = trimmed.slice(eqIdx + 1).trim();
    if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
      val = val.slice(1, -1);
    }
    result[key] = val;
  }
  return result;
}

export function parseVerdict(text: string): {
  verdict: string;
  severity_counts: Record<string, number>;
  blocking: number;
} {
  const verdicts = [...text.matchAll(/^verdict:\s*(pass|fail)/gm)].map((m) => m[1]);
  const verdict = verdicts[verdicts.length - 1] ?? "unknown";
  const sevs = [...text.matchAll(/severity:\s*(critical|high|medium|low)/g)].map((m) => m[1]);
  const counts = { critical: 0, high: 0, medium: 0, low: 0 };
  for (const s of sevs) counts[s as keyof typeof counts]++;
  return { verdict, severity_counts: counts, blocking: counts.critical + counts.high };
}

// #262 r3: 有効レビュー YAML の最低限のスキーマ判定。
// 引数 `yamlText` (= ```yaml 抽出後の本文) が top-level に `verdict:` と
// `issues:` の両方を持つ場合のみ「transport 成功」とみなす。
// Codex r2 F01 (3 persona 一致) の指摘: `verdict:` だけで成功扱いだと
// (a) diff に含まれる verdict 行を GLM が引用しただけ、(b) `verdict: pass` 単独で
// `issues:` が欠落、(c) `verdict: fail` 単独で blocking=0 になり「pass でも error
// でもない」宙ぶらりん状態が再発する。
export function isValidReviewYaml(yamlText: string): boolean {
  return (
    /^verdict:\s*(pass|fail)\b/m.test(yamlText) &&
    /^issues:\s*(\[\]|$)/m.test(yamlText)
  );
}

// #262: dispatch 失敗 (claude CLI 非ゼロ終了 / max-turns / verdict 行不在 /
// 契約違反) を明示的に検出する。`*.verdict.json` を "vacuous pass" にせず
// error verdict として書き出すための判定。
//
// #262 r2 F01 (Codex 3 persona 一致): 有効な YAML レビュー本文が
// "Error: Reached max turns" 等の文字列を引用しているだけで失敗扱いになる
// 偽陽性を避けるため、有効 YAML 形状 (verdict + issues 両方 top-level) が
// 揃っている場合のみ transport 成功と判定する。
//
// #262 r3 F01 (Codex 3 persona 一致): 形状だけでは contract 違反
// (verdict: fail なのに blocking=0 / verdict: pass なのに blocking>=1) を
// 検出できず STEP 7 の pass/blocking>=1 分岐どちらにも乗らない宙ぶらりん状態が
// 再発する。verdict-blocking 整合性も検証する。
//
// #293: `pass-with-blockers` (verdict: pass + blocking >= 1) を一律 dispatch_error
// 扱いにすると、GLM が defer 系の defensive note を high で出してくる頻発ケースで
// false-positive な auto-raise Issue を量産する。verdict=pass を尊重し、issues を
// medium にデモートして caller の verdict=pass 分岐に乗せる "demote" 経路を追加する。
// `fail-without-blockers` は本物の矛盾なので fail のまま残す (非対称)。
export type DispatchClassification =
  | { kind: "ok"; reason: "" }
  | { kind: "demote"; reason: "pass-with-blockers" }
  | { kind: "fail"; reason: string };

export function classifyDispatch(
  rawOut: string,
  exitCode: number,
  yamlText: string,
): DispatchClassification {
  if (exitCode !== 0) return { kind: "fail", reason: `claude-exit-${exitCode}` };
  if (isValidReviewYaml(yamlText)) {
    const { verdict, blocking } = parseVerdict(yamlText);
    if (verdict === "fail" && blocking === 0) return { kind: "fail", reason: "fail-without-blockers" };
    if (verdict === "pass" && blocking > 0) return { kind: "demote", reason: "pass-with-blockers" };
    return { kind: "ok", reason: "" };
  }
  if (/Error:\s*Reached\s+max\s+turns/i.test(rawOut)) return { kind: "fail", reason: "max-turns" };
  if (/^\s*Error:/m.test(yamlText)) return { kind: "fail", reason: "claude-error" };
  if (!/^verdict:\s*(pass|fail)\b/m.test(yamlText)) return { kind: "fail", reason: "no-verdict-line" };
  return { kind: "fail", reason: "missing-issues" };
}

// 後方互換 wrapper: 既存の caller が detectDispatchFailure を import している場合に
// 壊さないために残す。demote は failed=false 扱いにする (caller は警告ログのみ)。
// 本ファイル内 main() は classifyDispatch を直接使う。
export function detectDispatchFailure(
  rawOut: string,
  exitCode: number,
  yamlText: string,
): { failed: boolean; reason: string } {
  const c = classifyDispatch(rawOut, exitCode, yamlText);
  return { failed: c.kind === "fail", reason: c.reason };
}

// #293: pass-with-blockers のときに issues 配列の severity を medium に強制
// デモートする。yaml は GLM 出力をそのまま使うため、文字列 regex 置換で安全に
// 行単位の severity 行を書き換える (構造を変えず順序保持)。
export function demoteIssuesToMedium(yamlText: string): string {
  return yamlText.replace(
    /^(\s*severity:\s*)(critical|high|low)(\s*)$/gm,
    (_m, lead, _sev, tail) => `${lead}medium${tail}`,
  );
}

// #262: dispatch 失敗時に書き出す verdict.json 形状。
// blocking=-1 で "no findings (=0)" と明確に区別する (dispatch-codex.ts と同じ慣用)。
export function makeErrorVerdict(reason: string): {
  verdict: "error";
  dispatch_error: true;
  reason: string;
  severity_counts: Record<string, number>;
  blocking: number;
} {
  return {
    verdict: "error",
    dispatch_error: true,
    reason,
    severity_counts: { critical: 0, high: 0, medium: 0, low: 0 },
    blocking: -1,
  };
}

async function buildDesignContext(opts: {
  issueNum: string;
  round: number;
  inputFile: string;
  featureDir: string;
  adrContextFile?: string;
  rejectionFile?: string;
  judgmentSummaryFile?: string;
  planSnapshotDir?: string;
}): Promise<string> {
  let ctx = "";

  // Issue context
  try {
    const proc = Bun.spawn(
      ["gh", "issue", "view", opts.issueNum, "--json", "title,body,number"],
      { stdout: "pipe", stderr: "pipe" }
    );
    const json = JSON.parse(await new Response(proc.stdout).text());
    await proc.exited;
    const issueText = `Issue #${json.number ?? ""} ${json.title}\n\n${json.body}`;
    ctx += `===== ISSUE CONTEXT =====\n${issueText}\n===== END ISSUE CONTEXT =====\n\n`;
  } catch {
    ctx += `===== ISSUE CONTEXT =====\n(Issue 取得失敗)\n===== END ISSUE CONTEXT =====\n\n`;
  }

  // ADR excerpt
  if (opts.adrContextFile && existsSync(opts.adrContextFile)) {
    const adrText = readFileSync(opts.adrContextFile, "utf-8");
    ctx += `===== ADR EXCERPT =====\n以下は本 Issue が前提とする設計決定（ADR 抜粋）。この方式自体への異議は挙げないこと。\n${adrText}\n===== END ADR EXCERPT =====\n\n`;
  }

  // Non-Goals → scope defense
  const planText = readFileSync(opts.inputFile, "utf-8");
  const ngSection = planText.match(/^## Non-Goals\r?\n([\s\S]*?)(?=\r?\n## |\s*$)/m)?.[1] ?? "";
  const ngLines = ngSection.split("\n").filter((l) => l.trim()).join("\n");
  ctx += `===== SCOPE DEFENSE =====\n以下は本 Issue のスコープ外。指摘・拡張提案・改善要求の対象としないこと。\n${ngLines}\n===== END SCOPE DEFENSE =====\n\n`;

  // Prior rejections
  if (opts.rejectionFile && existsSync(opts.rejectionFile)) {
    const rejText = readFileSync(opts.rejectionFile, "utf-8");
    ctx += `===== PRIOR REJECTIONS =====\n以下は過去 round で棄却済み。蒸し返さないこと。\n${rejText}\n===== END PRIOR REJECTIONS =====\n\n`;
  }

  // Prior judgments (round 2+)
  if (opts.round >= 2 && opts.judgmentSummaryFile && existsSync(opts.judgmentSummaryFile)) {
    const jText = readFileSync(opts.judgmentSummaryFile, "utf-8");
    ctx += `===== PRIOR JUDGMENTS =====\n前 round で Claude が採用・棄却を判定済みの一覧。採用済み指摘は「足りない」と再指摘しない。棄却済み事項は再度持ち出さない。\n${jText}\n===== END PRIOR JUDGMENTS =====\n\n`;
  }

  // Plan diff (round 2+)
  if (opts.round >= 2 && opts.planSnapshotDir) {
    const prevSnapshot = `${opts.planSnapshotDir}/plan.md.round-${opts.round - 1}`;
    if (existsSync(prevSnapshot)) {
      const diffProc = Bun.spawn(["diff", "-u", prevSnapshot, opts.inputFile], { stdout: "pipe" });
      const diffText = await new Response(diffProc.stdout).text();
      await diffProc.exited;
      if (diffText.trim()) {
        ctx += `===== PLAN DIFF =====\n前 round からの plan 変更点（unified diff）。\n${diffText}\n===== END PLAN DIFF =====\n\n`;
      }
    }
  }

  return ctx;
}

async function main() {
  const args = process.argv.slice(2);
  let persona = "", issueNum = "", round = 1;
  let inputFile: string | undefined;
  let featureDir = "", resultFile = "";
  let adrContextFile: string | undefined;
  let rejectionFile: string | undefined;
  let judgmentSummaryFile: string | undefined;
  let planSnapshotDir: string | undefined;
  let testSummaryFile: string | undefined;

  // #262 r4 (contrarian F02): arg parse 失敗も fail-closed に倒す。
  // emergencyFailClosed は --result を再パースして verdict.json を書く。
  let unknownArg: string | null = null;
  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--persona":          persona = args[++i]; break;
      case "--issue":            issueNum = args[++i]; break;
      case "--round":            round = parseInt(args[++i]); break;
      case "--input":            inputFile = args[++i]; break;
      case "--feature-dir":      featureDir = args[++i]; break;
      case "--result":           resultFile = args[++i]; break;
      case "--adr-context":      adrContextFile = args[++i]; break;
      case "--rejection":        rejectionFile = args[++i]; break;
      case "--judgment-summary": judgmentSummaryFile = args[++i]; break;
      case "--plan-snapshot-dir": planSnapshotDir = args[++i]; break;
      case "--test-summary":     testSummaryFile = args[++i]; break;
      default: unknownArg = args[i];
    }
    if (unknownArg) break;
  }

  if (unknownArg !== null) {
    // emergencyFailClosed が argv から --result を再パースして fail-closed する。
    throw new Error(`Unknown arg: ${unknownArg}`);
  }

  if (!persona || !issueNum || !featureDir || !resultFile) {
    throw new Error("Usage: dispatch-glm-review.ts --persona <name> --issue <N> --feature-dir <dir> --result <file> [--input <plan>] ...");
  }

  const verdictPath = resultFile.replace(/\.yaml$/, ".verdict.json");
  const logPath = `${resultFile}.log`;

  // #262 r2 (migration F01): 前回 run の stale result/verdict が残らないように
  // dispatch 開始時に削除する。preflight 失敗時もエラー verdict を書いて
  // fail-closed にする (stale な pass/fail が読まれて分岐をすり抜ける問題の防止)。
  mkdirSync(featureDir, { recursive: true });
  for (const p of [resultFile, verdictPath, logPath]) {
    try { if (existsSync(p)) unlinkSync(p); } catch {}
  }

  const failClosed = (reason: string, detail: string): never => {
    const errVerdict = makeErrorVerdict(reason);
    try { writeFileSync(resultFile, detail, "utf-8"); } catch {}
    try { writeFileSync(verdictPath, JSON.stringify(errVerdict, null, 2), "utf-8"); } catch {}
    process.stderr.write(`[GLM dispatch preflight failed] reason=${reason}: ${detail}\n`);
    process.exit(2);
  };

  const agentFile = PERSONA_AGENT[persona];
  if (!agentFile) {
    failClosed("preflight-unknown-persona", `Unknown persona: ${persona}. Valid: ${Object.keys(PERSONA_AGENT).join(", ")}`);
  }

  if (!existsSync(agentFile)) {
    failClosed("preflight-agent-missing", `Agent file not found: ${agentFile}`);
  }

  // Z.AI env
  const zaiEnv = process.env.ZAI_ENV ?? `${process.env.HOME}/AutoClaudeKMP/.env`;
  if (!existsSync(zaiEnv)) {
    failClosed("preflight-env-file-missing", `ZAI_ENV file not found at ${zaiEnv}`);
  }
  const envVars = parseEnvFile(zaiEnv);
  if (!envVars.Z_AI_API_KEY) {
    failClosed("preflight-api-key-missing", `Z_AI_API_KEY not set in ${zaiEnv}`);
  }

  const model = resolveGlmModel(); // #309: GLM_MODEL env 優先、なければ GLM-5.1
  const glmEnv: Record<string, string> = {
    ...(process.env as Record<string, string>),
    ANTHROPIC_BASE_URL: "https://api.z.ai/api/anthropic",
    ANTHROPIC_AUTH_TOKEN: envVars.Z_AI_API_KEY,
    ...buildAnthropicModelEnv(model),
    API_TIMEOUT_MS: "3000000",
    CAD_WORKER: "1",
  };
  delete glmEnv.CLAUDECODE;

  let prompt: string;

  if (persona === "final") {
    // 最終レビュー: git diff + test summary を渡す
    const diffProc = Bun.spawn(["git", "diff", "main...HEAD"], { stdout: "pipe", stderr: "pipe" });
    const diffText = await new Response(diffProc.stdout).text();
    await diffProc.exited;

    let issueCtx = "";
    try {
      const proc = Bun.spawn(
        ["gh", "issue", "view", issueNum, "--json", "title,body,number"],
        { stdout: "pipe", stderr: "pipe" }
      );
      const json = JSON.parse(await new Response(proc.stdout).text());
      await proc.exited;
      issueCtx = `===== ISSUE CONTEXT =====\nIssue #${json.number ?? ""} ${json.title}\n\n${json.body}\n===== END ISSUE CONTEXT =====\n\n`;
    } catch {}

    let testSummaryCtx = "";
    if (testSummaryFile && existsSync(testSummaryFile)) {
      const sumText = readFileSync(testSummaryFile, "utf-8");
      testSummaryCtx = `===== TEST SUMMARY =====\n${sumText}\n===== END TEST SUMMARY =====\n\n`;
    }

    prompt = `${issueCtx}${testSummaryCtx}以下の実装差分（git diff）をレビューしてください。\n出力は定められた YAML フォーマット以外を含めないこと。\n\n${diffText}`;
  } else {
    // 設計レビュー: plan + context blocks を渡す
    if (!inputFile) {
      failClosed("preflight-input-missing", "--input is required for non-final personas");
    }

    const ctx = await buildDesignContext({
      issueNum, round, inputFile, featureDir,
      adrContextFile, rejectionFile, judgmentSummaryFile, planSnapshotDir,
    });

    const planText = readFileSync(inputFile, "utf-8");
    prompt = `${ctx}以下の設計書を [${persona.toUpperCase()}] ペルソナの観点でレビューしてください。\n出力は定められた YAML フォーマット以外を含めないこと。\n\n${planText}`;
  }

  process.stderr.write(`=== dispatch-glm-review: persona=${persona} issue=${issueNum} round=${round} ===\n`);
  process.stderr.write(`  agent:  ${agentFile}\n`);
  process.stderr.write(`  result: ${resultFile}\n`);

  // #262: final persona は diff 全文を読むため default 5 turn では足りない (Issue #255 で実測)。
  // design review は plan.md のみで小さいため従来の 5 turn を維持。
  const defaultMaxTurns = persona === "final" ? "30" : "5";
  const maxTurns = process.env.GLM_MAX_TURNS ?? defaultMaxTurns;
  const proc = Bun.spawn(
    [
      "claude", "-p", prompt,
      "--append-system-prompt-file", agentFile,
      "--allowedTools", "Read",
      "--max-turns", maxTurns,
      "--output-format", "text",
    ],
    { env: glmEnv, stdout: "pipe", stderr: "pipe" }
  );

  const [rawOut, rawErr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);

  // #262 r2 (contrarian F02): rawErr を log file に永続化 — 非ゼロ終了系の
  // 障害は stderr 側にしか情報が出ないことが多く、diagnosis に必要。
  try {
    writeFileSync(
      logPath,
      `=== stderr ===\n${rawErr}\n=== stdout ===\n${rawOut}\n`,
      "utf-8",
    );
  } catch {}
  if (rawErr.trim()) process.stderr.write(`[GLM stderr] ${rawErr.slice(0, 500)}\n`);

  // YAML ブロック抽出 (```yaml ... ``` を優先)
  const yamlBlock = rawOut.match(/```yaml\r?\n([\s\S]*?)```/)?.[1] ?? rawOut;
  const yamlText = yamlBlock.trim();

  // #262: dispatch 失敗を明示的に検出して vacuous pass を防ぐ。
  // #293: pass-with-blockers は dispatch_error にせず demote 経路で吸収する。
  const classification = classifyDispatch(rawOut, exitCode, yamlText);
  if (classification.kind === "fail") {
    writeFileSync(resultFile, rawOut, "utf-8");
    const errVerdict = makeErrorVerdict(classification.reason);
    writeFileSync(verdictPath, JSON.stringify(errVerdict, null, 2), "utf-8");
    process.stderr.write(
      `  verdict=error dispatch_error=true reason=${classification.reason} exit=${exitCode}\n`,
    );
    process.exit(2);
  }

  if (classification.kind === "demote") {
    // verdict=pass を尊重し issues を medium にデモートして書き出す。caller (STEP 7)
    // の verdict=pass 分岐に正しく乗る。デモート事実は verdict.json に記録する。
    const demotedYaml = demoteIssuesToMedium(yamlText);
    writeFileSync(resultFile, demotedYaml, "utf-8");
    const verdict = parseVerdict(demotedYaml);
    const out = {
      ...verdict,
      demoted: true,
      demote_reason: classification.reason,
    };
    writeFileSync(verdictPath, JSON.stringify(out, null, 2), "utf-8");
    process.stderr.write(
      `  verdict=${verdict.verdict} blocking=${verdict.blocking} (demoted: ${classification.reason}, issues→medium)\n`,
    );
    process.exit(0);
  }

  writeFileSync(resultFile, yamlText, "utf-8");

  const verdict = parseVerdict(yamlText);
  writeFileSync(verdictPath, JSON.stringify(verdict, null, 2), "utf-8");

  process.stderr.write(`  verdict=${verdict.verdict} blocking=${verdict.blocking} (C=${verdict.severity_counts.critical} H=${verdict.severity_counts.high})\n`);
  process.exit(0);
}

// #262 r3 F02 (Codex 3 persona 一致): top-level catch でも `*.verdict.json` を
// 書かないと、Bun.spawn / git diff / writeFileSync の例外 (stale 削除後に発生)
// で verdict ファイルが残らず、STEP 7 の dispatch_error 分岐に乗れない。
// args の resultFile を可能な限り推定して fail-closed に倒す。
function emergencyFailClosed(err: unknown): void {
  let resultFile = "";
  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--result") { resultFile = args[i + 1] ?? ""; break; }
  }
  const detail = err instanceof Error ? `${err.message}\n${err.stack ?? ""}` : String(err);
  console.error(`[unhandled-exception] ${detail}`);
  if (resultFile) {
    const verdictPath = resultFile.replace(/\.yaml$/, ".verdict.json");
    try { writeFileSync(resultFile, detail, "utf-8"); } catch {}
    try {
      writeFileSync(
        verdictPath,
        JSON.stringify(makeErrorVerdict("unhandled-exception"), null, 2),
        "utf-8",
      );
    } catch {}
  }
}

if (import.meta.main) {
  main().catch((e) => {
    emergencyFailClosed(e);
    process.exit(2);
  });
}
