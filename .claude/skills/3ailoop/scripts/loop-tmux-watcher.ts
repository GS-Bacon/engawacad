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
 *
 * #181 F01 (Codex 指摘): prev=null は「ベースライン未確立」を意味し、ここで true を返すと
 * 既存 state.json の古い ended_at を拾った起動直後に誤って /clear を送ってしまう。
 * 「ベースラインの確立」は呼び元の責務 (initialBaseline) で、本関数は確立後の差分検知に
 * 専念する。
 *
 * - prev=null,    curr=*     → false (ベースライン未確立。呼び元で baseline を書いてから再 poll)
 * - prev=値, curr=null      → false (state.json が消えた、異常系)
 * - prev=A,  curr=B (A!==B) → true  (サイクル完了)
 * - prev=A,  curr=A         → false (差分なし)
 */
export function detectCycleCompleted(prev: string | null, curr: string | null): boolean {
  if (!curr || !prev) return false;
  return prev !== curr;
}

export type WatcherAction =
  | { kind: "wait" }
  | { kind: "init-baseline"; baseline: string }
  | { kind: "send-clear-and-restart" }
  | { kind: "stop"; reason: string }
  | { kind: "stuck"; minutesIdle: number }
  | { kind: "worker-gone" }
  | { kind: "should-stop-error"; rc: number };

export interface DecideInput {
  prevEndedAt: string | null;
  currEndedAt: string | null;
  shouldStopRc: number | null; // 差分検知時のみ評価。null = まだ呼んでいない
  minutesSinceLastUpdate: number; // currEndedAt の age (分)
  stuckThresholdMin: number;
  workerPaneAlive: boolean;
}

/** 観測値・判定結果から次アクションを決める純粋関数。
 *
 * #181 F01: prev=null かつ curr=値 なら「ベースライン確立」を返す。watcher は baseline を
 * last-cycle-ended-at に書いてから次 poll に進む。
 * #181 F02: shouldStopRc は 0/1 のみ有効値とし、それ以外 (script missing / crash / RC=2 等) は
 * should-stop-error として安全側 (= watcher 停止) に倒す。
 */
export function decideAction(input: DecideInput): WatcherAction {
  if (!input.workerPaneAlive) return { kind: "worker-gone" };

  // F01: ベースライン未確立で curr が判明 → baseline を確立して次へ
  if (input.prevEndedAt === null && input.currEndedAt) {
    return { kind: "init-baseline", baseline: input.currEndedAt };
  }

  if (detectCycleCompleted(input.prevEndedAt, input.currEndedAt)) {
    // F02: shouldStopRc は厳密検証
    if (input.shouldStopRc === 1) {
      return { kind: "stop", reason: "loop-should-stop returned RC=1" };
    }
    if (input.shouldStopRc === 0) {
      return { kind: "send-clear-and-restart" };
    }
    return { kind: "should-stop-error", rc: input.shouldStopRc ?? -1 };
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

/** #181 R4-F01: tmux 呼び出し失敗は worker-gone と区別する。
 *  - true     : window 存在を確認
 *  - false    : tmux 呼び出し成功 + 対象が見つからなかった
 *  - "unknown": tmux 呼び出し自体が失敗 (socket 一時障害など)
 */
async function tmuxWindowExists(name: string): Promise<boolean | "unknown"> {
  try {
    const proc = Bun.spawn(["tmux", "list-windows", "-F", "#W"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const out = await new Response(proc.stdout).text();
    await proc.exited;
    if (proc.exitCode !== 0) return "unknown";
    return out.split("\n").map(s => s.trim()).includes(name);
  } catch {
    return "unknown";
  }
}

async function runShouldStop(): Promise<number> {
  // #181 R3-F02: smoke / オフライン CI 用のテストフック。env で RC を強制上書きできる。
  // 本番では未設定 (= フォール rough 経路) で gh issue list ベースの判定に乗る。
  const forced = process.env.LOOP_TMUX_FORCE_SHOULD_STOP_RC;
  if (forced !== undefined) {
    const v = parseInt(forced, 10);
    if (Number.isFinite(v)) {
      log(`runShouldStop: forced RC=${v} (LOOP_TMUX_FORCE_SHOULD_STOP_RC)`);
      return v;
    }
  }
  // #181 F02: script missing は安全側 (= 0/1 以外の RC) として上に伝える
  if (!existsSync(SHOULD_STOP_PATH)) {
    log(`runShouldStop: script not found at ${SHOULD_STOP_PATH}`);
    return -1;
  }
  try {
    const proc = Bun.spawn(["bun", SHOULD_STOP_PATH], { stdout: "pipe", stderr: "pipe" });
    const err = await new Response(proc.stderr).text();
    await proc.exited;
    const rc = proc.exitCode ?? -1;
    if (rc !== 0 && rc !== 1) {
      log(`runShouldStop: RC=${rc} stderr=${err.trim().slice(0, 200)}`);
    }
    return rc;
  } catch (e) {
    log(`runShouldStop: spawn error: ${(e as Error).message}`);
    return -1;
  }
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

// --- PID file 管理 (PID reuse 対策: cmdline_marker で同一プロセスを検証) ---
//
// #181 F03: 単純な PID 再利用攻撃を防ぐため、PID file に cmdline 一致のマーカーを含める。
// Linux なら /proc/<pid>/cmdline を読んで marker 文字列の包含を確認、それ以外の OS では
// `process.kill(pid, 0)` の生存確認 + 「marker フィールドが PID file に存在する」だけで
// best-effort 判定する。

export const WATCHER_CMDLINE_MARKER = "loop-tmux-watcher.ts";

export interface WatcherPidInfo {
  pid: number;
  started_at: string;
  cmdline_marker: string;
}

export function readWatcherPidFile(path: string = PID_PATH): WatcherPidInfo | null {
  if (!existsSync(path)) return null;
  try {
    const txt = readFileSync(path, "utf-8").trim();
    if (!txt.startsWith("{")) return null; // 旧形式 (raw PID) は無効扱い
    const obj = JSON.parse(txt) as Partial<WatcherPidInfo>;
    if (typeof obj.pid !== "number" || !Number.isFinite(obj.pid)) return null;
    if (typeof obj.cmdline_marker !== "string") return null;
    if (typeof obj.started_at !== "string") return null;
    return obj as WatcherPidInfo;
  } catch {
    return null;
  }
}

export function isWatcherAlive(info: WatcherPidInfo): boolean {
  try {
    process.kill(info.pid, 0);
  } catch {
    return false;
  }
  // #181 R2-F02: Linux なら /proc/<pid>/cmdline、それ以外は ps -p <pid> -o args= で argv を取得して
  // marker を検証する。どちらも失敗したら「未確認」= 安全側で alive とみなさない (false)。
  const cmdlinePath = `/proc/${info.pid}/cmdline`;
  if (existsSync(cmdlinePath)) {
    try {
      const cmdline = readFileSync(cmdlinePath, "utf-8");
      return cmdline.includes(info.cmdline_marker);
    } catch {
      return false;
    }
  }
  try {
    const proc = Bun.spawnSync(["ps", "-p", String(info.pid), "-o", "args="], {
      stdout: "pipe",
      stderr: "pipe",
    });
    if (proc.exitCode !== 0) return false;
    const out = new TextDecoder().decode(proc.stdout);
    return out.includes(info.cmdline_marker);
  } catch {
    return false;
  }
}

function writePidFile(): void {
  mkdirSync(dirname(PID_PATH), { recursive: true });
  const info: WatcherPidInfo = {
    pid: process.pid,
    started_at: new Date().toISOString(),
    cmdline_marker: WATCHER_CMDLINE_MARKER,
  };
  writeFileSync(PID_PATH, JSON.stringify(info, null, 2), "utf-8");
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

async function capturePane(window: string): Promise<string> {
  try {
    const proc = Bun.spawn(["tmux", "capture-pane", "-t", `=${window}`, "-p"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const out = await new Response(proc.stdout).text();
    await proc.exited;
    return out;
  } catch {
    return "";
  }
}

/** /clear の処理完了を capture-pane で検出。タイムアウト時は false。
 *  start.ts の waitForClaudeReady と同じマーカーセット。 */
async function waitForPaneReady(window: string, timeoutSec: number): Promise<boolean> {
  const markers = ["│ >", "Welcome", "Try ", "/help", "claude.ai/code"];
  const deadline = Date.now() + timeoutSec * 1000;
  while (Date.now() < deadline) {
    const buf = await capturePane(window);
    if (markers.some(m => buf.includes(m))) return true;
    await sleepMs(500);
  }
  return false;
}

async function performRestart(window: string, dryRun: boolean): Promise<void> {
  log(`send-keys: /clear → wait pane ready → /3ailoop`);
  await sendKeysToWorker(window, "/clear", dryRun);
  if (dryRun) {
    // dry-run は副作用なしでフォールバック秒だけ待機
    await sleepMs(CLEAR_WAIT_SEC * 1000);
  } else {
    // #181 R4-F02: 固定 sleep だと /clear 処理中の pane に /3ailoop を送り込んで
    // 取りこぼす可能性があるため、capture-pane でプロンプト復帰を待つ。
    // フォールバックタイムアウトは CLEAR_WAIT_SEC * 4 (最大 32s デフォルト) で
    // 余裕を持たせる。タイムアウト時は警告ログを残して /3ailoop を送る (worst case
    // でも次サイクルの差分検知で気付ける)。
    const ready = await waitForPaneReady(window, CLEAR_WAIT_SEC * 4);
    if (!ready) {
      log(`WARN: pane did not show ready marker within ${CLEAR_WAIT_SEC * 4}s after /clear; sending /3ailoop anyway`);
    }
  }
  await sendKeysToWorker(window, "/3ailoop", dryRun);
}

// --- main loop ---

async function runDaemon(dryRun: boolean, mockAlive: boolean, keepBaseline: boolean): Promise<void> {
  // #181 R2-F01: 残置 last-cycle-ended-at による誤検知を防ぐため、起動時に必ず破棄。
  // 最初の poll で init-baseline が走り、現在の state.json から baseline を再確立する。
  // --keep-baseline は smoke test 専用で、用意した baseline を保持したまま差分検知を試す。
  if (!keepBaseline) {
    try {
      if (existsSync(LAST_ENDED_PATH)) unlinkSync(LAST_ENDED_PATH);
    } catch {
      // best effort: 失敗しても続行
    }
  }
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
  let tmuxQueryFailures = 0;
  const TMUX_QUERY_FAIL_LIMIT = 3;

  // メインループ
  while (true) {
    try {
      let workerAlive: boolean;
      if (mockAlive) {
        workerAlive = true;
      } else {
        const q = await tmuxWindowExists(window);
        if (q === "unknown") {
          // #181 R4-F01: tmux 一時障害は worker-gone と区別。連続失敗で異常検知。
          tmuxQueryFailures++;
          log(`tmux query failed (${tmuxQueryFailures}/${TMUX_QUERY_FAIL_LIMIT})`);
          if (tmuxQueryFailures >= TMUX_QUERY_FAIL_LIMIT) {
            await notify(
              "loop-tmux-query-failed",
              `[STOP] watcher: tmux list-windows が ${tmuxQueryFailures} 連続で失敗`,
            );
            cleanup(`tmux query failed ${tmuxQueryFailures} times`);
            return;
          }
          // best-effort で続行 (この poll は wait と同等扱い)
          workerAlive = true;
        } else {
          tmuxQueryFailures = 0;
          workerAlive = q;
        }
      }
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
        case "init-baseline":
          writeLastObserved(action.baseline);
          log(`baseline initialized: ${action.baseline} (起動直後の既存 ended_at を採用)`);
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
        case "should-stop-error":
          await notify(
            "loop-tmux-should-stop-error",
            `[STOP] watcher: loop-should-stop が想定外 RC=${action.rc} を返した (0/1 以外は安全側で停止)`,
          );
          cleanup(`should-stop RC=${action.rc}`);
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
  const keepBaseline = rest.includes("--keep-baseline");
  switch (cmd) {
    case "run":
      await runDaemon(dryRun, mockAlive, keepBaseline);
      break;
    default:
      console.error("Usage: loop-tmux-watcher.ts run [--dry-run] [--mock-worker-alive] [--keep-baseline]");
      process.exit(2);
  }
}
