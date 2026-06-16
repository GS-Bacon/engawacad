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

function readWatcherPid(): number | null {
  if (!existsSync(PID_PATH)) return null;
  try {
    const v = parseInt(readFileSync(PID_PATH, "utf-8").trim(), 10);
    return Number.isFinite(v) ? v : null;
  } catch {
    return null;
  }
}

function isAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
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

async function killWatcher(pid: number, dryRun: boolean): Promise<{ ok: boolean; method: string }> {
  if (dryRun) return { ok: true, method: `DRY-RUN: kill -TERM ${pid}` };
  try {
    process.kill(pid, "SIGTERM");
  } catch {
    return { ok: true, method: "already dead" };
  }
  for (let i = 0; i < 5; i++) {
    await sleepMs(1000);
    if (!isAlive(pid)) return { ok: true, method: "SIGTERM" };
  }
  try {
    process.kill(pid, "SIGKILL");
  } catch {
    return { ok: true, method: "already dead (after SIGKILL)" };
  }
  await sleepMs(500);
  return { ok: !isAlive(pid), method: "SIGKILL" };
}

async function killWindow(window: string, dryRun: boolean, keep: boolean): Promise<string> {
  if (keep) return "kept (--keep-window)";
  if (dryRun) {
    return `DRY-RUN: tmux send-keys -t =${window} "/exit" Enter; tmux kill-window -t =${window}`;
  }
  // /exit を送って Claude に終了機会を与える
  await runCmd(["tmux", "send-keys", "-t", `=${window}`, "/exit", "Enter"]);
  await sleepMs(5000);
  const r = await runCmd(["tmux", "kill-window", "-t", `=${window}`]);
  return r.ok ? "killed" : `kill-window failed: ${r.stderr.trim()}`;
}

async function releaseLock(dryRun: boolean): Promise<string> {
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
  const pid = readWatcherPid();

  const result: Record<string, unknown> = { window };

  if (pid === null) {
    result.watcher = "no PID file";
  } else if (!isAlive(pid) && !dryRun) {
    result.watcher = `pid=${pid} already dead`;
  } else {
    const k = await killWatcher(pid, dryRun);
    result.watcher = { pid, ok: k.ok, method: k.method };
  }

  result.window_kill = await killWindow(window, dryRun, keepWindow);
  result.lock = await releaseLock(dryRun);
  result.cleaned = cleanupFiles(dryRun);

  console.log(JSON.stringify(result, null, 2));
}

if (import.meta.main) {
  await main();
}
