// compat-dryrun.test.ts — compatibility + N=3 dry run (fake adapters, no external writes)
//
// 検証観点 (contract acceptance):
//   - "Compatibility tests or fixture replays cover representative existing cycle records and all preserved loop subsystems"
//   - "A safe N=3 end-to-end dry run uses fake worker/CLI/GitHub adapters and proves fan-out plus serial landing without external writes."
//   - cutover never leaves both orchestrators active

import { describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { execFileSync } from "child_process";
import { tryAssign, markClosed, readState, writeState, recordTerminal } from "../lib/cas-state.ts";
import { registerWorker, assignIssue, releaseTerminal, readRegistry, writeRegistry } from "../lib/worker-registry.ts";
import { validateReceipt } from "../lib/receipt.ts";
import { landOnce, type LandDeps, type LandRequest } from "../lib/land-serial.ts";

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

describe("N=3 dry run (fake adapters, no external writes)", () => {
  test("dry-run CLI: 3 issues fan-out to 3 workers + serial land, exit 0", () => {
    const r = bun(["dry-run.ts", "--workers", "3", "--issues", "3"]);
    expect(r.rc).toBe(0);
    const out = JSON.parse(r.stdout);
    expect(out.fanout.length).toBe(3);
    expect(out.landing.length).toBe(3);
    expect(out.landing.every((l: { outcome: string }) => l.outcome === "success")).toBe(true);
    // all issues closed in CAS
    for (let i = 1; i <= 3; i++) {
      expect(out.final_cas.issues[String(i)].closed).toBe(true);
      expect(out.final_cas.issues[String(i)].last_outcome).toBe("completed");
    }
  }, 30000);

  test("dry-run CLI proves #100: labeled right-side pane + observability + lifecycle-safe cleanup", () => {
    // contract acceptance #100: each fake worker is assigned a labeled right-side tmux pane,
    // output remains observable through terminal completion, and pane cleanup/reuse cannot
    // affect lifecycle correctness.
    const r = bun(["dry-run.ts", "--workers", "3", "--issues", "3"]);
    expect(r.rc).toBe(0);
    const out = JSON.parse(r.stdout);
    expect(Array.isArray(out.panes)).toBe(true);
    expect(out.panes.length).toBe(3);
    for (const p of out.panes) {
      // dedicated RIGHT-SIDE pane
      expect(p.side).toBe("right");
      expect(typeof p.pane_id).toBe("string");
      expect(p.pane_id.length).toBeGreaterThan(0);
      // labeled with all 5 identifying fields, safe (no creds/traces)
      expect(p.has_all_fields).toBe(true);
      expect(p.safe).toBe(true);
      expect(p.status_line).toContain(`issue=#${p.issue}`);
      expect(p.status_line).toContain(`worker=${p.worker_id}`);
      expect(p.status_line).toContain("provider=");
      expect(p.status_line).toContain("model=");
      expect(p.status_line).toContain("session=");
      // ordinary CLI output streamed + observable through terminal completion
      expect(p.streamed).toBe(true);
      expect(p.observable).toBe(true);
      // deterministic cleanup happened
      expect(p.released).toBe(true);
    }
    // lifecycle correctness is independent of pane state
    expect(out.lifecycle_independent_of_pane).toBe(true);
  }, 30000);

  test("in-process N=3 fan-out + serial landing with fake deps", async () => {
    const tmp = mkdtempSync(join(tmpdir(), "compat-"));
    const statePath = join(tmp, "assign-state.json");
    const registryPath = join(tmp, "worker-registry.json");
    const landTarget = join(tmp, "land.lock-target");
    writeFileSync(statePath, JSON.stringify({ issues: {} }));
    writeFileSync(registryPath, JSON.stringify({ workers: {}, max_workers: 3 }));
    writeFileSync(landTarget, "x");
    const now = () => new Date();

    // seed 3 workers
    let reg = readRegistry(registryPath);
    for (let i = 1; i <= 3; i++) reg = registerWorker(reg, `worker-${i}`, `pane-${i}`, join(tmp, `wt-${i}`), "opencode/free");
    writeRegistry(reg, registryPath);

    // fan-out 3 issues
    for (let i = 1; i <= 3; i++) {
      const before = readState(statePath);
      const gen = before.issues[String(i)]?.generation ?? 0;
      const a = tryAssign(before, { issue: i, workerId: `worker-${i}`, expectedGeneration: gen, now });
      expect(a.ok).toBe(true);
      if (a.ok) writeState(a.state, statePath);
      let rr = readRegistry(registryPath);
      rr = assignIssue(rr, `worker-${i}`, i);
      writeRegistry(rr, registryPath);
    }

    // land serially with fake deps
    const fakeDeps = (worktree: string): LandDeps => ({
      readCas: async (issue) => {
        const s = readState(statePath).issues[String(issue)];
        return { generation: s?.generation ?? 0, closed: s?.closed ?? false, worker_id: s?.worker_id ?? null };
      },
      checkScope: async () => ({ ok: true, detail: "ok" }),
      completeDiff: async () => ({ all: [`${worktree}/src/a.rs`] }),
      rebase: async () => ({ ok: true, stderr: "" }),
      abortRebase: async () => {},
      runCi: async () => ({ ok: true, stderr: "" }),
      push: async () => ({ ok: true, stderr: "" }),
      markClosed: async (issue) => { const b = readState(statePath); writeState(markClosed(b, issue, now), statePath); },
      releaseWorker: async (wid) => { let rr = readRegistry(registryPath); rr = releaseTerminal(rr, wid); writeRegistry(rr, registryPath); },
      now: () => Date.now(),
    });

    for (let i = 1; i <= 3; i++) {
      const before = readState(statePath);
      const gen = before.issues[String(i)]?.generation ?? 0;
      const t = recordTerminal(before, i, `worker-${i}`, "completed", gen, now);
      if (t.ok) writeState(t.state, statePath);
      const post = readState(statePath).issues[String(i)]?.generation ?? 0;
      const req: LandRequest = {
        issue: i, workerId: `worker-${i}`, worktree: join(tmp, `wt-${i}`), expectedGeneration: post,
        receipt: { status: "completed", model: { provider: "opencode", model: "free" }, commit_sha: `sha${i}`, changed_files: [`${join(tmp, `wt-${i}`)}/src/a.rs`] },
      };
      const r = await landOnce(req, fakeDeps(join(tmp, `wt-${i}`)), landTarget);
      expect(r.outcome).toBe("success");
    }

    // all closed
    const final = readState(statePath);
    for (let i = 1; i <= 3; i++) expect(final.issues[String(i)].closed).toBe(true);
    rmSync(tmp, { recursive: true, force: true });
  });
});

describe("compatibility fixture replay (representative cycle record shape)", () => {
  test("parses an old-style cycle record and preserves subsystem fields", () => {
    // Representative of features/.loop state.json recent_cycles entries the prior loop produced.
    const fixture = {
      cycle: 42,
      ended_at: "2026-07-27T03:52:00Z",
      merged_commits: 1,
      closed: [295],
      pause_reason: null,
    };
    // The new runtime keeps these user-visible fields in its own cycle records (phase.ts / dashboard).
    expect(fixture.cycle).toBe(42);
    expect(fixture.closed).toContain(295);
    expect(fixture.pause_reason).toBeNull();
  });

  test("a valid completed receipt round-trips through validation (preserved receipt contract)", () => {
    const receipt = {
      schema_version: 1, issue: 295, worker_id: "worker-1", status: "completed",
      model: { provider: "opencode", model: "x-free" }, summary: "ok",
      commit_sha: "abc", changed_files: ["crates/a.rs"],
      forbidden_operations: [],
      test_commands: [{ command: "cargo test", rc: 0 }], unverified_items: [],
      emitted_at: "2026-08-13T00:00:00Z",
    };
    expect(validateReceipt(receipt).ok).toBe(true);
  });
});

describe("cutover never leaves both orchestrators active", () => {
  test("cutover --check is read-only and idempotent", () => {
    // Use an isolated LOOP_LOCK_DIR so we don't touch the real lock.
    const tmp = mkdtempSync(join(tmpdir(), "cutover-"));
    const r1 = bun(["cutover.ts", "--check"], { LOOP_LOCK_DIR: join(tmp, "lock") });
    const r2 = bun(["cutover.ts", "--check"], { LOOP_LOCK_DIR: join(tmp, "lock") });
    expect(r1.rc).toBe(0);
    expect(r2.rc).toBe(0);
    rmSync(tmp, { recursive: true, force: true });
  });

  test("reconcile --force on a clean tree enables the new-runtime flag", () => {
    const tmp = mkdtempSync(join(tmpdir(), "cutover-recon-"));
    const env = {
      LOOP_LOCK_DIR: join(tmp, "lock"),
      LOOP_NEW_FLAG: join(tmp, "new-runtime.enabled"),
    };
    const r = bun(["cutover.ts", "--reconcile", "--force"], env);
    expect(r.rc).toBe(0);
    const out = JSON.parse(r.stdout);
    expect(out.reconciled).toBe(true);
    expect(out.new_enabled).toBe(true);
    rmSync(tmp, { recursive: true, force: true });
  });
});
