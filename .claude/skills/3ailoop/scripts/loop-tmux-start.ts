#!/usr/bin/env bun
// loop-tmux-start.ts — /3ailoop tmux ランタイムの起動 (one-shot bootstrap)
//
// 設計 (ADR-012):
// - TMUX 環境変数を検査。tmux session 外なら exit 2 で拒否
// - 既存 watcher PID が live なら多重起動拒否
// - N=1 (LOOP_MAX_WORKERS=1): 従来の 2 pane 構成 (orchestrator + 1 worker)
// - N>=2 (default 3): Phase D-1 の (2+N) pane 構成
//     orchestrator + merge dispatcher + worker × N (別 worktree)
// - 各 worker pane は `claude` を起動し、capture-pane で起動完了マーカーを待機
//   (デフォルト 60 秒タイムアウト、LOOP_TMUX_BOOT_TIMEOUT_SEC で上書き可)
// - N=1: worker pane に `/3ailoop` を投入して初回サイクル開始
// - N>=2: worker pane は idle のまま待機。watcher が Issue を fan-out する
// - loop-tmux-watcher.ts を nohup で daemonize し PID を記録
//
// 使い方:
//   bun loop-tmux-start.ts                        # 通常起動 (N=3)
//   bun loop-tmux-start.ts --n 3                  # N を明示指定
//   bun loop-tmux-start.ts --n 1                  # 従来 2 pane モード
//   bun loop-tmux-start.ts --dry-run              # 計画を stdout に echo するのみ
//   bun loop-tmux-start.ts --dry-run --n 3        # N=3 の計画を表示
//   bun loop-tmux-start.ts --worktree-base /path  # worktree base を上書き
//   bun loop-tmux-start.ts --claude-cmd "claude --dangerously-skip-permissions --model sonnet"
//
// worker pane のデフォルト model は Sonnet 5:
//   - Phase A (#313) で Opus 4.7 → Sonnet 5 に変更
//   - 判断が重い STEP (STEP 3-D / 3.5-B / 6-C / 6.7 / 7 critical) は
//     /3ai 内から Agent tool 経由で Opus 4.7 subagent に委譲する
//   - Opus 4.8 は tool call 破壊のため禁止 (memory: opus-4-8-banned)
//
// 関連: ADR-012, Issue #181, Phase D-1 (parsed-wiggling-newt)

import { existsSync, mkdirSync, renameSync, writeFileSync } from "fs";
import { dirname, join, resolve } from "path";
import { isWatcherAlive, readWatcherPidFile, WORKER_PANE_TITLE } from "./loop-tmux-watcher.ts";

// #183: .claude/skills/3ailoop/scripts/ から 4 階層上が repo root。cwd 非依存にする。
const REPO_ROOT = resolve(import.meta.dir, "../../../..");
const TMUX_DIR = join(REPO_ROOT, "features/.loop/tmux");
const PID_PATH = join(TMUX_DIR, "watcher.pid");
const PANE_PATH = join(TMUX_DIR, "worker.pane");
const PANES_PATH = join(TMUX_DIR, "panes.json");
const WATCHER_PATH = join(REPO_ROOT, ".claude/skills/3ailoop/scripts/loop-tmux-watcher.ts");
const MERGE_DISPATCHER_PATH = join(REPO_ROOT, ".claude/skills/3ailoop/scripts/loop-tmux-merge-dispatcher.ts");
const REGISTRY_PATH = join(REPO_ROOT, ".claude/skills/3ailoop/scripts/loop-worker-registry.ts");
const DEFAULT_SPLIT_PCT = parseInt(process.env.LOOP_TMUX_PANE_PCT ?? "50", 10);
const DEFAULT_WORKTREE_BASE = process.env.LOOP_WORKTREE_BASE ?? "/home/bacon/worktrees";

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

/** dry-run で出力する 1 step。phase は grep / test 検証用の識別子。 */
export interface PlannedStep {
  phase:
    | "worktree-init"
    | "merge-split"
    | "merge-launch"
    | "worker-split"
    | "worktree-add"
    | "worker-cd"
    | "worker-claude"
    | "worker-register"
    | "worker-initial-3ailoop"
    | "layout"
    | "panes-json"
    | "watcher-daemon"
    | "wait-ready"
    | "select-title";
  desc: string;
  args?: string[];
}

export interface PlanOptions {
  n: number;
  claudeCmd: string;
  worktreeBase: string;
}

/** N ワーカー構成の計画を返す (dry-run 表示用 + 実起動時の順序参照用)。 */
export function buildPlan(opts: PlanOptions): PlannedStep[] {
  const { n, claudeCmd, worktreeBase } = opts;
  const steps: PlannedStep[] = [];

  if (n === 1) {
    // --- 従来の 2 pane 構成 (backward compat) ---
    steps.push({
      phase: "worker-split",
      desc: `ワーカー pane を現 window の右に split (幅 ${DEFAULT_SPLIT_PCT}%、session_id+window_id+pane_id 取得)`,
      args: [
        "tmux", "split-window", "-h", "-P", "-F",
        "#{session_id}|#{window_id}|#{pane_id}",
        "-l", `${DEFAULT_SPLIT_PCT}%`,
      ],
    });
    steps.push({
      phase: "select-title",
      desc: `worker pane に pane_title を設定 (metadata 欠落時 fallback)`,
      args: ["tmux", "select-pane", "-t", "<worker-1-pane>", "-T", WORKER_PANE_TITLE],
    });
    steps.push({
      phase: "worker-claude",
      desc: `ワーカー pane で claude を起動: ${claudeCmd}`,
      args: ["tmux", "send-keys", "-t", "<worker-1-pane>", claudeCmd, "Enter"],
    });
    steps.push({
      phase: "wait-ready",
      desc: `Claude 起動完了を capture-pane で待機 (timeout ${BOOT_TIMEOUT_SEC}s)`,
    });
    steps.push({
      phase: "worker-initial-3ailoop",
      desc: `worker pane に初回 /3ailoop を送信`,
      args: ["tmux", "send-keys", "-t", "<worker-1-pane>", "/3ailoop", "Enter"],
    });
    steps.push({
      phase: "panes-json",
      desc: `panes.json を書き込む (workers=1, merge=null)`,
      args: ["writeFile", PANES_PATH],
    });
    steps.push({
      phase: "watcher-daemon",
      desc: `watcher daemon を nohup で起動`,
      args: ["nohup", "bun", WATCHER_PATH, "run", "&"],
    });
    return steps;
  }

  // --- N >= 2: (2+N) pane 構成 ---
  steps.push({
    phase: "worktree-init",
    desc: `worktree base ディレクトリを準備: ${worktreeBase}`,
    args: ["mkdir", "-p", worktreeBase],
  });
  steps.push({
    phase: "merge-split",
    desc: `merge pane を現 window の右に split (幅 ${DEFAULT_SPLIT_PCT}%)`,
    args: [
      "tmux", "split-window", "-h", "-P", "-F",
      "#{session_id}|#{window_id}|#{pane_id}",
      "-l", `${DEFAULT_SPLIT_PCT}%`,
    ],
  });
  steps.push({
    phase: "merge-launch",
    desc: `merge pane で merge dispatcher を起動 (--watch)`,
    args: [
      "tmux", "send-keys", "-t", "<merge-pane>",
      `cd ${REPO_ROOT} && bun ${MERGE_DISPATCHER_PATH} --watch`, "Enter",
    ],
  });

  for (let i = 1; i <= n; i++) {
    const wt = `${worktreeBase}/w${i}`;
    const workerId = `worker-${i}`;
    steps.push({
      phase: "worker-split",
      desc: `${workerId} pane を split (幅 ${DEFAULT_SPLIT_PCT}%、session_id+window_id+pane_id 取得)`,
      args: [
        "tmux", "split-window", "-h", "-P", "-F",
        "#{session_id}|#{window_id}|#{pane_id}",
        "-l", `${DEFAULT_SPLIT_PCT}%`,
      ],
    });
    steps.push({
      phase: "select-title",
      desc: `${workerId} pane に pane_title を設定 (metadata 欠落時 fallback)`,
      args: ["tmux", "select-pane", "-t", `<${workerId}-pane>`, "-T", WORKER_PANE_TITLE],
    });
    steps.push({
      phase: "worktree-add",
      desc: `${workerId} 用 worktree を作成: ${wt}`,
      args: ["git", "-C", REPO_ROOT, "worktree", "add", wt, "HEAD"],
    });
    steps.push({
      phase: "worker-cd",
      desc: `${workerId} pane を worktree に cd`,
      args: ["tmux", "send-keys", "-t", `<${workerId}-pane>`, `cd ${wt}`, "Enter"],
    });
    steps.push({
      phase: "worker-claude",
      desc: `${workerId} pane で claude を起動: ${claudeCmd}`,
      args: ["tmux", "send-keys", "-t", `<${workerId}-pane>`, claudeCmd, "Enter"],
    });
    steps.push({
      phase: "worker-register",
      desc: `worker-registry に ${workerId} を登録 (pane_id, worktree=${wt})`,
      args: [
        "bun", REGISTRY_PATH, "register",
        "--worker-id", workerId,
        "--pane-id", `<${workerId}-pane>`,
        "--worktree", wt,
      ],
    });
  }

  steps.push({
    phase: "layout",
    desc: `tmux select-layout even-horizontal で pane 幅を整える`,
    args: ["tmux", "select-layout", "even-horizontal"],
  });
  steps.push({
    phase: "panes-json",
    desc: `panes.json を書き込む (workers=${n}, merge pane あり)`,
    args: ["writeFile", PANES_PATH],
  });
  steps.push({
    phase: "watcher-daemon",
    desc: `watcher daemon を nohup で起動 (fan-out モード)`,
    args: ["nohup", "bun", WATCHER_PATH, "run", "&"],
  });

  return steps;
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

async function runCmd(args: string[]): Promise<{ ok: boolean; stdout: string; stderr: string }> {
  const proc = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const [out, err] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  await proc.exited;
  return { ok: proc.exitCode === 0, stdout: out, stderr: err };
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

export interface PanesJsonWorker {
  id: string;
  pane_id: string;
  worktree: string;
}

export interface PanesJson {
  orchestrator: { pane_id: string | null };
  merge?: { pane_id: string };
  workers: PanesJsonWorker[];
  max_workers: number;
  saved_at: string;
}

function writePanesJson(payload: PanesJson): void {
  mkdirSync(dirname(PANES_PATH), { recursive: true });
  const tmp = `${PANES_PATH}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, JSON.stringify(payload, null, 2), "utf-8");
  renameSync(tmp, PANES_PATH);
}

async function currentPaneId(): Promise<string | null> {
  try {
    const r = await runCmd(["tmux", "display", "-p", "#{pane_id}"]);
    if (!r.ok) return null;
    const raw = r.stdout.trim();
    return /^%\d+$/.test(raw) ? raw : null;
  } catch {
    return null;
  }
}

function spawnWatcherDaemon(): number {
  // nohup 相当: detached child を起動し PID を返す
  const logPath = join(TMUX_DIR, "watcher.log");
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

function parseIntArg(args: string[], name: string): number | null {
  const i = args.indexOf(name);
  if (i < 0 || !args[i + 1]) return null;
  const v = parseInt(args[i + 1], 10);
  return Number.isFinite(v) ? v : null;
}

function stringArg(args: string[], name: string): string | null {
  const i = args.indexOf(name);
  if (i < 0 || !args[i + 1]) return null;
  return args[i + 1];
}

/** N の解決順序: --n > $LOOP_MAX_WORKERS > 3 (default)。 */
export function resolveN(cliN: number | null, env: NodeJS.ProcessEnv = process.env): number {
  if (cliN !== null && cliN > 0) return cliN;
  const envRaw = env.LOOP_MAX_WORKERS;
  if (envRaw !== undefined) {
    const parsed = parseInt(envRaw, 10);
    if (Number.isFinite(parsed) && parsed > 0) return parsed;
  }
  return 3;
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const dryRun = args.includes("--dry-run");
  // Phase A (#313): worker pane 起動 model を Sonnet 5 に変更。
  // Opus 4.7 は Agent tool 経由で個別 STEP 委譲時に使う。
  // Opus 4.8 は tool call 破壊のため禁止 (memory: opus-4-8-banned)。
  let claudeCmd = "claude --model sonnet";
  const ccIdx = args.indexOf("--claude-cmd");
  if (ccIdx >= 0 && args[ccIdx + 1]) claudeCmd = args[ccIdx + 1];

  const cliN = parseIntArg(args, "--n");
  const n = resolveN(cliN);
  const worktreeBase = stringArg(args, "--worktree-base") ?? DEFAULT_WORKTREE_BASE;

  checkTmuxEnv();
  checkExistingWatcher();
  if (!dryRun) await checkExistingWorkerPane();

  const steps = buildPlan({ n, claudeCmd, worktreeBase });

  if (dryRun) {
    process.stdout.write(`DRY-RUN: N=${n} worktree_base=${worktreeBase}\n`);
    for (const s of steps) {
      process.stdout.write(`DRY-RUN[${s.phase}]: ${s.desc}\n`);
      if (s.args) {
        process.stdout.write(`             ${s.args.map(a => JSON.stringify(a)).join(" ")}\n`);
      }
    }
    return;
  }

  if (n === 1) {
    await runN1(claudeCmd);
    return;
  }
  await runNMulti(n, claudeCmd, worktreeBase);
}

/** N=1 backward-compat 経路。従来の worker.pane 書き込み + 初回 /3ailoop 投入。 */
async function runN1(claudeCmd: string): Promise<void> {
  process.stderr.write(`STEP: worker pane を split (N=1 backward-compat モード)\n`);
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

  // N=1 でも panes.json を書いておく (watcher の判定用)
  const orchId = await currentPaneId();
  writePanesJson({
    orchestrator: { pane_id: orchId },
    workers: [{ id: "worker-1", pane_id: triple.pane_id, worktree: "" }],
    max_workers: 1,
    saved_at: new Date().toISOString(),
  });

  const watcherPid = spawnWatcherDaemon();
  process.stderr.write(`STEP: watcher daemon spawned pid=${watcherPid}\n`);
  console.log(JSON.stringify({ ok: true, pane: triple, watcher_pid: watcherPid }, null, 2));
}

/** N>=2 の (2+N) pane 経路。merge pane + N worker pane を worktree 付きで生成。 */
async function runNMulti(n: number, claudeCmd: string, worktreeBase: string): Promise<void> {
  process.stderr.write(`STEP: worktree base を準備: ${worktreeBase}\n`);
  mkdirSync(worktreeBase, { recursive: true });

  process.stderr.write(`STEP: merge pane を split\n`);
  const mergeTriple = await splitPane();
  process.stderr.write(`  merge pane_id=${mergeTriple.pane_id}\n`);

  process.stderr.write(`STEP: merge dispatcher を起動\n`);
  const mergeCmd = `cd ${REPO_ROOT} && bun ${MERGE_DISPATCHER_PATH} --watch`;
  const mergeR = await runTmux(["tmux", "send-keys", "-t", mergeTriple.pane_id, mergeCmd, "Enter"]);
  if (!mergeR.ok) {
    console.error(`FAIL: send-keys merge-dispatcher: ${mergeR.stderr.trim()}`);
    process.exit(1);
  }

  const workers: PanesJsonWorker[] = [];
  for (let i = 1; i <= n; i++) {
    const workerId = `worker-${i}`;
    const wt = `${worktreeBase}/w${i}`;

    process.stderr.write(`STEP: ${workerId} pane を split\n`);
    const wTriple = await splitPane();
    process.stderr.write(`  ${workerId} pane_id=${wTriple.pane_id}\n`);
    await runTmux(["tmux", "select-pane", "-t", wTriple.pane_id, "-T", WORKER_PANE_TITLE]);

    process.stderr.write(`STEP: ${workerId} worktree add: ${wt}\n`);
    const wtR = await runCmd(["git", "-C", REPO_ROOT, "worktree", "add", wt, "HEAD"]);
    if (!wtR.ok) {
      // 既存 worktree があれば警告のみ (再利用)
      const stderrTrim = wtR.stderr.trim();
      if (!/already exists/.test(stderrTrim)) {
        console.error(`FAIL: worktree add ${wt}: ${stderrTrim}`);
        process.exit(1);
      }
      process.stderr.write(`  (worktree 既存を再利用: ${wt})\n`);
    }

    process.stderr.write(`STEP: ${workerId} pane を worktree に cd\n`);
    await runTmux(["tmux", "send-keys", "-t", wTriple.pane_id, `cd ${wt}`, "Enter"]);

    process.stderr.write(`STEP: ${workerId} pane で claude を起動: ${claudeCmd}\n`);
    const cmdR = await runTmux(["tmux", "send-keys", "-t", wTriple.pane_id, claudeCmd, "Enter"]);
    if (!cmdR.ok) {
      console.error(`FAIL: send-keys claude on ${workerId}: ${cmdR.stderr.trim()}`);
      process.exit(1);
    }

    process.stderr.write(`STEP: worker-registry に ${workerId} を登録\n`);
    const regR = await runCmd([
      "bun", REGISTRY_PATH, "register",
      "--worker-id", workerId,
      "--pane-id", wTriple.pane_id,
      "--worktree", wt,
    ]);
    if (!regR.ok) {
      console.error(`FAIL: register ${workerId}: ${regR.stderr.trim()}`);
      process.exit(1);
    }
    workers.push({ id: workerId, pane_id: wTriple.pane_id, worktree: wt });
  }

  process.stderr.write(`STEP: tmux select-layout even-horizontal\n`);
  await runTmux(["tmux", "select-layout", "even-horizontal"]);

  // legacy worker.pane も worker-1 を指すよう書いておく (stop.ts / dispatch.ts 用)
  if (workers.length > 0) {
    writePaneInfo({
      pane_id: workers[0].pane_id,
      session_id: mergeTriple.session_id, // best-effort: merge pane と同 session/window
      window_id: mergeTriple.window_id,
    });
  }

  const orchId = await currentPaneId();
  writePanesJson({
    orchestrator: { pane_id: orchId },
    merge: { pane_id: mergeTriple.pane_id },
    workers,
    max_workers: n,
    saved_at: new Date().toISOString(),
  });

  const watcherPid = spawnWatcherDaemon();
  process.stderr.write(`STEP: watcher daemon spawned pid=${watcherPid} (fan-out mode)\n`);
  console.log(JSON.stringify({
    ok: true,
    n,
    merge_pane: mergeTriple.pane_id,
    workers,
    watcher_pid: watcherPid,
  }, null, 2));
}

if (import.meta.main) {
  await main();
}
