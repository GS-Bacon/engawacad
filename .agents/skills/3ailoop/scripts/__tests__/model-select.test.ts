// model-select.test.ts — no-free-worker stop + model registry lifecycle
//
// 検証観点 (contract acceptance):
//   - no free worker → stop & notify (policy #6, never Codex/paid)
//   - qualification + registry lifecycle (qualified/probation/retired)
//   - retirement report written

import { describe, expect, test } from "bun:test";
import {
  readModelRegistry,
  upsertModel,
  selectQualified,
  recordObservation,
  retire,
  setProbation,
  needsReevaluation,
  type ModelEntry,
} from "../lib/model-registry.ts";
import { runFallbackGate, type ExternalCandidate } from "../lib/external-fallback.ts";

const now = () => new Date("2026-08-13T00:00:00Z");

function entry(identity: string, status: "qualified" | "probation" | "retired" = "qualified"): ModelEntry {
  return {
    identity,
    harness: "opencode",
    provider: identity.split("/")[0],
    model: identity.split("/")[1],
    status,
    evaluation: { suite_version: "2", passed: 2, total: 3, passed_classes: ["bounded-rust-bugfix", "multi-crate-feature"], tested_at: "2026-08-13T00:00:00Z" },
    supported_task_class: "general",
    free_tier: { status: "active", input_cost: 0, output_cost: 0, toolcall: true, checked_at: "2026-08-13T00:00:00Z", expiry: null, expiry_known: false },
    observations: { successes: 0, failures: 0, rate_limits: 0, last_success_at: null, last_failure_at: null },
    last_evaluated_at: "2026-08-13T00:00:00Z",
  };
}

describe("no-free-worker stop (policy #6)", () => {
  test("empty registry → no qualified model → caller must stop & notify", () => {
    const reg = readModelRegistry("/nonexistent/registry.json");
    expect(selectQualified(reg)).toBeNull();
  });

  test("only retired/probation models → no qualified selection", () => {
    let reg = readModelRegistry("/nonexistent/registry.json");
    reg = upsertModel(reg, entry("opencode/x", "retired"));
    reg = upsertModel(reg, entry("opencode/y", "probation"));
    expect(selectQualified(reg)).toBeNull();
  });

  test("qualified model present → selected", () => {
    let reg = readModelRegistry("/nonexistent/registry.json");
    reg = upsertModel(reg, entry("opencode/z", "qualified"));
    expect(selectQualified(reg)?.identity).toBe("opencode/z");
  });
});

describe("registry lifecycle", () => {
  test("observations accumulate", () => {
    let reg = readModelRegistry("/nonexistent/registry.json");
    reg = upsertModel(reg, entry("opencode/a"));
    reg = recordObservation(reg, "opencode/a", "success", now);
    reg = recordObservation(reg, "opencode/a", "success", now);
    expect(reg.models["opencode/a"].observations.successes).toBe(2);
  });

  test("probation demotion", () => {
    let reg = readModelRegistry("/nonexistent/registry.json");
    reg = upsertModel(reg, entry("opencode/b"));
    reg = setProbation(reg, "opencode/b", now);
    expect(reg.models["opencode/b"].status).toBe("probation");
  });

  test("retirement writes a report", () => {
    let reg = readModelRegistry("/nonexistent/registry.json");
    reg = upsertModel(reg, entry("opencode/c"));
    const { registry, report } = retire(reg, "opencode/c", "free period ended", now);
    expect(registry.models["opencode/c"].status).toBe("retired");
    expect(report).not.toBeNull();
    expect(report!.replacement_reason).toBe("free period ended");
  });
});

describe("reevaluation gating (policy #7)", () => {
  test("expired free period → needs reeval", () => {
    const e = entry("opencode/d");
    e.free_tier = { ...e.free_tier, expiry: "2026-01-01T00:00:00Z", expiry_known: true };
    expect(needsReevaluation(e, now)).toBe("expired");
  });

  test("quality degradation (failures > successes) → needs reeval", () => {
    const e = entry("opencode/e");
    e.observations = { successes: 0, failures: 3, rate_limits: 0, last_success_at: null, last_failure_at: "2026-08-13T00:00:00Z" };
    expect(needsReevaluation(e, now)).toBe("quality-degradation");
  });

  test("healthy model → no reeval (no recurring discovery)", () => {
    const e = entry("opencode/f");
    expect(needsReevaluation(e, now)).toBeNull();
  });

  test("old evaluation suite → harness re-evaluation", () => {
    const e = entry("opencode/old-suite");
    e.evaluation.suite_version = "1";
    expect(needsReevaluation(e, now)).toBe("harness-failure");
  });
});

describe("external fallback gate (policy #4/#5)", () => {
  test("cannot enter fallback while OpenCode set is NOT exhausted", () => {
    const r = runFallbackGate({ opencodeExhausted: false, candidates: [] });
    expect(r.mayEnterFallback).toBe(false);
  });

  test("valid external candidate chosen when OpenCode exhausted", () => {
    const c: ExternalCandidate = {
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
    };
    const r = runFallbackGate({ opencodeExhausted: true, candidates: [c] });
    expect(r.mayEnterFallback).toBe(true);
    expect(r.chosen?.identity).toBe("cloudvendor/model-free");
  });

  test("missing credentials → stop for user (no bypass)", () => {
    const c: ExternalCandidate = {
      identity: "cloudvendor/x",
      provider: "cloudvendor",
      model: "x",
      harness: "https://api.cloudvendor.example/v1",
      officially_free: true,
      resumable_cli: true,
      paid_automatic_fallback_possible: false,
      toolcall: true,
      cost: { input: 0, output: 0 },
      credentials_present: false,
      terms_accepted: true,
    };
    const r = runFallbackGate({ opencodeExhausted: true, candidates: [c] });
    expect(r.chosen).toBeNull();
    expect(r.stopForUser).toContain("credentials missing");
  });

  test("paid fallback possible → rejected", () => {
    const c: ExternalCandidate = {
      identity: "router/x",
      provider: "router",
      model: "x",
      harness: "https://router.example/v1",
      officially_free: true,
      resumable_cli: true,
      paid_automatic_fallback_possible: true,
      toolcall: true,
      cost: { input: 0, output: 0 },
      credentials_present: true,
      terms_accepted: true,
    };
    const r = runFallbackGate({ opencodeExhausted: true, candidates: [c] });
    expect(r.chosen).toBeNull();
    expect(r.rejected[0].reason).toContain("paid/automatic fallback");
  });
});
