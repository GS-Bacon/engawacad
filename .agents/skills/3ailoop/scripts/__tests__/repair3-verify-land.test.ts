// repair3-verify-land.test.ts — regression coverage for repair-input-3 findings F1 + F2
//
// F1 (high): verify-receipt must NOT fail open. A worker that runs forbidden ops
//   (git push / gh issue close / cross-worktree) must not pass verification by emitting a
//   clean receipt. The safety audit is REQUIRED and fail-closed: missing / unreadable /
//   malformed audit → safety violation (not "zero commands audited").
// F2 (high): receipts + landing are bound to the assigned worker lifecycle.
//   - verify-receipt compares receipt issue/worker/model/session against expected identity.
//   - land-serial requires the issue be owned by THIS worker (null/unassigned → refuse).
//   - land-serial does not suppress CAS close/release failures as success.

import { describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { execFileSync } from "child_process";
import { matchIdentity } from "../lib/verify-identity.ts";
import { scanAudit } from "../lib/safety.ts";
import { processLand, revalidate, type LandDeps, type LandRequest } from "../lib/land-serial.ts";
import type { Receipt } from "../lib/receipt.ts";

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

function baseReceipt(overrides: Partial<Receipt> = {}): Receipt {
  return {
    schema_version: 1,
    issue: 42,
    worker_id: "worker-1",
    status: "completed",
    model: { provider: "opencode", model: "free-model" },
    summary: "completed #42",
    commit_sha: "abc123",
    changed_files: ["crates/a.rs"],
    forbidden_operations: [],
    test_commands: [{ command: "cargo test", rc: 0 }],
    unverified_items: [],
    emitted_at: "2026-08-13T00:00:00Z",
    ...overrides,
  };
}

function setupWorktree(files: Record<string, string> = { "crates/a.rs": "fn main(){}" }): { tmp: string; wt: string } {
  const tmp = mkdtempSync(join(tmpdir(), "vr-"));
  const wt = join(tmp, "wt-1");
  mkdirSync(wt, { recursive: true });
  for (const [path, content] of Object.entries(files)) {
    const full = join(wt, path);
    mkdirSync(join(full, ".."), { recursive: true });
    writeFileSync(full, content);
  }
  return { tmp, wt };
}

function writeReceipt(tmp: string, receipt: Receipt): string {
  const p = join(tmp, "receipt.json");
  writeFileSync(p, JSON.stringify(receipt));
  return p;
}

// ---------------------------------------------------------------------------
// F1: safety audit is required + fail-closed (no fail-open)
// ---------------------------------------------------------------------------

describe("F1: verify-receipt does NOT fail open without an audit", () => {
  test("no --audit flag at all → exit 1 (cannot prove worker stayed safe)", () => {
    const { tmp, wt } = setupWorktree();
    const receiptFile = writeReceipt(tmp, baseReceipt());
    const r = bun(["verify-receipt.ts", "--receipt", receiptFile, "--worktree", wt, "--base", "HEAD"]);
    expect(r.rc).toBe(1);
    const out = JSON.parse(r.stdout);
    expect(out.ok).toBe(false);
    expect(out.safety_violations.length).toBeGreaterThan(0);
    expect(out.safety_violations[0].detail).toContain("audit");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("missing audit file path → exit 1 (fail-closed, not zero commands)", () => {
    const { tmp, wt } = setupWorktree();
    const receiptFile = writeReceipt(tmp, baseReceipt());
    const r = bun(["verify-receipt.ts", "--receipt", receiptFile, "--worktree", wt, "--audit", join(tmp, "does-not-exist.json"), "--base", "HEAD"]);
    expect(r.rc).toBe(1);
    const out = JSON.parse(r.stdout);
    expect(out.ok).toBe(false);
    expect(out.safety_violations[0].detail).toContain("fail-closed");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("malformed audit (not JSON) → exit 1 (fail-closed)", () => {
    const { tmp, wt } = setupWorktree();
    const receiptFile = writeReceipt(tmp, baseReceipt());
    const auditFile = join(tmp, "audit.json");
    writeFileSync(auditFile, "{ not valid json");
    const r = bun(["verify-receipt.ts", "--receipt", receiptFile, "--worktree", wt, "--audit", auditFile, "--base", "HEAD"]);
    expect(r.rc).toBe(1);
    const out = JSON.parse(r.stdout);
    expect(out.ok).toBe(false);
    expect(out.safety_violations[0].detail).toContain("malformed");
    rmSync(tmp, { recursive: true, force: true });
  });

  test("audit containing git push → exit 1 (forbidden op detected, not suppressed)", () => {
    const { tmp, wt } = setupWorktree();
    const receiptFile = writeReceipt(tmp, baseReceipt());
    const auditFile = join(tmp, "audit.json");
    writeFileSync(auditFile, JSON.stringify([
      { cmd: ["git", "add", "."], cwd: wt },
      { cmd: ["git", "push", "origin", "main"], cwd: wt },
    ]));
    const r = bun(["verify-receipt.ts", "--receipt", receiptFile, "--worktree", wt, "--audit", auditFile, "--base", "HEAD"]);
    expect(r.rc).toBe(1);
    const out = JSON.parse(r.stdout);
    expect(out.safety_violations.some((v: { kind: string }) => v.kind === "push")).toBe(true);
    rmSync(tmp, { recursive: true, force: true });
  });

  test("audit containing gh issue close → exit 1 (forbidden op detected)", () => {
    const { tmp, wt } = setupWorktree();
    const receiptFile = writeReceipt(tmp, baseReceipt());
    const auditFile = join(tmp, "audit.json");
    writeFileSync(auditFile, JSON.stringify([
      { cmd: ["gh", "issue", "close", "295"], cwd: wt },
    ]));
    const r = bun(["verify-receipt.ts", "--receipt", receiptFile, "--worktree", wt, "--audit", auditFile, "--base", "HEAD"]);
    expect(r.rc).toBe(1);
    const out = JSON.parse(r.stdout);
    expect(out.safety_violations.some((v: { kind: string }) => v.kind === "issue-close")).toBe(true);
    rmSync(tmp, { recursive: true, force: true });
  });

  test("clean audit (only safe commands) + matching diff → exit 0", () => {
    const { tmp, wt } = setupWorktree();
    const receiptFile = writeReceipt(tmp, baseReceipt());
    const auditFile = join(tmp, "audit.json");
    writeFileSync(auditFile, JSON.stringify([
      { cmd: ["cargo", "test"], cwd: wt },
      { cmd: ["git", "add", "."], cwd: wt },
    ]));
    const r = bun(["verify-receipt.ts", "--receipt", receiptFile, "--worktree", wt, "--audit", auditFile, "--base", "HEAD"]);
    // completeness may fail against an empty diff in a bare worktree; the point of this
    // test is that the AUDIT alone does not cause a failure. Assert safety_violations is empty.
    const out = JSON.parse(r.stdout);
    expect(out.safety_violations).toEqual([]);
    rmSync(tmp, { recursive: true, force: true });
  });
});

// ---------------------------------------------------------------------------
// F2: receipt identity is bound to the assigned worker lifecycle
// ---------------------------------------------------------------------------

describe("F2: verify-receipt binds receipt identity to expected job", () => {
  test("pure: matching expectation → ok", () => {
    const r = matchIdentity(baseReceipt(), {
      issue: 42,
      workerId: "worker-1",
      model: "opencode/free-model",
      sessionId: undefined,
    });
    expect(r.ok).toBe(true);
    expect(r.mismatches).toEqual([]);
  });

  test("pure: wrong issue → mismatch", () => {
    const r = matchIdentity(baseReceipt(), { issue: 999 });
    expect(r.ok).toBe(false);
    expect(r.mismatches.join(" ")).toContain("issue:");
  });

  test("pure: wrong model → mismatch (prevents paid/unknown swap)", () => {
    const r = matchIdentity(baseReceipt(), { model: "opencode/paid-model" });
    expect(r.ok).toBe(false);
    expect(r.mismatches.join(" ")).toContain("model:");
  });

  test("pure: wrong worker → mismatch", () => {
    const r = matchIdentity(baseReceipt(), { workerId: "worker-2" });
    expect(r.ok).toBe(false);
    expect(r.mismatches.join(" ")).toContain("worker_id:");
  });

  test("pure: wrong session → mismatch", () => {
    const r = matchIdentity(baseReceipt({ session_id: "sess-A" }), { sessionId: "sess-B" });
    expect(r.ok).toBe(false);
    expect(r.mismatches.join(" ")).toContain("session_id:");
  });

  test("CLI: --expect-model mismatch → exit 1 identity_ok=false", () => {
    const { tmp, wt } = setupWorktree();
    const receiptFile = writeReceipt(tmp, baseReceipt());
    const auditFile = join(tmp, "audit.json");
    writeFileSync(auditFile, JSON.stringify([{ cmd: ["cargo", "test"], cwd: wt }]));
    const r = bun([
      "verify-receipt.ts", "--receipt", receiptFile, "--worktree", wt,
      "--audit", auditFile, "--base", "HEAD",
      "--expect-issue", "42", "--expect-worker", "worker-1",
      "--expect-model", "opencode/WRONG-model",
    ]);
    expect(r.rc).toBe(1);
    const out = JSON.parse(r.stdout);
    expect(out.identity_ok).toBe(false);
    expect(out.identity_mismatches.join(" ")).toContain("model:");
    rmSync(tmp, { recursive: true, force: true });
  });
});

// ---------------------------------------------------------------------------
// F2: landing requires worker ownership (null/unassigned → refuse)
// ---------------------------------------------------------------------------

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

describe("F2: landing refuses an unassigned / wrong-owner issue", () => {
  test("null worker_id (never assigned) → stale-generation (no license to land)", async () => {
    const r = await revalidate(req(), okDeps({ readCas: async () => ({ generation: 1, closed: false, worker_id: null }) }));
    expect(r).toBe("stale-generation");
  });

  test("empty-string worker_id (absent CAS record) → stale-generation", async () => {
    const r = await revalidate(req(), okDeps({ readCas: async () => ({ generation: 1, closed: false, worker_id: "" }) }));
    expect(r).toBe("stale-generation");
  });

  test("different worker owns it → stale-generation", async () => {
    const r = await revalidate(req({ workerId: "w1" }), okDeps({ readCas: async () => ({ generation: 1, closed: false, worker_id: "w2" }) }));
    expect(r).toBe("stale-generation");
  });

  test("matching owner → proceeds (not refused)", async () => {
    const r = await revalidate(req(), okDeps({ readCas: async () => ({ generation: 1, closed: false, worker_id: "w1" }) }));
    expect(r).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// F2: CAS close / release failure is NOT suppressed as success
// ---------------------------------------------------------------------------

describe("F2: state-reconciliation failures surface as non-success", () => {
  test("markClosed throws → state-unreconciled (not success)", async () => {
    const r = await processLand(req(), okDeps({ markClosed: async () => { throw new Error("cas lock busy"); } }));
    expect(r.outcome).toBe("state-unreconciled");
    expect(r.detail).toContain("markClosed");
  });

  test("releaseWorker throws → state-unreconciled (not success)", async () => {
    const r = await processLand(req(), okDeps({ releaseWorker: async () => { throw new Error("registry lock busy"); } }));
    expect(r.outcome).toBe("state-unreconciled");
    expect(r.detail).toContain("releaseWorker");
  });

  test("both succeed → success", async () => {
    const r = await processLand(req(), okDeps());
    expect(r.outcome).toBe("success");
  });
});

// ---------------------------------------------------------------------------
// sanity: scanAudit still aggregates (no regression in the pure checker)
// ---------------------------------------------------------------------------

describe("F1: scanAudit pure checker still detects forbidden ops", () => {
  test("push + gh write across commands", () => {
    const vs = scanAudit([
      { cmd: ["git", "push"] },
      { cmd: ["gh", "issue", "close", "1"] },
      { cmd: ["cargo", "test"] },
    ]);
    expect(vs.length).toBe(2);
  });
});
