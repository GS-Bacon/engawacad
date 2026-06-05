#!/usr/bin/env bun
// finalize-feature.ts — 3ai 作業成果物 (features/N-slug/) を git にコミットする
//
// モード1: 単一 finalize（STEP 8 から呼ぶ・冪等）
//   bun .claude/skills/3ai/scripts/finalize-feature.ts --issue N --slug SLUG
//
// モード2: sweep（取りこぼし回収・安全網）
//   bun .claude/skills/3ai/scripts/finalize-feature.ts --sweep [--dry-run]
//
// 冪等保証:
//   - 既に追跡済みで差分がないディレクトリは git add/commit をスキップする
//   - sweep は merge: passed の完走済み Issue だけを対象にする（中途半端な成果物を誤コミットしない）

import { existsSync, readdirSync } from "fs";
import { execSync, spawnSync } from "child_process";
import { getState, setState } from "./state.ts";

const CO_AUTHORED = "Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>";
const FEATURES_DIR = "features";

// -----------------------------------------------------------------------
// git ユーティリティ
// -----------------------------------------------------------------------

/** dir を git stage する。.gitignore (*.raw / *.log) は自動除外される。 */
function gitAdd(dir: string): void {
  execSync(`git add -- ${dir}`, { encoding: "utf-8" });
}

/** stage 済みの dir に実際の差分があるか（なければ up-to-date）*/
function hasStagedDiff(dir: string): boolean {
  const result = spawnSync("git", ["diff", "--cached", "--quiet", "--", dir], {
    encoding: "utf-8",
  });
  // exit 1 = 差分あり, exit 0 = 差分なし
  return result.status !== 0;
}

/** git commit。メッセージはシェルを経由せず spawnSync で直接渡す。 */
function gitCommit(message: string): void {
  const fullMessage = `${message}\n\n${CO_AUTHORED}`;
  const result = spawnSync("git", ["commit", "-m", fullMessage], {
    encoding: "utf-8",
    stdio: "inherit",
  });
  if (result.status !== 0) {
    console.error("git commit failed");
    process.exit(1);
  }
}

// -----------------------------------------------------------------------
// モード1: 単一 finalize
// -----------------------------------------------------------------------

/**
 * 単一の feature ディレクトリを stage → commit → state 記録する。
 * 既にコミット済み（差分なし）なら何もしない（冪等）。
 */
function finalizeOne(issue: number, slug: string): void {
  const dir = `${FEATURES_DIR}/${issue}-${slug}`;
  const statePath = `${dir}/state.json`;

  if (!existsSync(dir)) {
    console.error(`ERROR: ${dir} が存在しません`);
    process.exit(1);
  }
  if (!existsSync(statePath)) {
    console.error(`ERROR: ${statePath} が存在しません`);
    process.exit(1);
  }

  // setState を git add の前に実行し、state.json の更新も同一コミットに含める
  setState(statePath, "artifacts_committed", "passed");
  gitAdd(dir);

  if (!hasStagedDiff(dir)) {
    console.log(`up-to-date: ${dir} (差分なし、コミット不要)`);
    return;
  }

  const message = `chore(3ai): #${issue} 作業成果物をコミット`;
  gitCommit(message);
  console.log(`✓ ${dir} をコミットしました`);
}

// -----------------------------------------------------------------------
// モード2: sweep
// -----------------------------------------------------------------------

/**
 * git status --porcelain の出力から未追跡/変更ありのファイルを features/ 単位で集約し、
 * 「完走済み（merge: passed）」のディレクトリだけを対象に返す。
 */
function collectOrphanedDirs(): string[] {
  let statusOutput: string;
  try {
    statusOutput = execSync("git status --porcelain -- features/", {
      encoding: "utf-8",
    });
  } catch {
    statusOutput = "";
  }

  // 対象ディレクトリを抽出（重複排除）
  const dirSet = new Set<string>();
  for (const line of statusOutput.split("\n")) {
    if (!line.trim()) continue;
    // 形式: "?? features/47-foo/bar.md" or " M features/47-foo/bar.md"
    const parts = line.trimStart().split(/\s+/);
    const filePath = parts[parts.length - 1];
    const match = filePath.match(/^(features\/[^/]+)\//);
    if (match) dirSet.add(match[1]);
  }

  // merge: passed のもののみ選別
  const result: string[] = [];
  for (const dir of [...dirSet].sort()) {
    const statePath = `${dir}/state.json`;
    if (!existsSync(statePath)) {
      console.warn(`  skip: ${dir} (state.json なし — 未完走とみなす)`);
      continue;
    }
    const mergeVal = getState(statePath, "merge");
    if (mergeVal !== "passed") {
      console.warn(`  skip: ${dir} (merge=${mergeVal} — 未完走とみなす)`);
      continue;
    }
    result.push(dir);
  }
  return result;
}

/**
 * orphaned な feature ディレクトリを一括コミットする。
 * --dry-run 時は対象一覧を表示するだけで commit しない。
 */
function sweep(dryRun: boolean): void {
  const dirs = collectOrphanedDirs();

  if (dirs.length === 0) {
    console.log("no orphaned artifacts — 全成果物は追跡済みです");
    return;
  }

  const issueNums = dirs.map((d) => {
    const m = d.match(/features\/(\d+)-/);
    return m ? `#${m[1]}` : d;
  });

  console.log(`対象 ${dirs.length} 件: ${issueNums.join(", ")}`);

  if (dryRun) {
    console.log("(--dry-run: コミットは行いません)");
    for (const dir of dirs) console.log(`  ${dir}`);
    return;
  }

  // setState を git add の前に実行し、state.json の更新も同一コミットに含める
  for (const dir of dirs) {
    const statePath = `${dir}/state.json`;
    if (existsSync(statePath)) {
      setState(statePath, "artifacts_committed", "passed");
    }
  }

  // 一括 git add
  for (const dir of dirs) {
    gitAdd(dir);
    console.log(`  staged: ${dir}`);
  }

  // まとめて 1 コミット
  const numsStr = issueNums.join(" ");
  const message = `chore(3ai): 完了 feature の作業成果物を回収 (${numsStr})`;
  gitCommit(message);
  console.log(`✓ ${dirs.length} 件の成果物をコミットしました`);
}

// -----------------------------------------------------------------------
// CLI エントリポイント
// -----------------------------------------------------------------------

if (import.meta.main) {
  const args = process.argv.slice(2);

  // --sweep [--dry-run]
  if (args.includes("--sweep")) {
    const dryRun = args.includes("--dry-run");
    sweep(dryRun);
    process.exit(0);
  }

  // --issue N --slug SLUG
  let issueStr = "";
  let slug = "";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--issue") issueStr = args[++i];
    else if (args[i] === "--slug") slug = args[++i];
    else {
      console.error(`不明な引数: ${args[i]}`);
      process.exit(1);
    }
  }

  if (!issueStr || !slug) {
    console.error(
      "Usage:\n" +
        "  finalize-feature.ts --issue N --slug SLUG\n" +
        "  finalize-feature.ts --sweep [--dry-run]"
    );
    process.exit(1);
  }

  const issue = parseInt(issueStr, 10);
  if (isNaN(issue)) {
    console.error(`--issue の値が不正: ${issueStr}`);
    process.exit(1);
  }

  finalizeOne(issue, slug);
}
