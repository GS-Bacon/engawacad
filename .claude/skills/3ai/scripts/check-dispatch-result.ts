#!/usr/bin/env bun
// check-dispatch-result.ts — dispatch 結果ファイルの存在・status を検証する
// Usage: bun check-dispatch-result.ts --result <path> [--auto-raise] [--feature-dir <path>] [--step <name>] [--result-file <path>]
//
// exit 0: result file exists and status == "success"
// exit 1: result file missing or status != "success"
// exit 2: argument error

import { existsSync, readFileSync } from "fs";

const args = process.argv.slice(2);
let resultFile = "";
let autoRaise = false;
let featureDir = "";
let stepName = "";
let raiseResultFile = "";

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--result") resultFile = args[++i];
  else if (args[i] === "--auto-raise") autoRaise = true;
  else if (args[i] === "--feature-dir") featureDir = args[++i];
  else if (args[i] === "--step") stepName = args[++i];
  else if (args[i] === "--result-file") raiseResultFile = args[++i];
}

function tryRaiseIssue(errorSummary: string): void {
  if (!autoRaise || !featureDir || !stepName) return;
  const raiseArgs = [
    import.meta.dir + "/raise-issue-on-failure.ts",
    "--step", stepName,
    "--feature-dir", featureDir,
    "--error-summary", errorSummary,
  ];
  if (raiseResultFile) {
    raiseArgs.push("--result-file", raiseResultFile);
  }
  try {
    Bun.spawnSync(["bun", ...raiseArgs], { stdout: "inherit", stderr: "inherit" });
  } catch {
    // ベストエフォート: 起票失敗してもメインの exit code には影響させない
  }
}

if (!resultFile) {
  process.stderr.write("ERROR: --result <path> is required\n");
  process.exit(2);
}

if (!existsSync(resultFile)) {
  process.stderr.write(
    `ERROR: dispatch result file not found: ${resultFile}\n` +
    `  → dispatch が失敗したか、まだ完了していない可能性があります。\n` +
    `  → dispatch を & でバックグラウンド実行した場合は run_in_background:true のみに変更してください。\n`
  );
  tryRaiseIssue("dispatch result file が生成されなかった（dispatch が正常に完了しなかった可能性）");
  process.exit(1);
}

let result: { status?: string; ci_passed?: boolean; failed_reason?: string };
try {
  result = JSON.parse(readFileSync(resultFile, "utf-8"));
} catch (e) {
  process.stderr.write(`ERROR: result file is not valid JSON: ${resultFile}\n`);
  tryRaiseIssue("dispatch result file が生成されなかった（dispatch が正常に完了しなかった可能性）");
  process.exit(1);
}

if (result.status !== "success") {
  process.stderr.write(
    `ERROR: dispatch failed.\n` +
    `  status: ${result.status}\n` +
    `  ci_passed: ${result.ci_passed}\n` +
    `  failed_reason: ${result.failed_reason ?? "(none)"}\n`
  );
  tryRaiseIssue(`GLM dispatch が失敗: ${result.failed_reason ?? "(none)"}`);
  process.exit(1);
}

if (result.ci_passed === false) {
  process.stderr.write(
    `ERROR: dispatch succeeded but CI failed.\n` +
    `  failed_reason: ${result.failed_reason ?? "(none)"}\n`
  );
  tryRaiseIssue(`GLM dispatch 後の CI が失敗: ${result.failed_reason ?? "(none)"}`);
  process.exit(1);
}

process.stdout.write(`OK: dispatch result is success (${resultFile})\n`);
process.exit(0);
