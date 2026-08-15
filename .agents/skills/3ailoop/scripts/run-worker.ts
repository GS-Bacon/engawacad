#!/usr/bin/env bun
// run-worker.ts — free cloud worker の起動/再開 (async, event-driven, NO polling)
//
// AGENTS.md "External AI delegation":
//   - 現在のターンではジョブをキューへ登録し、返された委譲マーカーを最終メッセージへ含めてターンを終了。
//   - Codex の turn-complete 通知からジョブを起動し、プロセス終了後のコールバックで同じスレッドを再開。
//   - 実行中の write_stdin / プロセス一覧・状態ファイルの反復確認 / sleep loop / 定期進捗確認は禁止。
//
// したがってこの CLI は「委譲 marker を出力して即座に exit する」one-shot 設計。
// 実際の長時間 worker プロセスは、この CLI が job descriptor に書き込んだ **実行可能な起動コマンド**
// (launch) を AGENTS.md の $async-agent-delegation Skill 経由で起動する。終了通知
// (success/rate-limit/abnormal) 後に verify-receipt / land を呼ぶ。
//
// launch コマンドは非対話・再開可能・model pinned でなければならない:
//   - cwd = assigned isolated worktree (worker は worktree 内でのみ動作)
//   - --model <provider/model> で qualified-free model を pin (paid/未選定は混入しない)
//   - plan を prompt として渡し、worktree 内で実装 → receipt.json を書く
//   - --resume-from で same worker session を再開 (consultation 後)
//
// 使い方:
//   bun run-worker.ts --issue <N> --worker-id <id> --model <provider/model> --worktree <path>
//                     --plan <plan.md> [--resume-from <consult-id>]
//
// 出力 (stdout JSON): 委譲 marker
//   { delegated: true, marker, issue, worker_id, model, worktree, plan, resume_from,
//     launch: { command, cwd, env }, pane: { side, identity, stream_command, on_terminal },
//     receipt_path, constraints }
//
// worker はこの marker を受け取り、 isolated worktree 内で plan を実装 → receipt.json を書く。
// receipt の model/provider はここで pin したものと一致しなければ verify-receipt で弾く。
//
// pane (contract §Orchestration #5/#6): job descriptor は worker 専用の右側 tmux pane の
// 割当 contract を持つ。async-delegation harness は launch 実行前に pane を allocate し、
// status line (issue/provider/model/session/state) を設定、launch.command を pane に stream し、
// terminal 通知後に決定的に release する。pane は operator 可視化のみで lifecycle 駆動しない。
//
// 関連: contract §"Orchestration and authority" #3/#4/#5/#6, AGENTS.md "External AI delegation"

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { randomBytes } from "crypto";
import { arg, emitJson, emitStatus, REPO_ROOT } from "./cli/helpers.ts";
import type { PaneIdentity } from "./lib/worker-pane.ts";
import { parseSliceRequirements, profileMismatch, readModelRegistry, selectionBlocker, MODEL_REGISTRY_PATHS } from "./lib/model-registry.ts";
import { getIssue, mutateState, recordTerminal, tryAssign, STATE_PATHS } from "./lib/cas-state.ts";
import {
  assignIssue,
  mutateRegistry,
  registerWorker,
  releaseTerminal,
  setWorkerModel,
  REGISTRY_DEFAULTS,
} from "./lib/worker-registry.ts";

/** Default worker queue dir. Honors the LOOP_QUEUE_DIR env override so deployments/tests can
 *  relocate the queue without touching the repo default (features/.loop/worker-queue). */
const DEFAULT_QUEUE_DIR = process.env.LOOP_QUEUE_DIR
  ? resolve(process.env.LOOP_QUEUE_DIR)
  : resolve(REPO_ROOT, "features/.loop/worker-queue");

/** The noninteractive, model-pinned launch command the async-delegation harness executes.
 *  `opencode run` executes a prompt noninteractively under a pinned model; the plan file is
 *  fed as the prompt and the worker is told to write a receipt at `receipt_path` on a
 *  terminal outcome. `cwd` constrains the worker to its assigned isolated worktree. */
export interface LaunchSpec {
  /** argv[0..]; executable noninteractive command. */
  command: string[];
  /** working directory — always the assigned isolated worktree. */
  cwd: string;
  /** environment for the worker process (qualified model is pinned via args, not env). */
  env: Record<string, string>;
}

/** Build the executable, noninteractive launch command for a worker.
 *  Pure; exported for tests. The plan body is wrapped into the prompt so the worker reads it
 *  directly without depending on the caller's filesystem layout beyond `cwd`.
 *
 *  `harness` selects the resumable CLI the worker runs under. The qualified-model registry
 *  records the harness each model was qualified on; we honor it rather than hard-coding
 *  `opencode run`, so a qualified external candidate is launched on its own declared CLI
 *  (contract §Free-model policy #5: usable noninteractively through a resumable CLI/API,
 *  pinned to a specific model+provider). Defaults to `opencode` (the primary free path). */
export function buildLaunch(opts: {
  model: string;
  worktree: string;
  plan: string;
  receiptPath: string;
  resumeFrom?: string | null;
  harness?: string;
  issue?: number;
  workerId?: string;
}): LaunchSpec {
  const planBody = (() => {
    try {
      return readFileSync(opts.plan, "utf-8");
    } catch {
      return "";
    }
  })();

  // The prompt instructs the worker precisely what a terminal receipt must contain and where
  // to write it. The harness runs this noninteractively with the pinned model.
  const prompt = [
    `You are a 3ailoop free cloud worker. Implement the following Issue plan.`,
    `Constraints: work ONLY inside this worktree (${opts.worktree}).`,
    `No git push. No gh write (issue/pr/milestone/label/repo/release). No default-branch land.`,
    `No shared stash. No cross-worktree edit. No git config credential.* changes.`,
    `You may inspect, edit, test, and create a LOCAL commit inside this worktree.`,
    `When finished, write a receipt (schema_version 1) as JSON to: ${opts.receiptPath}`,
    `Receipt status must be one of: completed | needs_supervisor | failed | rate_limited.`,
    `Include schema_version=1, summary, forbidden_operations, test_commands, unverified_items, and emitted_at.`,
    `forbidden_operations means operations you ACTUALLY EXECUTED, not the prohibitions above. It MUST be [] when none were executed; never copy the constraint list into it.`,
    `test_commands must be objects: [{"command":"...","rc":0,"output_summary":"..."}].`,
    `Immediately before writing the receipt, inspect git status including untracked files.`,
    `For completed: include commit_sha + changed_files covering the FULL diff (tracked, committed, staged, and untracked). Do not omit generated files; instead remove unintended generated files before the local commit/receipt.`,
    `For completed, use this exact key/type shape (replace example values):`,
    `{"schema_version":1,"status":"completed","summary":"...","commit_sha":"<40-hex local commit>","changed_files":["path/from/repo/root"],"forbidden_operations":[],"test_commands":[{"command":"cargo test","rc":0,"output_summary":"..."}],"unverified_items":[],"emitted_at":"1970-01-01T00:00:00Z"}`,
    `schema_version is the NUMBER 1, not the string "1". The harness replaces emitted_at and binds issue/worker/model/session, so do not run a clock command and do not invent those identity fields.`,
    `For needs_supervisor: include consultation { question, options (>=2), recommendation }.`,
    ``,
    `--- PLAN ---`,
    planBody,
  ].join("\n");

  let command = launchCommand(opts.harness ?? "opencode", opts.model, opts.resumeFrom ?? null, prompt);
  if ((opts.harness ?? "opencode") === "opencode" && opts.issue !== undefined && opts.workerId) {
    command = [
      "bun", resolve(import.meta.dir, "opencode-worker-terminal.ts"),
      "--out", resolve(opts.worktree, ".worker/terminal.json"),
      "--audit", resolve(opts.worktree, ".worker/audit.json"),
      "--receipt", opts.receiptPath,
      "--issue", String(opts.issue), "--worker", opts.workerId, "--model", opts.model,
      "--", ...command,
    ];
  }

  return {
    command,
    cwd: opts.worktree,
    env: {},
  };
}

/** Compose the noninteractive, model-pinned launch argv for a given harness. Pure.
 *  - `opencode`: `opencode run --model <p/m> [--resume <id>] <prompt>` (primary free path)
 *  - any other harness recorded in the registry (qualified external candidate): the harness
 *    string IS the executable invocation prefix; we pin the model + append the prompt so the
 *    external resumable CLI runs the worker under the pinned provider/model. The harness
 *    must already be noninteractive + resumable (validated by the external-fallback gate). */
export function launchCommand(harness: string, model: string, resumeFrom: string | null, prompt: string): string[] {
  if (harness === "opencode") {
    const command = ["opencode", "run", "--pure", "--format", "json", "--model", model];
    if (resumeFrom) command.push("--session", resumeFrom);
    command.push(prompt);
    return command;
  }
  // external qualified harness: split the recorded executable prefix (space-separated argv),
  // pin the model, optional resume, then the prompt.
  const prefix = harness.trim().split(/\s+/).filter(Boolean);
  const command = [...prefix, "--model", model];
  if (resumeFrom) command.push("--resume", resumeFrom);
  command.push(prompt);
  return command;
}

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const issueStr = arg(args, "--issue");
  const workerId = arg(args, "--worker-id");
  const model = arg(args, "--model");
  const worktree = arg(args, "--worktree");
  const plan = arg(args, "--plan");
  const resumeFrom = arg(args, "--resume-from");
  const registryPath = arg(args, "--registry-path") ?? MODEL_REGISTRY_PATHS.DEFAULT_REGISTRY_PATH;
  const requirementsPath = arg(args, "--requirements");
  const statePath = resolve(arg(args, "--state-path") ?? STATE_PATHS.DEFAULT_STATE_PATH);
  const workerRegistryPath = resolve(
    arg(args, "--worker-registry-path") ?? REGISTRY_DEFAULTS.DEFAULT_REGISTRY_PATH,
  );
  const expectedGenerationRaw = arg(args, "--expected-generation");
  const paneId = arg(args, "--pane-id") ?? "pending";
  // The active launch path must use a QUALIFIED free model selected via the registry, never an
  // arbitrary --model (F5). Tests/dry-runs that intentionally bypass the registry set
  // LOOP_ALLOW_UNVERIFIED_MODEL=1 and pass --model; production leaves it unset so an
  // unqualified/expired/unknown model is refused before any delegation marker is emitted.
  const allowUnverified = process.env.LOOP_ALLOW_UNVERIFIED_MODEL === "1";

  if (!issueStr || !workerId || !model || !worktree || !plan) {
    emitStatus("Usage: run-worker.ts --issue <N> --worker-id <id> --model <p/m> --worktree <path> --plan <plan.md> [--resume-from <id>] [--registry-path <f>]");
    return 2;
  }
  if (!existsSync(plan)) {
    emitJson({ delegated: false, error: "plan-not-found", plan });
    return 1;
  }
  if (!existsSync(worktree)) {
    emitJson({ delegated: false, error: "worktree-not-found", worktree });
    return 1;
  }

  // Validate the requested --model is a qualified, still-selectable registry entry. This
  // prevents launching an unqualified/probation/retired/expired/paid model and also yields
  // the harness the model was qualified on (opencode or a qualified external resumable CLI).
  const reg = readModelRegistry(registryPath);
  const entry = reg.models[model];
  let harness = "opencode";
  if (allowUnverified) {
    // explicit test/dry-run bypass; still default to the opencode harness.
    harness = entry?.harness ?? "opencode";
  } else if (!entry) {
    emitJson({ delegated: false, error: "model-not-qualified", detail: `--model ${model} is not in the qualified-model registry; select via model-select first (policy #6)` });
    return 1;
  } else {
    const blocker = selectionBlocker(entry, () => new Date());
    if (blocker) {
      emitJson({ delegated: false, error: "model-not-selectable", detail: `--model ${model} is not selectable right now: ${blocker}` });
      return 1;
    }
    harness = entry.harness || "opencode";
    if (!requirementsPath) {
      emitJson({ delegated: false, error: "slice-requirements-required", detail: "production assignments must carry evaluation-matchable slice requirements" });
      return 1;
    }
    try {
      const requirements = parseSliceRequirements(JSON.parse(readFileSync(requirementsPath, "utf-8")));
      const mismatch = profileMismatch(entry.task_profile, requirements);
      if (mismatch.length > 0) {
        emitJson({ delegated: false, error: "model-slice-mismatch", detail: mismatch, task_profile: entry.task_profile, requirements });
        return 1;
      }
    } catch (e) {
      emitJson({ delegated: false, error: "invalid-slice-requirements", detail: (e as Error).message });
      return 1;
    }
  }

  if (!allowUnverified && expectedGenerationRaw === undefined) {
    emitJson({ delegated: false, error: "expected-generation-required", detail: "production assignment requires a CAS generation read before worker selection" });
    return 1;
  }
  const expectedGeneration = Number.parseInt(expectedGenerationRaw ?? "0", 10);
  if (!Number.isInteger(expectedGeneration) || expectedGeneration < 0) {
    emitJson({ delegated: false, error: "invalid-expected-generation", detail: expectedGenerationRaw });
    return 1;
  }

  const issue = parseInt(issueStr, 10);
  let assignedGeneration: number | null = null;
  const releaseFailedAssignment = (): void => {
    try {
      mutateState(statePath, current => {
        const assigned = getIssue(current, issue);
        if (assigned.worker_id !== workerId || assigned.closed) return current;
        const released = recordTerminal(
          current,
          issue,
          workerId,
          "failed",
          assigned.generation,
          () => new Date(),
        );
        return released.ok ? released.state : current;
      });
    } catch {
      // The launch failure is still returned to the caller. Reconciliation can
      // repair a separately damaged lifecycle file without hiding this cause.
    }
    try {
      mutateRegistry(workerRegistryPath, current => current.workers[workerId]
        ? releaseTerminal(current, workerId)
        : current);
    } catch {
      // Same bounded-failure rule as CAS cleanup above.
    }
  };

  try {
    mutateState(statePath, current => {
      const assigned = tryAssign(current, {
        issue,
        workerId,
        expectedGeneration,
        now: () => new Date(),
      });
      if (!assigned.ok) throw new Error(`${assigned.error}: ${assigned.message}`);
      assignedGeneration = assigned.newGeneration;
      return assigned.state;
    });
    mutateRegistry(workerRegistryPath, current => {
      let next = current;
      const existing = next.workers[workerId];
      if (!existing) {
        next = registerWorker(next, workerId, paneId, worktree, model);
      } else {
        if (existing.state !== "idle" || existing.current_issue !== null) {
          throw new Error(`worker-not-idle: ${workerId} state=${existing.state} issue=${existing.current_issue}`);
        }
        if (resolve(existing.worktree) !== resolve(worktree)) {
          throw new Error(`worker-worktree-mismatch: ${existing.worktree} != ${worktree}`);
        }
        next = setWorkerModel(next, workerId, model);
      }
      return assignIssue(next, workerId, issue);
    });
  } catch (error) {
    if (assignedGeneration !== null) releaseFailedAssignment();
    emitJson({ delegated: false, error: "lifecycle-assign-failed", detail: String(error) });
    return 1;
  }

  const runMarker = `worker:${workerId}:issue:${issue}:run:${randomBytes(8).toString("hex")}`;
  const receiptPath = resolve(worktree, ".worker", "receipt.json");

  const launch = buildLaunch({ model, worktree, plan, receiptPath, resumeFrom: resumeFrom ?? null, harness, issue, workerId });

  // Per-worker tmux pane identity (contract §Orchestration #5). The async-delegation harness
  // allocates a dedicated RIGHT-SIDE pane for this worker before running `launch`, stamps a
  // status line identifying issue/provider/pinned-model/session/lifecycle-state, streams the
  // worker's ordinary CLI output into it, and deterministically releases it on the terminal
  // notification. The pane is operator-only visibility — it never drives scheduling/completion
  // (contract #6). The 5 identifying fields below are exactly what the status line must carry.
  const paneIdentity: PaneIdentity = {
    issue,
    workerId,
    provider: model.split("/")[0] ?? "opencode",
    model,
    session: runMarker,
  };
  const pane = {
    // the harness runs: worker-pane.ts allocate → stream launch.command → (terminal) → release
    side: "right" as const,
    identity: paneIdentity,
    // the executable launch string to stream into the pane (single shell line in cwd)
    stream_command: `${launch.command.map(s => shellQuote(s)).join(" ")}`,
    on_terminal: "release" as const, // deterministic cleanup; does not touch CAS/registry
  };

  // Enqueue a non-interactive job descriptor with the executable launch command. The
  // async-delegation harness picks this up on Codex's turn-complete and runs `launch`
  // (noninteractive, model-pinned, worktree-scoped); on terminal exit it reads receiptPath
  // and resumes the Codex thread for verify/land. NO polling here.
  const job = {
    marker: runMarker,
    issue,
    worker_id: workerId,
    model, // pinned qualified-free identity
    worktree,
    plan,
    requirements: requirementsPath ?? null,
    resume_from: resumeFrom ?? null,
    enqueued_at: new Date().toISOString(),
    // executable launch contract: the harness runs exactly this command in this cwd.
    launch,
    // operator-facing right-side tmux pane the harness allocates for this worker (#5/#6).
    pane,
    // The worker MUST emit a receipt at this path on terminal outcome.
    receipt_path: receiptPath,
    terminal_state_path: resolve(worktree, ".worker/terminal.json"),
    audit_path: resolve(worktree, ".worker/audit.json"),
    // Worker constraints (also enforced by verify-receipt / safety.ts):
    //   - no push, no gh write, no default-branch land, no shared stash, no cross-worktree edit.
    constraints: [
      "inspect/edit/test/local-commit inside this worktree only",
      "no git push",
      "no gh write (issue/pr/milestone/label/repo/release)",
      "no default-branch land",
      "no shared stash",
      "no cross-worktree edit",
    ],
    lifecycle: {
      state_path: statePath,
      worker_registry_path: workerRegistryPath,
      assigned_generation: assignedGeneration,
    },
  };

  mkdirSync(DEFAULT_QUEUE_DIR, { recursive: true });
  const jobFile = resolve(DEFAULT_QUEUE_DIR, `${runMarker.replace(/[:]/g, "_")}.json`);
  writeFileSync(jobFile, JSON.stringify(job, null, 2), "utf-8");

  let delegation: { marker: string; job_id?: string; job_file?: string } = { marker: runMarker };
  if (!allowUnverified && process.env.LOOP_SKIP_ASYNC_SUBMIT !== "1") {
    const delegate = process.env.CODEX_ASYNC_DELEGATE
      ?? "/home/bacon/.codex/skills/async-agent-delegation/scripts/delegate.py";
    const visibleState = resolve(worktree, ".worker/visible-command.json");
    const visibleCommand = [
      "bun", resolve(import.meta.dir, "visible-command.ts"),
      "--label", `issue=#${issue} worker=${workerId} provider=${model.split("/")[0]} model=${model} session=${runMarker} state=running`,
      "--state-file", visibleState, "--cwd", worktree, "--", ...launch.command,
    ];
    const resumePrompt = [
      `Continue 3ailoop worker job for Issue #${issue}.`,
      `Read descriptor once: ${jobFile}`,
      `Read terminal, receipt, audit, and logs once; do not poll.`,
      `Verify identity/diff/safety, record the model observation, then continue the authorized next slice or bounded failure path.`,
      `Durable migration log: ${resolve(REPO_ROOT, "docs/development/3ailoop-codex-migration.md")}`,
    ].join(" ");
    const submitted = Bun.spawnSync([
      "python3", delegate, "submit", "--repo", worktree,
      "--label", `3ailoop #${issue} ${model}`,
      "--state-file", resolve(worktree, ".worker/terminal.json"),
      "--state-field", "stage", "--success-value", "completed",
      "--resume-conflict", "new-session", "--resume-prompt", resumePrompt,
      "--continuation-repo", REPO_ROOT,
      "--continuation-add-dir", worktree,
      "--continuation-sandbox", "workspace-write",
      "--", ...visibleCommand,
    ], { cwd: worktree, stdout: "pipe", stderr: "pipe" });
    if (submitted.exitCode !== 0) {
      releaseFailedAssignment();
      emitJson({ delegated: false, error: "async-submit-failed", detail: submitted.stderr.toString(), job_file: jobFile });
      return 1;
    }
    delegation = JSON.parse(submitted.stdout.toString());
  }

  emitStatus(`delegated: ${delegation.marker} (job file ${jobFile}, pane=right:${workerId})`);
  emitJson({ delegated: true, ...job, worker_run_marker: runMarker, marker: delegation.marker, delegation_job_id: delegation.job_id ?? null, delegation_job_file: delegation.job_file ?? null, job_file: jobFile });
  return 0;
}

/** Minimal POSIX shell quoting so the streamed launch string survives send-keys intact.
 *  The harness re-splits it in the pane's shell. */
function shellQuote(s: string): string {
  if (/^[A-Za-z0-9_@%:/=.,+-]+$/.test(s)) return s;
  return `'${s.replace(/'/g, `'\\''`)}'`;
}

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}

void dirname;
