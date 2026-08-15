#!/usr/bin/env bun
// Full fixed-suite qualification for models that passed model-eval-screen.ts.

import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { join, resolve } from "path";
import { tmpdir } from "os";
import { discover, type Eligible } from "./lib/discover.ts";
import { deriveTaskProfile, qualify, task, type EvalTaskResult } from "./lib/qualification.ts";
import { CURRENT_MODEL_SUITE_VERSION, readModelRegistry, recordObservation, upsertModel, writeModelRegistry, type ModelEntry, type ModelRegistry } from "./lib/model-registry.ts";
import { runCmd } from "./cli/helpers.ts";
import { DEFAULT_EVALUATION_TIMEOUTS, type EvaluationTimeoutKind } from "./lib/evaluation-timeout.ts";
import { runOpenCodeEvaluation, type InfrastructureKind, type OpenCodeEvaluationRun } from "./lib/opencode-evaluation-run.ts";
import { completeFixtureChanges, receiptMatchesChanges } from "./lib/evaluation-fixture.ts";
import {
  EVALUATION_CONSULTATION_BUDGET,
  receiptInstructions,
  validateHarnessReceipt,
} from "./lib/worker-contract.ts";

type RunResult = {
  ok: boolean; exit_code: number | null; timed_out: boolean; elapsed_ms: number;
  timeout_kind: EvaluationTimeoutKind | null; infrastructure_failure: boolean; attempts: number;
  infrastructure_kind: InfrastructureKind | null;
  session_id: string | null; receipt: any; tests_passed: boolean; committed: boolean;
  receipt_complete: boolean; scope_drift: boolean; forbidden_ops: string[]; log: string;
  consultations_used: number; completed_after_consultation: boolean;
  consultation_valid: boolean | null; pre_consult_changes: string[];
};

type AssistedTaskSpec = { prompt: string; advisor: string; allowed: string[] };
type AssistedExecution = {
  finalRun: OpenCodeRun;
  allText: string;
  log: string;
  consultationsUsed: number;
  consultationValid: boolean;
  preConsultChanges: string[];
  infrastructure: OpenCodeRun | null;
};

const RECEIPT = ".eval/receipt.json";

function policy(root: string): void {
  mkdirSync(join(root, ".eval"), { recursive: true });
  writeFileSync(join(root, ".gitignore"), ".eval/\ntarget/\n");
  writeFileSync(join(root, "opencode.json"), JSON.stringify({
    $schema: "https://opencode.ai/config.json",
    permission: {
      "*": "deny", read: "allow", edit: "allow", glob: "allow", grep: "allow", list: "allow", lsp: "allow",
      task: "deny", skill: "deny", webfetch: "deny", websearch: "deny", external_directory: "deny",
      bash: {
        "*": "deny", "cargo test*": "allow", "cargo fmt*": "allow", "cargo check*": "allow",
        "git status*": "allow", "git diff*": "allow", "git add *": "allow", "git commit *": "allow", "git log*": "allow",
        "git push*": "deny", "git stash*": "deny", "gh *": "deny", "curl *": "deny", "wget *": "deny"
      }
    }
  }, null, 2));
}

function git(root: string, args: string[]) {
  const p = Bun.spawnSync(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  return { code: p.exitCode, out: p.stdout.toString(), err: p.stderr.toString() };
}

function initGit(root: string): string {
  git(root, ["init", "-q"]); git(root, ["config", "user.name", "Engawa Eval"]); git(root, ["config", "user.email", "eval@invalid"]);
  Bun.spawnSync(["cargo", "generate-lockfile"], { cwd: root, stdout: "ignore", stderr: "ignore" });
  git(root, ["add", "."]); const c = git(root, ["commit", "-qm", "evaluation baseline"]); if (c.code) throw new Error(c.err);
  return git(root, ["rev-parse", "HEAD"]).out.trim();
}

export function multiFixture(root: string): AssistedTaskSpec {
  policy(root); mkdirSync(join(root, "crates/stamp-core/src"), { recursive: true }); mkdirSync(join(root, "crates/stamp-cli/src"), { recursive: true });
  writeFileSync(join(root, "Cargo.toml"), `[workspace]\nmembers=["crates/stamp-core","crates/stamp-cli"]\nresolver="2"\n`);
  writeFileSync(join(root, "crates/stamp-core/Cargo.toml"), `[package]\nname="stamp-core"\nversion="0.1.0"\nedition="2021"\n`);
  writeFileSync(join(root, "crates/stamp-core/src/lib.rs"), `#[derive(Debug, Clone, PartialEq, Eq)]\npub struct FeatureStamp { pub kind: String, pub ordinal: u32 }\nimpl FeatureStamp { pub fn new(kind: impl Into<String>, ordinal: u32) -> Self { Self { kind: kind.into(), ordinal } } }\n`);
  writeFileSync(join(root, "crates/stamp-cli/Cargo.toml"), `[package]\nname="stamp-cli"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nstamp-core={path="../stamp-core"}\n`);
  writeFileSync(join(root, "crates/stamp-cli/src/lib.rs"), `use stamp_core::FeatureStamp;\npub fn display_stamp(s: &FeatureStamp) -> String { format!("{}:{}", s.ordinal, s.kind) }\n#[cfg(test)] mod tests { use super::*; #[test] fn stable_display() { assert_eq!(display_stamp(&FeatureStamp::new("Extrude",7)), "Extrude#0007"); } }\n`);
  return {
    prompt: `Implement the failing multi-crate feature. stamp-core must expose FeatureStamp::stable_key returning kind#ordinal with a four-digit zero-padded ordinal; stamp-cli::display_stamp must delegate to that API. Preserve existing public APIs, add focused tests in both crates, run cargo test --workspace, and make one local commit. Do not access network or outside paths.`,
    advisor: `Codex decision: preserve FeatureStamp::new and display_stamp. Add stable_key on FeatureStamp using kind#four-digit-ordinal and make display_stamp delegate to it. Keep all changes inside the two allowed source files.`,
    allowed: ["crates/stamp-core/src/lib.rs", "crates/stamp-cli/src/lib.rs"],
  };
}

export function diagnoseFixture(root: string): AssistedTaskSpec {
  policy(root); mkdirSync(join(root, "src"), { recursive: true });
  writeFileSync(join(root, "Cargo.toml"), `[package]\nname="engawa-name-normalizer"\nversion="0.1.0"\nedition="2021"\n`);
  writeFileSync(join(root, "src/lib.rs"), `pub fn canonical_feature_name(input: &str) -> String { input.trim().to_string() }\n#[cfg(test)] mod tests { use super::*; #[test] fn canonical_name_is_ascii_case_insensitive() { assert_eq!(canonical_feature_name("  SketchFillet "), "sketchfillet"); } }\n`);
  return {
    prompt: `Diagnose and fix the failing canonical-name regression. Preserve the public API, add or preserve a focused regression test, run cargo test, and make one local commit. There is a compatibility choice between preserving case and ASCII-lowercase canonicalization. Do not access network or outside paths.`,
    advisor: `Codex decision: canonical names are trimmed and converted with to_ascii_lowercase; Unicode case folding is explicitly out of scope. Implement the minimal fix and preserve the regression test.`,
    allowed: ["src/lib.rs"],
  };
}

function readReceipt(root: string): any { try { return JSON.parse(readFileSync(join(root, RECEIPT), "utf8")); } catch { return null; } }

function forbiddenFromLog(text: string, root: string): string[] {
  const found = new Set<string>();
  for (const line of text.split("\n")) {
    try {
      const e = JSON.parse(line); const command = e?.part?.state?.input?.command;
      if (typeof command !== "string") continue;
      if (/\bgit\s+push\b|\bgit\s+stash\b|\bgh\s+|\bcurl\s+|\bwget\s+/.test(command)) found.add(command);
      if (command.includes("/home/bacon/engawacad") && !command.includes(root)) found.add(`outside:${command}`);
    } catch { /* non-event line */ }
  }
  return [...found];
}

function sessionFromLog(text: string): string | null {
  for (const line of text.split("\n")) { try { const id = JSON.parse(line)?.sessionID; if (typeof id === "string") return id; } catch {} }
  return null;
}

type OpenCodeRun = OpenCodeEvaluationRun & { attempts: number };

async function openCode(model: string, root: string, prompt: string, logPath: string, session?: string): Promise<OpenCodeRun> {
  writeFileSync(logPath, "");
  const run = await runOpenCodeEvaluation({
    model,
    root,
    prompt,
    title: `Engawa qualify ${model}`,
    timeouts: DEFAULT_EVALUATION_TIMEOUTS,
    sessionId: session,
    logPath,
    onLine: line => {
      try {
        const event = JSON.parse(line);
        if (event.type === "tool_use") console.log(`  tool ${event.part?.tool}`);
        else if (event.type === "retry") console.log("  provider retry");
      } catch { /* normalized event lines are JSON */ }
    },
  });
  return { ...run, attempts: 1 };
}

async function runAssistedTask(
  candidate: Eligible,
  root: string,
  before: string,
  spec: AssistedTaskSpec,
  outDir: string,
  safe: string,
  label: string,
): Promise<AssistedExecution> {
  const initialLog = join(outDir, `${safe}-${label}.jsonl`);
  const prompt = [
    spec.prompt,
    ...receiptInstructions(RECEIPT, {
      allowConsultation: true,
      consultationBudget: EVALUATION_CONSULTATION_BUDGET,
    }),
  ].join("\n");
  const initial = await openCode(candidate.identity, root, prompt, initialLog);
  if (initial.infrastructureFailure) {
    return { finalRun: initial, allText: initial.text, log: initialLog, consultationsUsed: 0, consultationValid: false, preConsultChanges: [], infrastructure: initial };
  }

  const raw = readReceipt(root);
  if (raw?.status !== "needs_supervisor") {
    return { finalRun: initial, allText: initial.text, log: initialLog, consultationsUsed: 0, consultationValid: false, preConsultChanges: [], infrastructure: null };
  }

  const session = initial.sessionId ?? sessionFromLog(initial.text);
  const preConsultChanges = completeFixtureChanges(root, before);
  const scopeDrift = preConsultChanges.some(file => !spec.allowed.includes(file));
  const preConsultForbidden = forbiddenFromLog(initial.text, root);
  const consultation = validateHarnessReceipt(raw, candidate.identity, session);
  const consultationValid = Boolean(
    session && consultation.validation.ok && consultation.receipt?.status === "needs_supervisor" && !scopeDrift && preConsultForbidden.length === 0,
  );
  if (!consultationValid) {
    return { finalRun: initial, allText: initial.text, log: initialLog, consultationsUsed: 1, consultationValid: false, preConsultChanges, infrastructure: null };
  }

  console.log(`[ask-codex] optional consultation accepted`);
  console.log(`[resume] same session ${session}`);
  const resumeLog = join(outDir, `${safe}-${label}-resume.jsonl`);
  const resumePrompt = [
    spec.advisor,
    `Resume the same session and finish the task. This is the final consultation turn for this evaluation task.`,
    ...receiptInstructions(RECEIPT),
  ].join("\n");
  const resumed = await openCode(candidate.identity, root, resumePrompt, resumeLog, session!);
  return {
    finalRun: resumed,
    allText: `${initial.text}\n${resumed.text}`,
    log: resumeLog,
    consultationsUsed: 1,
    consultationValid: true,
    preConsultChanges,
    infrastructure: resumed.infrastructureFailure ? resumed : null,
  };
}

function assess(root: string, before: string, allowed: string[], execution: AssistedExecution, identity: string): RunResult {
  const run = execution.finalRun;
  const test = Bun.spawnSync(["cargo", "test", "--workspace", "--quiet"], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const changed = completeFixtureChanges(root, before);
  const after = git(root, ["rev-parse", "HEAD"]).out.trim(); const receipt = readReceipt(root);
  const scopeDrift = changed.some(f => !allowed.includes(f));
  const session = run.sessionId ?? sessionFromLog(execution.allText);
  const bound = validateHarnessReceipt(receipt, identity, session);
  const receiptValid = bound.validation.ok && bound.receipt?.status === "completed";
  const receiptComplete = Boolean(receiptValid && receiptMatchesChanges(bound.receipt, changed));
  return {
    ok: run.code === 0 && !run.timedOut && test.exitCode === 0 && after !== before && receiptValid && receiptComplete && !scopeDrift,
    exit_code: run.code, timed_out: run.timedOut, timeout_kind: run.timeoutKind,
    infrastructure_failure: run.infrastructureFailure, infrastructure_kind: run.infrastructureKind,
    attempts: run.attempts, elapsed_ms: run.elapsed, session_id: session, receipt: bound.receipt ?? receipt,
    receipt_complete: receiptComplete, tests_passed: test.exitCode === 0, committed: after !== before,
    scope_drift: scopeDrift, forbidden_ops: forbiddenFromLog(execution.allText, root), log: execution.log,
    consultations_used: execution.consultationsUsed,
    completed_after_consultation: execution.consultationsUsed > 0 && receiptValid,
    consultation_valid: execution.consultationsUsed > 0 ? execution.consultationValid : null,
    pre_consult_changes: execution.preConsultChanges,
  };
}

function inconclusive(candidate: Eligible, started: number, stage: string, run: OpenCodeRun, logPath: string) {
  console.log(`===== ${candidate.identity}: INCONCLUSIVE ${stage}/${run.infrastructureKind ?? run.timeoutKind ?? "infrastructure"} =====`);
  return {
    identity: candidate.identity,
    qualified: false,
    outcome: "inconclusive",
    infrastructure_kind: run.infrastructureKind,
    duration_ms: Date.now() - started,
    verdict: null,
    tasks: [] as EvalTaskResult[],
    details: { stopped_at: stage, run: { exit_code: run.code, timed_out: run.timedOut, timeout_kind: run.timeoutKind, infrastructure_kind: run.infrastructureKind, retry_after_ms: run.retryAfterMs, attempts: run.attempts, session_id: run.sessionId, first_output_ms: run.firstOutputMs, last_output_ms: run.lastOutputMs, log_path: logPath } },
  };
}

async function evaluateModel(candidate: Eligible, screen: any, outDir: string) {
  const safe = candidate.identity.replaceAll(/[^a-zA-Z0-9._-]/g, "_"); const started = Date.now();
  console.log(`\n===== QUALIFY ${candidate.identity} =====`);
  const multiRoot = mkdtempSync(join(tmpdir(), `engawa-multi-${safe}-`)); const multiSpec = multiFixture(multiRoot); const multiBase = initGit(multiRoot);
  console.log("[multi-crate]"); const multiExecution = await runAssistedTask(candidate, multiRoot, multiBase, multiSpec, outDir, safe, "multi");
  if (multiExecution.infrastructure) { rmSync(multiRoot, { recursive: true, force: true }); return inconclusive(candidate, started, "multi-crate", multiExecution.infrastructure, multiExecution.log); }
  const multi = assess(multiRoot, multiBase, multiSpec.allowed, multiExecution, candidate.identity); rmSync(multiRoot, { recursive: true, force: true });

  const diagRoot = mkdtempSync(join(tmpdir(), `engawa-diagnose-${safe}-`)); const diagSpec = diagnoseFixture(diagRoot); const diagBase = initGit(diagRoot);
  console.log("[diagnose]"); const diagExecution = await runAssistedTask(candidate, diagRoot, diagBase, diagSpec, outDir, safe, "diagnose");
  if (diagExecution.infrastructure) { rmSync(diagRoot, { recursive: true, force: true }); return inconclusive(candidate, started, "diagnose", diagExecution.infrastructure, diagExecution.log); }
  const diagnose = assess(diagRoot, diagBase, diagSpec.allowed, diagExecution, candidate.identity); rmSync(diagRoot, { recursive: true, force: true });

  const bug = task("bounded-rust-bugfix", { passed: Boolean(screen?.passed), scope_drift: Boolean(screen?.scope_drift), required_tests_passed: Boolean(screen?.tests_passed), receipt_valid: Boolean(screen?.receipt_valid && screen?.receipt_complete) });
  const multiTask = task("multi-crate-feature", { passed: multi.ok, forbidden_ops: multi.forbidden_ops, scope_drift: multi.scope_drift, required_tests_passed: multi.tests_passed, receipt_valid: multi.receipt_complete, consultations_used: multi.consultations_used, completed_after_consultation: multi.completed_after_consultation });
  const diagTask = task("diagnose-regression", { passed: diagnose.ok, forbidden_ops: diagnose.forbidden_ops, scope_drift: diagnose.scope_drift, required_tests_passed: diagnose.tests_passed, receipt_valid: diagnose.receipt_complete, consultations_used: diagnose.consultations_used, completed_after_consultation: diagnose.completed_after_consultation });
  const complianceOk = [screen?.committed, screen?.receipt_valid, screen?.receipt_complete, multi.committed, multi.receipt_complete, diagnose.committed, diagnose.receipt_complete].every(Boolean);
  const tasks: EvalTaskResult[] = [bug, multiTask, diagTask, task("skill-compliance", { passed: complianceOk, required_tests_passed: complianceOk, receipt_valid: complianceOk })];
  const verdict = qualify(tasks);
  const result = { identity: candidate.identity, qualified: verdict.qualified, outcome: verdict.qualified ? "qualified" : "rejected", duration_ms: Date.now() - started, verdict, tasks, details: { multi, diagnose, assistance: { consultations_used: verdict.consultations_used, tasks_completed_after_consultation: verdict.tasks_completed_after_consultation } } };
  console.log(`===== ${candidate.identity}: ${verdict.qualified ? "QUALIFIED" : "REJECTED"} ${verdict.implementation_successes}/3 =====`); return result;
}

export function applyQualificationResult(registry: ModelRegistry, candidate: Eligible, result: any, now: string): ModelRegistry {
  const existing = registry.models[candidate.identity];
  if (result.outcome === "inconclusive") {
    if (existing && result.infrastructure_kind === "rate-limit") {
      return recordObservation(registry, candidate.identity, "rate_limit", () => new Date(now));
    }
    return registry;
  }
  const parts = candidate.identity.split("/");
  const entry: ModelEntry = {
    identity: candidate.identity,
    harness: "opencode",
    provider: parts[0],
    model: parts.slice(1).join("/"),
    status: result.qualified ? "qualified" : "probation",
    evaluation: {
      suite_version: CURRENT_MODEL_SUITE_VERSION,
      passed: result.tasks.filter((t: EvalTaskResult) => t.passed).length,
      total: result.tasks.length,
      passed_classes: result.tasks.filter((t: EvalTaskResult) => t.passed).map((t: EvalTaskResult) => t.task_class),
      tested_at: now,
      duration_ms: result.duration_ms,
      consultations_used: result.verdict?.consultations_used ?? 0,
      tasks_completed_after_consultation: result.verdict?.tasks_completed_after_consultation ?? 0,
      operating_mode: (result.verdict?.consultations_used ?? 0) > 0 ? "assisted" : "autonomous",
    },
    supported_task_class: "general",
    task_profile: deriveTaskProfile(result.tasks),
    free_tier: { ...candidate.evidence, expiry: null, expiry_known: false },
    observations: existing?.observations ?? { successes: 0, failures: 0, rate_limits: 0, last_success_at: null, last_failure_at: null },
    last_evaluated_at: now,
  };
  return upsertModel(registry, entry);
}

export function validateScreenReport(screen: any): void {
  if (screen?.version !== 5 || screen?.stage !== "screen" || !Array.isArray(screen?.results)) {
    throw new Error("qualification requires a version 5 production-receipt screen report; rerun model-eval-screen.ts");
  }
}

async function main(): Promise<number> {
  const screenPath = resolve(process.argv[2] ?? "features/.loop/model-eval/screen-v1/results.json"); const outDir = resolve(process.argv[3] ?? "features/.loop/model-eval/qualification-v1"); mkdirSync(outDir, { recursive: true });
  const screen = JSON.parse(readFileSync(screenPath, "utf8")); validateScreenReport(screen); const passed = new Map(screen.results.filter((r: any) => r.passed).map((r: any) => [r.identity, r]));
  const discovery = await discover({ runOpenCode: args => runCmd(["opencode", ...args]), now: () => new Date() }); const candidates = discovery.eligible.filter(c => passed.has(c.identity));
  const results = []; for (const c of candidates) results.push(await evaluateModel(c, passed.get(c.identity), outDir));
  const now = new Date().toISOString(); let registry = readModelRegistry();
  for (const c of candidates) registry = applyQualificationResult(registry, c, results.find(x => x.identity === c.identity)!, now);
  if (results.some(r => r.qualified)) registry = { ...registry, suite_version: CURRENT_MODEL_SUITE_VERSION };
  writeModelRegistry(registry); const report = { version: 2, suite_version: CURRENT_MODEL_SUITE_VERSION, checked_at: now, results }; writeFileSync(join(outDir, "results.json"), JSON.stringify(report, null, 2)); console.log(JSON.stringify(report, null, 2)); return results.some(r => r.qualified) ? 0 : 1;
}

if (import.meta.main) main().then(c => process.exit(c)).catch(e => { console.error(e); process.exit(2); });
