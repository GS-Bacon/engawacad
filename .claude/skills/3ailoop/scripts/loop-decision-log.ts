#!/usr/bin/env bun
// loop-decision-log.ts — 重要判断を時系列追記 (md + JSONL 併記、#232)
//
// 重要判断 (ADR draft / Issue 分割 / 失敗退避 / Phase 切替 / その他) を
// features/.loop/decisions.log.md と features/.loop/decisions.log.jsonl の
// 両方に時系列で追記する。dashboard が最新 N 件を md から抜粋表示する。
//
// 使い方:
//   bun loop-decision-log.ts append --kind <k> --message <text> \
//       [--cycle N] [--root-issue N] [--time-min N] \
//       [--token-claude N] [--token-glm N] [--token-codex N] \
//       [--blocked-on "207,gate:adr-review"]
//   bun loop-decision-log.ts show [--limit N]
//   bun loop-decision-log.ts stats [--last-cycles N] [--json]
//
// kind: adr-draft | issue-split | failure-escape | phase-close | policy-update | other
//       (互換) needs-human | phase-transition
//
// 環境変数:
//   DECISION_LOG_DIR=<path>  decision log の root を override (テスト用、default = features/.loop)

import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname } from "path";

export const DEFAULT_LOG_DIR = "features/.loop";

function logDir(): string {
  return process.env.DECISION_LOG_DIR ?? DEFAULT_LOG_DIR;
}

function mdPath(): string {
  return `${logDir()}/decisions.log.md`;
}

function jsonlPath(): string {
  return `${logDir()}/decisions.log.jsonl`;
}

export const VALID_KINDS = new Set([
  // Issue #232 spec
  "adr-draft",
  "issue-split",
  "failure-escape",
  "phase-close",
  "policy-update",
  "other",
  // 既存互換 (#232 以前から使われている)
  "needs-human",
  "phase-transition",
]);

export type TokenDelta = { claude?: number; glm?: number; codex?: number };

export type AppendOpts = {
  kind: string;
  message: string;
  cycle?: number | null;
  rootIssue?: number | null;
  timeMin?: number | null;
  tokenDelta?: TokenDelta | null;
  blockedOn?: string[];
};

export type DecisionRow = {
  at: string;
  cycle: number | null;
  kind: string;
  root_issue: number | null;
  time_min: number | null;
  token_delta: TokenDelta | null;
  blocked_on: string[];
  message: string;
};

export function appendEntry(opts: AppendOpts): void {
  if (!VALID_KINDS.has(opts.kind)) {
    throw new Error(`invalid kind '${opts.kind}' (valid: ${[...VALID_KINDS].join("|")})`);
  }
  mkdirSync(dirname(mdPath()), { recursive: true });
  if (!existsSync(mdPath())) {
    writeFileSync(mdPath(), `# /3ailoop Decision Log\n\n`, "utf-8");
  }
  const ts = new Date().toISOString();
  const mdLine = `- [${ts}] [${opts.kind}] ${opts.message}\n`;
  appendFileSync(mdPath(), mdLine, "utf-8");

  const row: DecisionRow = {
    at: ts,
    cycle: opts.cycle ?? null,
    kind: opts.kind,
    root_issue: opts.rootIssue ?? null,
    time_min: opts.timeMin ?? null,
    token_delta: opts.tokenDelta ?? null,
    blocked_on: opts.blockedOn ?? [],
    message: opts.message,
  };
  try {
    appendFileSync(jsonlPath(), JSON.stringify(row) + "\n", "utf-8");
  } catch (e) {
    process.stderr.write(`[warn] failed to write jsonl: ${(e as Error).message}\n`);
  }
}

function showEntries(limit: number): void {
  if (!existsSync(mdPath())) {
    console.log("(no decisions yet)");
    return;
  }
  const raw = readFileSync(mdPath(), "utf-8");
  const entries = raw.split("\n").filter(l => l.startsWith("- "));
  const recent = entries.slice(-limit).reverse();
  console.log(recent.join("\n"));
}

export function readJsonlRows(): DecisionRow[] {
  if (!existsSync(jsonlPath())) return [];
  const raw = readFileSync(jsonlPath(), "utf-8");
  const rows: DecisionRow[] = [];
  for (const line of raw.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      rows.push(JSON.parse(trimmed) as DecisionRow);
    } catch {
      // skip malformed
    }
  }
  return rows;
}

export type StatsResult = {
  total_rows: number;
  kind_distribution: { kind: string; count: number; pct: number }[];
  time_min: { avg: number | null; sum: number | null; reported_count: number };
  pause_root_cause_top: { root_issue: number; count: number }[];
  token_delta_sum: { claude: number; glm: number; codex: number };
  filter_last_cycles: number | null;
};

export function computeStats(rows: DecisionRow[], lastCycles: number | null = null): StatsResult {
  let filtered = rows;
  if (lastCycles !== null && rows.length > 0) {
    const cycles = rows.map(r => r.cycle).filter((c): c is number => typeof c === "number");
    if (cycles.length > 0) {
      const maxCycle = Math.max(...cycles);
      const minCycle = maxCycle - lastCycles + 1;
      filtered = rows.filter(r => typeof r.cycle === "number" && r.cycle >= minCycle);
    }
  }

  const total = filtered.length;
  const kindCount = new Map<string, number>();
  for (const r of filtered) {
    kindCount.set(r.kind, (kindCount.get(r.kind) ?? 0) + 1);
  }
  const kindDist = [...kindCount.entries()]
    .map(([kind, count]) => ({ kind, count, pct: total > 0 ? (count / total) * 100 : 0 }))
    .sort((a, b) => b.count - a.count);

  const timesReported = filtered
    .map(r => r.time_min)
    .filter((t): t is number => typeof t === "number");
  const timeAvg = timesReported.length > 0
    ? timesReported.reduce((a, b) => a + b, 0) / timesReported.length
    : null;
  const timeSum = timesReported.length > 0
    ? timesReported.reduce((a, b) => a + b, 0)
    : null;

  const rootCount = new Map<number, number>();
  for (const r of filtered) {
    if (typeof r.root_issue === "number") {
      rootCount.set(r.root_issue, (rootCount.get(r.root_issue) ?? 0) + 1);
    }
  }
  const rootTop = [...rootCount.entries()]
    .map(([root_issue, count]) => ({ root_issue, count }))
    .sort((a, b) => b.count - a.count || a.root_issue - b.root_issue);

  const tokenSum = { claude: 0, glm: 0, codex: 0 };
  for (const r of filtered) {
    if (r.token_delta) {
      tokenSum.claude += r.token_delta.claude ?? 0;
      tokenSum.glm += r.token_delta.glm ?? 0;
      tokenSum.codex += r.token_delta.codex ?? 0;
    }
  }

  return {
    total_rows: total,
    kind_distribution: kindDist,
    time_min: { avg: timeAvg, sum: timeSum, reported_count: timesReported.length },
    pause_root_cause_top: rootTop,
    token_delta_sum: tokenSum,
    filter_last_cycles: lastCycles,
  };
}

function formatStatsMarkdown(s: StatsResult): string {
  const lines: string[] = [];
  const filterLabel = s.filter_last_cycles !== null ? ` (last ${s.filter_last_cycles} cycles)` : "";
  lines.push(`# Decision Log Stats${filterLabel}`);
  lines.push(``);
  lines.push(`Total rows: ${s.total_rows}`);
  lines.push(``);
  if (s.total_rows === 0) {
    lines.push("(no data)");
    return lines.join("\n") + "\n";
  }
  lines.push(`## Kind distribution`);
  lines.push(`| kind | count | % |`);
  lines.push(`|------|-------|---|`);
  for (const k of s.kind_distribution) {
    lines.push(`| ${k.kind} | ${k.count} | ${k.pct.toFixed(1)}% |`);
  }
  lines.push(``);
  lines.push(`## Time`);
  if (s.time_min.reported_count === 0) {
    lines.push(`- (time_min not reported in any row)`);
  } else {
    lines.push(`- Average time_min (${s.time_min.reported_count} reported): ${s.time_min.avg?.toFixed(2)}`);
    lines.push(`- Total time_min sum: ${s.time_min.sum?.toFixed(2)}`);
  }
  lines.push(``);
  lines.push(`## Pause root cause top (root_issue 別 count)`);
  if (s.pause_root_cause_top.length === 0) {
    lines.push(`- (no root_issue reported)`);
  } else {
    lines.push(`| root_issue | count |`);
    lines.push(`|-----------|-------|`);
    for (const r of s.pause_root_cause_top) {
      lines.push(`| #${r.root_issue} | ${r.count} |`);
    }
  }
  lines.push(``);
  lines.push(`## Token delta sum`);
  lines.push(`- claude: ${s.token_delta_sum.claude}`);
  lines.push(`- glm: ${s.token_delta_sum.glm}`);
  lines.push(`- codex: ${s.token_delta_sum.codex}`);
  return lines.join("\n") + "\n";
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;
  function arg(name: string): string | undefined {
    const i = rest.indexOf(name);
    return i >= 0 ? rest[i + 1] : undefined;
  }
  function flag(name: string): boolean {
    return rest.includes(name);
  }
  function intArg(name: string): number | null {
    const v = arg(name);
    if (v === undefined) return null;
    const n = parseInt(v, 10);
    return Number.isFinite(n) ? n : null;
  }

  if (cmd === "append") {
    const kind = arg("--kind") ?? "";
    const message = arg("--message") ?? "";
    if (!kind || !message) {
      console.error("Usage: loop-decision-log.ts append --kind <kind> --message <text> [--cycle N] [--root-issue N] [--time-min N] [--token-claude N] [--token-glm N] [--token-codex N] [--blocked-on csv]");
      process.exit(2);
    }
    const tokenClaude = intArg("--token-claude");
    const tokenGlm = intArg("--token-glm");
    const tokenCodex = intArg("--token-codex");
    const tokenDelta = (tokenClaude !== null || tokenGlm !== null || tokenCodex !== null)
      ? { claude: tokenClaude ?? 0, glm: tokenGlm ?? 0, codex: tokenCodex ?? 0 }
      : null;
    const blockedRaw = arg("--blocked-on");
    const blockedOn = blockedRaw ? blockedRaw.split(",").map(s => s.trim()).filter(Boolean) : undefined;
    try {
      appendEntry({
        kind,
        message,
        cycle: intArg("--cycle"),
        rootIssue: intArg("--root-issue"),
        timeMin: intArg("--time-min"),
        tokenDelta,
        blockedOn,
      });
      console.log(`OK: appended [${kind}] ${message}`);
    } catch (e) {
      console.error(`ERROR: ${(e as Error).message}`);
      process.exit(1);
    }
  } else if (cmd === "show") {
    const limit = parseInt(arg("--limit") ?? "20");
    showEntries(limit);
  } else if (cmd === "stats") {
    const lastCycles = intArg("--last-cycles");
    const rows = readJsonlRows();
    const stats = computeStats(rows, lastCycles);
    if (flag("--json")) {
      console.log(JSON.stringify(stats, null, 2));
    } else {
      console.log(formatStatsMarkdown(stats));
    }
  } else {
    console.error("Usage: loop-decision-log.ts (append --kind <k> --message <text> [...] | show [--limit N] | stats [--last-cycles N] [--json])");
    process.exit(2);
  }
}
