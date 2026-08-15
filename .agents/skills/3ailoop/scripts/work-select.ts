#!/usr/bin/env bun
// work-select.ts — 次の actionable Issue 選定 CLI
//
// 使い方:
//   bun work-select.ts --candidates <file.json> [--state-path <path>] [--exclude-labels a,b]
//
// --candidates: CandidateIssue[] の JSON ファイル (通常は gh issue list から生成)。
//   各要素: { number, tier, title, labels[] }
//
// CAS state を読み、closed / assigned の Issue を除外した上で tier 順に選定する。
// (#333: stale plan が closed Issue を再割当するのを防ぐ)
//
// 出力 (stdout JSON): WorkSelectResult { ordered, next, dropped }
// exit: 0=next あり / 1=actionable なし / 2=usage

import { readFileSync } from "fs";
import { readState, STATE_PATHS } from "./lib/cas-state.ts";
import { selectWork, DEFAULT_EXCLUDE_LABELS, type CandidateIssue } from "./lib/work-select.ts";
import { arg, argLoose, emitJson, emitStatus } from "./cli/helpers.ts";

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const candidatesFile = arg(args, "--candidates");
  const statePath = arg(args, "--state-path") ?? STATE_PATHS.DEFAULT_STATE_PATH;
  const excludeRaw = argLoose(args, "--exclude-labels");

  if (!candidatesFile) {
    emitStatus("Usage: work-select.ts --candidates <file.json> [--state-path <path>] [--exclude-labels a,b]");
    return 2;
  }

  let candidates: CandidateIssue[] = [];
  try {
    candidates = JSON.parse(readFileSync(candidatesFile, "utf-8")) as CandidateIssue[];
  } catch (e) {
    emitJson({ error: "bad candidates file", detail: (e as Error).message });
    return 2;
  }

  const state = readState(statePath);
  const closedIssues = new Set<number>();
  const assignedIssues = new Set<number>();
  for (const [k, s] of Object.entries(state.issues)) {
    const n = Number(k);
    if (s.closed) closedIssues.add(n);
    if (s.status === "assigned" || s.status === "verifying" || s.status === "landing") assignedIssues.add(n);
  }

  const excludeLabels = excludeRaw ? excludeRaw.split(",").map(s => s.trim()).filter(Boolean) : DEFAULT_EXCLUDE_LABELS;

  const res = selectWork({ candidates, closedIssues, assignedIssues, excludeLabels });
  emitStatus(`work-select: next=${res.next ? "#" + res.next.number : "none"} (kept ${res.ordered.length}, dropped ${res.dropped.length})`);
  emitJson(res);
  return res.next ? 0 : 1;
}

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}
