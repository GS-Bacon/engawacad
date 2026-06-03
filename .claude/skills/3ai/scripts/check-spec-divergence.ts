#!/usr/bin/env bun
// check-spec-divergence.ts — plan.md のテスト計画と実装 assertion の突き合わせ材料を出力する
// 使い方: bun check-spec-divergence.ts --plan-file <path> --feature-dir <dir>
// 出力は stdout に整形テキスト。Claude が読んで期待値乖離を判定する。

import { readFileSync, existsSync } from "fs";
import { execSync } from "child_process";
import { resolve, dirname } from "path";

function parseArgs(): { planFile: string; featureDir: string } {
  const args = process.argv.slice(2);
  let planFile = "";
  let featureDir = "";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--plan-file" && args[i + 1]) planFile = args[++i];
    if (args[i] === "--feature-dir" && args[i + 1]) featureDir = args[++i];
  }
  if (!planFile) {
    console.error("Usage: check-spec-divergence.ts --plan-file <path> [--feature-dir <dir>]");
    process.exit(1);
  }
  return { planFile, featureDir };
}

function extractTestPlanSection(content: string): string {
  const match = content.match(/^##\s+テスト計画[^\n]*\n([\s\S]*?)(?=^##\s|\s*$)/m);
  return match ? match[0].trimEnd() : "";
}

function extractNumericParagraphs(content: string): string[] {
  // T\d+[a-z]? を参照する段落のうち、数値表記 (≈, 相対誤差, 理論値, expected) を含むもの
  const paragraphs: string[] = [];
  const blocks = content.split(/\n{2,}/);
  for (const block of blocks) {
    if (/T\d+[a-z]?/.test(block) && /[≈≒]|相対誤差|理論値|expected|期待値/.test(block)) {
      paragraphs.push(block.trim());
    }
  }
  return paragraphs;
}

function extractTIdsFromPlan(content: string): string[] {
  // "| T01 |" 形式の表行から T ID を抽出
  const ids: string[] = [];
  for (const line of content.split("\n")) {
    const m = line.match(/\|\s*(T\d+[a-zA-Z]?)\s*\|/);
    if (m) ids.push(m[1]);
  }
  return [...new Set(ids)];
}

function getChangedRustFiles(repoRoot: string): string[] {
  try {
    const out = execSync("git diff main..HEAD --name-only -- 'crates/**/*.rs'", {
      cwd: repoRoot,
      encoding: "utf-8",
      stdio: ["pipe", "pipe", "pipe"],
    }).trim();
    return out ? out.split("\n").map((p) => resolve(repoRoot, p)) : [];
  } catch {
    return [];
  }
}

function extractTestFunctions(filePath: string, tIds: string[]): Array<{ path: string; line: number; snippet: string }> {
  if (!existsSync(filePath)) return [];
  const lines = readFileSync(filePath, "utf-8").split("\n");
  const results: Array<{ path: string; line: number; snippet: string }> = [];

  // T ID から派生するパターン (t27_, t22b_, etc.) + 汎用 fn t\d+[a-z]?_
  const tPatterns = tIds.map((id) => {
    const lower = id.toLowerCase();
    return new RegExp(`fn\\s+${lower}_`);
  });
  const genericPattern = /fn\s+t\d+[a-z]?_\w+/;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const isMatch =
      tPatterns.some((p) => p.test(line)) || genericPattern.test(line);
    if (!isMatch) continue;

    // 関数本体を最大 35 行抽出
    const snippetLines: string[] = [];
    let depth = 0;
    let started = false;
    for (let j = i; j < Math.min(i + 50, lines.length); j++) {
      snippetLines.push(lines[j]);
      for (const ch of lines[j]) {
        if (ch === "{") { depth++; started = true; }
        if (ch === "}") depth--;
      }
      if (started && depth <= 0) break;
      if (snippetLines.length >= 35) break;
    }

    results.push({ path: filePath, line: i + 1, snippet: snippetLines.join("\n") });
  }
  return results;
}

function findRepoRoot(startDir: string): string {
  let dir = resolve(startDir);
  for (let i = 0; i < 10; i++) {
    if (existsSync(`${dir}/.git`)) return dir;
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return startDir;
}

function main() {
  const { planFile, featureDir } = parseArgs();
  const planContent = readFileSync(resolve(planFile), "utf-8");
  const repoRoot = findRepoRoot(featureDir || dirname(resolve(planFile)));

  const testPlanSection = extractTestPlanSection(planContent);
  const numericParagraphs = extractNumericParagraphs(planContent);
  const tIds = extractTIdsFromPlan(planContent);
  const changedFiles = getChangedRustFiles(repoRoot);

  const allFunctions: Array<{ path: string; line: number; snippet: string }> = [];
  for (const file of changedFiles) {
    allFunctions.push(...extractTestFunctions(file, tIds));
  }

  // --- output ---
  console.log("=== plan.md: ## テスト計画 ===");
  console.log(testPlanSection || "(テスト計画セクションが見つかりません)");

  console.log("\n=== plan.md: T ID 周辺の数値段落 ===");
  if (numericParagraphs.length === 0) {
    console.log("(数値段落なし)");
  } else {
    for (const p of numericParagraphs) {
      console.log(p);
      console.log();
    }
  }

  console.log("=== implementation: 変更されたテスト関数 ===");
  if (changedFiles.length === 0) {
    console.log("(git diff main..HEAD で変更された .rs ファイルなし — ブランチが main と同じか、crates/ 外の変更のみ)");
  } else if (allFunctions.length === 0) {
    console.log(`変更ファイル: ${changedFiles.map((f) => f.replace(repoRoot + "/", "")).join(", ")}`);
    console.log("(T ID にマッチするテスト関数が見つかりません)");
  } else {
    for (const fn of allFunctions) {
      const relPath = fn.path.replace(repoRoot + "/", "");
      console.log(`[${relPath}:${fn.line}]`);
      console.log(fn.snippet);
      console.log();
    }
  }
}

main();
