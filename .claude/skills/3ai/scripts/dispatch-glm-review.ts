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

import { readFileSync, writeFileSync, existsSync, mkdirSync } from "fs";

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

function parseVerdict(text: string): {
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
      default: console.error(`Unknown arg: ${args[i]}`); process.exit(1);
    }
  }

  if (!persona || !issueNum || !featureDir || !resultFile) {
    console.error("Usage: dispatch-glm-review.ts --persona <name> --issue <N> --feature-dir <dir> --result <file> [--input <plan>] ...");
    process.exit(1);
  }

  const agentFile = PERSONA_AGENT[persona];
  if (!agentFile) {
    console.error(`Unknown persona: ${persona}. Valid: ${Object.keys(PERSONA_AGENT).join(", ")}`);
    process.exit(1);
  }

  if (!existsSync(agentFile)) {
    console.error(`Agent file not found: ${agentFile}`);
    process.exit(1);
  }

  // Z.AI env
  const zaiEnv = process.env.ZAI_ENV ?? `${process.env.HOME}/AutoClaudeKMP/.env`;
  if (!existsSync(zaiEnv)) {
    console.error(`ERROR: ZAI_ENV file not found at ${zaiEnv}`);
    process.exit(1);
  }
  const envVars = parseEnvFile(zaiEnv);
  if (!envVars.Z_AI_API_KEY) {
    console.error(`ERROR: Z_AI_API_KEY not set in ${zaiEnv}`);
    process.exit(1);
  }

  const model = process.env.GLM_MODEL ?? "claude-opus-4-5-20251101";
  const glmEnv: Record<string, string> = {
    ...(process.env as Record<string, string>),
    ANTHROPIC_BASE_URL: "https://api.z.ai/api/anthropic",
    ANTHROPIC_AUTH_TOKEN: envVars.Z_AI_API_KEY,
    ANTHROPIC_DEFAULT_OPUS_MODEL: model,
    ANTHROPIC_DEFAULT_SONNET_MODEL: model,
    ANTHROPIC_DEFAULT_HAIKU_MODEL: model,
    API_TIMEOUT_MS: "3000000",
    CAD_WORKER: "1",
  };
  delete glmEnv.CLAUDECODE;

  mkdirSync(featureDir, { recursive: true });

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
      console.error("--input is required for non-final personas");
      process.exit(1);
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

  const maxTurns = process.env.GLM_MAX_TURNS ?? "5";
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

  const [rawOut, rawErr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  await proc.exited;

  if (rawErr.trim()) process.stderr.write(`[GLM stderr] ${rawErr.slice(0, 500)}\n`);

  // YAML ブロック抽出 (```yaml ... ``` を優先)
  const yamlBlock = rawOut.match(/```yaml\r?\n([\s\S]*?)```/)?.[1] ?? rawOut;
  const yamlText = yamlBlock.trim();

  writeFileSync(resultFile, yamlText, "utf-8");

  const verdict = parseVerdict(yamlText);
  const verdictPath = resultFile.replace(/\.yaml$/, ".verdict.json");
  writeFileSync(verdictPath, JSON.stringify(verdict, null, 2), "utf-8");

  process.stderr.write(`  verdict=${verdict.verdict} blocking=${verdict.blocking} (C=${verdict.severity_counts.critical} H=${verdict.severity_counts.high})\n`);
  process.exit(0);
}

main().catch((e) => { console.error(e); process.exit(1); });
