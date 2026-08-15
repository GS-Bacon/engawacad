#!/usr/bin/env bun
// Convert OpenCode JSONL into durable session/audit/receipt evidence. No polling/time limit.

import { spawn } from "child_process";
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname } from "path";
import { validateReceipt } from "./lib/receipt.ts";

export interface TerminalState {
  schema_version: 1;
  stage: "completed" | "failed";
  child_exit: number | null;
  logical_exit: number;
  session_id: string | null;
  error_type: string | null;
  error_summary: string | null;
  audit_commands: number;
  emitted_at: string;
}

export interface Accumulator {
  sessionId: string | null;
  failed: boolean;
  errorType: string | null;
  errorSummary: string | null;
  audit: Array<{ cmd: string[]; cwd: string }>;
}

export function newAccumulator(): Accumulator {
  return { sessionId: null, failed: false, errorType: null, errorSummary: null, audit: [] };
}

function bounded(value: unknown): string {
  const text = String(value ?? "unknown error").replace(/\s+/g, " ").trim();
  return text.length > 240 ? `${text.slice(0, 240)}…` : text;
}

/** Preserve the full shell text as one audited value; safety.ts scans the joined text. */
function auditCommand(command: string): string[] {
  return ["shell", "-c", command];
}

export function classifyEvent(acc: Accumulator, raw: string, cwd: string): void {
  if (!raw.trim()) return;
  let event: any;
  try {
    event = JSON.parse(raw);
  } catch {
    acc.failed = true;
    acc.errorType ??= "malformed-json";
    acc.errorSummary ??= "OpenCode emitted malformed JSONL";
    return;
  }
  if (event?.type === "error") {
    acc.failed = true;
    acc.errorType ??= "error-event";
    acc.errorSummary ??= bounded(event.message ?? event.error ?? event.content);
  }
  const session = event?.sessionID;
  if (typeof session === "string" && session.length > 0) {
    if (acc.sessionId && acc.sessionId !== session) {
      acc.failed = true;
      acc.errorType ??= "conflicting-session";
      acc.errorSummary ??= `conflicting session IDs: ${acc.sessionId} vs ${session}`;
    } else {
      acc.sessionId = session;
    }
  }
  const input = event?.part?.state?.input;
  if (event?.type === "tool_use" && typeof input?.command === "string") {
    acc.audit.push({ cmd: auditCommand(input.command), cwd: typeof input.workdir === "string" ? input.workdir : cwd });
  }
}

export function finalize(acc: Accumulator, childExit: number | null, receiptExists: boolean): TerminalState {
  let errorType = acc.errorType;
  let errorSummary = acc.errorSummary;
  if (childExit !== 0) {
    errorType ??= childExit === null ? "spawn-error" : "child-nonzero";
    errorSummary ??= childExit === null ? "worker process could not be started" : `child exited with code ${childExit}`;
  }
  if (!acc.sessionId) {
    errorType ??= "missing-session";
    errorSummary ??= "no sessionID captured in OpenCode JSONL";
  }
  if (!receiptExists) {
    errorType ??= "missing-receipt";
    errorSummary ??= "worker did not emit a receipt";
  }
  const failed = acc.failed || childExit !== 0 || !acc.sessionId || !receiptExists;
  return {
    schema_version: 1, stage: failed ? "failed" : "completed", child_exit: childExit,
    logical_exit: failed ? 1 : 0, session_id: acc.sessionId,
    error_type: failed ? errorType : null, error_summary: failed ? errorSummary : null,
    audit_commands: acc.audit.length, emitted_at: new Date().toISOString(),
  };
}

function atomicJson(path: string, value: unknown): void {
  mkdirSync(dirname(path), { recursive: true });
  const temporary = `${path}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, "utf-8");
  renameSync(temporary, path);
}

function arg(args: string[], name: string): string {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] ?? "" : "";
}

export function bindReceipt(path: string, identity: { issue: number; worker: string; model: string; session: string }): void {
  const receipt = JSON.parse(readFileSync(path, "utf-8"));
  const [provider, ...modelParts] = identity.model.split("/");
  receipt.issue = identity.issue;
  receipt.worker_id = identity.worker;
  receipt.model = { provider, model: modelParts.join("/") };
  receipt.session_id = identity.session;
  // Lifecycle facts are observed by the harness, not trusted from model output.
  receipt.emitted_at = new Date().toISOString();
  const validation = validateReceipt(receipt);
  if (!validation.ok) {
    throw new Error(`invalid receipt: ${validation.errors.map(error => `${error.field}: ${error.message}`).join("; ")}`);
  }
  atomicJson(path, receipt);
}

const SAFE_CONFIG = {
  permission: {
    "*": "deny", read: "allow", edit: "allow", glob: "allow", grep: "allow", list: "allow", lsp: "allow",
    task: "deny", skill: "allow", webfetch: "deny", websearch: "deny", external_directory: "deny",
    bash: {
      "*": "deny", "cargo *": "allow", "bun test*": "allow", "bun run*": "allow", "rg *": "allow",
      "sed *": "allow", "find *": "allow", "ls*": "allow", "pwd": "allow", "git status*": "allow",
      "git diff*": "allow", "git add *": "allow", "git commit *": "allow", "git log*": "allow",
      "git show*": "allow", "git rev-parse*": "allow", "git push*": "deny", "git stash*": "deny",
      "gh *": "deny", "curl *": "deny", "wget *": "deny"
    }
  }
};

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const out = arg(args, "--out"), audit = arg(args, "--audit"), receipt = arg(args, "--receipt");
  const issue = Number(arg(args, "--issue")), worker = arg(args, "--worker"), model = arg(args, "--model");
  const separator = args.indexOf("--"), command = separator >= 0 ? args.slice(separator + 1) : [];
  if (!out || !audit || !receipt || !Number.isInteger(issue) || !worker || !model || command.length === 0) return 2;

  const acc = newAccumulator();
  let pending = "";
  const child = spawn(command[0], command.slice(1), {
    cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, OPENCODE_CONFIG_CONTENT: JSON.stringify(SAFE_CONFIG) },
  });
  child.stdout.on("data", chunk => {
    const text = chunk.toString(); process.stdout.write(text); pending += text;
    const lines = pending.split("\n"); pending = lines.pop() ?? "";
    for (const line of lines) classifyEvent(acc, line, process.cwd());
  });
  child.stderr.on("data", chunk => process.stderr.write(chunk));
  let childExit: number | null = null;
  child.on("error", error => { acc.failed = true; acc.errorType = "spawn-error"; acc.errorSummary = bounded(error.message); });
  childExit = await new Promise(resolve => child.on("close", resolve));
  if (pending) classifyEvent(acc, pending, process.cwd());

  let receiptExists = existsSync(receipt);
  if (receiptExists && acc.sessionId) {
    try { bindReceipt(receipt, { issue, worker, model, session: acc.sessionId }); }
    catch (error) { acc.failed = true; acc.errorType ??= "invalid-receipt"; acc.errorSummary ??= bounded(error); receiptExists = false; }
  }
  atomicJson(audit, acc.audit);
  const state = finalize(acc, childExit, receiptExists);
  atomicJson(out, state);
  return state.logical_exit;
}

if (import.meta.main) main().then(code => process.exit(code)).catch(error => { console.error(error); process.exit(2); });
