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
const PANE_PATH = `${TMUX_DIR}/worker.pane`;
const LOCK_PATH = ".claude/skills/3ailoop/scripts/loop-lock.ts";

function readPaneId(): string | null {
  if (!existsSync(PANE_PATH)) return null;
  try {
    const v = readFileSync(PANE_PATH, "utf-8").trim();
    return /^%\d+$/.test(v) ? v : null;
  } catch {
    return null;
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

interface KillPaneResult {
  status: string;
  paneGone: boolean; // pane が確実に消えたと言えるとき true
}

async function killPane(paneId: string, dryRun: boolean, keep: boolean): Promise<KillPaneResult> {
  if (keep) return { status: "kept (--keep-pane)", paneGone: false };
  if (dryRun) {
    return {
      status: `DRY-RUN: tmux send-keys -t ${paneId} "/exit" Enter; tmux kill-pane -t ${paneId}`,
      paneGone: true,
    };
  }
  await runCmd(["tmux", "send-keys", "-t", paneId, "/exit", "Enter"]);
  await sleepMs(5000);
  const r = await runCmd(["tmux", "kill-pane", "-t", paneId]);
  if (!r.ok) {
    const msg = r.stderr.trim();
    const alreadyGone = /can't find pane|pane not found|no such pane/i.test(msg);
    return {
      status: alreadyGone ? `already gone (${msg})` : `kill-pane failed: ${msg}`,
      paneGone: alreadyGone,
    };
  }
  return { status: "killed", paneGone: true };
}

/** #181 R3-F01 / #185: 「pane が確実に消えた」場合のみ lock を解放する。 */
async function releaseLock(dryRun: boolean, paneGone: boolean, keepPane: boolean): Promise<string> {
  if (keepPane) return "skipped (--keep-pane: worker may still hold the lock)";
  if (!paneGone) return "skipped (pane kill not confirmed; worker may still hold the lock)";
  if (dryRun) return `DRY-RUN: bun ${LOCK_PATH} release --force`;
  if (!existsSync(LOCK_PATH)) return "lock script missing";
  const r = await runCmd(["bun", LOCK_PATH, "release", "--force"]);
  return r.ok ? "released" : `release failed: ${r.stderr.trim()}`;
}

function cleanupFiles(dryRun: boolean): string[] {
  const removed: string[] = [];
  for (const p of [PID_PATH, PANE_PATH]) {
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
  // --keep-window は #185 で --keep-pane にリネーム。後方互換は不要 (#181 直後のため)。
  const keepPane = args.includes("--keep-pane") || args.includes("--keep-window");

  const paneId = readPaneId();
  const info = readWatcherPidFile(PID_PATH);

  const result: Record<string, unknown> = { pane_id: paneId ?? "(none)" };

  if (info === null) {
    result.watcher = "no PID file (or invalid format)";
  } else if (!isWatcherAlive(info) && !dryRun) {
    result.watcher = `pid=${info.pid} already dead or reused by unrelated process`;
  } else {
    const k = await killWatcher(info, dryRun);
    result.watcher = { pid: info.pid, ok: k.ok, method: k.method };
  }

  if (paneId === null) {
    result.pane_kill = "skipped (no pane_id saved)";
    result.lock = await releaseLock(dryRun, false, keepPane);
  } else {
    const paneResult = await killPane(paneId, dryRun, keepPane);
    result.pane_kill = paneResult.status;
    result.lock = await releaseLock(dryRun, paneResult.paneGone, keepPane);
  }
  result.cleaned = cleanupFiles(dryRun);

  console.log(JSON.stringify(result, null, 2));
}

if (import.meta.main) {
  await main();
}
