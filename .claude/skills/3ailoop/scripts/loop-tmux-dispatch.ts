#!/usr/bin/env bun
// loop-tmux-dispatch.ts — /3ailoop 起動時の tmux 自走 dispatch 判定 (#185)
//
// 出力:
//   "start"    — TMUX あり + watcher 未起動。SKILL.md 側で loop-tmux-start.ts を呼ぶべき
//   "continue" — TMUX なし、または既に自走中 (= worker pane 内 / 別 worker 稼働中)。
//                通常の L-0〜L-9 を実行する
//
// 使い方:
//   bun loop-tmux-dispatch.ts
//     → stdout に "start" または "continue"、exit 0
//
// 関連: ADR-012, Issue #185

import path from "path";
import { fileURLToPath } from "url";
import { isWatcherAlive, readWatcherPidFile } from "./loop-tmux-watcher.ts";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
// scripts dir から 4 階層上 = repo root
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..", "..");
const PID_PATH = path.join(REPO_ROOT, "features/.loop/tmux/watcher.pid");

function decide(): "start" | "continue" {
  if (!process.env.TMUX) return "continue";
  const info = readWatcherPidFile(PID_PATH);
  if (info && isWatcherAlive(info)) return "continue";
  return "start";
}

if (import.meta.main) {
  console.log(decide());
}

export { decide };
