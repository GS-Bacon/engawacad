#!/usr/bin/env bun
// lint-regression-preserved.ts — regression test の暗黙削除/改名-away を検出
//
// 過去バグの再発防止用テスト (`crates/**/tests/regression_*.rs`) が
// 差分の中で削除・非 regression 名への改名で消えていないか確認する。
// STEP 5.5 (Acceptance Test Skeleton) で走らせて、意図しない記憶喪失を止める。
//
// 使い方:
//   bun lint-regression-preserved.ts                    # <default>...HEAD を比較
//   bun lint-regression-preserved.ts --base HEAD~1      # HEAD~1...HEAD
//   bun lint-regression-preserved.ts --base main --head origin/feature
//
// --base 未指定時の default:
//   1. origin/HEAD を解決 (git symbolic-ref refs/remotes/origin/HEAD) → origin/ 除去
//   2. 解決失敗なら "main"
// この repo のデフォルト branch は "main" とは限らない (実際は claude/add-claude-guidelines-BKKtD)。
//
// exit:
//   0 → 削除/改名-away なし
//   1 → 検出あり (stderr に一覧)
//   2 → 環境エラー (git repo でない / base ref 不明 / 使い方誤り)

import { spawnSync } from "child_process";

// crates/<name>/[.../]tests/regression_<something>.rs にマッチ
const REGRESSION_PATH = /^crates\/.+\/tests\/(?:.+\/)?regression_[^/]*\.rs$/;

export type DiffEntry =
  | { kind: "delete"; path: string }
  | { kind: "rename"; score: number; oldPath: string; newPath: string }
  | { kind: "other"; status: string; paths: string[] };

export interface Violation {
  kind: "delete" | "rename";
  oldPath: string;
  newPath?: string;
}

export function parseGitDiffOutput(raw: string): DiffEntry[] {
  const entries: DiffEntry[] = [];
  for (const line of raw.split("\n")) {
    if (!line.trim()) continue;
    const cols = line.split("\t");
    const status = cols[0] ?? "";
    if (status === "D") {
      const path = cols[1] ?? "";
      if (path) entries.push({ kind: "delete", path });
    } else if (status.startsWith("R")) {
      // R100 <tab> oldpath <tab> newpath
      const scoreStr = status.slice(1);
      const score = scoreStr ? Number(scoreStr) : NaN;
      const oldPath = cols[1] ?? "";
      const newPath = cols[2] ?? "";
      if (oldPath && newPath) {
        entries.push({
          kind: "rename",
          score: Number.isFinite(score) ? score : 0,
          oldPath,
          newPath,
        });
      }
    } else {
      entries.push({ kind: "other", status, paths: cols.slice(1) });
    }
  }
  return entries;
}

export function isRegressionPath(path: string): boolean {
  return REGRESSION_PATH.test(path);
}

export function filterRegressionDeletions(entries: DiffEntry[]): Violation[] {
  const violations: Violation[] = [];
  for (const e of entries) {
    if (e.kind === "delete" && isRegressionPath(e.path)) {
      violations.push({ kind: "delete", oldPath: e.path });
    } else if (e.kind === "rename" && isRegressionPath(e.oldPath) && !isRegressionPath(e.newPath)) {
      violations.push({ kind: "rename", oldPath: e.oldPath, newPath: e.newPath });
    }
  }
  return violations;
}

export function formatViolations(violations: Violation[]): string {
  const lines: string[] = [];
  lines.push(`ERROR: ${violations.length} 件の regression test 削除/改名を検出:`);
  for (const v of violations) {
    if (v.kind === "delete") {
      lines.push(`  D ${v.oldPath}`);
    } else {
      lines.push(`  R ${v.oldPath} -> ${v.newPath}`);
    }
  }
  lines.push("");
  lines.push(
    "復元するか、意図的削除なら本 Issue の plan.md に「Regression test 削除: <理由>」を明記してから再実行してください",
  );
  return lines.join("\n") + "\n";
}

// --- git 実行系 (テスト対象外; 本体でのみ使う) ---

function isInsideGitRepo(cwd?: string): boolean {
  const r = spawnSync("git", ["rev-parse", "--is-inside-work-tree"], {
    encoding: "utf-8",
    cwd,
  });
  return r.status === 0 && (r.stdout ?? "").trim() === "true";
}

function refExists(ref: string, cwd?: string): boolean {
  const r = spawnSync("git", ["rev-parse", "--verify", "--quiet", `${ref}^{commit}`], {
    encoding: "utf-8",
    cwd,
  });
  return r.status === 0;
}

/** origin/HEAD が指す branch 名を返す (例 "main" / "claude/add-guidelines-XYZ")。
 *  解決失敗時は null。呼び元でフォールバック値を決めること。 */
export function detectDefaultBranch(cwd?: string): string | null {
  const r = spawnSync(
    "git",
    ["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
    { encoding: "utf-8", cwd },
  );
  if (r.status !== 0) return null;
  const raw = (r.stdout ?? "").trim();
  if (!raw) return null;
  return raw.startsWith("origin/") ? raw.slice("origin/".length) : raw;
}

function runDiff(base: string, head: string, cwd?: string): string {
  // pathspec を渡さず全ファイル取ってから TS 側でフィルタする。
  // (glob 展開の shell/git 齟齬を避けるため。ADR-006 §1 の shell-agnostic 方針)
  const r = spawnSync(
    "git",
    ["diff", "--name-status", "--diff-filter=DR", `${base}...${head}`],
    { encoding: "utf-8", cwd, maxBuffer: 64 * 1024 * 1024 },
  );
  if (r.status !== 0) {
    throw new Error(`git diff failed (${r.status}): ${r.stderr ?? ""}`);
  }
  return r.stdout ?? "";
}

async function main(argv: string[]): Promise<number> {
  let base: string | null = null;
  let head = "HEAD";
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--base") {
      base = argv[++i] ?? "";
    } else if (argv[i] === "--head") {
      head = argv[++i] ?? "";
    } else if (argv[i] === "--help" || argv[i] === "-h") {
      process.stdout.write(
        "Usage: lint-regression-preserved.ts [--base <ref>] [--head <ref>]\n",
      );
      return 0;
    } else {
      process.stderr.write(`Unknown arg: ${argv[i]}\n`);
      return 2;
    }
  }

  if (!isInsideGitRepo()) {
    process.stderr.write("ERROR: git repo の外で実行されました\n");
    return 2;
  }

  // --base 未指定なら origin/HEAD を解決、失敗したら "main" にフォールバック
  if (base === null) {
    base = detectDefaultBranch() ?? "main";
  }

  if (!refExists(base)) {
    process.stderr.write(`ERROR: base ref '${base}' が見つかりません\n`);
    return 2;
  }
  if (!refExists(head)) {
    process.stderr.write(`ERROR: head ref '${head}' が見つかりません\n`);
    return 2;
  }

  let raw: string;
  try {
    raw = runDiff(base, head);
  } catch (e) {
    process.stderr.write(`ERROR: ${(e as Error).message}\n`);
    return 2;
  }

  const entries = parseGitDiffOutput(raw);
  const violations = filterRegressionDeletions(entries);

  if (violations.length === 0) {
    process.stdout.write("OK: regression test 削除なし\n");
    return 0;
  }

  process.stderr.write(formatViolations(violations));
  return 1;
}

if (import.meta.main) {
  main(process.argv.slice(2))
    .then((code) => process.exit(code))
    .catch((e) => {
      console.error(e);
      process.exit(2);
    });
}
