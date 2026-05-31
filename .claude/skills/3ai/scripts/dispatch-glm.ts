#!/usr/bin/env bun
// dispatch-glm.ts — GLM-5.1 (Z.AI) ワーカー起動スクリプト
// --mode core  : コア実装 + plan T01〜の最小テスト
// --mode test  : test-spec.md 主導のエッジケーステスト追加
// --mode (未指定): 実装+テスト一括

import { readFileSync, writeFileSync, existsSync, mkdirSync } from "fs";

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

function normalizePattern(s: string): string {
  return s
    .replace(/\x1b\[[0-9;]*m/g, "")
    .replace(/([A-Za-z0-9_./-]+\.rs):\d+:\d+/g, "$1:LINE:COL")
    .replace(/([A-Za-z0-9_./-]+\.rs):\d+/g, "$1:LINE")
    .replace(/  +/g, " ")
    .trim();
}

function extractErrorPattern(ciLog: string): { pattern: string | null; kind: string } {
  const lines = ciLog.split("\n");
  for (let i = 0; i < lines.length; i++) {
    if (/^error\[E\d+\]/.test(lines[i])) {
      const nxt = lines[i + 1] ?? "";
      let p = normalizePattern(lines[i]);
      if (nxt.trimStart().startsWith("-->")) p += " " + normalizePattern(nxt);
      return { pattern: p, kind: "compile" };
    }
  }
  for (const line of lines) {
    if (/^test \S+ \.\.\. FAILED$/.test(line))
      return { pattern: normalizePattern(line), kind: "test" };
  }
  for (const line of lines) {
    if (line.startsWith("error:"))
      return { pattern: normalizePattern(line), kind: "generic" };
  }
  return { pattern: null, kind: "none" };
}

function extractGlmSummary(raw: string): string {
  const lines = raw.trim().split("\n").reverse();
  for (const line of lines) {
    try {
      const obj = JSON.parse(line);
      if (typeof obj === "object" && obj !== null && "summary" in obj) return String(obj.summary);
    } catch {}
  }
  return "";
}

function buildPrompt(
  featureDir: string,
  planFile: string,
  resultFile: string,
  glmMode: string,
  debugSpecSection: string,
  testSpecSection: string
): string {
  const plan = readFileSync(planFile, "utf-8");

  if (glmMode === "core") {
    return `以下の確定プランに従い**コア実装**と**最小テスト**を完了させてください。

## 作業ディレクトリ
${featureDir}

## 確定プラン
${plan}
${debugSpecSection}
## 完了条件（コアモード）
1. プランに記載された全機能を実装する
2. テスト計画のうち **core テスト**（T01〜 のうち決定性・正常系の最小セット）を実装し通過させる
   - エッジケーステスト・敵対テストの実装は不要（後続ステップで追加される）
3. \`cargo xtask ci\` が green（build / test / clippy -D warnings / fmt --check）
4. 結果を ${resultFile} に JSON で書き出す:
   { "status": "success|failed", "ci_passed": true|false, "summary": "...", "failed_reason": "...", "tests_added": N }

## 禁止事項
- git commit/push は行わない（オーケストレーターが行う）
- プラン外の機能追加・リファクタは行わない`;
  }

  if (glmMode === "test") {
    return `以下のテスト仕様に従い**エッジケーステスト**を追加して CI を通してください。

## 作業ディレクトリ
${featureDir}

## 確定プラン（参照用）
${plan}
${testSpecSection}
${debugSpecSection}
## 完了条件（テストモード）
1. テスト仕様の全ケースを実装し通過させる
2. テストで露見した本体の明白なバグは最小修正可（それ以外の本体変更は禁止）
3. \`cargo xtask ci\` が green（build / test / clippy -D warnings / fmt --check）
4. 結果を ${resultFile} に JSON で書き出す:
   { "status": "success|failed", "ci_passed": true|false, "summary": "...", "failed_reason": "...", "tests_added": N, "tests_added_in_phase_2": N }

## 禁止事項
- git commit/push は行わない（オーケストレーターが行う）
- テスト仕様外の機能追加・リファクタは行わない`;
  }

  return `以下の確定プランに従い実装・テスト・CI を完了させてください。

## 作業ディレクトリ
${featureDir}

## 確定プラン
${plan}
${debugSpecSection}
## 完了条件
1. プランに記載された全機能を実装する
2. テスト計画の全ケースを実装し通過させる
3. \`cargo xtask ci\` が green（build / test / clippy -D warnings / fmt --check）
4. エッジケーステスト: "壊しに行く" 敵対ペルソナで境界・退化入力を網羅する
5. 結果を ${resultFile} に JSON で書き出す:
   { "status": "success|failed", "ci_passed": true|false, "summary": "...", "failed_reason": "..." }

## 禁止事項
- git commit/push は行わない（オーケストレーターが行う）
- プラン外の機能追加・リファクタは行わない`;
}

if (import.meta.main) {
  let agentFile = "";
  let planFile = "";
  let featureDir = "";
  let resultFile = "";
  let maxTurns = 80;
  let model = "GLM-5.1";
  let debugSpec = "";
  let glmMode = "";
  let testSpecFile = "";

  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--agent") agentFile = args[++i];
    else if (args[i] === "--plan-file") planFile = args[++i];
    else if (args[i] === "--feature-dir") featureDir = args[++i];
    else if (args[i] === "--result-file") resultFile = args[++i];
    else if (args[i] === "--max-turns") maxTurns = parseInt(args[++i]);
    else if (args[i] === "--model") model = args[++i];
    else if (args[i] === "--debug-spec") debugSpec = args[++i];
    else if (args[i] === "--mode") glmMode = args[++i];
    else if (args[i] === "--test-spec") testSpecFile = args[++i];
    else { console.error(`Unknown arg: ${args[i]}`); process.exit(1); }
  }

  if (!agentFile || !planFile || !featureDir || !resultFile) {
    console.error("ERROR: --agent, --plan-file, --feature-dir, --result-file are required");
    process.exit(1);
  }
  if (debugSpec && !existsSync(debugSpec)) {
    console.error(`ERROR: --debug-spec file not found: ${debugSpec}`); process.exit(1);
  }
  if (glmMode === "test" && !testSpecFile) {
    console.error("ERROR: --test-spec required for --mode test"); process.exit(1);
  }
  if (testSpecFile && !existsSync(testSpecFile)) {
    console.error(`ERROR: --test-spec file not found: ${testSpecFile}`); process.exit(1);
  }

  // Load Z.AI credentials
  const zaiEnv = process.env.ZAI_ENV ?? `${process.env.HOME}/AutoClaudeKMP/.env`;
  if (!existsSync(zaiEnv)) {
    console.error(`ERROR: Z.AI env file not found: ${zaiEnv}`);
    console.error("  Set ZAI_ENV env var to the path of the .env file containing Z_AI_API_KEY");
    process.exit(1);
  }
  const envVars = parseEnvFile(zaiEnv);
  if (!envVars.Z_AI_API_KEY) {
    console.error(`ERROR: Z_AI_API_KEY not set in ${zaiEnv}`); process.exit(1);
  }

  const glmEnv: Record<string, string> = {
    ...process.env as Record<string, string>,
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

  const debugSpecSection = debugSpec
    ? `\n## オーケストレーター（Claude）からの修正仕様\n以下の仕様は前回 CI 失敗の根本原因仮説と修正方針です。実装はこの仕様に従ってください。\n仕様の論理的整合性に疑義があればコードを書く前に summary に明記して停止してください。\n\n${readFileSync(debugSpec, "utf-8")}`
    : "";

  const testSpecSection = testSpecFile
    ? `\n## テスト仕様（オーケストレーターが作成）\n以下の仕様に従いテストを追加してください。\nテストに問題なく追加できる場合は本体実装の最小修正も可。\n\n${readFileSync(testSpecFile, "utf-8")}`
    : "";

  const prompt = buildPrompt(featureDir, planFile, resultFile, glmMode, debugSpecSection, testSpecSection);

  process.stderr.write(`=== dispatch-glm: starting GLM worker ===\n`);
  process.stderr.write(`  agent:   ${agentFile}\n`);
  process.stderr.write(`  plan:    ${planFile}\n`);
  process.stderr.write(`  feature: ${featureDir}\n`);
  process.stderr.write(`  result:  ${resultFile}\n`);
  process.stderr.write(`  model:   ${model}\n`);
  process.stderr.write(`  mode:    ${glmMode || "default"}\n`);

  const claudeProc = Bun.spawn(
    [
      "claude",
      "-p",
      prompt,
      "--append-system-prompt-file",
      agentFile,
      "--allowedTools",
      "Read,Edit,Write,Bash(cargo *),Bash(npm *),Bash(npx *),Bash(node *),Bash(mkdir *),Bash(cat *),Bash(ls *),Bash(find *),Glob,Grep",
      "--max-turns",
      String(maxTurns),
      "--output-format",
      "json",
    ],
    {
      env: glmEnv,
      stdout: "pipe",
      stderr: "pipe",
    }
  );
  const [glmOut, glmErr, glmExit] = await Promise.all([
    new Response(claudeProc.stdout).text(),
    new Response(claudeProc.stderr).text(),
    claudeProc.exited,
  ]);
  writeFileSync(`${resultFile}.raw`, glmOut + glmErr);

  // Authoritative CI check
  const root = Bun.spawnSync(["git", "rev-parse", "--show-toplevel"]).stdout.toString().trim();
  const ciProc = Bun.spawn(["cargo", "xtask", "ci"], {
    cwd: root,
    env: { ...process.env as Record<string, string>, CARGO_TERM_COLOR: "never" },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [ciOut, ciErr, ciExit] = await Promise.all([
    new Response(ciProc.stdout).text(),
    new Response(ciProc.stderr).text(),
    ciProc.exited,
  ]);
  const ciPassed = ciExit === 0;
  const ciLogPath = `${featureDir}/ci.log`;
  writeFileSync(ciLogPath, ciOut + ciErr);

  const summary = extractGlmSummary(glmOut + glmErr);
  const out: Record<string, unknown> = {
    status: ciPassed ? "success" : "failed",
    ci_passed: ciPassed,
    summary: summary || (ciPassed ? "CI green" : "CI red"),
    glm_exit: glmExit,
    debug_spec_used: Boolean(debugSpec),
  };

  if (!ciPassed) {
    out.failed_reason = `cargo xtask ci failed — see ${ciLogPath}`;
    const { pattern, kind } = extractErrorPattern(ciOut + ciErr);
    out.error_pattern = pattern;
    out.error_pattern_kind = kind;
  }

  writeFileSync(resultFile, JSON.stringify(out, null, 2));
}
