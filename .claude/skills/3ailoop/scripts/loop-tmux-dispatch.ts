#!/usr/bin/env bun
// loop-tmux-dispatch.ts — /3ailoop 起動時の tmux 自走 dispatch 判定 (#185)
//
// 出力:
//   "start"           — TMUX あり + watcher 未起動。SKILL.md 側で loop-tmux-start.ts を呼ぶ
//   "continue"        — TMUX なし、または worker pane 内で /clear→/3ailoop で再投入された経路。
//                       通常の L-0〜L-9 を実行する
//   "already-running" — TMUX あり + watcher 生存 + worker pane 以外の pane から /3ailoop が
//                       打たれた経路 (#185 R4-F01)。lock 競合や二重実行を防ぐため何もせず終了
//
// 使い方:
//   bun loop-tmux-dispatch.ts
//     → stdout に "start" または "continue"、exit 0
//
// 関連: ADR-012, Issue #185

import path from "path";
import { fileURLToPath } from "url";
import { isWatcherAlive, readPaneInfo, readWatcherPidFile } from "./loop-tmux-watcher.ts";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
// scripts dir から 4 階層上 = repo root
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..", "..");
const PID_PATH = path.join(REPO_ROOT, "features/.loop/tmux/watcher.pid");
const PANE_PATH = path.join(REPO_ROOT, "features/.loop/tmux/worker.pane");

/** #185 R5-F01: 現 pane の 3 つ組を tmux から取得 (display-message)。
 *  pane_id 単体ではなく session_id+window_id+pane_id の組み合わせで識別する。 */
async function getCurrentPaneTriple(): Promise<string | null> {
  if (!process.env.TMUX_PANE) return null;
  try {
    const proc = Bun.spawn(
      ["tmux", "display-message", "-p", "-t", process.env.TMUX_PANE,
       "#{session_id}|#{window_id}|#{pane_id}"],
      { stdout: "pipe", stderr: "pipe" },
    );
    const out = await new Response(proc.stdout).text();
    await proc.exited;
    if (proc.exitCode !== 0) return null;
    return out.trim();
  } catch {
    return null;
  }
}

async function decide(): Promise<"start" | "continue" | "already-running"> {
  if (!process.env.TMUX) return "continue";

  const paneInfo = readPaneInfo(PANE_PATH);
  const watcherInfo = readWatcherPidFile(PID_PATH);
  const watcherAlive = !!(watcherInfo && isWatcherAlive(watcherInfo));

  // #185 R5-F01: worker 判定は 3 つ組完全一致で行う。pane_id 単体一致だと
  // stale metadata で別 pane を誤認するリスクがある (R4-F02 と同根)。
  if (paneInfo) {
    const currentTriple = await getCurrentPaneTriple();
    const expectedTriple = `${paneInfo.session_id}|${paneInfo.window_id}|${paneInfo.pane_id}`;
    if (currentTriple && currentTriple === expectedTriple) {
      return "continue";
    }
  }

  // #185 R4-F01: watcher が走っているのに worker 以外の pane から /3ailoop が打たれた場合は
  // lock 競合・多重実行を防ぐため何もしない (already-running)。
  if (watcherAlive) return "already-running";

  return "start";
}

if (import.meta.main) {
  console.log(await decide());
}

export { decide };
