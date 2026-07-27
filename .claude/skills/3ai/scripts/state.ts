#!/usr/bin/env bun
// state.ts — /3ai フロー状態の記録・検証
//   state.ts init   <state.json> <issue> <slug>
//   state.ts set    <state.json> <step> <passed|failed>
//   state.ts get    <state.json> <step>
//   state.ts assert <state.json> <step>
//   state.ts inc    <state.json> <key>       # ドット区切りネストキー対応
//   state.ts assert-critical-zero <state.json> <verdict.json>
//   state.ts judge  <state.json> <round> <adopted> <rejected>
//   state.ts check-full-adoption-warning <state.json>
//
// 3ailoop 用 failure_streak API (Issue #167-169):
//   state.ts inc-failure   --issue N         # features/.loop/failure-streak/<N>.json を +1、stdout に新値
//   state.ts reset-failure --issue N         # 該当ファイルを削除
//   state.ts get-failure   --issue N         # stdout に現在値 (未設定なら 0、破損なら exit 1)
//
// failure_streak は state.json から分離して issue ごとの個別ファイル管理。state.json への
// 並行書込競合を回避し、破損時は fail-closed (throw + exit 1) で全カウンタ消失を防ぐ。

import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "fs";
import { dirname, join } from "path";
import type { StateData } from "./types.ts";
import { withFileLockSync } from "../../3ailoop/scripts/loop-file-lock.ts";

// === failure_streak (個別ファイル管理) ===

const FAILURE_STREAK_DIR = "features/.loop/failure-streak";

interface FailureCountFile {
  count: number;
}

function failureFilePath(issue: number): string {
  return join(FAILURE_STREAK_DIR, `${issue}.json`);
}

/** issue 別ファイル読み出し。破損時は throw (fail-closed)、未存在は 0 を返す。 */
function readFailureFile(issue: number): number {
  const path = failureFilePath(issue);
  if (!existsSync(path)) return 0;
  let text: string;
  try {
    text = readFileSync(path, "utf-8");
  } catch (e) {
    throw new Error(`failed to read ${path}: ${(e as Error).message}`);
  }
  let obj: Partial<FailureCountFile>;
  try {
    obj = JSON.parse(text) as Partial<FailureCountFile>;
  } catch (e) {
    throw new Error(`corrupt failure file ${path}: parse error ${(e as Error).message}`);
  }
  if (typeof obj.count !== "number" || !Number.isFinite(obj.count) || obj.count < 0) {
    throw new Error(`corrupt failure file ${path}: invalid count field`);
  }
  return obj.count;
}

function atomicWriteFailureFile(issue: number, count: number): void {
  const path = failureFilePath(issue);
  mkdirSync(dirname(path), { recursive: true });
  const tmpPath = `${path}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmpPath, JSON.stringify({ count }, null, 2), "utf-8");
  renameSync(tmpPath, path);
}

export function incFailureStreak(issue: number): number {
  return withFileLockSync(failureFilePath(issue), () => {
    const cur = readFailureFile(issue);
    const n = cur + 1;
    atomicWriteFailureFile(issue, n);
    return n;
  });
}

export function resetFailureStreak(issue: number): void {
  const path = failureFilePath(issue);
  if (existsSync(path)) {
    rmSync(path);
  }
}

export function getFailureStreak(issue: number): number {
  return readFailureFile(issue);
}

// === state.json (既存 /3ai 用) ===

function readState(path: string): StateData {
  return JSON.parse(readFileSync(path, "utf-8")) as StateData;
}

function writeState(path: string, data: StateData): void {
  writeFileSync(path, JSON.stringify(data), "utf-8");
}

export function initState(path: string, issue: number, slug: string): void {
  const data: StateData = {
    issue,
    slug,
    steps: {},
    loops: {},
    judgments: [],
    phases: { core_impl: { glm_runs: 0 }, test_impl: { glm_runs: 0 } },
  };
  writeState(path, data);
}

export function setState(path: string, step: string, value: string): void {
  withFileLockSync(path, () => {
    const data = readState(path);
    data.steps[step] = value;
    writeState(path, data);
  });
}

export function getState(path: string, step: string): string {
  return readState(path).steps?.[step] ?? "none";
}

export function assertState(path: string, step: string): boolean {
  return readState(path).steps?.[step] === "passed";
}

export function incState(path: string, key: string): number {
  return withFileLockSync(path, () => {
    const data = readState(path);
    let n: number;
    if (key.includes(".")) {
      const parts = key.split(".");
      let node = data as unknown as Record<string, unknown>;
      for (const p of parts.slice(0, -1)) {
        if (typeof node[p] !== "object" || node[p] === null) node[p] = {};
        node = node[p] as Record<string, unknown>;
      }
      const last = parts[parts.length - 1];
      n = ((node[last] as number) ?? 0) + 1;
      node[last] = n;
    } else {
      if (!data.loops) data.loops = {};
      n = (data.loops[key] ?? 0) + 1;
      data.loops[key] = n;
    }
    writeState(path, data);
    return n;
  });
}

export function judgeState(
  path: string,
  round: number,
  adopted: number,
  rejected: number
): void {
  withFileLockSync(path, () => {
    const data = readState(path);
    if (!data.judgments) data.judgments = [];
    data.judgments.push({ round, adopted, rejected });
    writeState(path, data);
  });
}

export function checkFullAdoptionWarning(path: string): boolean {
  const data = readState(path);
  const j = data.judgments ?? [];
  if (j.length < 2) return false;
  // adopted > 0 のガード必須 (#334): adopted=0/rejected=0 (指摘ゼロ) は
  // adopted>0/rejected=0 (全採用でスコープ防衛が緩んでいる) とは別物。
  return j.slice(-2).every((x) => x.adopted > 0 && x.rejected === 0);
}

// 2 連続 round で全指摘を棄却 (adopted = 0) → early-stop シグナル
// exit 1 = early-stop すべき, exit 0 = 続行可
export function checkEarlyStop(path: string): boolean {
  const data = readState(path);
  const j = data.judgments ?? [];
  if (j.length < 2) return false;
  return j.slice(-2).every((x) => x.adopted === 0 && x.rejected > 0);
}

export function assertCriticalZero(_statePath: string, verdictPath: string): boolean {
  try {
    const v = JSON.parse(readFileSync(verdictPath, "utf-8"));
    return (v?.severity_counts?.critical ?? 0) === 0;
  } catch {
    return false;
  }
}

// === CLI ===

if (import.meta.main) {
  const [, , cmd, ...args] = process.argv;
  if (!cmd) {
    console.error("Usage: state.ts <command> [args...]");
    process.exit(1);
  }

  // failure 系: positional state.json を取らず --issue だけ受ける (Issue #169 で CLI 整理)
  if (cmd === "inc-failure" || cmd === "reset-failure" || cmd === "get-failure") {
    const issueIdx = args.indexOf("--issue");
    const issueArg = issueIdx >= 0 ? parseInt(args[issueIdx + 1] ?? "") : NaN;
    if (!issueArg || isNaN(issueArg)) {
      console.error(`Usage: state.ts ${cmd} --issue <N>`);
      process.exit(2);
    }
    // positional 余剰引数の検出 (旧形式 `state.ts inc-failure <state.json> --issue N` を拒否)
    const positional = args.filter((a, i) => {
      if (a === "--issue") return false;
      if (i > 0 && args[i - 1] === "--issue") return false;
      return !a.startsWith("--");
    });
    if (positional.length > 0) {
      console.error(
        `Usage: state.ts ${cmd} --issue <N>  (positional argument '${positional[0]}' not allowed; failure_streak uses dedicated per-issue files)`,
      );
      process.exit(2);
    }
    try {
      if (cmd === "inc-failure") {
        console.log(incFailureStreak(issueArg));
      } else if (cmd === "reset-failure") {
        resetFailureStreak(issueArg);
        console.log("0");
      } else {
        console.log(getFailureStreak(issueArg));
      }
      process.exit(0);
    } catch (e) {
      console.error(`ERROR: ${(e as Error).message}`);
      process.exit(1);
    }
  }

  // 既存系: positional <state.json> を要求
  const file = args[0];
  const rest = args.slice(1);
  if (!file) {
    console.error(`Usage: state.ts ${cmd} <state.json> [args...]`);
    process.exit(1);
  }

  switch (cmd) {
    case "init":
      initState(file, parseInt(rest[0]), rest[1]);
      break;
    case "set":
      setState(file, rest[0], rest[1]);
      break;
    case "get":
      console.log(getState(file, rest[0]));
      break;
    case "assert":
      process.exit(assertState(file, rest[0]) ? 0 : 1);
      break;
    case "inc": {
      const incKey = rest[0];
      let raiseAt: number | undefined;
      let incFeatureDir = "";
      let incStep = "";
      for (let i = 1; i < rest.length; i++) {
        if (rest[i] === "--raise-at") raiseAt = parseInt(rest[++i]);
        else if (rest[i] === "--feature-dir") incFeatureDir = rest[++i];
        else if (rest[i] === "--step") incStep = rest[++i];
      }
      const incResult = incState(file, incKey);
      console.log(incResult);
      if (raiseAt !== undefined && incResult >= raiseAt && incFeatureDir && incStep) {
        const errorSummary = `${incKey} が ${raiseAt} に達しました（ループ上限超過）`;
        Bun.spawnSync(
          [
            "bun",
            import.meta.dir + "/raise-issue-on-failure.ts",
            "--step", incStep,
            "--feature-dir", incFeatureDir,
            "--error-summary", errorSummary,
          ],
          { stdout: "inherit", stderr: "inherit" }
        );
        process.exit(2);
      }
      break;
    }
    case "assert-critical-zero":
      process.exit(assertCriticalZero(file, rest[0]) ? 0 : 1);
      break;
    case "judge":
      judgeState(file, parseInt(rest[0]), parseInt(rest[1]), parseInt(rest[2]));
      break;
    case "check-full-adoption-warning":
      process.exit(checkFullAdoptionWarning(file) ? 1 : 0);
      break;
    case "check-early-stop":
      process.exit(checkEarlyStop(file) ? 1 : 0);
      break;
    default:
      console.error(`unknown cmd: ${cmd}`);
      process.exit(1);
  }
}
