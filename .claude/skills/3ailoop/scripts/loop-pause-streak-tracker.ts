#!/usr/bin/env bun
// loop-pause-streak-tracker.ts — Issue × Category 2 軸の連続 pause カウンタ (#284 欠陥 B)
//
// 既存 loop-failure-tracker (Issue 単軸) と loop-intent-guard (intent-aligned-no 専用)
// では「pause_reason の最初の #N」しか拾えず、ADR 番号と実 Issue を取り違えるケース
// (例: "#279 滞留により #274 が pick できない" → 279 をカウント) を捕捉できなかった。
//
// このトラッカーは pause した呼び元が明示的に "skip された Issue 一覧" と
// "カテゴリ" を渡す前提で、Issue × Category の独立カウンタを永続化する。
// 閾値 (3) で needs-human を自動付与し、人間判断に escalate する。
//
// 状態ファイル: features/.loop/pause-streak/<issue>-<category>.json
//   { issue: N, category: K, count: N, last_at: ISO, last_reason: text }
//
// 使い方:
//   bun loop-pause-streak-tracker.ts inc       --issue N --category K [--reason TEXT]
//   bun loop-pause-streak-tracker.ts check     --issue N --category K
//   bun loop-pause-streak-tracker.ts reset     --issue N --category K
//   bun loop-pause-streak-tracker.ts reset-all --issue N   # 全 category クリア (Issue close 時)

import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "fs";
import { dirname, join } from "path";
import { runChecked } from "./loop-spawn-checked.ts";

const ROOT = "features/.loop/pause-streak";
export const PAUSE_STREAK_THRESHOLD = 3;

// kebab-case 英数 + ハイフン + アンダースコアのみ許容。path traversal や空白を全部弾く。
const CATEGORY_PATTERN = /^[a-z0-9][a-z0-9_-]*$/;

export interface StreakState {
  issue: number;
  category: string;
  count: number;
  last_at: string;
  last_reason?: string;
}

export interface IncResult {
  state: StreakState;
  count: number;
  capped: boolean;
}

function nowIso(): string {
  return new Date().toISOString();
}

function validateCategory(category: string): void {
  if (!CATEGORY_PATTERN.test(category)) {
    throw new Error(`invalid category "${category}" — must match ${CATEGORY_PATTERN}`);
  }
}

function stateFile(issue: number, category: string): string {
  validateCategory(category);
  return join(ROOT, `${issue}-${category}.json`);
}

function readState(issue: number, category: string): StreakState | null {
  const path = stateFile(issue, category);
  if (!existsSync(path)) return null;
  try {
    return JSON.parse(readFileSync(path, "utf-8")) as StreakState;
  } catch {
    return null;
  }
}

function writeState(state: StreakState): void {
  const path = stateFile(state.issue, state.category);
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp.${process.pid}.${Math.random().toString(36).slice(2, 8)}`;
  writeFileSync(tmp, JSON.stringify(state, null, 2), "utf-8");
  renameSync(tmp, path);
}

export function getStreak(issue: number, category: string): StreakState | null {
  return readState(issue, category);
}

export function incStreak(issue: number, category: string, reason?: string): IncResult {
  const cur = readState(issue, category);
  const state: StreakState = cur ?? { issue, category, count: 0, last_at: nowIso() };
  state.count += 1;
  state.last_at = nowIso();
  if (reason !== undefined) state.last_reason = reason;
  writeState(state);
  return { state, count: state.count, capped: state.count >= PAUSE_STREAK_THRESHOLD };
}

export function resetStreak(issue: number, category: string): boolean {
  const path = stateFile(issue, category);
  if (!existsSync(path)) return false;
  rmSync(path);
  return true;
}

/** Issue 単位で全 category の streak を削除し、消した category 一覧を返す。
 *  Issue が close した瞬間や、loop が実際に前進した瞬間に呼ぶ想定。 */
export function resetAllForIssue(issue: number): string[] {
  if (!existsSync(ROOT)) return [];
  const removed: string[] = [];
  const prefix = `${issue}-`;
  for (const fname of readdirSync(ROOT)) {
    if (!fname.startsWith(prefix) || !fname.endsWith(".json")) continue;
    const category = fname.slice(prefix.length, -".json".length);
    rmSync(join(ROOT, fname));
    removed.push(category);
  }
  return removed;
}

async function addNeedsHumanLabel(issue: number): Promise<void> {
  await runChecked(["gh", "issue", "edit", String(issue), "--add-label", "needs-human"]);
}

async function postComment(issue: number, body: string): Promise<void> {
  const r = await runChecked(
    ["gh", "issue", "comment", String(issue), "--body", body],
    { allowFailure: true },
  );
  if (r.exitCode !== 0 && r.stderr) {
    process.stderr.write(`WARN: comment #${issue} exit=${r.exitCode}: ${r.stderr.trim()}\n`);
  }
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;
  function arg(name: string): string | undefined {
    const i = rest.indexOf(name);
    return i >= 0 ? rest[i + 1] : undefined;
  }
  const issueStr = arg("--issue");
  const issue = parseInt(issueStr ?? "");
  if (!issue || isNaN(issue)) {
    console.error("Usage: loop-pause-streak-tracker.ts <inc|check|reset|reset-all> --issue N [--category K] [--reason TEXT]");
    process.exit(2);
  }

  try {
    if (cmd === "reset-all") {
      const removed = resetAllForIssue(issue);
      console.log(JSON.stringify({ issue, removed }));
      process.exit(0);
    }

    const category = arg("--category");
    if (!category) {
      console.error(`${cmd}: --category required`);
      process.exit(2);
    }

    if (cmd === "inc") {
      const reason = arg("--reason");
      const r = incStreak(issue, category, reason);
      console.log(JSON.stringify(r));
      if (r.capped) {
        await addNeedsHumanLabel(issue);
        const lastLine = reason ? `\n\n直近 pause 理由: ${reason}` : "";
        await postComment(
          issue,
          `[auto] 同一 Issue × 同一カテゴリ \`${category}\` の連続 pause が ${r.count} 回に達したため \`needs-human\` を付与しました。loop は次サイクル以降この Issue を pick しません。${lastLine}`,
        );
        process.stderr.write(`WARN: #${issue} category=${category} reached ${r.count}, needs-human added\n`);
      }
      process.exit(0);
    } else if (cmd === "check") {
      const s = readState(issue, category);
      const count = s?.count ?? 0;
      console.log(count);
      process.exit(count >= PAUSE_STREAK_THRESHOLD ? 1 : 0);
    } else if (cmd === "reset") {
      const ok = resetStreak(issue, category);
      console.log(JSON.stringify({ issue, category, removed: ok }));
      process.exit(0);
    } else {
      console.error(`unknown command: ${cmd}`);
      process.exit(2);
    }
  } catch (e) {
    console.error(`ERROR: ${(e as Error).message}`);
    process.exit(1);
  }
}
