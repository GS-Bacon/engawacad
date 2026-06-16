#!/usr/bin/env bun
// loop-tmux-watcher.ts — /3ailoop tmux ランタイムの watcher daemon
//
// 設計 (ADR-012):
// - state.json の recent_cycles[-1].ended_at を 10 秒 polling して差分検知
// - 差分 = サイクル完了 → loop-should-stop.ts を呼び:
//     RC=0 → worker pane に `/clear` → 復帰確認 → `/3ailoop` を send-keys
//     RC=1 → loop-notify で通知して watcher 自身も終了
// - 異常検知:
//     worker pane 不在 → 通知して終了 (再生成はせず人間判断)
//     state.json が LOOP_TMUX_STUCK_MIN (default 45) 分以上更新なし → 通知 → 終了
//     state.json 破損 → 3 連続失敗で死亡扱い
// - SIGTERM ハンドラ: PID ファイル削除、最終ステータスをログに残す
//
// コアロジック (detectCycleCompleted / decideAction) は pure 関数として export し
// bun test でカバー。tmux/gh/fs といった副作用層は薄いラッパで分離する。
//
// 使い方:
//   bun loop-tmux-watcher.ts run                       # daemon として常駐
//   bun loop-tmux-watcher.ts run --dry-run             # tmux 送信は echo のみ、restart 後 1 周で終了
//   bun loop-tmux-watcher.ts run --dry-run --mock-worker-alive
//                                                       # workerPaneAlive を true 固定 (smoke test 用)
//
// 関連: ADR-012, Issue #181

import { appendFileSync, existsSync, mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "fs";
import { dirname } from "path";

const STATE_PATH = "features/.loop/state.json";
const TMUX_DIR = "features/.loop/tmux";
const PID_PATH = `${TMUX_DIR}/watcher.pid`;
const LOG_PATH = `${TMUX_DIR}/watcher.log`;
const LAST_ENDED_PATH = `${TMUX_DIR}/last-cycle-ended-at`;
const WINDOW_PATH = `${TMUX_DIR}/worker.window`;
const DEFAULT_WINDOW = "3ailoop-worker";

const POLL_SEC = parseInt(process.env.LOOP_TMUX_POLL_SEC ?? "10", 10);
const STUCK_MIN = parseInt(process.env.LOOP_TMUX_STUCK_MIN ?? "45", 10);
const CLEAR_WAIT_SEC = parseInt(process.env.LOOP_TMUX_CLEAR_WAIT_SEC ?? "8", 10);
const SHOULD_STOP_PATH = ".claude/skills/3ailoop/scripts/loop-should-stop.ts";
const NOTIFY_PATH = ".claude/skills/3ailoop/scripts/loop-notify.ts";

// --- pure logic (bun test 対象) ---

export interface CycleState {
  recent_cycles?: Array<{ ended_at?: string }>;
}

/** state.json から最終 ended_at を抜き出す。空 / 無効なら null。 */
export function extractLastEndedAt(state: CycleState | null): string | null {
  if (!state || !Array.isArray(state.recent_cycles)) return null;
  const last = state.recent_cycles[state.recent_cycles.length - 1];
  if (!last || typeof last.ended_at !== "string") return null;
  return last.ended_at;
}

/** 前回観測値と現在値の差分でサイクル完了を判定。
 * - prev=null, curr=null → 未完了 (state.json まだ書かれていない)
 * - prev=null, curr=値    → 完了 (初サイクル)
 * - prev=値,  curr=null  → 未完了 (state.json が消えた、異常系は別経路で扱う)
 * - prev=A,   curr=B     → 完了 (A !== B)
 * - prev=A,   curr=A     → 未完了 (差分なし)
 */
export function detectCycleCompleted(prev: string | null, curr: string | null): boolean {
  if (!curr) return false;
  return prev !== curr;
}

export type WatcherAction =
  | { kind: "wait" }
  | { kind: "send-clear-and-restart" }
  | { kind: "stop"; reason: string }
  | { kind: "stuck"; minutesIdle: number }
  | { kind: "worker-gone" };

export interface DecideInput {
  prevEndedAt: string | null;
  currEndedAt: string | null;
  shouldStopRc: number | null; // 差分検知時のみ評価
  minutesSinceLastUpdate: number; // currEndedAt の age (分)
  stuckThresholdMin: number;
  workerPaneAlive: boolean;
}

/** 観測値・判定結果から次アクションを決める純粋関数。 */
export function decideAction(input: DecideInput): WatcherAction {
  if (!input.workerPaneAlive) return { kind: "worker-gone" };

  if (detectCycleCompleted(input.prevEndedAt, input.currEndedAt)) {
    if (input.shouldStopRc === 1) {
      return { kind: "stop", reason: "loop-should-stop returned RC=1" };
    }
    return { kind: "send-clear-and-restart" };
  }

  // 差分なし: stuck 判定 (currEndedAt が存在し、stuckThreshold 分以上更新なし)
  if (input.currEndedAt && input.minutesSinceLastUpdate >= input.stuckThresholdMin) {
    return { kind: "stuck", minutesIdle: input.minutesSinceLastUpdate };
  }
  return { kind: "wait" };
}

// --- 副作用 layer ---

function nowIso(): string {
  return new Date().toISOString();
}

function log(msg: string): void {
  const line = `[${nowIso()}] ${msg}\n`;
  try {
    mkdirSync(dirname(LOG_PATH), { recursive: true });
    appendFileSync(LOG_PATH, line, "utf-8");
  } catch {
    // ログ失敗で daemon は止めない
  }
  process.stderr.write(line);
}

function readState(): CycleState | null {
  if (!existsSync(STATE_PATH)) return null;
  try {
    return JSON.parse(readFileSync(STATE_PATH, "utf-8")) as CycleState;
  } catch {
    return null;
  }
}

function readLastObserved(): string | null {
  if (!existsSync(LAST_ENDED_PATH)) return null;
  try {
    return readFileSync(LAST_ENDED_PATH, "utf-8").trim() || null;
  } catch {
    return null;
  }
}

function writeLastObserved(v: string): void {
  mkdirSync(dirname(LAST_ENDED_PATH), { recursive: true });
  const tmp = `${LAST_ENDED_PATH}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, v, "utf-8");
  renameSync(tmp, LAST_ENDED_PATH);
}

function readWindowName(): string {
  if (!existsSync(WINDOW_PATH)) return DEFAULT_WINDOW;
  try {
    return readFileSync(WINDOW_PATH, "utf-8").trim() || DEFAULT_WINDOW;
  } catch {
    return DEFAULT_WINDOW;
  }
}

async function tmuxWindowExists(name: string): Promise<boolean> {
  const proc = Bun.spawn(["tmux", "list-windows", "-F", "#W"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  if (proc.exitCode !== 0) return false;
  return out.split("\n").map(s => s.trim()).includes(name);
}

async function runShouldStop(): Promise<number> {
  const proc = Bun.spawn(["bun", SHOULD_STOP_PATH], { stdout: "pipe", stderr: "pipe" });
  await proc.exited;
  return proc.exitCode ?? 2;
}

async function notify(kind: string, text: string): Promise<void> {
  if (!existsSync(NOTIFY_PATH)) return;
  const proc = Bun.spawn(["bun", NOTIFY_PATH, "--kind", kind, "--text", text], {
    stdout: "pipe",
    stderr: "pipe",
  });
  await proc.exited;
}

async function sendKeysToWorker(window: string, keys: string, dryRun: boolean): Promise<void> {
  if (dryRun) {
    process.stdout.write(`DRY-RUN: tmux send-keys -t =${window} ${JSON.stringify(keys)} Enter\n`);
    return;
  }
  const proc = Bun.spawn(["tmux", "send-keys", "-t", `=${window}`, keys, "Enter"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  await proc.exited;
  if (proc.exitCode !== 0) {
    const err = await new Response(proc.stderr).text();
    throw new Error(`tmux send-keys failed: ${err.trim()}`);
  }
}

function writePidFile(): void {
  mkdirSync(dirname(PID_PATH), { recursive: true });
  writeFileSync(PID_PATH, String(process.pid), "utf-8");
}

function removePidFile(): void {
  try {
    if (existsSync(PID_PATH)) unlinkSync(PID_PATH);
  } catch {
    // best effort
  }
}

function minutesAgo(iso: string): number {
  const ms = Date.now() - new Date(iso).getTime();
  return ms / 60000;
}

async function sleepMs(ms: number): Promise<void> {
  await new Promise(resolve => setTimeout(resolve, ms));
}

async function performRestart(window: string, dryRun: boolean): Promise<void> {
  log(`send-keys: /clear → wait ${CLEAR_WAIT_SEC}s → /3ailoop`);
  await sendKeysToWorker(window, "/clear", dryRun);
  await sleepMs(CLEAR_WAIT_SEC * 1000);
  await sendKeysToWorker(window, "/3ailoop", dryRun);
}

// --- main loop ---

async function runDaemon(dryRun: boolean, mockAlive: boolean): Promise<void> {
  writePidFile();
  log(`watcher start pid=${process.pid} poll=${POLL_SEC}s stuck=${STUCK_MIN}min dryRun=${dryRun} mockAlive=${mockAlive}`);

  const cleanup = (reason: string) => {
    log(`watcher exit: ${reason}`);
    removePidFile();
  };
  process.on("SIGTERM", () => {
    cleanup("SIGTERM");
    process.exit(0);
  });
  process.on("SIGINT", () => {
    cleanup("SIGINT");
    process.exit(0);
  });

  const window = readWindowName();
  let parseFailures = 0;

  // メインループ
  while (true) {
    try {
      const workerAlive = mockAlive ? true : await tmuxWindowExists(window);
      const state = readState();
      if (state === null && existsSync(STATE_PATH)) {
        parseFailures++;
        if (parseFailures >= 3) {
          await notify("loop-tmux-state-corrupt", `[STOP] watcher: state.json 3 連続パース失敗`);
          cleanup("state.json corrupt");
          process.exit(2);
        }
      } else {
        parseFailures = 0;
      }

      const currEndedAt = extractLastEndedAt(state);
      const prevEndedAt = readLastObserved();
      const minutesIdle = currEndedAt ? minutesAgo(currEndedAt) : 0;

      let shouldStopRc: number | null = null;
      if (detectCycleCompleted(prevEndedAt, currEndedAt)) {
        shouldStopRc = await runShouldStop();
        log(`cycle completed: prev=${prevEndedAt ?? "(none)"} → curr=${currEndedAt} shouldStopRc=${shouldStopRc}`);
      }

      const action = decideAction({
        prevEndedAt,
        currEndedAt,
        shouldStopRc,
        minutesSinceLastUpdate: minutesIdle,
        stuckThresholdMin: STUCK_MIN,
        workerPaneAlive: workerAlive,
      });

      switch (action.kind) {
        case "wait":
          break;
        case "send-clear-and-restart":
          await performRestart(window, dryRun);
          if (currEndedAt) writeLastObserved(currEndedAt);
          if (dryRun) {
            cleanup("dry-run exit after one restart");
            return;
          }
          break;
        case "stop":
          await notify("loop-tmux-stop", `[STOP] watcher: ${action.reason}`);
          cleanup(action.reason);
          return;
        case "stuck":
          await notify(
            "loop-tmux-stuck",
            `[STOP] watcher: state.json ${action.minutesIdle.toFixed(0)}min 更新なし (threshold ${STUCK_MIN}min)`,
          );
          cleanup(`stuck ${action.minutesIdle.toFixed(0)}min`);
          return;
        case "worker-gone":
          await notify("loop-tmux-worker-gone", `[STOP] watcher: worker pane '${window}' が tmux から消失`);
          cleanup("worker-gone");
          return;
      }
    } catch (e) {
      log(`loop error: ${(e as Error).message}`);
    }
    await sleepMs(POLL_SEC * 1000);
  }
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;
  const dryRun = rest.includes("--dry-run");
  const mockAlive = rest.includes("--mock-worker-alive");
  switch (cmd) {
    case "run":
      await runDaemon(dryRun, mockAlive);
      break;
    default:
      console.error("Usage: loop-tmux-watcher.ts run [--dry-run] [--mock-worker-alive]");
      process.exit(2);
  }
}
