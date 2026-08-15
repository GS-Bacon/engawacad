import { describe, expect, test } from "bun:test";
import { shouldRetryScreenAttempt } from "../model-eval-screen.ts";
import { applyQualificationResult, validateScreenReport } from "../model-eval-qualify.ts";
import type { Eligible } from "../lib/discover.ts";
import type { ModelEntry, ModelRegistry } from "../lib/model-registry.ts";

const now = "2026-08-14T12:00:00.000Z";

function candidate(): Eligible {
  return {
    identity: "opencode/test-free",
    model: { id: "test-free", providerID: "opencode", name: "Test", status: "active", cost: { input: 0, output: 0 }, capabilities: { toolcall: true } },
    evidence: { status: "active", input_cost: 0, output_cost: 0, toolcall: true, checked_at: now },
  };
}

function entry(): ModelEntry {
  return {
    identity: "opencode/test-free", harness: "opencode", provider: "opencode", model: "test-free", status: "qualified",
    evaluation: { suite_version: "2", passed: 5, total: 5, passed_classes: ["bounded-rust-bugfix"], tested_at: "2026-08-13T00:00:00Z" },
    supported_task_class: "general",
    task_profile: { max_scope: "medium", max_primary_touchpoints: 6, allowed_classes: ["general"], ambiguity: "bounded", required_negative_tests: true, starting_context: "standard" },
    free_tier: { status: "active", input_cost: 0, output_cost: 0, toolcall: true, checked_at: now, expiry: null, expiry_known: false },
    observations: { successes: 3, failures: 1, rate_limits: 0, last_success_at: now, last_failure_at: null },
    last_evaluated_at: "2026-08-13T00:00:00Z",
  };
}

describe("evaluation retry policy", () => {
  test("never retries a rate limit immediately", () => {
    expect(shouldRetryScreenAttempt({ infrastructure_failure: true, infrastructure_kind: "rate-limit" })).toBe(false);
    expect(shouldRetryScreenAttempt({ infrastructure_failure: true, infrastructure_kind: "upstream" })).toBe(true);
    expect(shouldRetryScreenAttempt({ infrastructure_failure: false, infrastructure_kind: null })).toBe(false);
  });
});

describe("qualification registry integrity", () => {
  test("inconclusive infrastructure does not erase a qualified result", () => {
    const current = entry();
    const registry: ModelRegistry = { suite_version: "2", models: { [current.identity]: current } };
    const next = applyQualificationResult(registry, candidate(), { outcome: "inconclusive", infrastructure_kind: "upstream" }, now);
    expect(next.models[current.identity]).toEqual(registry.models[current.identity]);
  });

  test("rate-limit evidence increments observations without resetting qualification", () => {
    const current = entry();
    const registry: ModelRegistry = { suite_version: "2", models: { [current.identity]: current } };
    const next = applyQualificationResult(registry, candidate(), { outcome: "inconclusive", infrastructure_kind: "rate-limit" }, now);
    expect(next.models[current.identity].status).toBe("qualified");
    expect(next.models[current.identity].observations.rate_limits).toBe(1);
    expect(next.models[current.identity].evaluation).toEqual(current.evaluation);
  });

  test("a real rejection demotes but preserves production observations", () => {
    const current = entry();
    const registry: ModelRegistry = { suite_version: "2", models: { [current.identity]: current } };
    const result = { outcome: "rejected", qualified: false, duration_ms: 100, tasks: [] };
    const next = applyQualificationResult(registry, candidate(), result, now);
    expect(next.models[current.identity].status).toBe("probation");
    expect(next.models[current.identity].observations).toEqual(current.observations);
  });

  test("suite v3 stores assisted-delivery cost without activating the registry early", () => {
    const current = entry();
    const registry: ModelRegistry = { suite_version: "2", models: { [current.identity]: current } };
    const tasks = [
      { task_class: "bounded-rust-bugfix", passed: true, forbidden_ops: [], scope_drift: false, required_tests_passed: true, receipt_valid: true, consultations_used: 0, completed_after_consultation: false },
      { task_class: "multi-crate-feature", passed: true, forbidden_ops: [], scope_drift: false, required_tests_passed: true, receipt_valid: true, consultations_used: 1, completed_after_consultation: true },
      { task_class: "diagnose-regression", passed: true, forbidden_ops: [], scope_drift: false, required_tests_passed: true, receipt_valid: true, consultations_used: 0, completed_after_consultation: false },
      { task_class: "skill-compliance", passed: true, forbidden_ops: [], scope_drift: false, required_tests_passed: true, receipt_valid: true, consultations_used: 0, completed_after_consultation: false },
    ];
    const result = { outcome: "qualified", qualified: true, duration_ms: 100, tasks, verdict: { consultations_used: 1, tasks_completed_after_consultation: 1 } };
    const next = applyQualificationResult(registry, candidate(), result, now);
    expect(next.suite_version).toBe("2");
    expect(next.models[current.identity].evaluation).toMatchObject({ suite_version: "3", operating_mode: "assisted", consultations_used: 1 });
  });

  test("refuses legacy screen evidence", () => {
    expect(() => validateScreenReport({ version: 4, stage: "screen", results: [] })).toThrow("version 5");
    expect(() => validateScreenReport({ version: 5, stage: "screen", results: [] })).not.toThrow();
  });
});
