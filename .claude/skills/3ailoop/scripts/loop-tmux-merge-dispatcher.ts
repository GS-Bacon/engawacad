#!/usr/bin/env bun
// loop-tmux-merge-dispatcher.ts — Phase D-1 直列マージ処理 (merge pane で常駐)
//
// 設計:
// - N=3 worker が並列で /3ai を走らせる Phase D-1 では、push だけは直列化しないと
//   force-with-lease の意味 (rebase → 即 push で他者を巻き込まない) が崩れる。
// - 各 worker の /3ai STEP 8 は push せず、features/.loop/merge-queue.jsonl に entry を append。
// - このスクリプトが --watch で常駐し、queue を fifo で 1 件ずつ処理:
//     1. loop-lock acquire --owner merge
//     2. git -C <worktree> fetch + rebase origin/<default_branch>
//     3. rebase 衝突 → abort + worker error + merge-errors.jsonl append + notify + 次へ
//     4. cargo xtask ci
//     5. CI 赤 → Issue に retry:rebase-conflict label + worker error + 次へ
//     6. git push --force-with-lease (rebase 直後なので安全)
//     7. closes_issue: true なら Issue 状態確認、open なら gh issue close
//     8. loop-worker-registry release --worker-id <id> で idle 復帰
//     9. merge lock release
//    10. merge-history.jsonl に結果 append
//
// --watch: 常駐 (merge pane 用)
// --run-once: queue から 1 件だけ処理して exit (テスト用 / cron 用)
//
// 関連: Phase D-1 plan parsed-wiggling-newt.md, project_3ailoop_policy (merge policy)

import { appendFileSync, existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";

const REPO_ROOT = resolve(import.meta.dir, "../../../..");
const DEFAULT_QUEUE_PATH = resolve(REPO_ROOT, "features/.loop/merge-queue.jsonl");
const DEFAULT_HISTORY_PATH = resolve(REPO_ROOT, "features/.loop/merge-history.jsonl");
const DEFAULT_ERRORS_PATH = resolve(REPO_ROOT, "features/.loop/merge-errors.jsonl");
const LOCK_PATH = resolve(REPO_ROOT, ".claude/skills/3ailoop/scripts/loop-lock.ts");
const NOTIFY_PATH = resolve(REPO_ROOT, ".claude/skills/3ailoop/scripts/loop-notify.ts");
const REGISTRY_PATH = resolve(REPO_ROOT, ".claude/skills/3ailoop/scripts/loop-worker-registry.ts");

const POLL_MS = parseInt(process.env.LOOP_MERGE_POLL_MS ?? "5000", 10);
const RETRY_LABEL = "retry:rebase-conflict";

// --- types ---

export interface MergeQueueEntry {
  issue: number;
  worker_id: string;
  worktree: string;
  commit_sha: string;
  closes_issue: boolean;
  enqueued_at: string;
}

export type MergeOutcome = "success" | "rebase-conflict" | "ci-red" | "push-failed" | "internal-error";

export interface MergeResult {
  issue: number;
  worker_id: string;
  outcome: MergeOutcome;
  sha_before: string;
  sha_after: string;
  duration_ms: number;
  detail?: string;
}

export interface MergeDeps {
  /** git fetch origin + rebase origin/<default_branch>. throw or return non-zero rc via `.ok=false`. */
  rebase: (worktree: string) => Promise<{ ok: boolean; stderr: string }>;
  /** cargo xtask ci in worktree. */
  runCi: (worktree: string) => Promise<{ ok: boolean; stderr: string }>;
  /** git push --force-with-lease HEAD. */
  push: (worktree: string) => Promise<{ ok: boolean; stderr: string }>;
  /** current HEAD sha. */
  headSha: (worktree: string) => Promise<string>;
  /** gh issue close if issue is still open. Returns true if final state is CLOSED. */
  closeIssueIfOpen: (issue: number) => Promise<boolean>;
  /** gh issue add-label <RETRY_LABEL>. */
  addRetryLabel: (issue: number) => Promise<void>;
  /** loop-worker-registry release --worker-id X. */
  releaseWorker: (workerId: string) => Promise<void>;
  /** loop-worker-registry mark-error --worker-id X --reason "...". */
  markWorkerError: (workerId: string, reason: string) => Promise<void>;
  /** loop-notify.ts wrapper. */
  notify: (kind: string, text: string) => Promise<void>;
  /** git rebase --abort. */
  abortRebase: (worktree: string) => Promise<void>;
  /** now() for duration_ms. */
  now: () => number;
}

// --- parseQueueLine / enqueueMerge (pure or fs-only helpers) ---

export function parseQueueLine(raw: string): MergeQueueEntry {
  const obj = JSON.parse(raw) as Partial<MergeQueueEntry>;
  if (typeof obj.issue !== "number" || !Number.isFinite(obj.issue)) {
    throw new Error("parseQueueLine: issue must be number");
  }
  if (typeof obj.worker_id !== "string") {
    throw new Error("parseQueueLine: worker_id must be string");
  }
  if (typeof obj.worktree !== "string") {
    throw new Error("parseQueueLine: worktree must be string");
  }
  if (typeof obj.commit_sha !== "string") {
    throw new Error("parseQueueLine: commit_sha must be string");
  }
  if (typeof obj.closes_issue !== "boolean") {
    throw new Error("parseQueueLine: closes_issue must be boolean");
  }
  if (typeof obj.enqueued_at !== "string") {
    throw new Error("parseQueueLine: enqueued_at must be string");
  }
  return {
    issue: obj.issue,
    worker_id: obj.worker_id,
    worktree: obj.worktree,
    commit_sha: obj.commit_sha,
    closes_issue: obj.closes_issue,
    enqueued_at: obj.enqueued_at,
  };
}

/** append 1 JSONL entry。ディレクトリは自動作成。追記だけなので lock 不要 (POSIX O_APPEND 保証)。 */
export function enqueueMerge(entry: MergeQueueEntry, path: string = DEFAULT_QUEUE_PATH): void {
  mkdirSync(dirname(path), { recursive: true });
  appendFileSync(path, JSON.stringify(entry) + "\n", "utf-8");
}

/** queue file を全読み込み、valid entry を返す。破損行は skip。 */
export function readQueue(path: string): MergeQueueEntry[] {
  if (!existsSync(path)) return [];
  const text = readFileSync(path, "utf-8");
  const out: MergeQueueEntry[] = [];
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      out.push(parseQueueLine(trimmed));
    } catch {
      // 破損行はスキップ (errors.jsonl には残す運用余地はあるが、queue は fifo 順を守るため無視)
    }
  }
  return out;
}

/** 先頭 N 件を除いた残りで queue を書き戻す (処理済みを外す)。tmp+rename で atomic。 */
export function popFirst(path: string): void {
  const entries = readQueue(path);
  if (entries.length === 0) return;
  const remaining = entries.slice(1).map(e => JSON.stringify(e) + "\n").join("");
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, remaining, "utf-8");
  renameSync(tmp, path);
}

function appendHistory(result: MergeResult, path: string = DEFAULT_HISTORY_PATH): void {
  mkdirSync(dirname(path), { recursive: true });
  appendFileSync(path, JSON.stringify({ ...result, at: new Date().toISOString() }) + "\n", "utf-8");
}

function appendError(entry: MergeQueueEntry, reason: string, path: string = DEFAULT_ERRORS_PATH): void {
  mkdirSync(dirname(path), { recursive: true });
  appendFileSync(
    path,
    JSON.stringify({ ...entry, reason, at: new Date().toISOString() }) + "\n",
    "utf-8",
  );
}

// --- core: processEntry (DI 経由でテストしやすく) ---

export async function processEntry(
  entry: MergeQueueEntry,
  deps: MergeDeps,
): Promise<MergeResult> {
  const started = deps.now();
  let shaBefore = "";
  try {
    shaBefore = await deps.headSha(entry.worktree);
  } catch {
    // sha 取れない worktree = そもそも壊れている。error 扱い。
    await deps.markWorkerError(entry.worker_id, "worktree headSha failed");
    await deps.notify("merge-error", `#${entry.issue} worktree ${entry.worktree} unreadable`);
    return {
      issue: entry.issue,
      worker_id: entry.worker_id,
      outcome: "internal-error",
      sha_before: "",
      sha_after: "",
      duration_ms: deps.now() - started,
      detail: "worktree headSha failed",
    };
  }

  // 1. rebase
  const rebased = await deps.rebase(entry.worktree);
  if (!rebased.ok) {
    await deps.abortRebase(entry.worktree);
    await deps.markWorkerError(entry.worker_id, `rebase conflict: ${rebased.stderr.slice(0, 200)}`);
    await deps.notify("merge-rebase-conflict", `#${entry.issue} rebase conflict on ${entry.worker_id}`);
    return {
      issue: entry.issue,
      worker_id: entry.worker_id,
      outcome: "rebase-conflict",
      sha_before: shaBefore,
      sha_after: shaBefore,
      duration_ms: deps.now() - started,
      detail: rebased.stderr.slice(0, 500),
    };
  }

  // 2. CI
  const ci = await deps.runCi(entry.worktree);
  if (!ci.ok) {
    await deps.addRetryLabel(entry.issue);
    await deps.markWorkerError(entry.worker_id, `ci red: ${ci.stderr.slice(0, 200)}`);
    await deps.notify("merge-ci-red", `#${entry.issue} CI red on ${entry.worker_id}`);
    const shaAfterCiRed = await deps.headSha(entry.worktree).catch(() => shaBefore);
    return {
      issue: entry.issue,
      worker_id: entry.worker_id,
      outcome: "ci-red",
      sha_before: shaBefore,
      sha_after: shaAfterCiRed,
      duration_ms: deps.now() - started,
      detail: ci.stderr.slice(0, 500),
    };
  }

  // 3. push --force-with-lease
  const pushed = await deps.push(entry.worktree);
  if (!pushed.ok) {
    await deps.markWorkerError(entry.worker_id, `push failed: ${pushed.stderr.slice(0, 200)}`);
    await deps.notify("merge-push-failed", `#${entry.issue} push failed on ${entry.worker_id}`);
    const shaAfterPushFail = await deps.headSha(entry.worktree).catch(() => shaBefore);
    return {
      issue: entry.issue,
      worker_id: entry.worker_id,
      outcome: "push-failed",
      sha_before: shaBefore,
      sha_after: shaAfterPushFail,
      duration_ms: deps.now() - started,
      detail: pushed.stderr.slice(0, 500),
    };
  }

  // 4. close issue if requested
  if (entry.closes_issue) {
    try {
      await deps.closeIssueIfOpen(entry.issue);
    } catch (e) {
      // close 失敗は merge 成功を損なわない (gh 側の一時エラーで別途通知)
      await deps.notify("merge-close-warn", `#${entry.issue} close warn: ${(e as Error).message}`);
    }
  }

  // 5. release worker
  await deps.releaseWorker(entry.worker_id);

  const shaAfter = await deps.headSha(entry.worktree).catch(() => shaBefore);
  return {
    issue: entry.issue,
    worker_id: entry.worker_id,
    outcome: "success",
    sha_before: shaBefore,
    sha_after: shaAfter,
    duration_ms: deps.now() - started,
  };
}

// --- real dependency implementations (git/gh/cargo/bun subprocesses) ---

async function runCmd(args: string[], cwd?: string): Promise<{ ok: boolean; stdout: string; stderr: string }> {
  const proc = Bun.spawn(args, {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  const [out, err] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  await proc.exited;
  return { ok: proc.exitCode === 0, stdout: out, stderr: err };
}

/** origin/HEAD が指す branch 名。失敗時は null。 */
async function detectDefaultBranch(worktree: string): Promise<string | null> {
  const r = await runCmd(
    ["git", "symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
    worktree,
  );
  if (!r.ok) return null;
  const raw = r.stdout.trim();
  if (!raw) return null;
  return raw.startsWith("origin/") ? raw.slice("origin/".length) : raw;
}

function realDeps(): MergeDeps {
  return {
    rebase: async (worktree: string) => {
      const fetch = await runCmd(["git", "-C", worktree, "fetch", "origin"]);
      if (!fetch.ok) return { ok: false, stderr: `fetch: ${fetch.stderr}` };
      const branch = (await detectDefaultBranch(worktree)) ?? "main";
      const r = await runCmd(["git", "-C", worktree, "rebase", `origin/${branch}`]);
      return { ok: r.ok, stderr: r.stderr };
    },
    runCi: async (worktree: string) => {
      const r = await runCmd(["cargo", "xtask", "ci"], worktree);
      return { ok: r.ok, stderr: r.stderr };
    },
    push: async (worktree: string) => {
      const r = await runCmd(["git", "-C", worktree, "push", "--force-with-lease", "origin", "HEAD"]);
      return { ok: r.ok, stderr: r.stderr };
    },
    headSha: async (worktree: string) => {
      const r = await runCmd(["git", "-C", worktree, "rev-parse", "HEAD"]);
      if (!r.ok) throw new Error(`rev-parse HEAD failed: ${r.stderr}`);
      return r.stdout.trim();
    },
    closeIssueIfOpen: async (issue: number) => {
      const view = await runCmd(["gh", "issue", "view", String(issue), "--json", "state"]);
      if (view.ok) {
        try {
          const obj = JSON.parse(view.stdout) as { state?: string };
          if (obj.state?.toUpperCase() === "CLOSED") return true;
        } catch { /* fall through and try close */ }
      }
      const r = await runCmd(["gh", "issue", "close", String(issue)]);
      return r.ok;
    },
    addRetryLabel: async (issue: number) => {
      await runCmd(["gh", "issue", "edit", String(issue), "--add-label", RETRY_LABEL]);
    },
    releaseWorker: async (workerId: string) => {
      await runCmd(["bun", REGISTRY_PATH, "release", "--worker-id", workerId]);
    },
    markWorkerError: async (workerId: string, reason: string) => {
      await runCmd(["bun", REGISTRY_PATH, "mark-error", "--worker-id", workerId, "--reason", reason]);
    },
    notify: async (kind: string, text: string) => {
      await runCmd(["bun", NOTIFY_PATH, "--kind", kind, "--text", text]);
    },
    abortRebase: async (worktree: string) => {
      await runCmd(["git", "-C", worktree, "rebase", "--abort"]);
    },
    now: () => Date.now(),
  };
}

// --- merge lock (loop-lock.ts owner=merge 相当。実装は separate spawn で通す) ---
// loop-lock.ts は owner="loop"|"intake" のみ。merge dispatcher 用に別 lock を持つ方が
// 汚染しない (loop cycle の acquire を妨げないため)。ここでは merge 専用 file lock を使う。

import { withFileLock } from "./loop-file-lock.ts";

const MERGE_LOCK_TARGET = resolve(REPO_ROOT, "features/.loop/merge-dispatcher.lock-target");

// --- run-once / --watch ---

async function ensureLockTargetExists(): Promise<void> {
  mkdirSync(dirname(MERGE_LOCK_TARGET), { recursive: true });
  if (!existsSync(MERGE_LOCK_TARGET)) {
    writeFileSync(MERGE_LOCK_TARGET, "merge dispatcher lock target\n", "utf-8");
  }
}

export async function runOnce(
  queuePath: string = DEFAULT_QUEUE_PATH,
  deps: MergeDeps = realDeps(),
): Promise<MergeResult | null> {
  const queue = readQueue(queuePath);
  if (queue.length === 0) return null;
  const entry = queue[0];
  await ensureLockTargetExists();
  const result = await withFileLock(MERGE_LOCK_TARGET, async () => {
    const r = await processEntry(entry, deps);
    popFirst(queuePath);
    appendHistory(r);
    if (r.outcome === "rebase-conflict" || r.outcome === "ci-red" || r.outcome === "push-failed") {
      appendError(entry, r.detail ?? r.outcome);
    }
    return r;
  }, { timeoutMs: 30_000 });
  return result;
}

async function watchLoop(queuePath: string = DEFAULT_QUEUE_PATH): Promise<void> {
  process.stderr.write(`[merge-dispatcher] watch mode, polling ${queuePath} every ${POLL_MS}ms\n`);
  while (true) {
    try {
      const r = await runOnce(queuePath);
      if (r) {
        process.stderr.write(
          `[merge-dispatcher] #${r.issue} (${r.worker_id}) → ${r.outcome} in ${r.duration_ms}ms\n`,
        );
        continue; // 続けて次の entry を処理
      }
    } catch (e) {
      process.stderr.write(`[merge-dispatcher] error: ${(e as Error).message}\n`);
    }
    await new Promise(r => setTimeout(r, POLL_MS));
  }
}

// --- CLI ---

function arg(args: string[], name: string): string | undefined {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
}
function hasFlag(args: string[], name: string): boolean {
  return args.includes(name);
}

async function cli(): Promise<void> {
  const rest = process.argv.slice(2);
  const queuePath = arg(rest, "--queue-path") ?? DEFAULT_QUEUE_PATH;

  if (hasFlag(rest, "--watch")) {
    await watchLoop(queuePath);
    return;
  }

  if (hasFlag(rest, "--run-once")) {
    const r = await runOnce(queuePath);
    if (!r) {
      process.stderr.write("[merge-dispatcher] queue empty, nothing to do\n");
      process.exit(0);
    }
    console.log(JSON.stringify(r, null, 2));
    process.exit(r.outcome === "success" ? 0 : 1);
    return;
  }

  if (hasFlag(rest, "--enqueue")) {
    // helper CLI: --enqueue --issue N --worker-id X --worktree /path --commit-sha SHA [--no-close]
    const issueStr = arg(rest, "--issue");
    const workerId = arg(rest, "--worker-id");
    const worktree = arg(rest, "--worktree");
    const commitSha = arg(rest, "--commit-sha");
    if (!issueStr || !workerId || !worktree || !commitSha) {
      console.error("Usage: --enqueue --issue N --worker-id X --worktree /path --commit-sha SHA [--no-close]");
      process.exit(2);
    }
    const entry: MergeQueueEntry = {
      issue: parseInt(issueStr, 10),
      worker_id: workerId,
      worktree,
      commit_sha: commitSha,
      closes_issue: !hasFlag(rest, "--no-close"),
      enqueued_at: new Date().toISOString(),
    };
    enqueueMerge(entry, queuePath);
    console.log(JSON.stringify(entry, null, 2));
    process.exit(0);
    return;
  }

  console.error("Usage: loop-tmux-merge-dispatcher.ts (--watch | --run-once | --enqueue ...) [--queue-path PATH]");
  process.exit(2);
}

if (import.meta.main) {
  await cli();
}
