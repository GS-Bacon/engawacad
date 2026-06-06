#!/usr/bin/env bun
// check-dispatch-result.ts — dispatch 結果ファイルの存在・status を検証する
// Usage: bun check-dispatch-result.ts --result <path>
//
// exit 0: result file exists and status == "success"
// exit 1: result file missing or status != "success"
// exit 2: argument error

import { existsSync, readFileSync } from "fs";

const args = process.argv.slice(2);
let resultFile = "";

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--result") resultFile = args[++i];
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
  process.exit(1);
}

let result: { status?: string; ci_passed?: boolean; failed_reason?: string };
try {
  result = JSON.parse(readFileSync(resultFile, "utf-8"));
} catch (e) {
  process.stderr.write(`ERROR: result file is not valid JSON: ${resultFile}\n`);
  process.exit(1);
}

if (result.status !== "success") {
  process.stderr.write(
    `ERROR: dispatch failed.\n` +
    `  status: ${result.status}\n` +
    `  ci_passed: ${result.ci_passed}\n` +
    `  failed_reason: ${result.failed_reason ?? "(none)"}\n`
  );
  process.exit(1);
}

if (result.ci_passed === false) {
  process.stderr.write(
    `ERROR: dispatch succeeded but CI failed.\n` +
    `  failed_reason: ${result.failed_reason ?? "(none)"}\n`
  );
  process.exit(1);
}

process.stdout.write(`OK: dispatch result is success (${resultFile})\n`);
process.exit(0);
