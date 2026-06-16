#!/usr/bin/env bun
// loop-tmux-start.ts — /3ailoop tmux ランタイムの起動 (one-shot bootstrap)
//
// 設計 (ADR-012):
// - TMUX 環境変数を検査。tmux session 外なら exit 2 で拒否
// - 既存 watcher PID が live なら多重起動拒否
// - tmux new-window -d -n 3ailoop-worker でワーカー pane を生成
// - ワーカー pane で `claude` を起動し、capture-pane で起動完了マーカーを待機
//   (デフォルト 60 秒タイムアウト、LOOP_TMUX_BOOT_TIMEOUT_SEC で上書き可)
// - tmux send-keys '/3ailoop' Enter で初回サイクル開始
// - loop-tmux-watcher.ts を nohup で daemonize し PID を記録
//
// 使い方:
//   bun loop-tmux-start.ts                # 通常起動
//   bun loop-tmux-start.ts --dry-run      # tmux コマンド列を stdout に echo するのみ
//   bun loop-tmux-start.ts --claude-cmd "claude --dangerously-skip-permissions"
//
// 関連: ADR-012, Issue #181

import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname } from "path";
import { isWatcherAlive, readWatcherPidFile } from "./loop-tmux-watcher.ts";

const TMUX_DIR = "features/.loop/tmux";
const PID_PATH = `${TMUX_DIR}/watcher.pid`;
const WINDOW_PATH = `${TMUX_DIR}/worker.window`;
const DEFAULT_WINDOW = "3ailoop-worker";
const WATCHER_PATH = ".claude/skills/3ailoop/scripts/loop-tmux-watcher.ts";

const BOOT_TIMEOUT_SEC = parseInt(process.env.LOOP_TMUX_BOOT_TIMEOUT_SEC ?? "60", 10);
const BOOT_POLL_MS = 1000;

function checkTmuxEnv(): void {
  if (!process.env.TMUX) {
    console.error("ERROR: $TMUX が設定されていません。tmux session 内から起動してください。");
    console.error("  例: tmux new-session -A -s engawa");
    process.exit(2);
  }
}

function checkExistingWatcher(): void {
  // #181 F03: PID + cmdline_marker による厳格な多重起動判定
  const info = readWatcherPidFile(PID_PATH);
  if (info && isWatcherAlive(info)) {
    console.error(`ERROR: watcher pid=${info.pid} (started_at=${info.started_at}) が既に走っています (${PID_PATH})`);
    console.error("  停止するには: bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts");
    process.exit(1);
  }
  // 死んだ / 別プロセスに reuse された / 旧形式 / 不正な PID ファイルは start.ts が
  // 引き継いで上書きするので何もしない
}

interface TmuxCmd {
  args: string[];
  desc: string;
}

function plan(claudeCmd: string, window: string): TmuxCmd[] {
  return [
    {
      args: ["tmux", "new-window", "-d", "-n", window],
      desc: `ワーカー pane 生成 (window=${window})`,
    },
    {
      args: ["tmux", "send-keys", "-t", `=${window}`, claudeCmd, "Enter"],
      desc: `ワーカー pane で claude を起動: ${claudeCmd}`,
    },
  ];
}

async function runTmux(args: string[]): Promise<{ ok: boolean; stderr: string }> {
  const proc = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const err = await new Response(proc.stderr).text();
  await proc.exited;
  return { ok: proc.exitCode === 0, stderr: err };
}

async function capturePane(window: string): Promise<string> {
  const proc = Bun.spawn(["tmux", "capture-pane", "-t", `=${window}`, "-p"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return out;
}

/** Claude Code の起動完了を検出。capture-pane に「>」プロンプト、または "Try "/"Welcome" 文言を待つ。 */
async function waitForClaudeReady(window: string): Promise<boolean> {
  const deadline = Date.now() + BOOT_TIMEOUT_SEC * 1000;
  const markers = ["Welcome", "Try ", "/help", "claude.ai/code", "│ >"];
  while (Date.now() < deadline) {
    const buf = await capturePane(window);
    if (markers.some(m => buf.includes(m))) return true;
    await new Promise(r => setTimeout(r, BOOT_POLL_MS));
  }
  return false;
}

function writeWindow(window: string): void {
  mkdirSync(dirname(WINDOW_PATH), { recursive: true });
  writeFileSync(WINDOW_PATH, window, "utf-8");
}

function spawnWatcherDaemon(): number {
  // nohup 相当: detached child を起動し PID を返す
  const logPath = `${TMUX_DIR}/watcher.log`;
  mkdirSync(dirname(logPath), { recursive: true });
  const out = Bun.file(logPath);
  const proc = Bun.spawn(["bun", WATCHER_PATH, "run"], {
    stdout: out,
    stderr: out,
    stdin: "ignore",
    detached: true,
  });
  // 親が exit してもいいように proc.unref 相当
  proc.unref?.();
  return proc.pid;
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const dryRun = args.includes("--dry-run");
  let claudeCmd = "claude";
  const ccIdx = args.indexOf("--claude-cmd");
  if (ccIdx >= 0 && args[ccIdx + 1]) claudeCmd = args[ccIdx + 1];
  const window = DEFAULT_WINDOW;

  checkTmuxEnv();
  checkExistingWatcher();

  const steps = plan(claudeCmd, window);

  if (dryRun) {
    for (const s of steps) {
      process.stdout.write(`DRY-RUN: ${s.desc}\n`);
      process.stdout.write(`         ${s.args.map(a => JSON.stringify(a)).join(" ")}\n`);
    }
    process.stdout.write(`DRY-RUN: wait Claude ready (capture-pane, timeout ${BOOT_TIMEOUT_SEC}s)\n`);
    process.stdout.write(`DRY-RUN: tmux send-keys -t =${window} "/3ailoop" Enter\n`);
    process.stdout.write(`DRY-RUN: nohup bun ${WATCHER_PATH} run & (PID は実起動時に記録)\n`);
    process.stdout.write(`DRY-RUN: writeFile ${WINDOW_PATH} = ${window}\n`);
    return;
  }

  // 実起動
  for (const s of steps) {
    process.stderr.write(`STEP: ${s.desc}\n`);
    const r = await runTmux(s.args);
    if (!r.ok) {
      console.error(`FAIL: ${s.desc}: ${r.stderr.trim()}`);
      process.exit(1);
    }
  }

  writeWindow(window);

  process.stderr.write(`STEP: Claude の起動完了を待機 (timeout ${BOOT_TIMEOUT_SEC}s)\n`);
  const ready = await waitForClaudeReady(window);
  if (!ready) {
    console.error(`FAIL: Claude が ${BOOT_TIMEOUT_SEC}s 以内に起動しませんでした`);
    process.exit(1);
  }

  process.stderr.write(`STEP: 初回 /3ailoop を送信\n`);
  const sendR = await runTmux(["tmux", "send-keys", "-t", `=${window}`, "/3ailoop", "Enter"]);
  if (!sendR.ok) {
    console.error(`FAIL: send-keys /3ailoop: ${sendR.stderr.trim()}`);
    process.exit(1);
  }

  const watcherPid = spawnWatcherDaemon();
  process.stderr.write(`STEP: watcher daemon spawned pid=${watcherPid}\n`);
  console.log(JSON.stringify({ ok: true, window, watcher_pid: watcherPid }, null, 2));
}

if (import.meta.main) {
  await main();
}
