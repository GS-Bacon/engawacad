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

import { existsSync, mkdirSync, renameSync, writeFileSync } from "fs";
import { dirname } from "path";
import { isWatcherAlive, readWatcherPidFile, WORKER_PANE_TITLE } from "./loop-tmux-watcher.ts";

const TMUX_DIR = "features/.loop/tmux";
const PID_PATH = `${TMUX_DIR}/watcher.pid`;
const PANE_PATH = `${TMUX_DIR}/worker.pane`;
const WATCHER_PATH = ".claude/skills/3ailoop/scripts/loop-tmux-watcher.ts";
const DEFAULT_SPLIT_PCT = parseInt(process.env.LOOP_TMUX_PANE_PCT ?? "50", 10);

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

/** 既存 worker.pane が tmux 上に生きていれば起動中止 (重複防止)。
 *  #185 R1-F02: 3 つ組 (session_id + window_id + pane_id) で完全一致確認。
 *  #185 R3-F01: metadata 欠落/破損時の fallback として tmux 側の pane_title で
 *  孤立 worker を発見し、それも検知できれば exit 1。 */
async function checkExistingWorkerPane(): Promise<void> {
  const { readPaneInfo } = await import("./loop-tmux-watcher.ts");
  const info = readPaneInfo(PANE_PATH);
  if (info) {
    const proc = Bun.spawn(
      ["tmux", "list-panes", "-a", "-F", "#{session_id}|#{window_id}|#{pane_id}"],
      { stdout: "pipe", stderr: "pipe" },
    );
    const out = await new Response(proc.stdout).text();
    await proc.exited;
    if (proc.exitCode === 0) {
      const expected = `${info.session_id}|${info.window_id}|${info.pane_id}`;
      if (out.split("\n").map(s => s.trim()).includes(expected)) {
        console.error(`ERROR: 既存 worker pane ${info.pane_id} (session=${info.session_id} window=${info.window_id}) が生存中です`);
        console.error("  対処: bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts で掃除してから再試行してください。");
        process.exit(1);
      }
    }
  }

  // metadata なし or 不一致 → pane_title fallback で孤立 worker 検出
  const titleProc = Bun.spawn(
    ["tmux", "list-panes", "-a", "-F", "#{session_id}|#{window_id}|#{pane_id}|#{pane_title}"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const titleOut = await new Response(titleProc.stdout).text();
  await titleProc.exited;
  if (titleProc.exitCode !== 0) return;
  const orphans = titleOut.split("\n")
    .map(s => s.trim())
    .filter(l => l.endsWith(`|${WORKER_PANE_TITLE}`));
  if (orphans.length > 0) {
    console.error(`ERROR: 孤立 worker pane (pane_title=${WORKER_PANE_TITLE}) を ${orphans.length} 個検出 (metadata と不一致)`);
    for (const o of orphans) console.error(`  ${o}`);
    console.error("  対処: bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts で掃除するか、tmux kill-pane -t <pane_id> で手動削除してから再試行してください。");
    process.exit(1);
  }
}

interface TmuxCmd {
  args: string[];
  desc: string;
}

/** dry-run 表示用のコマンド列。実際は splitPane() / sendKeys() を直接呼ぶ。 */
function plan(claudeCmd: string): TmuxCmd[] {
  return [
    {
      args: [
        "tmux", "split-window", "-h", "-P", "-F",
        "#{session_id}|#{window_id}|#{pane_id}",
        "-l", `${DEFAULT_SPLIT_PCT}%`,
      ],
      desc: `ワーカー pane を現 window の右に split (幅 ${DEFAULT_SPLIT_PCT}%、session_id+window_id+pane_id 取得)`,
    },
    {
      args: ["tmux", "send-keys", "-t", "<pane_id>", claudeCmd, "Enter"],
      desc: `ワーカー pane で claude を起動: ${claudeCmd}`,
    },
  ];
}

interface PaneTriple {
  session_id: string;
  window_id: string;
  pane_id: string;
}

async function splitPane(): Promise<PaneTriple> {
  const proc = Bun.spawn(
    [
      "tmux", "split-window", "-h", "-P", "-F",
      "#{session_id}|#{window_id}|#{pane_id}",
      "-l", `${DEFAULT_SPLIT_PCT}%`,
    ],
    { stdout: "pipe", stderr: "pipe" },
  );
  const out = await new Response(proc.stdout).text();
  const err = await new Response(proc.stderr).text();
  await proc.exited;
  if (proc.exitCode !== 0) {
    throw new Error(`tmux split-window failed: ${err.trim()}`);
  }
  const parts = out.trim().split("|");
  if (parts.length !== 3) {
    throw new Error(`unexpected split-window output: '${out.trim()}'`);
  }
  const [sid, wid, pid] = parts;
  if (!/^\$\d+$/.test(sid) || !/^@\d+$/.test(wid) || !/^%\d+$/.test(pid)) {
    throw new Error(`unexpected id format: session=${sid} window=${wid} pane=${pid}`);
  }
  return { session_id: sid, window_id: wid, pane_id: pid };
}

async function runTmux(args: string[]): Promise<{ ok: boolean; stderr: string }> {
  const proc = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const err = await new Response(proc.stderr).text();
  await proc.exited;
  return { ok: proc.exitCode === 0, stderr: err };
}

async function capturePane(paneId: string): Promise<string> {
  const proc = Bun.spawn(["tmux", "capture-pane", "-t", paneId, "-p"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return out;
}

/** Claude Code の起動完了を検出。capture-pane に「>」プロンプト、または "Try "/"Welcome" 文言を待つ。 */
async function waitForClaudeReady(paneId: string): Promise<boolean> {
  const deadline = Date.now() + BOOT_TIMEOUT_SEC * 1000;
  const markers = ["Welcome", "Try ", "/help", "claude.ai/code", "│ >"];
  while (Date.now() < deadline) {
    const buf = await capturePane(paneId);
    if (markers.some(m => buf.includes(m))) return true;
    await new Promise(r => setTimeout(r, BOOT_POLL_MS));
  }
  return false;
}

/** #185 R3-F01: atomic write (tmp + rename) で worker.pane の途中破損を防ぐ。 */
function writePaneInfo(triple: PaneTriple): void {
  mkdirSync(dirname(PANE_PATH), { recursive: true });
  const info = {
    pane_id: triple.pane_id,
    session_id: triple.session_id,
    window_id: triple.window_id,
    saved_at: new Date().toISOString(),
  };
  const tmp = `${PANE_PATH}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, JSON.stringify(info, null, 2), "utf-8");
  renameSync(tmp, PANE_PATH);
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

  checkTmuxEnv();
  checkExistingWatcher();
  if (!dryRun) await checkExistingWorkerPane();

  const steps = plan(claudeCmd);

  if (dryRun) {
    for (const s of steps) {
      process.stdout.write(`DRY-RUN: ${s.desc}\n`);
      process.stdout.write(`         ${s.args.map(a => JSON.stringify(a)).join(" ")}\n`);
    }
    process.stdout.write(`DRY-RUN: wait Claude ready (capture-pane, timeout ${BOOT_TIMEOUT_SEC}s)\n`);
    process.stdout.write(`DRY-RUN: tmux send-keys -t <pane_id> "/3ailoop" Enter\n`);
    process.stdout.write(`DRY-RUN: nohup bun ${WATCHER_PATH} run & (PID は実起動時に記録)\n`);
    process.stdout.write(`DRY-RUN: writeFile ${PANE_PATH} = {pane_id, session_id, window_id, saved_at} JSON\n`);
    return;
  }

  // 実起動: pane を split で生成し 3 つ組を取得
  process.stderr.write(`STEP: ${steps[0].desc}\n`);
  let triple: PaneTriple;
  try {
    triple = await splitPane();
  } catch (e) {
    console.error(`FAIL: ${(e as Error).message}`);
    process.exit(1);
  }
  process.stderr.write(`  session=${triple.session_id} window=${triple.window_id} pane=${triple.pane_id}\n`);
  writePaneInfo(triple);

  // #185 R3-F01: metadata 破損時のリカバリ用に tmux 側にもタイトルを残す
  await runTmux(["tmux", "select-pane", "-t", triple.pane_id, "-T", WORKER_PANE_TITLE]);

  process.stderr.write(`STEP: ワーカー pane で claude を起動: ${claudeCmd}\n`);
  const cmdR = await runTmux(["tmux", "send-keys", "-t", triple.pane_id, claudeCmd, "Enter"]);
  if (!cmdR.ok) {
    console.error(`FAIL: send-keys claude: ${cmdR.stderr.trim()}`);
    process.exit(1);
  }

  process.stderr.write(`STEP: Claude の起動完了を待機 (timeout ${BOOT_TIMEOUT_SEC}s)\n`);
  const ready = await waitForClaudeReady(triple.pane_id);
  if (!ready) {
    console.error(`FAIL: Claude が ${BOOT_TIMEOUT_SEC}s 以内に起動しませんでした`);
    process.exit(1);
  }

  process.stderr.write(`STEP: 初回 /3ailoop を送信\n`);
  const sendR = await runTmux(["tmux", "send-keys", "-t", triple.pane_id, "/3ailoop", "Enter"]);
  if (!sendR.ok) {
    console.error(`FAIL: send-keys /3ailoop: ${sendR.stderr.trim()}`);
    process.exit(1);
  }

  const watcherPid = spawnWatcherDaemon();
  process.stderr.write(`STEP: watcher daemon spawned pid=${watcherPid}\n`);
  console.log(JSON.stringify({ ok: true, pane: triple, watcher_pid: watcherPid }, null, 2));
}

if (import.meta.main) {
  await main();
}
