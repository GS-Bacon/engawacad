// repair-regressions.test.ts — regression coverage for the three Codex review findings
//
// Finding 1 (high): model-select must not return exit 0 (selection success) for a
//   probation/unqualified model. Only `qualified` free workers are selectable (policy #6).
// Finding 2 (high): run-worker's job descriptor must carry an executable, noninteractive,
//   model-pinned launch command (and a receipt contract) so the async-delegation harness
//   can actually start the worker.
// Finding 3 (critical): land.ts realDeps must read/mutate real CAS state + worker registry,
//   and run a real (fail-closed) scope check — not hard-code generation 0 / scope ok / no-ops.

import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync, mkdirSync, readdirSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { execFileSync } from "child_process";
import { buildLaunch } from "../run-worker.ts";
import { matchGlob, checkFilesScope, checkScope, loadScopeManifest } from "../lib/scope-check.ts";
import { readModelRegistry, upsertModel, selectQualified, type ModelEntry } from "../lib/model-registry.ts";
import { readState, writeState, tryAssign, markClosed } from "../lib/cas-state.ts";
import { readRegistry, writeRegistry, registerWorker, assignIssue, releaseTerminal } from "../lib/worker-registry.ts";
import { landOnce, revalidate, type LandDeps, type LandRequest } from "../lib/land-serial.ts";

const SCRIPTS = join(import.meta.dir, "..");

function bun(args: string[], env?: Record<string, string>): { rc: number; stdout: string; stderr: string } {
  try {
    const stdout = execFileSync("bun", args, { cwd: SCRIPTS, encoding: "utf-8", env: { ...process.env, ...env } });
    return { rc: 0, stdout, stderr: "" };
  } catch (e: unknown) {
    const err = e as { status?: number; stdout?: string; stderr?: string };
    return { rc: err.status ?? 1, stdout: err.stdout ?? "", stderr: err.stderr ?? "" };
  }
}

function qualifiedEntry(identity: string): ModelEntry {
  return {
    identity,
    harness: "opencode",
    provider: identity.split("/")[0],
    model: identity.split("/")[1],
    status: "qualified",
    evaluation: { suite_version: "2", passed: 2, total: 3, passed_classes: ["bounded-rust-bugfix", "multi-crate-feature"], tested_at: "2026-08-13T00:00:00Z" },
    supported_task_class: "general",
    free_tier: { status: "active", input_cost: 0, output_cost: 0, toolcall: true, checked_at: "2026-08-13T00:00:00Z", expiry: null, expiry_known: false },
    observations: { successes: 1, failures: 0, rate_limits: 0, last_success_at: "2026-08-13T00:00:00Z", last_failure_at: null },
    last_evaluated_at: "2026-08-13T00:00:00Z",
  };
}

const now = () => new Date();

// ---------------------------------------------------------------------------
// Finding 1: probation model is registered but NOT selected (no exit 0)
// ---------------------------------------------------------------------------

describe("Finding 1: model-select never selects a probation/unqualified model", () => {
  test("pure: selectQualified ignores a registry containing only a probation model", () => {
    let reg = readModelRegistry("/nonexistent/registry.json");
    const probation: ModelEntry = { ...qualifiedEntry("opencode/candidate-free"), status: "probation" };
    reg = upsertModel(reg, probation);
    expect(selectQualified(reg)).toBeNull();
  });

  test("CLI: only-probation registry + --discover-if-empty → exit 1 NO_QUALIFIED_FREE_WORKER", () => {
    const tmp = mkdtempSync(join(tmpdir(), "msel-cli-"));
    const registryPath = join(tmp, "model-registry.json");
    const probation: ModelEntry = { ...qualifiedEntry("opencode/candidate-free"), status: "probation" };
    writeFileSync(registryPath, JSON.stringify({ models: { [probation.identity]: probation }, suite_version: "2" }, null, 2));
    // Put a `opencode` shim that fails (exit 1) ahead of PATH so discovery reports exhausted,
    // without disturbing the parent's ability to launch `bun`. A probation entry must NOT be
    // selected even though it is the only candidate.
    const binDir = join(tmp, "bin");
    mkdirSync(binDir, { recursive: true });
    writeFileSync(join(binDir, "opencode"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
    const r = bun(["model-select.ts", "--discover-if-empty", "--registry-path", registryPath], { PATH: `${binDir}:${process.env.PATH}` });
    expect(r.rc).toBe(1);
    const out = JSON.parse(r.stdout);
    expect(out.stop).toBe("NO_QUALIFIED_FREE_WORKER");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("CLI positive control: a qualified model → exit 0 + identity", () => {
    const tmp = mkdtempSync(join(tmpdir(), "msel-ok-"));
    const registryPath = join(tmp, "model-registry.json");
    const q = qualifiedEntry("opencode/qualified-free");
    writeFileSync(registryPath, JSON.stringify({ models: { [q.identity]: q }, suite_version: "2" }, null, 2));
    // a qualified model is selected without invoking opencode at all
    const binDir = join(tmp, "bin");
    mkdirSync(binDir, { recursive: true });
    writeFileSync(join(binDir, "opencode"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
    const r = bun(["model-select.ts", "--registry-path", registryPath], { PATH: `${binDir}:${process.env.PATH}` });
    expect(r.rc).toBe(0);
    const out = JSON.parse(r.stdout);
    expect(out.identity).toBe("opencode/qualified-free");
    expect(out.status).toBe("qualified");
    rmSync(tmp, { recursive: true, force: true });
  });
});

// ---------------------------------------------------------------------------
// Finding 2: run-worker descriptor carries an executable launch command
// ---------------------------------------------------------------------------

describe("Finding 2: run-worker emits an executable launch contract", () => {
  function setupWorktree(): { tmp: string; wt: string; plan: string } {
    const tmp = mkdtempSync(join(tmpdir(), "rw-"));
    const wt = join(tmp, "wt-1");
    mkdirSync(wt, { recursive: true });
    mkdirSync(join(wt, ".worker"), { recursive: true });
    const plan = join(tmp, "plan.md");
    writeFileSync(plan, "# plan\nimplement issue 1", "utf-8");
    return { tmp, wt, plan };
  }

  test("buildLaunch produces a noninteractive, model-pinned command with cwd = worktree", () => {
    const { tmp, wt, plan } = setupWorktree();
    const receiptPath = join(wt, ".worker", "receipt.json");
    const launch = buildLaunch({ model: "opencode/free-model", worktree: wt, plan, receiptPath });
    expect(launch.command[0]).toBe("opencode");
    expect(launch.command[1]).toBe("run");
    const modelIdx = launch.command.indexOf("--model");
    expect(modelIdx).toBeGreaterThan(-1);
    expect(launch.command[modelIdx + 1]).toBe("opencode/free-model");
    expect(launch.cwd).toBe(wt);
    const prompt = launch.command[launch.command.length - 1];
    expect(prompt).toContain(receiptPath);
    expect(prompt).toContain("No git push");
    expect(prompt).toContain("It MUST be [] when none were executed");
    expect(prompt).toContain("including untracked files");
    expect(prompt).toContain("test_commands must be objects");
    expect(prompt).toContain('"schema_version":1,"status":"completed"');
    expect(prompt).toContain("schema_version is the NUMBER 1");
    expect(prompt).toContain("do not run a clock command");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("buildLaunch adds --resume when resumeFrom is set (same worker session)", () => {
    const { tmp, wt, plan } = setupWorktree();
    const launch = buildLaunch({ model: "opencode/free-model", worktree: wt, plan, receiptPath: join(wt, ".worker", "receipt.json"), resumeFrom: "consult-7" });
    const resumeIdx = launch.command.indexOf("--session");
    expect(resumeIdx).toBeGreaterThan(-1);
    expect(launch.command[resumeIdx + 1]).toBe("consult-7");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("CLI: writes a job descriptor WITH launch + receipt_path into the queue", () => {
    const { tmp, wt, plan } = setupWorktree();
    const queueDir = join(tmp, "worker-queue");
    const r = bun(
      ["run-worker.ts", "--issue", "1", "--worker-id", "worker-1", "--model", "opencode/free-model",
       "--worktree", wt, "--plan", plan,
       "--expected-generation", "0",
       "--state-path", join(tmp, "assign-state.json"),
       "--worker-registry-path", join(tmp, "worker-registry.json")],
      // This test exercises the launch-descriptor *shape* (Finding 2), not the qualification
      // gate (F5). It deliberately bypasses the qualified-model registry check via the test
      // escape hatch; production leaves LOOP_ALLOW_UNVERIFIED_MODEL unset.
      { LOOP_QUEUE_DIR: queueDir, LOOP_ALLOW_UNVERIFIED_MODEL: "1" },
    );
    expect(r.rc).toBe(0);
    const out = JSON.parse(r.stdout);
    expect(out.delegated).toBe(true);
    expect(out.marker).toMatch(/^worker:worker-1:issue:1:run:/);
    // executable launch command present and model-pinned
    expect(out.launch).toBeDefined();
    expect(out.launch.command[0]).toBe("bun");
    expect(out.launch.command).toContain("opencode");
    expect(out.launch.cwd).toBe(wt);
    expect(out.launch.command).toContain("--model");
    // receipt contract present
    expect(out.receipt_path).toBe(join(wt, ".worker", "receipt.json"));

    // the queue file was actually written and carries the same launch contract
    const queued = readdirSync(queueDir).filter(f => f.endsWith(".json"));
    expect(queued.length).toBe(1);
    const job = JSON.parse(readFileSync(join(queueDir, queued[0]), "utf-8"));
    expect(job.launch.command[0]).toBe("bun");
    expect(job.launch.command).toContain("opencode");
    expect(job.launch.command).toContain("--model");
    expect(job.receipt_path).toBe(join(wt, ".worker", "receipt.json"));
    rmSync(tmp, { recursive: true, force: true });
  });
});

// ---------------------------------------------------------------------------
// Finding 3: land.ts realDeps reads/mutates real CAS + registry, real scope (fail-closed)
// ---------------------------------------------------------------------------

describe("Finding 3: scope-check lib is fail-closed and glob-correct", () => {
  test("matchGlob: exact, *, ** segment, and nested", () => {
    expect(matchGlob("crates/a/src.rs", "crates/a/src.rs")).toBe(true);
    expect(matchGlob("crates/a/src.rs", "crates/*/src.rs")).toBe(true);
    expect(matchGlob("crates/a/b/src.rs", "crates/**/src.rs")).toBe(true);
    expect(matchGlob("crates/a/b/src.rs", "crates/**/*.rs")).toBe(true);
    expect(matchGlob("web/x.ts", "crates/**")).toBe(false);
    expect(matchGlob("crates/a.rs", "crates/*.ts")).toBe(false);
  });

  test("checkFilesScope: empty allowed → fail-closed (all files out of scope)", () => {
    const r = checkFilesScope(["crates/a.rs"], []);
    expect(r.ok).toBe(false);
    expect(r.outOfScope).toEqual(["crates/a.rs"]);
  });

  test("checkFilesScope: all within declared globs → ok", () => {
    const r = checkFilesScope(["crates/a/src.rs", "crates/a/b/mod.rs"], ["crates/**"]);
    expect(r.ok).toBe(true);
    expect(r.outOfScope).toEqual([]);
  });

  test("checkFilesScope: any file outside → fail with outOfScope list", () => {
    const r = checkFilesScope(["crates/a.rs", "web/app.tsx"], ["crates/**"]);
    expect(r.ok).toBe(false);
    expect(r.outOfScope).toEqual(["web/app.tsx"]);
  });

  test("checkScope: missing manifest → fail-closed", () => {
    const tmp = mkdtempSync(join(tmpdir(), "scope-"));
    mkdirSync(join(tmp, ".worker"), { recursive: true });
    const r = checkScope(tmp, ["crates/a.rs"]);
    expect(r.ok).toBe(false);
    expect(loadScopeManifest(tmp)).toBeNull();
    rmSync(tmp, { recursive: true, force: true });
  });

  test("checkScope: manifest present and covering → ok", () => {
    const tmp = mkdtempSync(join(tmpdir(), "scope-ok-"));
    mkdirSync(join(tmp, ".worker"), { recursive: true });
    writeFileSync(join(tmp, ".worker", "scope.json"), JSON.stringify({ allowed_paths: ["crates/**"] }), "utf-8");
    const r = checkScope(tmp, ["crates/a.rs", "crates/b/c.rs"]);
    expect(r.ok).toBe(true);
    rmSync(tmp, { recursive: true, force: true });
  });
});

describe("Finding 3: land path revalidates real CAS / scope and mutates real state", () => {
  test("closed CAS state → closed-issue regardless of matching generation", async () => {
    const tmp = mkdtempSync(join(tmpdir(), "land-closed-"));
    const statePath = join(tmp, "assign-state.json");
    writeFileSync(statePath, JSON.stringify({ issues: {} }));
    let st = readState(statePath);
    const a = tryAssign(st, { issue: 5, workerId: "worker-1", expectedGeneration: 0, now });
    if (a.ok) st = a.state;
    st = markClosed(st, 5, now);
    writeState(st, statePath);
    const cur = readState(statePath).issues["5"];
    expect(cur.closed).toBe(true);
    const deps: LandDeps = {
      readCas: async (issue) => {
        const s = readState(statePath).issues[String(issue)];
        return { generation: s?.generation ?? 0, closed: s?.closed ?? false, worker_id: s?.worker_id ?? null };
      },
      checkScope: async () => ({ ok: true, detail: "ok" }),
      completeDiff: async () => ({ all: ["crates/a.rs"] }),
      rebase: async () => ({ ok: true, stderr: "" }),
      abortRebase: async () => {},
      runCi: async () => ({ ok: true, stderr: "" }),
      push: async () => ({ ok: true, stderr: "" }),
      markClosed: async () => {},
      releaseWorker: async () => {},
      now: () => Date.now(),
    };
    const req: LandRequest = {
      issue: 5, workerId: "worker-1", worktree: "/wt", expectedGeneration: cur.generation,
      receipt: { status: "completed", model: { provider: "opencode", model: "free" }, commit_sha: "abc", changed_files: ["crates/a.rs"] },
    };
    const outcome = await revalidate(req, deps);
    expect(outcome).toBe("closed-issue");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("real scope check fail-closed → scope-drift", async () => {
    const tmp = mkdtempSync(join(tmpdir(), "land-scope-"));
    const wt = join(tmp, "wt-1");
    mkdirSync(join(wt, ".worker"), { recursive: true });
    // NO scope.json → real scope check is fail-closed
    const deps: LandDeps = {
      readCas: async () => ({ generation: 1, closed: false, worker_id: "worker-1" }),
      checkScope: async (_issue, w) => checkScope(w, ["crates/a.rs", "web/x.ts"]),
      completeDiff: async () => ({ all: ["crates/a.rs", "web/x.ts"] }),
      rebase: async () => ({ ok: true, stderr: "" }),
      abortRebase: async () => {},
      runCi: async () => ({ ok: true, stderr: "" }),
      push: async () => ({ ok: true, stderr: "" }),
      markClosed: async () => {},
      releaseWorker: async () => {},
      now: () => Date.now(),
    };
    const req: LandRequest = {
      issue: 9, workerId: "worker-1", worktree: wt, expectedGeneration: 1,
      receipt: { status: "completed", model: { provider: "opencode", model: "free" }, commit_sha: "abc", changed_files: ["crates/a.rs", "web/x.ts"] },
    };
    const outcome = await revalidate(req, deps);
    expect(outcome).toBe("scope-drift");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("success mutates real CAS (closed) and worker registry (released)", async () => {
    const tmp = mkdtempSync(join(tmpdir(), "land-mutate-"));
    const statePath = join(tmp, "assign-state.json");
    const registryPath = join(tmp, "worker-registry.json");
    const landTarget = join(tmp, "land.lock-target");
    const wt = join(tmp, "wt-1");
    mkdirSync(wt, { recursive: true });
    writeFileSync(landTarget, "x");
    writeFileSync(statePath, JSON.stringify({ issues: {} }));
    writeFileSync(registryPath, JSON.stringify({ workers: {}, max_workers: 3 }));

    let st = readState(statePath);
    const a = tryAssign(st, { issue: 3, workerId: "worker-1", expectedGeneration: 0, now });
    if (a.ok) { st = a.state; writeState(st, statePath); }
    let reg = readRegistry(registryPath);
    reg = registerWorker(reg, "worker-1", "pane-1", wt, "opencode/free");
    reg = assignIssue(reg, "worker-1", 3);
    writeRegistry(reg, registryPath);

    const gen = readState(statePath).issues["3"].generation;
    const deps: LandDeps = {
      readCas: async (issue) => {
        const s = readState(statePath).issues[String(issue)];
        return { generation: s?.generation ?? 0, closed: s?.closed ?? false, worker_id: s?.worker_id ?? null };
      },
      checkScope: async () => ({ ok: true, detail: "ok" }),
      completeDiff: async () => ({ all: ["crates/a.rs"] }),
      rebase: async () => ({ ok: true, stderr: "" }),
      abortRebase: async () => {},
      runCi: async () => ({ ok: true, stderr: "" }),
      push: async () => ({ ok: true, stderr: "" }),
      markClosed: async (issue) => { const b = readState(statePath); writeState(markClosed(b, issue, now), statePath); },
      releaseWorker: async (wid) => { let rr = readRegistry(registryPath); rr = releaseTerminal(rr, wid); writeRegistry(rr, registryPath); },
      now: () => Date.now(),
    };
    const req: LandRequest = {
      issue: 3, workerId: "worker-1", worktree: wt, expectedGeneration: gen,
      receipt: { status: "completed", model: { provider: "opencode", model: "free" }, commit_sha: "abc", changed_files: ["crates/a.rs"] },
    };
    const r = await landOnce(req, deps, landTarget);
    expect(r.outcome).toBe("success");
    // CAS now closed (mutated — not a no-op)
    expect(readState(statePath).issues["3"].closed).toBe(true);
    // worker released to idle (mutated — not a no-op)
    expect(readRegistry(registryPath).workers["worker-1"].state).toBe("idle");
    expect(readRegistry(registryPath).workers["worker-1"].current_issue).toBeNull();
    rmSync(tmp, { recursive: true, force: true });
  });
});
