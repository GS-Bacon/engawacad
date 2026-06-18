#!/usr/bin/env bun
// loop-cycle-record.ts — /3ailoop 1 サイクル末尾の集計記録
//
// features/.loop/state.json に以下を追記:
// - cycle: 連番
// - last_cycle_at: ISO8601
// - recent_cycles[]: {cycle, started_at, closed, raised, merged_commits, adr_drafts, pause_reason?}
// - cumulative: {cycles_total, issues_closed, raised_resolved, adr_total, token_*}
//
// 使い方:
//   bun loop-cycle-record.ts record [--started-at <ISO8601>] [--pause-reason <reason>]
//   bun loop-cycle-record.ts show

import { appendFileSync, existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname } from "path";
import { withFileLock } from "./loop-file-lock.ts";

const STATE_PATH = "features/.loop/state.json";
// #177 指摘 2: append-only journal で state.json 破損耐性確保
const JOURNAL_PATH = "features/.loop/cycle-journal.log";
const RECENT_LIMIT = 20;

interface RecentCycle {
  cycle: number;
  started_at: string;
  ended_at: string;
  closed: number[];
  raised: number[];
  merged_commits: number;
  adr_drafts: string[];
  pause_reason?: string;
}

interface CumulativeStats {
  cycles_total: number;
  issues_closed: number;
  raised_resolved: number;
  adr_total: number;
  token_claude: number;
  token_glm: number;
  token_codex: number;
}

interface LoopState {
  loop_start: string;
  cycle: number;
  last_cycle_at: string;
  recent_cycles: RecentCycle[];
  cumulative: CumulativeStats;
}

function readState(): LoopState {
  if (!existsSync(STATE_PATH)) {
    return {
      loop_start: new Date().toISOString(),
      cycle: 0,
      last_cycle_at: new Date(0).toISOString(),
      recent_cycles: [],
      cumulative: {
        cycles_total: 0,
        issues_closed: 0,
        raised_resolved: 0,
        adr_total: 0,
        token_claude: 0,
        token_glm: 0,
        token_codex: 0,
      },
    };
  }
  try {
    return JSON.parse(readFileSync(STATE_PATH, "utf-8")) as LoopState;
  } catch {
    throw new Error(`corrupt ${STATE_PATH}`);
  }
}

function atomicWriteState(state: LoopState): void {
  mkdirSync(dirname(STATE_PATH), { recursive: true });
  const tmp = `${STATE_PATH}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, JSON.stringify(state, null, 2), "utf-8");
  renameSync(tmp, STATE_PATH);
}

async function runGh(args: string[]): Promise<string> {
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return out;
}

async function runGit(args: string[]): Promise<string> {
  const proc = Bun.spawn(["git", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return out;
}

async function fetchClosedIssuesSince(sinceIso: string): Promise<number[]> {
  const out = await runGh([
    "issue", "list", "--state", "closed", "--limit", "100",
    "--search", `closed:>=${sinceIso.slice(0, 10)}`,
    "--json", "number,closedAt",
  ]);
  try {
    const raw = JSON.parse(out) as Array<{ number: number; closedAt: string }>;
    return raw.filter(i => new Date(i.closedAt).getTime() >= new Date(sinceIso).getTime())
      .map(i => i.number);
  } catch {
    return [];
  }
}

async function fetchRaisedIssuesSince(sinceIso: string): Promise<number[]> {
  const out = await runGh([
    "issue", "list", "--state", "all", "--limit", "100",
    "--search", `created:>=${sinceIso.slice(0, 10)}`,
    "--json", "number,createdAt",
  ]);
  try {
    const raw = JSON.parse(out) as Array<{ number: number; createdAt: string }>;
    return raw.filter(i => new Date(i.createdAt).getTime() >= new Date(sinceIso).getTime())
      .map(i => i.number);
  } catch {
    return [];
  }
}

async function fetchMergedCountSince(sinceIso: string): Promise<number> {
  const out = await runGit(["log", `--since=${sinceIso}`, "--oneline"]);
  return out.trim().split("\n").filter(l => l.length > 0).length;
}

async function fetchNewAdrDraftsSince(sinceIso: string): Promise<string[]> {
  const out = await runGit([
    "log", `--since=${sinceIso}`, "--diff-filter=A", "--name-only",
    "--pretty=format:", "--", "docs/decisions/*.md",
  ]);
  return out.split("\n").map(l => l.trim()).filter(l => l.length > 0);
}

async function recordCycle(opts: { startedAt?: string; pauseReason?: string }): Promise<void> {
  // Fetch GH/git data outside the lock — these are slow I/O on read-only sources,
  // safe to run without serialization. We re-read state.json under the lock before
  // merge to pick up any concurrent updates (e.g. token-meter updating cumulative).
  const preliminary = readState();
  const startedAt = opts.startedAt ?? preliminary.last_cycle_at;
  const sinceIso = startedAt > "1970" ? startedAt : new Date(Date.now() - 60 * 60 * 1000).toISOString();
  const endedAt = new Date().toISOString();

  const [closed, raised, merged, adrDrafts] = await Promise.all([
    fetchClosedIssuesSince(sinceIso),
    fetchRaisedIssuesSince(sinceIso),
    fetchMergedCountSince(sinceIso),
    fetchNewAdrDraftsSince(sinceIso),
  ]);

  // RMW under lock: re-read latest state, merge, write.
  // #226 prevents lost updates against concurrent state.json writers (token-meter, intent-guard).
  const entry: RecentCycle = await withFileLock(STATE_PATH, async () => {
    const state = readState();
    const newCycle = state.cycle + 1;
    const e: RecentCycle = {
      cycle: newCycle,
      started_at: sinceIso,
      ended_at: endedAt,
      closed,
      raised,
      merged_commits: merged,
      adr_drafts: adrDrafts,
    };
    if (opts.pauseReason) e.pause_reason = opts.pauseReason;

    state.cycle = newCycle;
    state.last_cycle_at = endedAt;
    state.recent_cycles = [...state.recent_cycles, e].slice(-RECENT_LIMIT);
    state.cumulative.cycles_total += 1;
    state.cumulative.issues_closed += closed.length;
    state.cumulative.adr_total += adrDrafts.length;

    atomicWriteState(state);
    return e;
  });

  // Append-only journal (#177 指摘 2): state.json 破損時の復旧用
  try {
    mkdirSync(dirname(JOURNAL_PATH), { recursive: true });
    appendFileSync(JOURNAL_PATH, JSON.stringify(entry) + "\n", "utf-8");
  } catch (e) {
    process.stderr.write(`WARN: journal append failed: ${(e as Error).message}\n`);
  }

  console.log(JSON.stringify(entry, null, 2));
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;
  function arg(name: string): string | undefined {
    const i = rest.indexOf(name);
    return i >= 0 ? rest[i + 1] : undefined;
  }

  if (cmd === "record") {
    await recordCycle({ startedAt: arg("--started-at"), pauseReason: arg("--pause-reason") });
  } else if (cmd === "show") {
    const state = readState();
    console.log(JSON.stringify(state, null, 2));
  } else {
    console.error("Usage: loop-cycle-record.ts (record [--started-at ISO] [--pause-reason reason] | show)");
    process.exit(2);
  }
}
