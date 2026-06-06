#!/usr/bin/env bun
// build-codex-input.ts — codex-input.md を plan の Non-Goals + テストサマリーから生成
// Usage: bun build-codex-input.ts --plan-file <path> --test-summary <path> --output <path>
//
// 生成する codex-input.md:
//   ===== TEST SUMMARY =====   (from test-summary.json)
//   ===== NON-GOALS =====      (from plan.md ## Non-Goals)
//   ===== KNOWN IGNORED TESTS ===== (from ci.log / test-summary of ignored tests)

import { readFileSync, writeFileSync, existsSync } from "fs";

const args = process.argv.slice(2);
let planFile = "";
let testSummaryFile = "";
let outputFile = "";
let ciLogFile = "";

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--plan-file") planFile = args[++i];
  else if (args[i] === "--test-summary") testSummaryFile = args[++i];
  else if (args[i] === "--output") outputFile = args[++i];
  else if (args[i] === "--ci-log") ciLogFile = args[++i];
}

if (!planFile || !testSummaryFile || !outputFile) {
  process.stderr.write("Usage: build-codex-input.ts --plan-file <p> --test-summary <s> --output <o> [--ci-log <l>]\n");
  process.exit(2);
}

// --- TEST SUMMARY ---
let testSummaryText = "";
if (existsSync(testSummaryFile)) {
  testSummaryText = readFileSync(testSummaryFile, "utf-8");
}

// --- Non-Goals from plan.md ---
let nonGoalsText = "(plan.md に ## Non-Goals セクションがありません)";
if (existsSync(planFile)) {
  const plan = readFileSync(planFile, "utf-8");
  const m = plan.match(/^##\s+Non-Goals\s*\n([\s\S]*?)(?=^##|\z)/m);
  if (m) {
    nonGoalsText = m[1].trim();
  }
}

// --- Ignored tests from ci.log ---
let ignoredTests: string[] = [];
if (ciLogFile && existsSync(ciLogFile)) {
  const ciLog = readFileSync(ciLogFile, "utf-8");
  for (const m of ciLog.matchAll(/^test\s+(\S+)\s+\.\.\.\s+ignored,\s*(.+)$/gm)) {
    ignoredTests.push(`- ${m[1]}: ${m[2].trim()}`);
  }
}

// --- Build output ---
const sections: string[] = [];

sections.push("===== TEST SUMMARY =====");
sections.push(testSummaryText || "(test-summary.json not found)");
sections.push("===== END TEST SUMMARY =====");

sections.push("");
sections.push("===== NON-GOALS (SCOPE OUT — Codex はこれらを指摘しないこと) =====");
sections.push(nonGoalsText);
sections.push("===== END NON-GOALS =====");

if (ignoredTests.length > 0) {
  sections.push("");
  sections.push("===== KNOWN IGNORED TESTS (理由付き #[ignore] — watertight 不可等の既知制約) =====");
  sections.push(ignoredTests.join("\n"));
  sections.push("===== END KNOWN IGNORED TESTS =====");
}

sections.push("");
sections.push("===== NOTE: total_added=0 について =====");
sections.push("テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の");
sections.push("集計に乗らないことがある。git diff で実際の追加テストを確認すること。");
sections.push("===== END NOTE =====");

writeFileSync(outputFile, sections.join("\n") + "\n");
process.stdout.write(`OK: ${outputFile} を生成しました\n`);
