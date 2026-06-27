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
import { runChecked } from "./loop-spawn-checked.ts";
import { collect as collectTokens } from "./loop-token-meter.ts";

const STATE_PATH = "features/.loop/state.json";
// #177 指摘 2: append-only journal で state.json 破損耐性確保
const JOURNAL_PATH = "features/.loop/cycle-journal.log";
const RECENT_LIMIT = 20;
// #229: same-state pause を検出したとき failure-tracker を直接押し上げる閾値。
// loop-failure-tracker の FAILURE_THRESHOLD と一致させること。
const FAILURE_THRESHOLD = 3;

/** #229: pause_reason から root cause hash (主要 Issue 番号 + STEP 名) を抽出する。
 *  Issue 番号が取れなければ null。step が取れなければ "_" placeholder。
 *  実 pause_reason は "Cycle #N: #220 STEP 6-D ..." の prefix を含むことがある (Cycle #22 実例)。
 *  cycle 番号を root cause として拾わないよう "Cycle #N:" prefix を剥がしてから #NNN 抽出する。 */
export function extractRootCauseHash(pauseReason: string | undefined): { issueNum: number; step: string; hash: string } | null {
  if (!pauseReason) return null;
  const stripped = pauseReason.replace(/^Cycle\s+#\d+:\s*/i, "");
  const issueMatch = stripped.match(/#(\d+)/);
  if (!issueMatch) return null;
  const issueNum = parseInt(issueMatch[1], 10);
  // STEP 6-D / STEP 7.5 / STEP B-3 / STEP 6-D escalation 等にマッチ。
  // 先頭の英数 token (-_. 区切り) を拾う。
  const stepMatch = stripped.match(/STEP[\s-]?[A-Z0-9]+(?:[-_.][A-Z0-9]+)*/i);
  const step = stepMatch ? stepMatch[0].toUpperCase().replace(/\s+/g, " ").trim() : "_";
  return { issueNum, step, hash: `${issueNum}:${step}` };
}

/** #229: 直前 cycle と現サイクルの pause_reason が同じ root cause hash を持つか判定。
 *  両方の hash が一致した場合のみ same-state とみなす (片方が null なら false)。 */
export function isSameStatePause(
  current: string | undefined,
  previous: string | undefined,
): { sameState: boolean; issueNum: number | null; hash: string | null } {
  const cur = extractRootCauseHash(current);
  const prev = extractRootCauseHash(previous);
  if (!cur || !prev) return { sameState: false, issueNum: null, hash: null };
  if (cur.hash !== prev.hash) return { sameState: false, issueNum: null, hash: null };
  return { sameState: true, issueNum: cur.issueNum, hash: cur.hash };
}

interface CycleTokenDelta {
  claude: number;
  glm: number;
  codex: number;
}

interface RecentCycle {
  cycle: number;
  started_at: string;
  ended_at: string;
  closed: number[];
  raised: number[];
  merged_commits: number;
  adr_drafts: string[];
  pause_reason?: string;
  /** #228: per-cycle token consumption (delta from previous cumulative). */
  tokens?: CycleTokenDelta;
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
  // #227: 失敗は呼び元 (fetchClosedIssuesSince 等) で空配列に degrade されるが、
  // stderr を WARN ログに出して silent fail を可視化。
  const r = await runChecked(["gh", ...args], { allowFailure: true });
  if (r.exitCode !== 0) {
    process.stderr.write(`WARN: gh ${args.slice(0, 3).join(" ")} exit=${r.exitCode}: ${r.stderr.trim().slice(0, 200)}\n`);
  }
  return r.stdout;
}

async function runGit(args: string[]): Promise<string> {
  // #227: 同上。git log が失敗するのは異常 (リポジトリ破損等) なので WARN を出す。
  const r = await runChecked(["git", ...args], { allowFailure: true });
  if (r.exitCode !== 0) {
    process.stderr.write(`WARN: git ${args.slice(0, 3).join(" ")} exit=${r.exitCode}: ${r.stderr.trim().slice(0, 200)}\n`);
  }
  return r.stdout;
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

/** #229: loop-failure-tracker.ts inc を threshold まで繰り返し呼び、needs-human ラベルを付与させる。
 *  loop-failure-tracker.ts は閾値到達時に gh edit --add-label needs-human を発火するので、
 *  needs-human label 付与もここで自動的に行われる。 */
async function bumpFailureToNeedsHuman(issue: number): Promise<void> {
  for (let i = 0; i < FAILURE_THRESHOLD; i++) {
    const r = await runChecked(
      ["bun", ".claude/skills/3ailoop/scripts/loop-failure-tracker.ts", "inc", "--issue", String(issue)],
      { allowFailure: true },
    );
    if (r.exitCode !== 0) {
      process.stderr.write(`WARN: failure-tracker inc #${issue} exit=${r.exitCode}: ${r.stderr.trim().slice(0, 200)}\n`);
      return;
    }
    const count = parseInt(r.stdout.trim(), 10);
    if (Number.isFinite(count) && count >= FAILURE_THRESHOLD) return;
  }
}

/** #284 欠陥 B: pause した呼び元が指定した Issue × Category について、
 *  loop-pause-streak-tracker.ts inc を呼ぶ。閾値到達時は tracker 内部で
 *  needs-human ラベル付与 + コメント投稿が走る (gh edit / gh comment)。
 *  各 issue について 1 回ずつ呼ぶ (tracker 側 inc は副作用込みのため再呼び不要)。 */
async function bumpPauseStreak(
  issues: number[],
  category: string,
  reason: string | undefined,
): Promise<void> {
  for (const issue of issues) {
    const args = [
      ".claude/skills/3ailoop/scripts/loop-pause-streak-tracker.ts",
      "inc",
      "--issue", String(issue),
      "--category", category,
    ];
    if (reason) args.push("--reason", reason);
    const r = await runChecked(["bun", ...args], { allowFailure: true });
    if (r.exitCode !== 0) {
      process.stderr.write(`WARN: pause-streak inc #${issue} category=${category} exit=${r.exitCode}: ${r.stderr.trim().slice(0, 200)}\n`);
    }
  }
}

/** #284: Issue が前進した瞬間 (closed) に、その Issue の全 Category streak を一掃する。
 *  これがないと「pause→close→再 open→pause」のサイクルで偽の連続検出になる。 */
async function resetPauseStreaksForClosed(closed: number[]): Promise<void> {
  for (const issue of closed) {
    const r = await runChecked(
      [
        "bun", ".claude/skills/3ailoop/scripts/loop-pause-streak-tracker.ts",
        "reset-all", "--issue", String(issue),
      ],
      { allowFailure: true },
    );
    if (r.exitCode !== 0) {
      process.stderr.write(`WARN: pause-streak reset-all #${issue} exit=${r.exitCode}: ${r.stderr.trim().slice(0, 200)}\n`);
    }
  }
}

async function recordCycle(opts: {
  startedAt?: string;
  pauseReason?: string;
  pauseIssues?: number[];
  pauseCategory?: string;
}): Promise<void> {
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

  // #228: per-cycle token delta を計算する。collect() は features/*/ の log を全 scan して
  // 現時点の累積を返す。state.cumulative との差分が本 cycle で消費した token。
  const tokenSnapshot = collectTokens();

  // RMW under lock: re-read latest state, merge, write.
  // #226 prevents lost updates against concurrent state.json writers (token-meter, intent-guard).
  // #229: lock 内で previous pause_reason を取得し、外で short-circuit 用 issue 番号を解決する。
  let sameStateIssue: number | null = null;
  let sameStateHash: string | null = null;
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

    // #229: 直前 cycle と pause_reason の root cause hash が一致したら same-state とマーク
    const prevCycle = state.recent_cycles[state.recent_cycles.length - 1];
    const sameState = isSameStatePause(opts.pauseReason, prevCycle?.pause_reason);
    if (sameState.sameState && sameState.issueNum !== null) {
      sameStateIssue = sameState.issueNum;
      sameStateHash = sameState.hash;
    }

    // #228: per-cycle delta = snapshot - 前 cumulative (負値は 0 にクランプ、log 削除等の異常対策)
    e.tokens = {
      claude: Math.max(0, tokenSnapshot.claude - state.cumulative.token_claude),
      glm: Math.max(0, tokenSnapshot.glm - state.cumulative.token_glm),
      codex: Math.max(0, tokenSnapshot.codex - state.cumulative.token_codex),
    };

    state.cycle = newCycle;
    state.last_cycle_at = endedAt;
    state.recent_cycles = [...state.recent_cycles, e].slice(-RECENT_LIMIT);
    state.cumulative.cycles_total += 1;
    state.cumulative.issues_closed += closed.length;
    state.cumulative.adr_total += adrDrafts.length;
    // cumulative は snapshot で上書き (token-meter check と同等)
    state.cumulative.token_claude = tokenSnapshot.claude;
    state.cumulative.token_glm = tokenSnapshot.glm;
    state.cumulative.token_codex = tokenSnapshot.codex;

    atomicWriteState(state);
    return e;
  });

  // #229: same-state pause 検出時は loop-failure-tracker を 1 サイクルで needs-human 閾値まで押し上げる。
  // 反復 (cycle #20→#21) を 1 cycle で打ち切り、別 Issue へ進めるようにする。
  if (sameStateIssue !== null) {
    process.stderr.write(
      `WARN: same-state pause detected (hash=${sameStateHash}) — bumping failure-tracker for #${sameStateIssue} to ${FAILURE_THRESHOLD}\n`,
    );
    await bumpFailureToNeedsHuman(sameStateIssue);
  }

  // #284 欠陥 B: pause で skip された Issue 一覧 × カテゴリで streak を inc。
  // 既存 #229 hash 抽出は ADR 番号と実 Issue の取り違えに弱いので、
  // 呼び元 (/3ailoop の L-5) が明示的に渡してきたものを正規ルートとして扱う。
  if (opts.pauseIssues && opts.pauseIssues.length > 0 && opts.pauseCategory) {
    await bumpPauseStreak(opts.pauseIssues, opts.pauseCategory, opts.pauseReason);
  }

  // #284: closed[] の Issue は前進した = 過去の連続 pause は意味を失う。
  // 各 Issue の全 category streak をリセット。
  if (closed.length > 0) {
    await resetPauseStreaksForClosed(closed);
  }

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
    // #284: --pause-issue は CSV (例: "274,275,276")。pauseCategory と組み合わせて
    // pause-streak tracker を inc する。両方そろっていないと streak は更新しない。
    const pauseIssuesCsv = arg("--pause-issue");
    const pauseIssues = pauseIssuesCsv
      ? pauseIssuesCsv.split(",").map(s => parseInt(s.trim(), 10)).filter(n => Number.isFinite(n) && n > 0)
      : undefined;
    await recordCycle({
      startedAt: arg("--started-at"),
      pauseReason: arg("--pause-reason"),
      pauseIssues,
      pauseCategory: arg("--pause-category"),
    });
  } else if (cmd === "show") {
    const state = readState();
    console.log(JSON.stringify(state, null, 2));
  } else {
    console.error("Usage: loop-cycle-record.ts (record [--started-at ISO] [--pause-reason reason] | show)");
    process.exit(2);
  }
}
