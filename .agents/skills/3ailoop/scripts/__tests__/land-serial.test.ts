// land-serial.test.ts — serialized landing revalidation + bounded outcomes
//
// 検証観点 (contract acceptance):
//   - rebase conflict → bounded rebase-conflict (no destructive recovery)
//   - failing verification/CI → ci-red
//   - stale generation → stale-generation
//   - closed issue → closed-issue
//   - completeness fail → completeness-fail (#330)
//   - serialized landing (file lock mutual exclusion)

import { describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { landOnce, processLand, type LandDeps, type LandRequest } from "../lib/land-serial.ts";

function okDeps(overrides: Partial<LandDeps> = {}): LandDeps {
  return {
    readCas: async () => ({ generation: 1, closed: false, worker_id: "w1" }),
    checkScope: async () => ({ ok: true, detail: "ok" }),
    completeDiff: async () => ({ all: ["crates/a.rs"] }),
    rebase: async () => ({ ok: true, stderr: "" }),
    abortRebase: async () => {},
    runCi: async () => ({ ok: true, stderr: "" }),
    push: async () => ({ ok: true, stderr: "" }),
    markClosed: async () => {},
    releaseWorker: async () => {},
    now: () => 1000,
    ...overrides,
  };
}

function req(overrides: Partial<LandRequest> = {}): LandRequest {
  return {
    issue: 1,
    workerId: "w1",
    worktree: "/wt",
    expectedGeneration: 1,
    receipt: {
      status: "completed",
      model: { provider: "opencode", model: "x-free" },
      commit_sha: "abc",
      changed_files: ["crates/a.rs"],
    },
    ...overrides,
  };
}

describe("revalidation outcomes", () => {
  test("happy path → success", async () => {
    const r = await processLand(req(), okDeps());
    expect(r.outcome).toBe("success");
  });

  test("rebase conflict → rebase-conflict (bounded)", async () => {
    const r = await processLand(req(), okDeps({ rebase: async () => ({ ok: false, stderr: "conflict" }) }));
    expect(r.outcome).toBe("rebase-conflict");
  });

  test("CI red → ci-red", async () => {
    const r = await processLand(req(), okDeps({ runCi: async () => ({ ok: false, stderr: "test failed" }) }));
    expect(r.outcome).toBe("ci-red");
  });

  test("push failed → push-failed", async () => {
    const r = await processLand(req(), okDeps({ push: async () => ({ ok: false, stderr: "rejected" }) }));
    expect(r.outcome).toBe("push-failed");
  });

  test("closed issue → closed-issue", async () => {
    const r = await processLand(req(), okDeps({ readCas: async () => ({ generation: 1, closed: true, worker_id: "w1" }) }));
    expect(r.outcome).toBe("closed-issue");
  });

  test("stale generation → stale-generation", async () => {
    const r = await processLand(req({ expectedGeneration: 999 }), okDeps());
    expect(r.outcome).toBe("stale-generation");
  });

  test("receipt not completed → receipt-invalid", async () => {
    const r = await processLand(req({ receipt: { status: "needs_supervisor", model: { provider: "opencode", model: "x" } } }), okDeps());
    expect(r.outcome).toBe("receipt-invalid");
  });

  test("scope drift → scope-drift", async () => {
    const r = await processLand(req(), okDeps({ checkScope: async () => ({ ok: false, detail: "out of scope" }) }));
    expect(r.outcome).toBe("scope-drift");
  });

  test("completeness fail (#330) → completeness-fail", async () => {
    const r = await processLand(req(), okDeps({ completeDiff: async () => ({ all: ["crates/a.rs", "crates/b.rs"] }) }));
    expect(r.outcome).toBe("completeness-fail");
  });
});

describe("serialized landing (file lock)", () => {
  test("two concurrent lands are serialized (no interleaving)", async () => {
    const tmp = mkdtempSync(join(tmpdir(), "land-serial-"));
    const lockTarget = join(tmp, "land.lock-target");
    writeFileSync(lockTarget, "x");
    let inFlight = 0;
    let maxInFlight = 0;
    // issue-aware CAS so each land revalidates against its own worker ownership
    const slow: LandDeps = {
      ...okDeps(),
      readCas: async (issue) => ({ generation: 1, closed: false, worker_id: issue === 1 ? "w1" : "w2" }),
      runCi: async () => { inFlight++; maxInFlight = Math.max(maxInFlight, inFlight); await new Promise(r => setTimeout(r, 20)); inFlight--; return { ok: true, stderr: "" }; },
    };
    const p1 = landOnce(req({ issue: 1, workerId: "w1" }), slow, lockTarget);
    const p2 = landOnce(req({ issue: 2, workerId: "w2" }), slow, lockTarget);
    const [r1, r2] = await Promise.all([p1, p2]);
    expect(r1.outcome).toBe("success");
    expect(r2.outcome).toBe("success");
    expect(maxInFlight).toBe(1); // never overlapped
    rmSync(tmp, { recursive: true, force: true });
  });
});
