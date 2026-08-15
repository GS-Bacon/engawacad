// receipt.test.ts — receipt validation + consultation/resume fields
//
// 検証観点 (contract acceptance):
//   - receipt validation (schema, status-specific requirements)
//   - completed requires commit_sha + changed_files
//   - needs_supervisor requires consultation { question, options>=2, recommendation }
//   - forbidden_operations non-empty → fail

import { describe, expect, test } from "bun:test";
import { validateReceipt } from "../lib/receipt.ts";
import { validateHarnessReceipt } from "../lib/worker-contract.ts";

function base(overrides: Record<string, unknown>) {
  return {
    schema_version: 1,
    issue: 1,
    worker_id: "worker-1",
    status: "completed",
    model: { provider: "opencode", model: "x-free" },
    summary: "did the thing",
    commit_sha: "abc123",
    changed_files: ["crates/foo.rs"],
    forbidden_operations: [],
    test_commands: [{ command: "cargo test", rc: 0 }],
    unverified_items: [],
    emitted_at: "2026-08-13T00:00:00Z",
    ...overrides,
  };
}

describe("receipt validation", () => {
  test("valid completed receipt", () => {
    expect(validateReceipt(base({})).ok).toBe(true);
  });

  test("completed without commit_sha → invalid", () => {
    const v = validateReceipt(base({ commit_sha: undefined }));
    expect(v.ok).toBe(false);
    expect(v.errors.some(e => e.field === "commit_sha")).toBe(true);
  });

  test("completed without changed_files → invalid", () => {
    const v = validateReceipt(base({ changed_files: undefined }));
    expect(v.ok).toBe(false);
    expect(v.errors.some(e => e.field === "changed_files")).toBe(true);
  });

  test("bad status → invalid", () => {
    const v = validateReceipt(base({ status: "oops" }));
    expect(v.ok).toBe(false);
  });

  test("forbidden_operations non-empty → invalid (hard fail)", () => {
    const v = validateReceipt(base({ forbidden_operations: ["git push"] }));
    expect(v.ok).toBe(false);
    expect(v.errors.some(e => e.field === "forbidden_operations")).toBe(true);
  });

  test("legacy evaluation test_command is rejected by the production contract", () => {
    const legacy = base({ test_commands: undefined, test_command: "cargo test" });
    const v = validateHarnessReceipt(legacy, "opencode/x-free", "ses_eval");
    expect(v.validation.ok).toBe(false);
    expect(v.validation.errors.some(e => e.field === "test_commands")).toBe(true);
  });
});

describe("consultation / resume (needs_supervisor)", () => {
  test("needs_supervisor requires consultation", () => {
    const v = validateReceipt(base({ status: "needs_supervisor", commit_sha: undefined, changed_files: undefined }));
    expect(v.ok).toBe(false);
    expect(v.errors.some(e => e.field === "consultation")).toBe(true);
  });

  test("needs_supervisor with full consultation is valid", () => {
    const v = validateReceipt(base({
      status: "needs_supervisor",
      commit_sha: undefined,
      changed_files: undefined,
      consultation: {
        question: "Which error type?",
        options: ["thiserror", "anyhow"],
        recommendation: "thiserror",
      },
    }));
    expect(v.ok).toBe(true);
  });

  test("consultation options < 2 → invalid", () => {
    const v = validateReceipt(base({
      status: "needs_supervisor",
      commit_sha: undefined,
      changed_files: undefined,
      consultation: { question: "q", options: ["only one"], recommendation: "only one" },
    }));
    expect(v.ok).toBe(false);
  });
});
