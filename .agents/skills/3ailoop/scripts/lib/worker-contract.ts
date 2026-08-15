// worker-contract.ts — production と model evaluation で共有する worker 契約

import { validateReceipt, type Receipt, type ValidationResult } from "./receipt.ts";

export const PRODUCTION_CONSULTATION_BUDGET = 2;
export const EVALUATION_CONSULTATION_BUDGET = 1;

export function receiptInstructions(
  receiptPath: string,
  options: { allowConsultation?: boolean; consultationBudget?: number } = {},
): string[] {
  const lines = [
    `When finished, write a receipt (schema_version 1) as JSON to: ${receiptPath}`,
    `Receipt status must be one of: completed | needs_supervisor | failed | rate_limited.`,
    `Include schema_version=1, summary, forbidden_operations, test_commands, unverified_items, and emitted_at.`,
    `forbidden_operations means operations you ACTUALLY EXECUTED. It MUST be [] when none were executed.`,
    `test_commands must be objects: [{"command":"...","rc":0,"output_summary":"..."}].`,
    `Immediately before a completed receipt, inspect git status including untracked files.`,
    `For completed: include commit_sha + changed_files covering the FULL diff. Remove unintended generated files before the local commit/receipt.`,
    `Use this exact completed shape (replace example values):`,
    `{"schema_version":1,"status":"completed","summary":"...","commit_sha":"<40-hex local commit>","changed_files":["path/from/repo/root"],"forbidden_operations":[],"test_commands":[{"command":"cargo test","rc":0,"output_summary":"..."}],"unverified_items":[],"emitted_at":"1970-01-01T00:00:00Z"}`,
    `schema_version is the NUMBER 1. The harness replaces emitted_at and binds issue/worker/model/session, so do not invent those identity fields or run a clock command.`,
  ];
  if (options.allowConsultation) {
    const budget = options.consultationBudget ?? PRODUCTION_CONSULTATION_BUDGET;
    lines.push(
      `$ask-codex is an OPTIONAL capability, not a required phase. Use it only when a material decision remains after inspecting local evidence and a wrong choice would cause meaningful rework.`,
      `Do not ask Codex about receipt formatting, commands, or facts you can inspect locally. You have at most ${budget} consultation turn(s) for this task.`,
      `To ask, stop safely with status="needs_supervisor". Work-in-progress edits are allowed but must remain inside scope; do not push or land.`,
      `Use this exact consultation shape:`,
      `{"schema_version":1,"status":"needs_supervisor","summary":"...","forbidden_operations":[],"test_commands":[],"unverified_items":[],"emitted_at":"1970-01-01T00:00:00Z","consultation":{"question":"one concrete decision","options":["option A","option B"],"recommendation":"your recommendation"}}`,
      `If you can complete the task correctly without consultation, do so. Qualification is based on the final delivery, not on whether you consulted.`,
    );
  }
  return lines;
}

export function bindHarnessReceipt(
  raw: unknown,
  identity: string,
  sessionId: string | null,
  now: () => Date = () => new Date(0),
): unknown {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return raw;
  const [provider, ...modelParts] = identity.split("/");
  return {
    ...(raw as Record<string, unknown>),
    issue: 1,
    worker_id: "evaluation-worker",
    model: { provider: provider || "unknown", model: modelParts.join("/") || "unknown" },
    session_id: sessionId ?? undefined,
    emitted_at: now().toISOString(),
  };
}

export function validateHarnessReceipt(
  raw: unknown,
  identity: string,
  sessionId: string | null,
): { receipt: Receipt | null; validation: ValidationResult } {
  const bound = bindHarnessReceipt(raw, identity, sessionId);
  const validation = validateReceipt(bound);
  return { receipt: validation.ok ? bound as Receipt : null, validation };
}
