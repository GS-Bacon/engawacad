#!/usr/bin/env bun
// reset-needs-human.ts — pause-streak-tracker 由来の needs-human を bulk reset
//
// 目的: Codex CLI usage limit のような「時間で復帰するインフラ障害」で needs-human が
//   付いた Issue を一括で復帰させ、/3ailoop の候補プールに戻す。
//
// デフォルト対象: features/.loop/pause-streak/*-codex-usage-limit.json
//   → 各対象 Issue から needs-human ラベルを剥がし、pause-streak カウンタを reset。
//
// 使い方:
//   bun reset-needs-human.ts [--dry-run] [--category <cat>] [--issue N] [--all-categories]
//
// exit code:
//   0 = 成功 (dry-run 含む)
//   1 = 対象走査中の致命的エラー

import { existsSync, readdirSync, readFileSync, rmSync } from "fs";
import { join } from "path";

export const PAUSE_STREAK_ROOT = "features/.loop/pause-streak";
export const DEFAULT_CATEGORY = "codex-usage-limit";

export interface Target {
  issue: number;
  category: string;
  count: number;
  last_reason?: string;
}

/**
 * pause-streak 履歴ディレクトリを走査し、対象カテゴリの状態ファイルを列挙する。
 * ファイル名フォーマット: <issue>-<category>.json (例: 276-codex-usage-limit.json)
 * category は kebab-case 前提。
 */
export function scanTargets(root: string, categoryFilter: string | null): Target[] {
  if (!existsSync(root)) return [];
  const targets: Target[] = [];
  for (const fname of readdirSync(root)) {
    if (!fname.endsWith(".json")) continue;
    if (fname.endsWith(".tmp")) continue;
    if (fname.includes(".tmp.")) continue;
    const stem = fname.slice(0, -".json".length);
    // 最初の "-" で issue と category を分離 (category 側にも "-" が入りうる)
    const dash = stem.indexOf("-");
    if (dash < 0) continue;
    const issueStr = stem.slice(0, dash);
    const category = stem.slice(dash + 1);
    const issue = parseInt(issueStr);
    if (isNaN(issue)) continue;
    if (!category) continue;
    if (categoryFilter !== null && category !== categoryFilter) continue;

    try {
      const state = JSON.parse(readFileSync(join(root, fname), "utf-8")) as {
        count?: number;
        last_reason?: string;
      };
      targets.push({
        issue,
        category,
        count: state.count ?? 0,
        last_reason: state.last_reason,
      });
    } catch {
      // 破損 JSON は skip
    }
  }
  return targets;
}

async function ghRun(
  args: string[],
  allowFailure = false,
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  await proc.exited;
  const exitCode = proc.exitCode ?? 0;
  if (exitCode !== 0 && !allowFailure) {
    throw new Error(
      `gh ${args.join(" ")} failed exit=${exitCode}: ${stderr.trim()}`,
    );
  }
  return { exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
}

async function hasNeedsHumanLabel(issue: number): Promise<boolean> {
  const r = await ghRun(
    ["issue", "view", String(issue), "--json", "labels"],
    true,
  );
  if (r.exitCode !== 0) return false;
  try {
    const json = JSON.parse(r.stdout) as { labels?: Array<{ name: string }> };
    return (json.labels ?? []).some((l) => l.name === "needs-human");
  } catch {
    return false;
  }
}

async function removeNeedsHuman(issue: number): Promise<void> {
  await ghRun([
    "issue",
    "edit",
    String(issue),
    "--remove-label",
    "needs-human",
  ]);
}

async function postComment(issue: number, body: string): Promise<void> {
  await ghRun(["issue", "comment", String(issue), "--body", body], true);
}

interface ResetResult {
  issue: number;
  category: string;
  had_label: boolean;
  removed: boolean;
  streak_reset: boolean;
}

async function main() {
  const args = process.argv.slice(2);
  let dryRun = false;
  let categoryFilter: string | null = DEFAULT_CATEGORY;
  let singleIssue: number | null = null;

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--dry-run":
        dryRun = true;
        break;
      case "--all-categories":
        categoryFilter = null;
        break;
      case "--category":
        categoryFilter = args[++i];
        break;
      case "--issue":
        singleIssue = parseInt(args[++i]);
        break;
      case "--help":
        console.log(
          "Usage: reset-needs-human.ts [--dry-run] [--category <cat>] [--issue N] [--all-categories]",
        );
        process.exit(0);
      default:
        console.error(`Unknown arg: ${args[i]}`);
        process.exit(2);
    }
  }

  let targets = scanTargets(PAUSE_STREAK_ROOT, categoryFilter);
  if (singleIssue !== null) {
    targets = targets.filter((t) => t.issue === singleIssue);
    // pause-streak 履歴に無くても --issue 指定なら強制対象
    if (targets.length === 0) {
      targets = [{ issue: singleIssue, category: "(none)", count: 0 }];
    }
  }

  console.log(`=== reset-needs-human ${dryRun ? "(DRY RUN)" : ""} ===`);
  console.log(`Category filter: ${categoryFilter ?? "(all)"}`);
  console.log(`Targets: ${targets.length}`);

  if (targets.length === 0) {
    console.log(JSON.stringify({ ok: true, targets: [], results: [] }));
    return;
  }

  const results: ResetResult[] = [];

  for (const t of targets) {
    const had = await hasNeedsHumanLabel(t.issue);
    console.log(
      `  #${t.issue} category=${t.category} count=${t.count} needs-human=${had}`,
    );
    if (dryRun) {
      results.push({
        issue: t.issue,
        category: t.category,
        had_label: had,
        removed: false,
        streak_reset: false,
      });
      continue;
    }

    // ラベル解除
    let removed = false;
    if (had) {
      try {
        await removeNeedsHuman(t.issue);
        removed = true;
      } catch (e) {
        process.stderr.write(
          `WARN: remove needs-human #${t.issue} failed: ${(e as Error).message}\n`,
        );
      }
    }

    // pause-streak カウンタ reset (履歴に存在するもののみ)
    let streakReset = false;
    if (t.category !== "(none)") {
      const path = join(PAUSE_STREAK_ROOT, `${t.issue}-${t.category}.json`);
      if (existsSync(path)) {
        try {
          rmSync(path);
          streakReset = true;
        } catch (e) {
          process.stderr.write(
            `WARN: reset streak #${t.issue}/${t.category} failed: ${(e as Error).message}\n`,
          );
        }
      }
    }

    // コメント追記
    if (removed) {
      try {
        await postComment(
          t.issue,
          `[auto reset-needs-human] category=\`${t.category}\` の pause-streak が復旧待ちインフラ障害由来と判定されたため needs-human ラベルを自動解除しました。次サイクル以降 loop pick 対象に戻ります。`,
        );
      } catch (e) {
        process.stderr.write(
          `WARN: comment #${t.issue} failed: ${(e as Error).message}\n`,
        );
      }
    }

    results.push({
      issue: t.issue,
      category: t.category,
      had_label: had,
      removed,
      streak_reset: streakReset,
    });
  }

  console.log(
    JSON.stringify(
      { ok: true, dry_run: dryRun, targets: targets.length, results },
      null,
      2,
    ),
  );
}

if (import.meta.main) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
