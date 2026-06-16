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

import { existsSync, unlinkSync } from "fs";
import {
  isWatcherAlive,
  readPaneInfo,
  readWatcherPidFile,
  WORKER_PANE_TITLE,
  type PaneInfo,
  type WatcherPidInfo,
} from "./loop-tmux-watcher.ts";

const TMUX_DIR = "features/.loop/tmux";
const PID_PATH = `${TMUX_DIR}/watcher.pid`;
const PANE_PATH = `${TMUX_DIR}/worker.pane`;
const LOCK_PATH = ".claude/skills/3ailoop/scripts/loop-lock.ts";

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

/** #185 R1-F02 / R2-F02: pane_id 単体ではなく 3 つ組で識別。kill 前に
 *  session_id+window_id+pane_id が tmux 上で完全一致するかを確認し、
 *  不一致なら誤 kill を避ける。tmux 一時障害は "unknown" として返し、呼び元で
 *  「停止失敗」(paneGone=false) に倒す。
 */
async function verifyPane(info: PaneInfo): Promise<boolean | "unknown"> {
  // #185 R4-F02: pane_id 単体フォールバックは tmux server 再起動後に新規 pane_id が
  // 再採番される経路で stale metadata が無関係 pane に誤一致するため撤回。3 つ組完全一致
  // のみを live 判定とする。pane 移動の許容は本 Issue のスコープ外。
  const proc = Bun.spawn(
    ["tmux", "list-panes", "-a", "-F", "#{session_id}|#{window_id}|#{pane_id}"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  if (proc.exitCode !== 0) return "unknown";
  const expected = `${info.session_id}|${info.window_id}|${info.pane_id}`;
  return out.split("\n").map(s => s.trim()).includes(expected);
}

async function killPane(info: PaneInfo, dryRun: boolean, keep: boolean): Promise<KillPaneResult> {
  if (keep) return { status: "kept (--keep-pane)", paneGone: false };
  if (dryRun) {
    return {
      status: `DRY-RUN: tmux send-keys -t ${info.pane_id} "/exit" Enter; tmux kill-pane -t ${info.pane_id}`,
      paneGone: true,
    };
  }
  const verified = await verifyPane(info);
  if (verified === "unknown") {
    // #185 R2-F02: tmux 一時障害は paneGone=false で残し、live worker を誤って
    // 解放しない。次回 stop で再試行可能。
    return {
      status: `tmux query failed (cannot verify pane state); not killing, lock/metadata kept for retry`,
      paneGone: false,
    };
  }
  if (verified === false) {
    return {
      status: `not found by 3-tuple (session=${info.session_id} window=${info.window_id} pane=${info.pane_id}); already gone or stale`,
      paneGone: true,
    };
  }
  // verified === true → kill 実行
  await runCmd(["tmux", "send-keys", "-t", info.pane_id, "/exit", "Enter"]);
  await sleepMs(5000);
  const r = await runCmd(["tmux", "kill-pane", "-t", info.pane_id]);
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

/** #185 R1-F01: paneGone=true を確認できた場合のみ worker.pane を削除する。
 *  --keep-pane / kill-pane 失敗 / pane_id 読み取り不能の経路では metadata を残し、
 *  次回 stop で再試行できるようにする。watcher.pid は無条件削除 (PID は別管理)。 */
function cleanupFiles(dryRun: boolean, paneGone: boolean, keepPane: boolean): string[] {
  const removed: string[] = [];
  const targets = [PID_PATH];
  if (paneGone && !keepPane) targets.push(PANE_PATH);
  for (const p of targets) {
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
  if (!paneGone && !keepPane && existsSync(PANE_PATH)) {
    removed.push(`kept ${PANE_PATH} (paneGone not confirmed; rerun stop to retry)`);
  }
  return removed;
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const dryRun = args.includes("--dry-run");
  const keepPane = args.includes("--keep-pane") || args.includes("--keep-window");

  const paneInfo = readPaneInfo(PANE_PATH);
  const watcherInfo = readWatcherPidFile(PID_PATH);

  const result: Record<string, unknown> = {
    pane_id: paneInfo?.pane_id ?? "(none)",
  };

  if (watcherInfo === null) {
    result.watcher = "no PID file (or invalid format)";
  } else if (!isWatcherAlive(watcherInfo) && !dryRun) {
    result.watcher = `pid=${watcherInfo.pid} already dead or reused by unrelated process`;
  } else {
    const k = await killWatcher(watcherInfo, dryRun);
    result.watcher = { pid: watcherInfo.pid, ok: k.ok, method: k.method };
  }

  let paneGone = false;
  if (paneInfo === null) {
    // #185 R3-F01: metadata 欠落時は tmux 側の pane_title で孤立 worker を探す
    const titleProc = Bun.spawn(
      ["tmux", "list-panes", "-a", "-F", "#{session_id}|#{window_id}|#{pane_id}|#{pane_title}"],
      { stdout: "pipe", stderr: "pipe" },
    );
    const titleOut = await new Response(titleProc.stdout).text();
    await titleProc.exited;
    if (titleProc.exitCode !== 0) {
      result.pane_kill = "skipped (no pane info, tmux list-panes failed)";
    } else {
      const orphans = titleOut.split("\n")
        .map(s => s.trim())
        .filter(l => l.endsWith(`|${WORKER_PANE_TITLE}`));
      if (orphans.length === 0) {
        result.pane_kill = "skipped (no pane info and no orphan worker found by pane_title)";
      } else {
        const orphanResults: Array<Record<string, unknown>> = [];
        for (const o of orphans) {
          const parts = o.split("|");
          if (parts.length < 4) continue;
          const [sid, wid, pid] = parts;
          const recovered: PaneInfo = {
            session_id: sid,
            window_id: wid,
            pane_id: pid,
            saved_at: "(recovered by pane_title)",
          };
          const r = await killPane(recovered, dryRun, keepPane);
          orphanResults.push({ pane_id: pid, status: r.status });
          if (r.paneGone) paneGone = true;
        }
        result.pane_kill = { recovered_by_title: orphanResults };
      }
    }
  } else {
    const paneResult = await killPane(paneInfo, dryRun, keepPane);
    result.pane_kill = paneResult.status;
    paneGone = paneResult.paneGone;
  }
  result.lock = await releaseLock(dryRun, paneGone, keepPane);
  result.cleaned = cleanupFiles(dryRun, paneGone, keepPane);

  console.log(JSON.stringify(result, null, 2));
}

if (import.meta.main) {
  await main();
}
