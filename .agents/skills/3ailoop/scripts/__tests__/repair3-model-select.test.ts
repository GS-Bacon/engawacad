// repair3-model-select.test.ts — regression coverage for repair-input-3 finding F5
//
// F5 (high): free-worker qualification and fallback gates can select unqualified or
//   expired models.
//   - model-registry selectQualified must re-check active/cost/toolcall/expiry/rate-limit/
//     needsReevaluation at selection time (a stale `qualified` entry is NOT selectable).
//   - model-select must not hard-code opencodeExhausted:true nor return an external
//     candidate as exit-0 external-probation (it must be qualified first → stop & notify).
//   - run-worker must not launch an arbitrary --model; it must be a qualified+selectable
//     registry entry, and honor the entry's harness.

import { describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync, mkdirSync, readFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { execFileSync } from "child_process";
import {
  readModelRegistry,
  upsertModel,
  selectQualified,
  qualifiedModels,
  selectionBlocker,
  setProbation,
  type ModelEntry,
} from "../lib/model-registry.ts";
import { buildLaunch, launchCommand } from "../run-worker.ts";

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

function qualifiedEntry(identity: string, overrides: Partial<ModelEntry> = {}): ModelEntry {
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
    ...overrides,
  };
}

const now = () => new Date("2026-08-13T00:00:00Z");

// ---------------------------------------------------------------------------
// selectionBlocker re-checks free-tier evidence at selection time
// ---------------------------------------------------------------------------

describe("F5: selectionBlocker re-checks evidence (a stale qualified entry is blocked)", () => {
  test("healthy qualified → null (selectable)", () => {
    expect(selectionBlocker(qualifiedEntry("opencode/a"), now)).toBeNull();
  });

  test("status probation → blocked", () => {
    expect(selectionBlocker(qualifiedEntry("opencode/a", { status: "probation" }), now)).toMatch(/not qualified/);
  });

  test("inactive free-tier → blocked", () => {
    const e = qualifiedEntry("opencode/a");
    e.free_tier = { ...e.free_tier, status: "deprecated" };
    expect(selectionBlocker(e, now)).toMatch(/not active/);
  });

  test("nonzero cost → blocked (not free)", () => {
    const e = qualifiedEntry("opencode/a");
    e.free_tier = { ...e.free_tier, input_cost: 0.001, output_cost: 0 };
    expect(selectionBlocker(e, now)).toMatch(/nonzero cost/);
  });

  test("unknown cost → blocked", () => {
    const e = qualifiedEntry("opencode/a");
    e.free_tier = { ...e.free_tier, input_cost: undefined as unknown as number, output_cost: 0 };
    expect(selectionBlocker(e, now)).toMatch(/cost unknown/);
  });

  test("no tool calling → blocked", () => {
    const e = qualifiedEntry("opencode/a");
    e.free_tier = { ...e.free_tier, toolcall: false };
    expect(selectionBlocker(e, now)).toMatch(/no tool calling/);
  });

  test("expired free period → blocked", () => {
    const e = qualifiedEntry("opencode/a");
    e.free_tier = { ...e.free_tier, expiry: "2026-01-01T00:00:00Z", expiry_known: true };
    expect(selectionBlocker(e, now)).toMatch(/expired/);
  });

  test("quality-degradation (failures > successes) → blocked (needs reeval)", () => {
    const e = qualifiedEntry("opencode/a");
    e.observations = { successes: 0, failures: 3, rate_limits: 0, last_success_at: null, last_failure_at: "2026-08-13T00:00:00Z" };
    expect(selectionBlocker(e, now)).toMatch(/needs re-evaluation/);
  });

  test("rate-limit cluster → blocked (needs reeval)", () => {
    const e = qualifiedEntry("opencode/a");
    e.observations = { successes: 5, failures: 1, rate_limits: 4, last_success_at: "2026-08-13T00:00:00Z", last_failure_at: "2026-08-13T00:00:00Z" };
    expect(selectionBlocker(e, now)).toMatch(/needs re-evaluation/);
  });
});

describe("F5: selectQualified/qualifiedModels skip stale qualified entries", () => {
  test("qualified-but-expired is NOT returned even though status==='qualified'", () => {
    let reg = readModelRegistry("/nonexistent/registry.json");
    reg = upsertModel(reg, qualifiedEntry("opencode/expired", {
      free_tier: { status: "active", input_cost: 0, output_cost: 0, toolcall: true, checked_at: "2026-08-13T00:00:00Z", expiry: "2026-01-01T00:00:00Z", expiry_known: true },
    }));
    expect(selectQualified(reg, now)).toBeNull();
    expect(qualifiedModels(reg, now)).toEqual([]);
  });

  test("qualified-but-paid is NOT returned", () => {
    let reg = readModelRegistry("/nonexistent/registry.json");
    const paid = qualifiedEntry("opencode/paid");
    paid.free_tier = { ...paid.free_tier, input_cost: 0.01, output_cost: 0.02 };
    reg = upsertModel(reg, paid);
    expect(selectQualified(reg, now)).toBeNull();
  });

  test("healthy qualified still returned", () => {
    let reg = readModelRegistry("/nonexistent/registry.json");
    reg = upsertModel(reg, qualifiedEntry("opencode/healthy"));
    expect(selectQualified(reg, now)?.identity).toBe("opencode/healthy");
  });

  test("mix: expired + healthy → only healthy returned", () => {
    let reg = readModelRegistry("/nonexistent/registry.json");
    reg = upsertModel(reg, qualifiedEntry("opencode/expired", {
      free_tier: { status: "active", input_cost: 0, output_cost: 0, toolcall: true, checked_at: "2026-08-13T00:00:00Z", expiry: "2026-01-01T00:00:00Z", expiry_known: true },
    }));
    reg = upsertModel(reg, qualifiedEntry("opencode/healthy"));
    const list = qualifiedModels(reg, now);
    expect(list.length).toBe(1);
    expect(list[0].identity).toBe("opencode/healthy");
  });
});

// ---------------------------------------------------------------------------
// model-select CLI: external candidate is NOT exit-0 (must qualify first)
// ---------------------------------------------------------------------------

describe("F5: model-select external fallback stops & notifies (no exit 0)", () => {
  test("external candidate passing the gate → registered probation, exit 1 NO_QUALIFIED_FREE_WORKER", () => {
    const tmp = mkdtempSync(join(tmpdir(), "msel-ext-"));
    const registryPath = join(tmp, "model-registry.json");
    writeFileSync(registryPath, JSON.stringify({ models: {}, suite_version: "2" }));
    const fallbackFile = join(tmp, "fallback.json");
    const candidate = [{
      identity: "cloudvendor/model-free",
      provider: "cloudvendor",
      model: "model-free",
      harness: "https://api.cloudvendor.example/v1",
      officially_free: true,
      resumable_cli: true,
      paid_automatic_fallback_possible: false,
      toolcall: true,
      cost: { input: 0, output: 0 },
      credentials_present: true,
      terms_accepted: true,
    }];
    writeFileSync(fallbackFile, JSON.stringify(candidate));
    // opencode shim fails → OpenCode genuinely exhausted, so the fallback gate MAY run.
    const binDir = join(tmp, "bin");
    mkdirSync(binDir, { recursive: true });
    writeFileSync(join(binDir, "opencode"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
    const r = bun(["model-select.ts", "--fallback-candidates", fallbackFile, "--registry-path", registryPath], { PATH: `${binDir}:${process.env.PATH}` });
    expect(r.rc).toBe(1);
    const out = JSON.parse(r.stdout);
    // must NOT be returned as a directly-usable worker
    expect(out.identity).toBeUndefined();
    expect(out.stop).toBe("NO_QUALIFIED_FREE_WORKER");
    // the candidate was registered as probation (harness recorded) — not qualified/selected
    const reread = JSON.parse(readFileSync(registryPath, "utf-8"));
    const entry = reread.models["cloudvendor/model-free"];
    expect(entry).toBeDefined();
    expect(entry.status).toBe("probation");
    expect(entry.harness).toBe("https://api.cloudvendor.example/v1");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("OpenCode NOT exhausted (--discover-if-empty finds eligible) → fallback gate does not run", () => {
    // When discovery returns eligible candidates, opencodeExhausted is false and the external
    // fallback must not be consulted at all (policy #4). We assert the model-select pure path:
    // a qualified model is still selected even if a fallback file is present.
    const tmp = mkdtempSync(join(tmpdir(), "msel-noexhaust-"));
    const registryPath = join(tmp, "model-registry.json");
    const q = qualifiedEntry("opencode/qualified-free");
    writeFileSync(registryPath, JSON.stringify({ models: { [q.identity]: q }, suite_version: "2" }));
    const fallbackFile = join(tmp, "fallback.json");
    writeFileSync(fallbackFile, JSON.stringify([{
      identity: "cloudvendor/x", provider: "cloudvendor", model: "x", harness: "https://x/v1",
      officially_free: true, resumable_cli: true, paid_automatic_fallback_possible: false,
      toolcall: true, cost: { input: 0, output: 0 }, credentials_present: true, terms_accepted: true,
    }]));
    const r = bun(["model-select.ts", "--fallback-candidates", fallbackFile, "--registry-path", registryPath]);
    expect(r.rc).toBe(0);
    const out = JSON.parse(r.stdout);
    expect(out.identity).toBe("opencode/qualified-free");
    rmSync(tmp, { recursive: true, force: true });
  });
});

// ---------------------------------------------------------------------------
// run-worker: arbitrary --model is refused unless qualified+selectable
// ---------------------------------------------------------------------------

describe("F5: run-worker refuses an unqualified / expired model", () => {
  function setupWorktree(): { tmp: string; wt: string; plan: string; requirements: string } {
    const tmp = mkdtempSync(join(tmpdir(), "rw-f5-"));
    const wt = join(tmp, "wt-1");
    mkdirSync(wt, { recursive: true });
    mkdirSync(join(wt, ".worker"), { recursive: true });
    const plan = join(tmp, "plan.md");
    writeFileSync(plan, "# plan\nimpl", "utf-8");
    const requirements = join(tmp, "requirements.json");
    writeFileSync(requirements, JSON.stringify({ scope: "small", primary_touchpoints: 2, task_class: "bugfix", ambiguity: "low", negative_tests_required: true }));
    return { tmp, wt, plan, requirements };
  }

  function lifecycleArgs(tmp: string): string[] {
    return [
      "--expected-generation", "0",
      "--state-path", join(tmp, "assign-state.json"),
      "--worker-registry-path", join(tmp, "worker-registry.json"),
    ];
  }

  test("arbitrary --model not in registry → exit 1 model-not-qualified", () => {
    const { tmp, wt, plan, requirements } = setupWorktree();
    const registryPath = join(tmp, "model-registry.json");
    writeFileSync(registryPath, JSON.stringify({ models: {}, suite_version: "2" }));
    const r = bun(
      ["run-worker.ts", "--issue", "1", "--worker-id", "worker-1", "--model", "opencode/anything",
       "--worktree", wt, "--plan", plan, "--registry-path", registryPath],
      { LOOP_QUEUE_DIR: join(tmp, "q"), LOOP_SKIP_ASYNC_SUBMIT: "1" },
    );
    expect(r.rc).toBe(1);
    const out = JSON.parse(r.stdout);
    expect(out.delegated).toBe(false);
    expect(out.error).toBe("model-not-qualified");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("qualified-but-expired model → exit 1 model-not-selectable", () => {
    const { tmp, wt, plan, requirements } = setupWorktree();
    const registryPath = join(tmp, "model-registry.json");
    const expired = qualifiedEntry("opencode/expired", {
      free_tier: { status: "active", input_cost: 0, output_cost: 0, toolcall: true, checked_at: "2026-08-13T00:00:00Z", expiry: "2026-01-01T00:00:00Z", expiry_known: true },
    });
    writeFileSync(registryPath, JSON.stringify({ models: { [expired.identity]: expired }, suite_version: "2" }));
    const r = bun(
      ["run-worker.ts", "--issue", "1", "--worker-id", "worker-1", "--model", "opencode/expired",
       "--worktree", wt, "--plan", plan, "--registry-path", registryPath, "--requirements", requirements],
      { LOOP_QUEUE_DIR: join(tmp, "q"), LOOP_SKIP_ASYNC_SUBMIT: "1" },
    );
    expect(r.rc).toBe(1);
    const out = JSON.parse(r.stdout);
    expect(out.error).toBe("model-not-selectable");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("probation model → exit 1 model-not-selectable", () => {
    const { tmp, wt, plan, requirements } = setupWorktree();
    const registryPath = join(tmp, "model-registry.json");
    let reg = readModelRegistry("/nonexistent/registry.json");
    reg = upsertModel(reg, qualifiedEntry("opencode/prob"));
    reg = setProbation(reg, "opencode/prob", now);
    writeFileSync(registryPath, JSON.stringify(reg));
    const r = bun(
      ["run-worker.ts", "--issue", "1", "--worker-id", "worker-1", "--model", "opencode/prob",
       "--worktree", wt, "--plan", plan, "--registry-path", registryPath],
      { LOOP_QUEUE_DIR: join(tmp, "q") },
    );
    expect(r.rc).toBe(1);
    expect(JSON.parse(r.stdout).error).toBe("model-not-selectable");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("qualified+selectable model → exit 0 delegated, harness from registry", () => {
    const { tmp, wt, plan, requirements } = setupWorktree();
    const registryPath = join(tmp, "model-registry.json");
    const q = qualifiedEntry("opencode/healthy");
    writeFileSync(registryPath, JSON.stringify({ models: { [q.identity]: q }, suite_version: "2" }));
    const r = bun(
      ["run-worker.ts", "--issue", "1", "--worker-id", "worker-1", "--model", "opencode/healthy",
       "--worktree", wt, "--plan", plan, "--registry-path", registryPath, "--requirements", requirements,
       ...lifecycleArgs(tmp)],
      { LOOP_QUEUE_DIR: join(tmp, "q"), LOOP_SKIP_ASYNC_SUBMIT: "1" },
    );
    expect(r.rc).toBe(0);
    const out = JSON.parse(r.stdout);
    expect(out.delegated).toBe(true);
    expect(out.launch.command.slice(0, 2)).toEqual(["bun", expect.stringContaining("opencode-worker-terminal.ts")]);
    expect(out.launch.command).toContain("opencode");
    const assigned = JSON.parse(readFileSync(join(tmp, "assign-state.json"), "utf-8"));
    expect(assigned.issues["1"]).toMatchObject({ status: "assigned", worker_id: "worker-1", generation: 1 });
    const workers = JSON.parse(readFileSync(join(tmp, "worker-registry.json"), "utf-8"));
    expect(workers.workers["worker-1"]).toMatchObject({ state: "busy", current_issue: 1, model_id: "opencode/healthy" });
    rmSync(tmp, { recursive: true, force: true });
  });

  test("production path submits the visible worker command to async delegation", () => {
    const { tmp, wt, plan, requirements } = setupWorktree();
    const registryPath = join(tmp, "model-registry.json");
    const q = qualifiedEntry("opencode/healthy");
    writeFileSync(registryPath, JSON.stringify({ models: { [q.identity]: q }, suite_version: "2" }));
    const fakeDelegate = join(tmp, "delegate.py");
    writeFileSync(fakeDelegate, `import json,sys\nassert sys.argv[1] == "submit"\nassert "visible-command.ts" in " ".join(sys.argv)\nprint(json.dumps({"marker":"[agent-delegation:11111111-1111-1111-1111-111111111111]","job_id":"j1","job_file":"/tmp/j1"}))\n`);
    const r = bun(
      ["run-worker.ts", "--issue", "1", "--worker-id", "worker-1", "--model", "opencode/healthy",
       "--worktree", wt, "--plan", plan, "--registry-path", registryPath, "--requirements", requirements,
       ...lifecycleArgs(tmp)],
      { LOOP_QUEUE_DIR: join(tmp, "q"), CODEX_ASYNC_DELEGATE: fakeDelegate },
    );
    expect(r.rc).toBe(0);
    const out = JSON.parse(r.stdout);
    expect(out.marker).toBe("[agent-delegation:11111111-1111-1111-1111-111111111111]");
    expect(out.delegation_job_id).toBe("j1");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("async submit failure releases CAS and worker registry", () => {
    const { tmp, wt, plan, requirements } = setupWorktree();
    const registryPath = join(tmp, "model-registry.json");
    const q = qualifiedEntry("opencode/healthy");
    writeFileSync(registryPath, JSON.stringify({ models: { [q.identity]: q }, suite_version: "2" }));
    const fakeDelegate = join(tmp, "delegate.py");
    writeFileSync(fakeDelegate, "raise SystemExit(9)\n");
    const r = bun(
      ["run-worker.ts", "--issue", "1", "--worker-id", "worker-1", "--model", "opencode/healthy",
       "--worktree", wt, "--plan", plan, "--registry-path", registryPath, "--requirements", requirements,
       ...lifecycleArgs(tmp)],
      { LOOP_QUEUE_DIR: join(tmp, "q"), CODEX_ASYNC_DELEGATE: fakeDelegate },
    );
    expect(r.rc).toBe(1);
    expect(JSON.parse(r.stdout).error).toBe("async-submit-failed");
    const assigned = JSON.parse(readFileSync(join(tmp, "assign-state.json"), "utf-8"));
    expect(assigned.issues["1"]).toMatchObject({ status: "unassigned", worker_id: null, last_outcome: "failed" });
    const workers = JSON.parse(readFileSync(join(tmp, "worker-registry.json"), "utf-8"));
    expect(workers.workers["worker-1"]).toMatchObject({ state: "idle", current_issue: null });
    rmSync(tmp, { recursive: true, force: true });
  });
});

// ---------------------------------------------------------------------------
// launchCommand honors a non-opencode harness (qualified external resumable CLI)
// ---------------------------------------------------------------------------

describe("F5: launchCommand honors the qualified harness, not a hard-coded opencode", () => {
  test("opencode harness → opencode run --model", () => {
    const cmd = launchCommand("opencode", "opencode/free", null, "PROMPT");
    expect(cmd.slice(0, 4)).toEqual(["opencode", "run", "--pure", "--format"]);
    expect(cmd).toContain("opencode/free");
    expect(cmd[cmd.length - 1]).toBe("PROMPT");
  });

  test("external harness → prefix split + --model pin", () => {
    const cmd = launchCommand("extcli --api free-endpoint", "extv/model-free", null, "PROMPT");
    expect(cmd.slice(0, 3)).toEqual(["extcli", "--api", "free-endpoint"]);
    const modelIdx = cmd.indexOf("--model");
    expect(cmd[modelIdx + 1]).toBe("extv/model-free");
    expect(cmd[cmd.length - 1]).toBe("PROMPT");
  });

  test("buildLaunch with harness=opencode still pins model + cwd", () => {
    const tmp = mkdtempSync(join(tmpdir(), "bl-"));
    const wt = join(tmp, "wt");
    mkdirSync(wt, { recursive: true });
    const plan = join(tmp, "plan.md");
    writeFileSync(plan, "plan body");
    const launch = buildLaunch({ model: "opencode/free", worktree: wt, plan, receiptPath: join(wt, "r.json"), harness: "opencode" });
    expect(launch.command[0]).toBe("opencode");
    expect(launch.cwd).toBe(wt);
    rmSync(tmp, { recursive: true, force: true });
  });
});
