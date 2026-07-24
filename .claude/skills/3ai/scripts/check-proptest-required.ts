#!/usr/bin/env bun
// check-proptest-required.ts — property test (T_PROP_*) の必須性チェック (#318 Phase C)
//
// 使い方:
//   bun check-proptest-required.ts --feature-dir <dir> [--phase <N>] [--issue-title <title>]
//
// ルール:
//   - Phase 11 以降の Issue は T_PROP_* ID が最低 1 件必須 (blocking)
//   - Phase ≤ 10 でも Boolean / Tessellation / 曲面 系 Issue は必須 (blocking)
//   - それ以外は非必須 (exit 0)
//
// exit code:
//   0 = OK (必須 & 存在 / 非必須)
//   1 = 必須だが test-spec.md に T_PROP_* が 0 件
//   2 = test-spec.md が存在しない (STEP 6.5 未実施)
//
// workspace Cargo.toml に proptest 依存が無い場合、stderr へ warn を出す (exit code は変えない)。

import { existsSync, readFileSync } from "fs";
import { basename, dirname, resolve } from "path";

export type CheckOptions = {
  featureDir: string;
  phase?: number;
  issueTitle?: string;
};

export type CheckResult = {
  exitCode: 0 | 1 | 2;
  stdout?: string;
  stderr?: string;
  warnings: string[];
};

const PROPTEST_ID_RE = /T_PROP_[A-Za-z0-9_]+/g;
// 大文字小文字非依存で英語キーワード、日本語は原文でヒットさせる。
const REQUIRED_TITLE_RE = /boolean|tessellation|tessellate|曲面|自由曲面/i;

export function parseProptestIds(content: string): string[] {
  return [...content.matchAll(PROPTEST_ID_RE)].map((m) => m[0]);
}

export function isProptestRequired(phase: number, issueTitle: string): boolean {
  if (Number.isFinite(phase) && phase >= 11) return true;
  return REQUIRED_TITLE_RE.test(issueTitle ?? "");
}

export function inferPhaseFromDir(featureDir: string): number | undefined {
  const base = basename(resolve(featureDir));
  const m = base.match(/^\d+-phase(\d+)[-_]/i);
  return m ? Number(m[1]) : undefined;
}

export function readIssueTitleFromPlan(featureDir: string): string {
  const planPath = resolve(featureDir, "plan.md");
  if (!existsSync(planPath)) return "";
  const content = readFileSync(planPath, "utf-8");
  // 最初の h1 (`# タイトル`) を拾う。`##` は除外。
  const m = content.match(/^#(?!#)[ \t]+([^\n]+)$/m);
  return m ? m[1].trim() : "";
}

export function detectProptestInWorkspace(startDir: string): boolean {
  let dir = resolve(startDir);
  for (let i = 0; i < 12; i++) {
    const cargoPath = resolve(dir, "Cargo.toml");
    if (existsSync(cargoPath)) {
      try {
        const src = readFileSync(cargoPath, "utf-8");
        // `proptest = "..."` あるいは `proptest.workspace = true` 等を許容
        if (/(^|\n)\s*proptest[\s=.]/.test(src)) return true;
      } catch {
        // fall through
      }
    }
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return false;
}

export function runCheck(opts: CheckOptions): CheckResult {
  const warnings: string[] = [];
  const specPath = resolve(opts.featureDir, "test-spec.md");

  if (!existsSync(specPath)) {
    return {
      exitCode: 2,
      stderr: `ERROR: ${specPath} が存在しません (STEP 6.5 未実施の可能性)\n`,
      warnings,
    };
  }

  const specContent = readFileSync(specPath, "utf-8");
  const propIds = parseProptestIds(specContent);

  const phase =
    opts.phase ?? inferPhaseFromDir(opts.featureDir) ?? Number.NaN;
  const title = opts.issueTitle ?? readIssueTitleFromPlan(opts.featureDir);

  if (!detectProptestInWorkspace(opts.featureDir)) {
    warnings.push(
      "WARN: workspace Cargo.toml に proptest 依存が見つかりません (informational, exit code には影響しません)",
    );
  }

  const required = isProptestRequired(phase, title);
  const phaseLabel = Number.isFinite(phase) ? String(phase) : "不明";

  if (required) {
    if (propIds.length >= 1) {
      return {
        exitCode: 0,
        stdout: `OK: proptest ID ${propIds.length} 件確認 (Phase ${phaseLabel})\n`,
        warnings,
      };
    }
    return {
      exitCode: 1,
      stderr: `ERROR: Phase ${phaseLabel} / Boolean-系 Issue には T_PROP_* ID が最低 1 件必要です。test-spec.md に追記してください。例: T_PROP_manifold_after_fuse\n`,
      warnings,
    };
  }

  return {
    exitCode: 0,
    stdout: `OK: 本 Issue で proptest は非必須 (Phase ${phaseLabel}, Boolean/Tessellation 非該当)\n`,
    warnings,
  };
}

function parseArgs(argv: string[]): CheckOptions {
  let featureDir = "";
  let phase: number | undefined;
  let issueTitle: string | undefined;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--feature-dir" && argv[i + 1]) featureDir = argv[++i];
    else if (a === "--phase" && argv[i + 1]) {
      const n = Number(argv[++i]);
      if (Number.isFinite(n)) phase = n;
    } else if (a === "--issue-title" && argv[i + 1]) issueTitle = argv[++i];
  }
  if (!featureDir) {
    process.stderr.write(
      "Usage: check-proptest-required.ts --feature-dir <dir> [--phase <N>] [--issue-title <title>]\n",
    );
    process.exit(2);
  }
  return { featureDir, phase, issueTitle };
}

if (import.meta.main) {
  const opts = parseArgs(process.argv.slice(2));
  const res = runCheck(opts);
  for (const w of res.warnings) process.stderr.write(w + "\n");
  if (res.stdout) process.stdout.write(res.stdout);
  if (res.stderr) process.stderr.write(res.stderr);
  process.exit(res.exitCode);
}
