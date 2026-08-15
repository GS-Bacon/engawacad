import { describe, expect, test } from "bun:test";
import { deriveTaskProfile, task } from "../lib/qualification.ts";
import {
  conservativeTaskProfile,
  parseSliceRequirements,
  profileMismatch,
  selectQualifiedForSlice,
  type ModelEntry,
  type ModelRegistry,
} from "../lib/model-registry.ts";

function model(identity: string, profile = conservativeTaskProfile()): ModelEntry {
  return {
    identity, harness: "opencode", provider: "opencode", model: identity.split("/")[1], status: "qualified",
    evaluation: { suite_version: "2", passed: 5, total: 5, passed_classes: [], tested_at: "2026-08-13T00:00:00Z" },
    supported_task_class: "general", task_profile: profile,
    free_tier: { status: "active", input_cost: 0, output_cost: 0, toolcall: true, checked_at: "2026-08-13T00:00:00Z", expiry: null, expiry_known: false },
    observations: { successes: 0, failures: 0, rate_limits: 0, last_success_at: null, last_failure_at: null },
    last_evaluated_at: "2026-08-13T00:00:00Z",
  };
}

describe("evaluation-derived task profiles", () => {
  test("full fixed-suite evidence permits medium bounded work", () => {
    const profile = deriveTaskProfile([
      task("bounded-rust-bugfix", { passed: true }), task("multi-crate-feature", { passed: true }),
      task("diagnose-regression", { passed: true }), task("skill-compliance", { passed: true }),
      task("supervisor-resume", { passed: true }),
    ]);
    expect(profile.max_scope).toBe("medium");
    expect(profile.allowed_classes).toContain("multi-crate");
    expect(profile.ambiguity).toBe("bounded");
    expect(profile.required_negative_tests).toBe(true);
  });

  test("partial evidence yields a conservative profile", () => {
    const profile = deriveTaskProfile([
      task("bounded-rust-bugfix", { passed: true }), task("multi-crate-feature", { passed: false }),
      task("diagnose-regression", { passed: false }), task("skill-compliance", { passed: true }),
      task("supervisor-resume", { passed: true }),
    ]);
    expect(profile.max_scope).toBe("small");
    expect(profile.allowed_classes).toEqual(["bugfix", "focused-test"]);
    expect(profile.required_negative_tests).toBe(false);
  });
});

describe("slice fit", () => {
  const smallBug = parseSliceRequirements({ scope: "small", primary_touchpoints: 3, task_class: "bugfix", ambiguity: "low", negative_tests_required: true });

  test("common safety does not become model-specific; only capability fit varies", () => {
    expect(profileMismatch(conservativeTaskProfile(), smallBug)).toEqual([]);
    expect(profileMismatch(conservativeTaskProfile(), { ...smallBug, scope: "medium" })).toContain("scope medium exceeds small");
  });

  test("selection skips the nominally best model when its profile does not fit", () => {
    const narrow = model("opencode/narrow");
    narrow.evaluation.passed = 5;
    const broad = model("opencode/broad", { ...conservativeTaskProfile(), max_scope: "medium", max_primary_touchpoints: 6, allowed_classes: ["general"], ambiguity: "bounded" });
    broad.evaluation.passed = 4;
    const reg: ModelRegistry = { suite_version: "2", models: { [narrow.identity]: narrow, [broad.identity]: broad } };
    const req = { ...smallBug, scope: "medium" as const, task_class: "multi-crate" as const };
    expect(selectQualifiedForSlice(reg, req, () => new Date("2026-08-13T00:00:00Z"))?.identity).toBe(broad.identity);
  });

  test("invalid requirements fail closed", () => {
    expect(() => parseSliceRequirements({ scope: "huge" })).toThrow();
  });
});
