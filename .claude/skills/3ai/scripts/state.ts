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

import { readFileSync, writeFileSync } from "fs";
import type { StateData } from "./types.ts";

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
  const data = readState(path);
  data.steps[step] = value;
  writeState(path, data);
}

export function getState(path: string, step: string): string {
  return readState(path).steps[step] ?? "none";
}

export function assertState(path: string, step: string): boolean {
  return readState(path).steps[step] === "passed";
}

export function incState(path: string, key: string): number {
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
}

export function judgeState(
  path: string,
  round: number,
  adopted: number,
  rejected: number
): void {
  const data = readState(path);
  if (!data.judgments) data.judgments = [];
  data.judgments.push({ round, adopted, rejected });
  writeState(path, data);
}

export function checkFullAdoptionWarning(path: string): boolean {
  const data = readState(path);
  const j = data.judgments ?? [];
  if (j.length < 2) return false;
  return j.slice(-2).every((x) => x.rejected === 0);
}

// 2 連続 round で全指摘を棄却 (adopted = 0) → early-stop シグナル
// exit 1 = early-stop すべき, exit 0 = 続行可
export function checkEarlyStop(path: string): boolean {
  const data = readState(path);
  const j = data.judgments ?? [];
  if (j.length < 2) return false;
  return j.slice(-2).every((x) => x.adopted === 0 && x.rejected > 0);
}

export function assertCriticalZero(statePath: string, verdictPath: string): boolean {
  try {
    const v = JSON.parse(readFileSync(verdictPath, "utf-8"));
    return (v?.severity_counts?.critical ?? 0) === 0;
  } catch {
    return false;
  }
}

if (import.meta.main) {
  const [, , cmd, file, ...rest] = process.argv;
  if (!cmd || !file) {
    console.error("Usage: state.ts <command> <state.json> [args...]");
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
      // parse optional flags: [--raise-at N] [--feature-dir dir] [--step text]
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
