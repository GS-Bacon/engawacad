#!/usr/bin/env bun
// receipt.ts — worker receipt schema と validation (new runtime)
//
// contract §"State, receipts, Git, and landing" #2 が定義する receipt:
//   status (completed | needs_supervisor | failed | rate_limited),
//   provider/model/session identity, summary, commit SHA + changed files (when applicable),
//   test commands/results, unverified items, consultation fields (when applicable).
//
// 本モジュールは receipt の純粋な parse + validate を提供する。検証は:
//   - 必須フィールド存在 + 型
//   - status は 4 値のいずれか
//   - completed のとき commit_sha + changed_files[] が必須
//   - needs_supervisor のとき consultation { question, options[], recommendation } が必須
//   - session identity に provider/model が必須 (paid/unknown が紛れないよう)
//
// 関連: contract §"State, receipts, Git, and landing"

export type ReceiptStatus = "completed" | "needs_supervisor" | "failed" | "rate_limited";

export interface TestResult {
  command: string;
  rc: number;
  /** brief stdout/stderr tail or summary; not required to be full output */
  output_summary?: string;
}

export interface Consultation {
  question: string;
  options: string[];
  recommendation: string;
}

export interface Receipt {
  schema_version: 1;
  issue: number;
  worker_id: string;
  status: ReceiptStatus;
  /** provider/model identity (e.g. "opencode/deepseek-v4-flash-free"). Required so the
   *  verifier can confirm the work was done by a qualified-free model, not Codex/paid. */
  model: { provider: string; model: string };
  /** opaque session/run id from the worker harness. */
  session_id?: string;
  summary: string;
  /** commit SHA in the worker's isolated worktree (completed only). */
  commit_sha?: string | null;
  /** changed files (completed only). Must be reconciled against the actual git diff
   *  by the verifier — a missing untracked file must fail (#330). */
  changed_files?: string[];
  /** forbidden external operations detected during the run (push, gh write, etc.).
   *  A non-empty array fails qualification/verification immediately. */
  forbidden_operations?: string[];
  test_commands?: TestResult[];
  unverified_items?: string[];
  consultation?: Consultation;
  /** when the receipt was emitted (ISO). */
  emitted_at: string;
}

export interface ValidationFailure {
  field: string;
  message: string;
}
export interface ValidationResult {
  ok: boolean;
  errors: ValidationFailure[];
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function isString(v: unknown): v is string {
  return typeof v === "string" && v.length > 0;
}

function isStringArray(v: unknown): v is string[] {
  return Array.isArray(v) && v.every(x => typeof x === "string");
}

/** Validate a parsed receipt object against the schema. Pure, no I/O. */
export function validateReceipt(raw: unknown): ValidationResult {
  const errors: ValidationFailure[] = [];
  if (!isObject(raw)) {
    return { ok: false, errors: [{ field: "$root", message: "receipt must be an object" }] };
  }

  if (raw.schema_version !== 1) {
    errors.push({ field: "schema_version", message: "must be 1" });
  }
  if (typeof raw.issue !== "number" || !Number.isFinite(raw.issue) || raw.issue <= 0) {
    errors.push({ field: "issue", message: "must be a positive number" });
  }
  if (!isString(raw.worker_id)) {
    errors.push({ field: "worker_id", message: "must be a non-empty string" });
  }
  if (
    raw.status !== "completed" &&
    raw.status !== "needs_supervisor" &&
    raw.status !== "failed" &&
    raw.status !== "rate_limited"
  ) {
    errors.push({ field: "status", message: "must be one of completed|needs_supervisor|failed|rate_limited" });
  }
  if (!isString(raw.summary)) {
    errors.push({ field: "summary", message: "must be a non-empty string" });
  }
  if (!isString(raw.emitted_at) || isNaN(new Date(raw.emitted_at).getTime())) {
    errors.push({ field: "emitted_at", message: "must be a valid ISO timestamp" });
  }

  // model identity
  if (!isObject(raw.model)) {
    errors.push({ field: "model", message: "must be an object {provider, model}" });
  } else {
    if (!isString(raw.model.provider)) {
      errors.push({ field: "model.provider", message: "must be a non-empty string" });
    }
    if (!isString(raw.model.model)) {
      errors.push({ field: "model.model", message: "must be a non-empty string" });
    }
  }

  // completed → commit_sha + changed_files required
  if (raw.status === "completed") {
    if (!isString(raw.commit_sha)) {
      errors.push({ field: "commit_sha", message: "required when status=completed" });
    }
    if (!isStringArray(raw.changed_files)) {
      errors.push({ field: "changed_files", message: "required (string[]) when status=completed" });
    }
  }

  // needs_supervisor → consultation required
  if (raw.status === "needs_supervisor") {
    const c = raw.consultation;
    if (!isObject(c)) {
      errors.push({ field: "consultation", message: "required when status=needs_supervisor" });
    } else {
      if (!isString(c.question)) {
        errors.push({ field: "consultation.question", message: "must be a non-empty string" });
      }
      if (!isStringArray(c.options) || c.options.length < 2) {
        errors.push({ field: "consultation.options", message: "must be an array of >=2 strings" });
      }
      if (!isString(c.recommendation)) {
        errors.push({ field: "consultation.recommendation", message: "must be a non-empty string" });
      }
    }
  }

  // forbidden_operations: any non-empty array is a hard fail
  if (Array.isArray(raw.forbidden_operations) && raw.forbidden_operations.length > 0) {
    errors.push({
      field: "forbidden_operations",
      message: `forbidden external operations detected: ${JSON.stringify(raw.forbidden_operations)}`,
    });
  }

  return { ok: errors.length === 0, errors };
}

/** Parse + validate a JSON receipt string. Throws on unparseable JSON. */
export function parseReceipt(json: string): Receipt {
  const obj = JSON.parse(json);
  const v = validateReceipt(obj);
  if (!v.ok) {
    throw new Error(`invalid receipt: ${v.errors.map(e => `${e.field}: ${e.message}`).join("; ")}`);
  }
  return obj as Receipt;
}

export function isConsultationReceipt(r: Receipt): boolean {
  return r.status === "needs_supervisor";
}
