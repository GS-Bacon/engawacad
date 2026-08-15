#!/usr/bin/env bun
// model-registry.ts — qualified free cloud model 台帳 (new runtime)
//
// contract §"Evaluation and registry" #4 が定義する最低保持項目:
//   harness/provider/model, status (qualified|probation|retired), evaluation suite version/results,
//   supported task class, free-tier evidence and check date, known/unknown expiry,
//   recent success/failure/rate-limit observations.
//
// contract §"Free-model policy" #6/#7/#8:
//   - qualified free worker が無ければ stop & notify (Codex/paid に黙って降りない)。
//   - 再評価は初期 + その後は expiry / auth 失敗 / rate limit / harness 持続失敗 / 品質劣化 時のみ (定期週次 discovery しない)。
//   - retirement 時は 簡潔な report (dates/evidence, eval/production outcomes, failures/rate limits, replacement reason) を記録。
//
// 設計:
// - features/.loop/model-registry.json (1 file)
// - 全 write は withFileLockSync + tmpfile+rename atomic
// - pure logic + side-effectful mutate に分離 (DI test 可能)
//
// 関連: contract §"Free-model policy", §"Evaluation and registry"

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { appendFileSync } from "fs";
import { withFileLockSync } from "./file-lock.ts";

const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
const DEFAULT_REGISTRY_PATH = resolve(REPO_ROOT, "features/.loop/model-registry.json");
const DEFAULT_REPORT_PATH = resolve(REPO_ROOT, "features/.loop/model-retirements.jsonl");
export const CURRENT_MODEL_SUITE_VERSION = "3";

export type ModelStatus = "qualified" | "probation" | "retired";

export interface EvaluationResult {
  suite_version: string;
  passed: number;
  total: number;
  /** which task classes the model passed: bugfix | multi-crate | diagnose | skill-compliance | supervisor. */
  passed_classes: string[];
  tested_at: string;
  /** Total wall-clock time for the fixed suite. Used only as a deterministic tie-breaker. */
  duration_ms?: number;
  /** Codex consultation is a cost signal after delivery success, never a standalone score. */
  consultations_used?: number;
  tasks_completed_after_consultation?: number;
  operating_mode?: "autonomous" | "assisted";
}

export interface FreeTierEvidence {
  status: string;
  input_cost: number;
  output_cost: number;
  toolcall: boolean;
  checked_at: string;
  /** known expiry of the free period, or null/unknown. */
  expiry: string | null;
  expiry_known: boolean;
}

export interface Observations {
  successes: number;
  failures: number;
  rate_limits: number;
  last_success_at: string | null;
  last_failure_at: string | null;
}

export type SliceScope = "tiny" | "small" | "medium";
export type WorkerTaskClass = "bugfix" | "multi-crate" | "diagnose" | "focused-test" | "general";

/** Evaluation-derived operating envelope. Safety rules never vary by model. */
export interface TaskProfile {
  max_scope: SliceScope;
  max_primary_touchpoints: number;
  allowed_classes: WorkerTaskClass[];
  ambiguity: "consult" | "bounded";
  required_negative_tests: boolean;
  starting_context: "minimal" | "standard";
}

export interface SliceRequirements {
  scope: SliceScope;
  primary_touchpoints: number;
  task_class: WorkerTaskClass;
  ambiguity: "low" | "material-choice";
  negative_tests_required: boolean;
}

export interface ModelEntry {
  /** canonical "provider/model" identity. */
  identity: string;
  harness: string;
  provider: string;
  model: string;
  status: ModelStatus;
  evaluation: EvaluationResult;
  supported_task_class: string;
  free_tier: FreeTierEvidence;
  observations: Observations;
  /** Short, evaluation-derived usage profile; never a model-specific procedure list. */
  task_profile: TaskProfile;
  /** ISO of when the entry was last re-evaluated (policy #7). */
  last_evaluated_at: string;
}

export interface ModelRegistry {
  models: Record<string, ModelEntry>;
  /** suite version the registry currently trusts. */
  suite_version: string;
}

export interface RetirementReport {
  identity: string;
  retired_at: string;
  free_period: { from: string | null; to: string | null };
  evidence: FreeTierEvidence;
  evaluation_summary: string;
  production_summary: string;
  failures_and_rate_limits: Observations;
  replacement_reason: string;
}

function emptyRegistry(): ModelRegistry {
  return { models: {}, suite_version: CURRENT_MODEL_SUITE_VERSION };
}

function validStatus(s: unknown): ModelStatus {
  return s === "qualified" || s === "probation" || s === "retired" ? s : "probation";
}

export function conservativeTaskProfile(): TaskProfile {
  return {
    max_scope: "small",
    max_primary_touchpoints: 4,
    allowed_classes: ["bugfix", "focused-test"],
    ambiguity: "consult",
    required_negative_tests: true,
    starting_context: "minimal",
  };
}

function readTaskProfile(raw: unknown): TaskProfile {
  if (!raw || typeof raw !== "object") return conservativeTaskProfile();
  const r = raw as Partial<TaskProfile>;
  const scopes: SliceScope[] = ["tiny", "small", "medium"];
  const classes: WorkerTaskClass[] = ["bugfix", "multi-crate", "diagnose", "focused-test", "general"];
  return {
    max_scope: scopes.includes(r.max_scope as SliceScope) ? r.max_scope as SliceScope : "small",
    max_primary_touchpoints: Number.isInteger(r.max_primary_touchpoints) && Number(r.max_primary_touchpoints) > 0
      ? Number(r.max_primary_touchpoints) : 4,
    allowed_classes: Array.isArray(r.allowed_classes)
      ? r.allowed_classes.filter((x): x is WorkerTaskClass => classes.includes(x as WorkerTaskClass))
      : ["bugfix", "focused-test"],
    ambiguity: r.ambiguity === "bounded" ? "bounded" : "consult",
    required_negative_tests: r.required_negative_tests !== false,
    starting_context: r.starting_context === "standard" ? "standard" : "minimal",
  };
}

export function readModelRegistry(path: string = DEFAULT_REGISTRY_PATH): ModelRegistry {
  if (!existsSync(path)) return emptyRegistry();
  try {
    const text = readFileSync(path, "utf-8");
    const obj = JSON.parse(text) as Partial<ModelRegistry>;
    const models: Record<string, ModelEntry> = {};
    if (obj.models && typeof obj.models === "object") {
      for (const [identity, raw] of Object.entries(obj.models)) {
        const r = raw as Partial<ModelEntry>;
        if (typeof r?.identity !== "string") continue;
        models[identity] = {
          identity: r.identity,
          harness: typeof r.harness === "string" ? r.harness : "opencode",
          provider: typeof r.provider === "string" ? r.provider : r.identity.split("/")[0] ?? "unknown",
          model: typeof r.model === "string" ? r.model : r.identity.split("/")[1] ?? "unknown",
          status: validStatus(r.status),
          evaluation: r.evaluation as EvaluationResult,
          supported_task_class: typeof r.supported_task_class === "string" ? r.supported_task_class : "general",
          free_tier: r.free_tier as FreeTierEvidence,
          observations: (r.observations as Observations) ?? {
            successes: 0, failures: 0, rate_limits: 0, last_success_at: null, last_failure_at: null,
          },
          task_profile: readTaskProfile(r.task_profile),
          last_evaluated_at: typeof r.last_evaluated_at === "string" ? r.last_evaluated_at : "",
        };
      }
    }
    return {
      models,
      suite_version: typeof obj.suite_version === "string" ? obj.suite_version : CURRENT_MODEL_SUITE_VERSION,
    };
  } catch {
    return emptyRegistry();
  }
}

export function writeModelRegistry(reg: ModelRegistry, path: string = DEFAULT_REGISTRY_PATH): void {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, JSON.stringify(reg, null, 2), "utf-8");
  renameSync(tmp, path);
}

// --- pure transitions ---

/** Upsert a model entry (newly evaluated, or re-evaluated). */
export function upsertModel(reg: ModelRegistry, entry: ModelEntry): ModelRegistry {
  return {
    models: { ...reg.models, [entry.identity]: entry },
    suite_version: reg.suite_version,
  };
}

/** Record an observation (success/failure/rate_limit). Bumps counters + timestamps. */
export function recordObservation(
  reg: ModelRegistry,
  identity: string,
  kind: "success" | "failure" | "rate_limit",
  now: () => Date,
): ModelRegistry {
  const cur = reg.models[identity];
  if (!cur) return reg;
  const obs = { ...cur.observations };
  if (kind === "success") {
    obs.successes += 1;
    obs.last_success_at = now().toISOString();
  } else if (kind === "failure") {
    obs.failures += 1;
    obs.last_failure_at = now().toISOString();
  } else {
    obs.rate_limits += 1;
    obs.last_failure_at = now().toISOString();
  }
  return {
    models: { ...reg.models, [identity]: { ...cur, observations: obs } },
    suite_version: reg.suite_version,
  };
}

/** Demote a model to probation (e.g. after observed quality degradation). */
export function setProbation(reg: ModelRegistry, identity: string, now: () => Date): ModelRegistry {
  const cur = reg.models[identity];
  if (!cur) return reg;
  return {
    models: { ...reg.models, [identity]: { ...cur, status: "probation", last_evaluated_at: now().toISOString() } },
    suite_version: reg.suite_version,
  };
}

/** Retire a model and produce a retirement report (policy #8). */
export function retire(
  reg: ModelRegistry,
  identity: string,
  replacementReason: string,
  now: () => Date,
): { registry: ModelRegistry; report: RetirementReport | null } {
  const cur = reg.models[identity];
  if (!cur) return { registry: reg, report: null };
  const retiredAt = now().toISOString();
  const next: ModelEntry = { ...cur, status: "retired", last_evaluated_at: retiredAt };
  const report: RetirementReport = {
    identity,
    retired_at: retiredAt,
    free_period: {
      from: cur.free_tier.checked_at,
      to: cur.free_tier.expiry ?? retiredAt,
    },
    evidence: cur.free_tier,
    evaluation_summary: `suite v${cur.evaluation.suite_version}: ${cur.evaluation.passed}/${cur.evaluation.total} (${cur.evaluation.passed_classes.join(",") || "none"})`,
    production_summary: `${cur.observations.successes} succ / ${cur.observations.failures} fail / ${cur.observations.rate_limits} rate-limited`,
    failures_and_rate_limits: cur.observations,
    replacement_reason: replacementReason,
  };
  return {
    registry: { models: { ...reg.models, [identity]: next }, suite_version: reg.suite_version },
    report,
  };
}

/** Append a retirement report to the JSONL log (best-effort). */
export function appendRetirementReport(report: RetirementReport, path: string = DEFAULT_REPORT_PATH): void {
  mkdirSync(dirname(path), { recursive: true });
  appendFileSync(path, JSON.stringify(report) + "\n", "utf-8");
}

// --- selection helpers ---

/** Re-verify a stored entry's free-tier evidence AT SELECTION TIME.
 *  The registry is a cache; the live model may have expired, gone inactive, started
 *  charging, lost tool calling, or be flagged for re-evaluation. A model that was
 *  `qualified` at evaluation time must still look free/active/uncosted/tool-capable and
 *  must not be due for re-evaluation, otherwise it is NOT selectable right now (F5).
 *  Pure. Returns null when still selectable, or a reason string when it must be skipped. */
export function selectionBlocker(entry: ModelEntry, now: () => Date, trustedSuiteVersion?: string): string | null {
  if (entry.status !== "qualified") {
    return `status=${entry.status} (not qualified)`;
  }
  const ft = entry.free_tier;
  if (!ft) return "no free-tier evidence";
  // active status
  if (ft.status !== "active") return `free-tier status=${ft.status} (not active)`;
  // zero cost (input AND output)
  if (typeof ft.input_cost !== "number" || typeof ft.output_cost !== "number") {
    return "cost unknown (not free)";
  }
  if (ft.input_cost !== 0 || ft.output_cost !== 0) return "nonzero cost (not free)";
  // tool calling
  if (!ft.toolcall) return "no tool calling";
  // expiry
  if (ft.expiry_known && ft.expiry) {
    if (now().getTime() >= new Date(ft.expiry).getTime()) return "free period expired";
  }
  // rate-limit cluster (recent failures / rate limits → needs re-eval before selection)
  const reeval = needsReevaluation(entry, now, trustedSuiteVersion ?? entry.evaluation.suite_version);
  if (reeval) return `needs re-evaluation (${reeval})`;
  return null;
}

/** All qualified models that are STILL selectable at selection time (free-tier evidence
 *  re-checked). Probation/retired models are excluded; qualified-but-now-expired/-inactive/
 *  -uncosted/-rate-limited models are also excluded (F5). Sorted by observed successes. */
export function qualifiedModels(reg: ModelRegistry, now: () => Date = () => new Date()): ModelEntry[] {
  return Object.values(reg.models)
    .filter(m => selectionBlocker(m, now, reg.suite_version) === null)
    .sort((a, b) =>
      b.evaluation.passed - a.evaluation.passed ||
      (a.evaluation.consultations_used ?? 0) - (b.evaluation.consultations_used ?? 0) ||
      (a.evaluation.duration_ms ?? Number.MAX_SAFE_INTEGER) - (b.evaluation.duration_ms ?? Number.MAX_SAFE_INTEGER) ||
      a.identity.localeCompare(b.identity)
    );
}

/** Pick the best still-selectable qualified model, or null if none (→ caller must stop &
 *  notify, policy #6). Re-checks free-tier evidence so a stale-but-`qualified` entry that
 *  has expired / gone paid / lost tool calling / hit a rate-limit cluster is NOT returned. */
export function selectQualified(reg: ModelRegistry, now: () => Date = () => new Date()): ModelEntry | null {
  return qualifiedModels(reg, now)[0] ?? null;
}

const SCOPE_RANK: Record<SliceScope, number> = { tiny: 0, small: 1, medium: 2 };

/** Check only capability fit. Authority, cost, expiry and safety remain common gates. */
export function profileMismatch(profile: TaskProfile, req: SliceRequirements): string[] {
  const reasons: string[] = [];
  if (SCOPE_RANK[req.scope] > SCOPE_RANK[profile.max_scope]) reasons.push(`scope ${req.scope} exceeds ${profile.max_scope}`);
  if (req.primary_touchpoints > profile.max_primary_touchpoints) reasons.push(`touchpoints ${req.primary_touchpoints} exceeds ${profile.max_primary_touchpoints}`);
  if (!profile.allowed_classes.includes("general") && !profile.allowed_classes.includes(req.task_class)) reasons.push(`task class ${req.task_class} not qualified`);
  if (req.ambiguity === "material-choice" && profile.ambiguity !== "bounded") reasons.push("material choice requires bounded-ambiguity profile");
  if (req.negative_tests_required && !profile.required_negative_tests) reasons.push("negative-test capability not established");
  return reasons;
}

export function selectQualifiedForSlice(
  reg: ModelRegistry,
  req: SliceRequirements,
  now: () => Date = () => new Date(),
): ModelEntry | null {
  return qualifiedModels(reg, now).find(entry => profileMismatch(entry.task_profile, req).length === 0) ?? null;
}

export function parseSliceRequirements(raw: unknown): SliceRequirements {
  if (!raw || typeof raw !== "object") throw new Error("slice requirements must be an object");
  const r = raw as Partial<SliceRequirements>;
  if (!(["tiny", "small", "medium"] as unknown[]).includes(r.scope)) throw new Error("invalid slice scope");
  if (!Number.isInteger(r.primary_touchpoints) || Number(r.primary_touchpoints) < 1) throw new Error("primary_touchpoints must be a positive integer");
  if (!(["bugfix", "multi-crate", "diagnose", "focused-test", "general"] as unknown[]).includes(r.task_class)) throw new Error("invalid task_class");
  if (r.ambiguity !== "low" && r.ambiguity !== "material-choice") throw new Error("invalid ambiguity");
  if (typeof r.negative_tests_required !== "boolean") throw new Error("negative_tests_required must be boolean");
  return r as SliceRequirements;
}

// --- re-evaluation gating (policy #7) ---

export type ReevalReason =
  | "initial"
  | "expired"
  | "auth-failure"
  | "rate-limited"
  | "harness-failure"
  | "quality-degradation";

/** Decide whether a model needs re-evaluation per policy #7. Pure. */
export function needsReevaluation(entry: ModelEntry, now: () => Date, trustedSuiteVersion: string = CURRENT_MODEL_SUITE_VERSION): ReevalReason | null {
  if (trustedSuiteVersion && entry.evaluation?.suite_version !== trustedSuiteVersion) return "harness-failure";
  // expired free period
  if (entry.free_tier.expiry_known && entry.free_tier.expiry) {
    if (now().getTime() >= new Date(entry.free_tier.expiry).getTime()) {
      return "expired";
    }
  }
  // Suite v3 requalification is an evidence checkpoint: preserve lifetime counters for reports,
  // but do not let failures that predate a successful v3 evaluation block the model forever.
  // Legacy suites keep their historical aggregate behavior until they are re-evaluated on v3.
  const obs = entry.observations;
  const failureAt = obs.last_failure_at ? new Date(obs.last_failure_at).getTime() : Number.NEGATIVE_INFINITY;
  const evaluatedAt = entry.last_evaluated_at ? new Date(entry.last_evaluated_at).getTime() : Number.NEGATIVE_INFINITY;
  const unresolvedObservation = entry.evaluation.suite_version !== "3" || failureAt >= evaluatedAt;
  if (unresolvedObservation && obs.failures >= 2 && obs.failures > obs.successes) {
    return "quality-degradation";
  }
  // rate-limit cluster
  if (unresolvedObservation && obs.rate_limits >= 3) {
    return "rate-limited";
  }
  return null;
}

// --- side-effectful RMW ---

export function mutateModelRegistry(
  path: string,
  fn: (reg: ModelRegistry) => ModelRegistry,
): ModelRegistry {
  return withFileLockSync(path, () => {
    const cur = readModelRegistry(path);
    const next = fn(cur);
    writeModelRegistry(next, path);
    return next;
  });
}

export const MODEL_REGISTRY_PATHS = {
  DEFAULT_REGISTRY_PATH,
  DEFAULT_REPORT_PATH,
};
