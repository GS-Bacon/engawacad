#!/usr/bin/env bun
// dashboard-cruft-trend.ts — テスト債務 (cruft) の傾向を記録・表示 (Issue #313)
//
// L-6 の loop-dashboard.ts と同タイミングで snapshot を取り、
// features/.loop/cruft-trend.jsonl に 1 行 append する。
// render は最新 vs 3 snapshot 前 (無ければ最古) を比較した Markdown を stdout に出す。
//
// 使い方:
//   bun dashboard-cruft-trend.ts snapshot   # 現在値を計測して JSONL に append
//   bun dashboard-cruft-trend.ts render     # dashboard.md に埋め込む Markdown を stdout へ

import { appendFileSync, existsSync, mkdirSync, readFileSync } from "fs";
import { dirname } from "path";

const JSONL_PATH = "features/.loop/cruft-trend.jsonl";

// Rust 属性の cruft パターン。git grep の basic regex に合わせて escape する。
const IGNORE_PATTERN = "#\\[ignore";
const ALLOW_CLIPPY_PATTERN = "#\\[allow(clippy::";
const RS_PATHSPEC = "crates/**/*.rs";
const REGRESSION_PATHSPEC = "crates/*/tests/regression_*.rs";

export interface CruftSnapshot {
  sha: string;
  timestamp: string;
  ignore: number;
  allow_clippy: number;
  regression_files: number;
}

/** git grep --count の stdout ("<path>:<n>\n" が並ぶ) を集計。
 *  空 (git grep が exit 1 = no match) なら 0。壊れた行は 0 として扱う。 */
export function parseGitGrepCount(stdout: string): number {
  if (!stdout) return 0;
  let sum = 0;
  for (const line of stdout.split("\n")) {
    if (!line) continue;
    const idx = line.lastIndexOf(":");
    if (idx < 0) continue;
    const n = parseInt(line.slice(idx + 1), 10);
    if (Number.isFinite(n)) sum += n;
  }
  return sum;
}

/** git grep --count でパターン一致行の総数 (matched files ではなく) を返す。
 *  exit 1 (no match) は 0 に degrade、それ以外の失敗は throw。 */
function countPattern(pattern: string, pathspec: string): number {
  const r = Bun.spawnSync(["git", "grep", "--count", pattern, "--", pathspec]);
  if (r.exitCode === 1) return 0;
  if (r.exitCode !== 0) {
    const stderr = new TextDecoder().decode(r.stderr);
    throw new Error(`git grep failed (exit=${r.exitCode}): ${stderr.trim()}`);
  }
  return parseGitGrepCount(new TextDecoder().decode(r.stdout));
}

/** git ls-files でパススペック該当ファイル数を返す。 */
function countFiles(pathspec: string): number {
  const r = Bun.spawnSync(["git", "ls-files", "--", pathspec]);
  if (r.exitCode !== 0) {
    const stderr = new TextDecoder().decode(r.stderr);
    throw new Error(`git ls-files failed (exit=${r.exitCode}): ${stderr.trim()}`);
  }
  const out = new TextDecoder().decode(r.stdout);
  return out.split("\n").filter(l => l.length > 0).length;
}

function currentSha(): string {
  const r = Bun.spawnSync(["git", "rev-parse", "HEAD"]);
  if (r.exitCode !== 0) return "unknown";
  return new TextDecoder().decode(r.stdout).trim();
}

export function collectSnapshot(): CruftSnapshot {
  return {
    sha: currentSha(),
    timestamp: new Date().toISOString(),
    ignore: countPattern(IGNORE_PATTERN, RS_PATHSPEC),
    allow_clippy: countPattern(ALLOW_CLIPPY_PATTERN, RS_PATHSPEC),
    regression_files: countFiles(REGRESSION_PATHSPEC),
  };
}

export function appendSnapshot(snap: CruftSnapshot, path: string = JSONL_PATH): void {
  mkdirSync(dirname(path), { recursive: true });
  appendFileSync(path, JSON.stringify(snap) + "\n", "utf-8");
}

export function readSnapshots(path: string = JSONL_PATH): CruftSnapshot[] {
  if (!existsSync(path)) return [];
  const raw = readFileSync(path, "utf-8");
  const out: CruftSnapshot[] = [];
  for (const line of raw.split("\n")) {
    if (!line.trim()) continue;
    try {
      out.push(JSON.parse(line) as CruftSnapshot);
    } catch {
      // 壊れた行はスキップ (append-only journal の耐性)
    }
  }
  return out;
}

export type CruftMetric = "ignore" | "allow_clippy" | "regression_files";

/** delta を算出し、metric の意味論に沿って symbol (⚠️ 悪化 / ✓ 改善) を付ける。
 *  ignore / allow_clippy: 増加が悪、減少が善
 *  regression_files:     増加が善、減少が悪 */
export function computeDelta(
  latest: number,
  past: number,
  metric: CruftMetric,
): { value: number; symbol: string } {
  const value = latest - past;
  if (value === 0) return { value: 0, symbol: "0" };
  const goodIncrease = metric === "regression_files";
  const isImprovement = value > 0 ? goodIncrease : !goodIncrease;
  const sign = value > 0 ? "+" : "";
  const marker = isImprovement ? "✓" : "⚠️";
  return { value, symbol: `${sign}${value} ${marker}` };
}

export function renderMarkdown(entries: CruftSnapshot[]): string {
  if (entries.length === 0) {
    return "## Cruft Trend\n\n_No data yet — run `dashboard-cruft-trend snapshot` first._\n";
  }
  const latest = entries[entries.length - 1];
  if (entries.length === 1) {
    const lines = [
      "## Cruft Trend",
      "",
      "| Metric | Now | 3 snapshots ago | Δ |",
      "|---|---|---|---|",
      `| \`#[ignore]\` | ${latest.ignore} | - | - (baseline) |`,
      `| \`#[allow(clippy::)]\` | ${latest.allow_clippy} | - | - (baseline) |`,
      `| \`regression_*.rs\` files | ${latest.regression_files} | - | - (baseline) |`,
      "",
      `_Latest snapshot: ${latest.sha} at ${latest.timestamp}_`,
      "",
    ];
    return lines.join("\n");
  }
  // 4 以上なら 3 前、それ未満なら最古 (index 0) と比較
  const pastIdx = entries.length >= 4 ? entries.length - 4 : 0;
  const past = entries[pastIdx];
  const dIgnore = computeDelta(latest.ignore, past.ignore, "ignore");
  const dAllow = computeDelta(latest.allow_clippy, past.allow_clippy, "allow_clippy");
  const dRegr = computeDelta(latest.regression_files, past.regression_files, "regression_files");
  const lines = [
    "## Cruft Trend",
    "",
    "| Metric | Now | 3 snapshots ago | Δ |",
    "|---|---|---|---|",
    `| \`#[ignore]\` | ${latest.ignore} | ${past.ignore} | ${dIgnore.symbol} |`,
    `| \`#[allow(clippy::)]\` | ${latest.allow_clippy} | ${past.allow_clippy} | ${dAllow.symbol} |`,
    `| \`regression_*.rs\` files | ${latest.regression_files} | ${past.regression_files} | ${dRegr.symbol} |`,
    "",
    `_Latest snapshot: ${latest.sha} at ${latest.timestamp}_`,
    "",
  ];
  return lines.join("\n");
}

if (import.meta.main) {
  const cmd = process.argv[2];
  if (cmd === "snapshot") {
    const snap = collectSnapshot();
    appendSnapshot(snap);
    console.log(JSON.stringify(snap));
  } else if (cmd === "render") {
    process.stdout.write(renderMarkdown(readSnapshots()));
  } else {
    console.error("Usage: dashboard-cruft-trend.ts (snapshot | render)");
    process.exit(2);
  }
}
