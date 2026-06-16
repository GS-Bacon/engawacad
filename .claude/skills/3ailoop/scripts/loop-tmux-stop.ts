#!/usr/bin/env bun
// loop-tmux-stop.ts — /3ailoop tmux ランタイムの停止
//
// 設計 (ADR-012):
// - PID ファイル読み → kill -TERM → 5 秒待って残れば KILL
// - ワーカー pane に /exit 送信 → 5 秒後に tmux kill-window -t =3ailoop-worker
// - loop-lock.ts release --force で lock 解放 (念のため)
// - 結果を JSON で stdout に返す
//
// 使い方:
//   bun loop-tmux-stop.ts                 # 通常停止
//   bun loop-tmux-stop.ts --dry-run       # 実行内容を echo するのみ
//   bun loop-tmux-stop.ts --keep-window   # ワーカー pane を残す (デバッグ用)
//
// 関連: ADR-012, Issue #181

import { existsSync, readFileSync, unlinkSync } from "fs";
import { isWatcherAlive, readWatcherPidFile, type WatcherPidInfo } from "./loop-tmux-watcher.ts";

const TMUX_DIR = "features/.loop/tmux";
const PID_PATH = `${TMUX_DIR}/watcher.pid`;
const WINDOW_PATH = `${TMUX_DIR}/worker.window`;
const DEFAULT_WINDOW = "3ailoop-worker";
const LOCK_PATH = ".claude/skills/3ailoop/scripts/loop-lock.ts";

function readWindow(): string {
  if (!existsSync(WINDOW_PATH)) return DEFAULT_WINDOW;
  try {
    return readFileSync(WINDOW_PATH, "utf-8").trim() || DEFAULT_WINDOW;
  } catch {
    return DEFAULT_WINDOW;
  }
}

async function sleepMs(ms: number): Promise<void> {
  await new Promise(r => setTimeout(r, ms));
}

async function runCmd(args: string[]): Promise<{ ok: boolean; stderr: string }> {
  const proc = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const err = await new Response(proc.stderr).text();
  await proc.exited;
  return { ok: proc.exitCode === 0, stderr: err };
}

async function killWatcher(info: WatcherPidInfo, dryRun: boolean): Promise<{ ok: boolean; method: string }> {
  if (dryRun) return { ok: true, method: `DRY-RUN: kill -TERM ${info.pid}` };
  try {
    process.kill(info.pid, "SIGTERM");
  } catch {
    return { ok: true, method: "already dead" };
  }
  for (let i = 0; i < 5; i++) {
    await sleepMs(1000);
    if (!isWatcherAlive(info)) return { ok: true, method: "SIGTERM" };
  }
  try {
    process.kill(info.pid, "SIGKILL");
  } catch {
    return { ok: true, method: "already dead (after SIGKILL)" };
  }
  await sleepMs(500);
  return { ok: !isWatcherAlive(info), method: "SIGKILL" };
}

interface KillWindowResult {
  status: string;
  windowGone: boolean; // window が確実に消えたと言えるとき true
}

async function killWindow(window: string, dryRun: boolean, keep: boolean): Promise<KillWindowResult> {
  if (keep) return { status: "kept (--keep-window)", windowGone: false };
  if (dryRun) {
    return {
      status: `DRY-RUN: tmux send-keys -t =${window} "/exit" Enter; tmux kill-window -t =${window}`,
      windowGone: true, // dry-run は副作用なしで「成功シミュレーション」扱い
    };
  }
  await runCmd(["tmux", "send-keys", "-t", `=${window}`, "/exit", "Enter"]);
  await sleepMs(5000);
  const r = await runCmd(["tmux", "kill-window", "-t", `=${window}`]);
  if (!r.ok) {
    // 既に存在しない場合 (`can't find window`) は windowGone=true として安全側に倒す
    const msg = r.stderr.trim();
    const alreadyGone = /can't find window|window not found|no such window/i.test(msg);
    return {
      status: alreadyGone ? `already gone (${msg})` : `kill-window failed: ${msg}`,
      windowGone: alreadyGone,
    };
  }
  return { status: "killed", windowGone: true };
}

/** #181 R3-F01: 「window が確実に消えた」場合のみ lock を解放する。
 *  --keep-window 経由や kill-window 失敗時は worker が生きている可能性があるので
 *  lock に触らない (= 別 owner や生存 worker の lock を強制解放しない)。 */
async function releaseLock(dryRun: boolean, windowGone: boolean, keepWindow: boolean): Promise<string> {
  if (keepWindow) return "skipped (--keep-window: worker may still hold the lock)";
  if (!windowGone) return "skipped (window kill not confirmed; worker may still hold the lock)";
  if (dryRun) return `DRY-RUN: bun ${LOCK_PATH} release --force`;
  if (!existsSync(LOCK_PATH)) return "lock script missing";
  const r = await runCmd(["bun", LOCK_PATH, "release", "--force"]);
  return r.ok ? "released" : `release failed: ${r.stderr.trim()}`;
}

function cleanupFiles(dryRun: boolean): string[] {
  const removed: string[] = [];
  for (const p of [PID_PATH]) {
    if (!existsSync(p)) continue;
    if (dryRun) {
      removed.push(`DRY-RUN: rm ${p}`);
      continue;
    }
    try {
      unlinkSync(p);
      removed.push(p);
    } catch (e) {
      removed.push(`${p}: ${(e as Error).message}`);
    }
  }
  return removed;
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const dryRun = args.includes("--dry-run");
  const keepWindow = args.includes("--keep-window");

  const window = readWindow();
  const info = readWatcherPidFile(PID_PATH);

  const result: Record<string, unknown> = { window };

  if (info === null) {
    result.watcher = "no PID file (or invalid format)";
  } else if (!isWatcherAlive(info) && !dryRun) {
    // #181 F03: PID 単体ではなく cmdline_marker 込みで verify した上で stale 判定
    result.watcher = `pid=${info.pid} already dead or reused by unrelated process`;
  } else {
    const k = await killWatcher(info, dryRun);
    result.watcher = { pid: info.pid, ok: k.ok, method: k.method };
  }

  const windowResult = await killWindow(window, dryRun, keepWindow);
  result.window_kill = windowResult.status;
  result.lock = await releaseLock(dryRun, windowResult.windowGone, keepWindow);
  result.cleaned = cleanupFiles(dryRun);

  console.log(JSON.stringify(result, null, 2));
}

if (import.meta.main) {
  await main();
}
