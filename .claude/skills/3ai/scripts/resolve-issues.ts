#!/usr/bin/env bun
// resolve-issues.ts — raise-issue-on-failure.ts が起票した Issue を自動 close する
//
// 対象: state.json の raised_issues[] に記録された Issue のうち、
//       対応 step が後に passed になったもの（= 自己解消）を close する。
//
// Usage（単一 feature）:
//   bun resolve-issues.ts --feature-dir features/N-slug [--dry-run]
//
// Usage（全 feature を一括走査）:
//   bun resolve-issues.ts --sweep [--dry-run]
//
// exit 0: 正常終了（close 0 件も含む）
// exit 1: 処理中にエラー

import { readFileSync, existsSync, readdirSync } from "fs";
import type { StateData } from "./types.ts";

const CO_RESOLVE_COMMENT =
  "対応ステップが passed になったため自動解消します。\n\n" +
  "このIssueは `raise-issue-on-failure.ts` により自動起票されましたが、" +
  "その後フローが正常完了したため `resolve-issues.ts` により close します。";

// -----------------------------------------------------------------------
// ユーティリティ
// -----------------------------------------------------------------------

function readStateFile(statePath: string): StateData | null {
  try {
    return JSON.parse(readFileSync(statePath, "utf-8")) as StateData;
  } catch {
    return null;
  }
}

/** step が state.json で passed かどうかを判定する */
function isStepPassed(state: StateData, step: string): boolean {
  // step 名 "STEP 8 pre-check (untracked)" のような文字列から
  // state.steps のどのキーに対応するかを判定する。
  // 完全一致がなければ部分一致（ステップ名がキーに含まれるか）で判定する。
  const steps = state.steps ?? {};
  if (steps[step] === "passed") return true;
  // 部分一致: ステップ名がキーに含まれるキーを探す
  for (const [k, v] of Object.entries(steps)) {
    if (v === "passed" && (k.includes(step) || step.includes(k))) return true;
  }
  return false;
}

/** 特定の feature の raised_issues を処理する */
async function resolveForFeature(featureDir: string, dryRun: boolean): Promise<number> {
  const statePath = `${featureDir}/state.json`;
  if (!existsSync(statePath)) {
    process.stdout.write(`  skip: ${featureDir} (state.json なし)\n`);
    return 0;
  }

  const state = readStateFile(statePath);
  if (!state) {
    process.stderr.write(`  WARN: ${statePath} の読み込みに失敗\n`);
    return 0;
  }

  const raisedIssues = state.raised_issues ?? [];
  if (raisedIssues.length === 0) {
    process.stdout.write(`  skip: ${featureDir} (raised_issues なし)\n`);
    return 0;
  }

  let closedCount = 0;

  for (const ri of raisedIssues) {
    // まず GitHub で open かどうか確認
    const checkProc = Bun.spawn(
      ["gh", "issue", "view", String(ri.number), "--json", "state", "--jq", ".state"],
      { stdout: "pipe", stderr: "pipe" }
    );
    const checkOut = (await new Response(checkProc.stdout).text()).trim();
    await checkProc.exited;

    if (checkOut !== "OPEN") {
      process.stdout.write(`  skip: #${ri.number} (state=${checkOut || "unknown"}, 既に close 済み)\n`);
      continue;
    }

    // state.json で対応 step が passed かどうか確認
    const resolved = isStepPassed(state, ri.step);
    if (!resolved) {
      process.stdout.write(`  skip: #${ri.number} (step="${ri.step}" はまだ passed でない)\n`);
      continue;
    }

    process.stdout.write(`  → close: #${ri.number} (step="${ri.step}" が passed)\n`);

    if (dryRun) {
      process.stdout.write(`    (--dry-run: close は行いません)\n`);
      closedCount++;
      continue;
    }

    const closeProc = Bun.spawn(
      ["gh", "issue", "close", String(ri.number), "--comment", CO_RESOLVE_COMMENT],
      { stdout: "pipe", stderr: "pipe" }
    );
    const closeErr = await new Response(closeProc.stderr).text();
    await closeProc.exited;

    if (closeProc.exitCode !== 0) {
      process.stderr.write(`  ERROR: #${ri.number} の close に失敗: ${closeErr}\n`);
    } else {
      process.stdout.write(`  ✓ #${ri.number} を close しました\n`);
      closedCount++;
    }
  }

  return closedCount;
}

// -----------------------------------------------------------------------
// エントリポイント
// -----------------------------------------------------------------------

const args = process.argv.slice(2);
let featureDir = "";
let sweep = false;
let dryRun = false;

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--feature-dir") featureDir = args[++i];
  else if (args[i] === "--sweep") sweep = true;
  else if (args[i] === "--dry-run") dryRun = true;
}

if (!sweep && !featureDir) {
  process.stderr.write(
    "Usage:\n" +
    "  resolve-issues.ts --feature-dir features/N-slug [--dry-run]\n" +
    "  resolve-issues.ts --sweep [--dry-run]\n"
  );
  process.exit(2);
}

if (featureDir) {
  // 単一 feature モード
  process.stdout.write(`=== resolve-issues: ${featureDir} ===\n`);
  const count = await resolveForFeature(featureDir, dryRun);
  process.stdout.write(`${count} 件 close${dryRun ? " (dry-run)" : ""}\n`);
  process.exit(0);
}

// sweep モード: features/ 配下の全 state.json を走査
const featuresDir = "features";
let allDirs: string[] = [];
try {
  allDirs = readdirSync(featuresDir)
    .filter((d) => d.match(/^\d+-/))
    .map((d) => `${featuresDir}/${d}`)
    .sort();
} catch {
  process.stderr.write(`ERROR: ${featuresDir}/ の読み込みに失敗\n`);
  process.exit(1);
}

process.stdout.write(`=== resolve-issues --sweep (${allDirs.length} dirs) ===\n`);
let total = 0;
for (const dir of allDirs) {
  const n = await resolveForFeature(dir, dryRun);
  total += n;
}
process.stdout.write(`\n合計 ${total} 件 close${dryRun ? " (dry-run)" : ""}\n`);
