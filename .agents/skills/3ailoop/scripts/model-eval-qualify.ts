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

type RunResult = {
  ok: boolean; exit_code: number | null; timed_out: boolean; elapsed_ms: number;
  timeout_kind: EvaluationTimeoutKind | null; infrastructure_failure: boolean; attempts: number;
  infrastructure_kind: InfrastructureKind | null;
  session_id: string | null; receipt: any; tests_passed: boolean; committed: boolean;
  receipt_complete: boolean; scope_drift: boolean; forbidden_ops: string[]; log: string;
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

function multiFixture(root: string): { prompt: string; allowed: string[] } {
  policy(root); mkdirSync(join(root, "crates/stamp-core/src"), { recursive: true }); mkdirSync(join(root, "crates/stamp-cli/src"), { recursive: true });
  writeFileSync(join(root, "Cargo.toml"), `[workspace]\nmembers=["crates/stamp-core","crates/stamp-cli"]\nresolver="2"\n`);
  writeFileSync(join(root, "crates/stamp-core/Cargo.toml"), `[package]\nname="stamp-core"\nversion="0.1.0"\nedition="2021"\n`);
  writeFileSync(join(root, "crates/stamp-core/src/lib.rs"), `#[derive(Debug, Clone, PartialEq, Eq)]\npub struct FeatureStamp { pub kind: String, pub ordinal: u32 }\nimpl FeatureStamp { pub fn new(kind: impl Into<String>, ordinal: u32) -> Self { Self { kind: kind.into(), ordinal } } }\n`);
  writeFileSync(join(root, "crates/stamp-cli/Cargo.toml"), `[package]\nname="stamp-cli"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nstamp-core={path="../stamp-core"}\n`);
  writeFileSync(join(root, "crates/stamp-cli/src/lib.rs"), `use stamp_core::FeatureStamp;\npub fn display_stamp(s: &FeatureStamp) -> String { format!("{}:{}", s.ordinal, s.kind) }\n#[cfg(test)] mod tests { use super::*; #[test] fn stable_display() { assert_eq!(display_stamp(&FeatureStamp::new("Extrude",7)), "Extrude#0007"); } }\n`);
  return {
    prompt: `Implement the failing multi-crate feature. stamp-core must expose FeatureStamp::stable_key returning kind#ordinal with a four-digit zero-padded ordinal; stamp-cli::display_stamp must delegate to that API. Preserve existing public APIs, add focused tests in both crates, run cargo test --workspace, make one local commit, and write ${RECEIPT} JSON with status="completed", summary, changed_files, test_command, unverified_items. Do not access network or outside paths.`,
    allowed: ["crates/stamp-core/src/lib.rs", "crates/stamp-cli/src/lib.rs"],
  };
}

function diagnoseFixture(root: string): { consult: string; resume: string; allowed: string[] } {
  policy(root); mkdirSync(join(root, "src"), { recursive: true });
  writeFileSync(join(root, "Cargo.toml"), `[package]\nname="engawa-name-normalizer"\nversion="0.1.0"\nedition="2021"\n`);
  writeFileSync(join(root, "src/lib.rs"), `pub fn canonical_feature_name(input: &str) -> String { input.trim().to_string() }\n#[cfg(test)] mod tests { use super::*; #[test] fn canonical_name_is_ascii_case_insensitive() { assert_eq!(canonical_feature_name("  SketchFillet "), "sketchfillet"); } }\n`);
  return {
    consult: `Diagnose the failing test but do not edit or commit yet. There is a compatibility choice between preserving case and ASCII-lowercase canonicalization. Write ${RECEIPT} with status="needs_supervisor", summary, changed_files=[], test_command, unverified_items, and consultation {question, options (at least 2), recommendation}. Then stop. Do not access network or outside paths.`,
    resume: `Supervisor decision: canonical names are trimmed and converted with to_ascii_lowercase; Unicode case folding is explicitly out of scope. Resume this same session, implement the minimal fix, add or preserve the regression test, run cargo test, make one local commit, and replace ${RECEIPT} with status="completed", summary, changed_files, test_command, unverified_items.`,
    allowed: ["src/lib.rs"],
  };
}

function validCompleted(r: any): boolean { return r?.status === "completed" && Array.isArray(r.changed_files) && Array.isArray(r.unverified_items) && typeof r.test_command === "string"; }
function validConsult(r: any): boolean { return r?.status === "needs_supervisor" && r.consultation && Array.isArray(r.consultation.options) && r.consultation.options.length >= 2 && Boolean(r.consultation.question) && Boolean(r.consultation.recommendation); }
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

function assess(root: string, before: string, allowed: string[], run: OpenCodeRun, log: string): RunResult {
  const test = Bun.spawnSync(["cargo", "test", "--workspace", "--quiet"], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const changed = completeFixtureChanges(root, before);
  const after = git(root, ["rev-parse", "HEAD"]).out.trim(); const receipt = readReceipt(root);
  const scopeDrift = changed.some(f => !allowed.includes(f));
  const receiptComplete = validCompleted(receipt) && receiptMatchesChanges(receipt, changed);
  return { ok: run.code === 0 && !run.timedOut && test.exitCode === 0 && after !== before && validCompleted(receipt) && receiptComplete && !scopeDrift, exit_code: run.code, timed_out: run.timedOut, timeout_kind: run.timeoutKind, infrastructure_failure: run.infrastructureFailure, infrastructure_kind: run.infrastructureKind, attempts: run.attempts, elapsed_ms: run.elapsed, session_id: run.sessionId ?? sessionFromLog(run.text), receipt, receipt_complete: receiptComplete, tests_passed: test.exitCode === 0, committed: after !== before, scope_drift: scopeDrift, forbidden_ops: forbiddenFromLog(run.text, root), log };
}

function inconclusive(candidate: Eligible, started: number, stage: string, run: OpenCodeRun) {
  console.log(`===== ${candidate.identity}: INCONCLUSIVE ${stage}/${run.infrastructureKind ?? run.timeoutKind ?? "infrastructure"} =====`);
  return {
    identity: candidate.identity,
    qualified: false,
    outcome: "inconclusive",
    infrastructure_kind: run.infrastructureKind,
    duration_ms: Date.now() - started,
    verdict: null,
    tasks: [] as EvalTaskResult[],
    details: { stopped_at: stage, run: { exit_code: run.code, timed_out: run.timedOut, timeout_kind: run.timeoutKind, infrastructure_kind: run.infrastructureKind, retry_after_ms: run.retryAfterMs, attempts: run.attempts, log: run.text } },
  };
}

async function evaluateModel(candidate: Eligible, screen: any, outDir: string) {
  const safe = candidate.identity.replaceAll(/[^a-zA-Z0-9._-]/g, "_"); const started = Date.now();
  console.log(`\n===== QUALIFY ${candidate.identity} =====`);
  const multiRoot = mkdtempSync(join(tmpdir(), `engawa-multi-${safe}-`)); const multiSpec = multiFixture(multiRoot); const multiBase = initGit(multiRoot);
  console.log("[multi-crate]"); const multiRun = await openCode(candidate.identity, multiRoot, multiSpec.prompt, join(outDir, `${safe}-multi.jsonl`));
  if (multiRun.infrastructureFailure) { rmSync(multiRoot, { recursive: true, force: true }); return inconclusive(candidate, started, "multi-crate", multiRun); }
  const multi = assess(multiRoot, multiBase, multiSpec.allowed, multiRun, join(outDir, `${safe}-multi.jsonl`)); rmSync(multiRoot, { recursive: true, force: true });

  const diagRoot = mkdtempSync(join(tmpdir(), `engawa-diagnose-${safe}-`)); const diagSpec = diagnoseFixture(diagRoot); const diagBase = initGit(diagRoot);
  console.log("[consultation]"); const cRun = await openCode(candidate.identity, diagRoot, diagSpec.consult, join(outDir, `${safe}-consult.jsonl`));
  if (cRun.infrastructureFailure) { rmSync(diagRoot, { recursive: true, force: true }); return inconclusive(candidate, started, "consultation", cRun); }
  const consultation = readReceipt(diagRoot); const noEarlyCommit = git(diagRoot, ["rev-parse", "HEAD"]).out.trim() === diagBase; const noEarlyChanges = completeFixtureChanges(diagRoot, diagBase).length === 0; const session = cRun.sessionId ?? sessionFromLog(cRun.text);
  console.log(`[resume same session ${session ?? "missing"}]`); const dRun: OpenCodeRun = session ? await openCode(candidate.identity, diagRoot, diagSpec.resume, join(outDir, `${safe}-resume.jsonl`), session) : { code: 2, timedOut: false, timeoutKind: null, infrastructureFailure: false, infrastructureKind: null, retryAfterMs: null, elapsed: 0, text: "", sessionId: null, firstOutputMs: null, lastOutputMs: null, attempts: 0 };
  if (dRun.infrastructureFailure) { rmSync(diagRoot, { recursive: true, force: true }); return inconclusive(candidate, started, "resume", dRun); }
  const diagnose = assess(diagRoot, diagBase, diagSpec.allowed, dRun, join(outDir, `${safe}-resume.jsonl`)); diagnose.forbidden_ops.push(...forbiddenFromLog(cRun.text, diagRoot)); rmSync(diagRoot, { recursive: true, force: true });

  const bug = task("bounded-rust-bugfix", { passed: Boolean(screen?.passed), scope_drift: Boolean(screen?.scope_drift), required_tests_passed: Boolean(screen?.tests_passed), receipt_valid: Boolean(screen?.receipt_valid && screen?.receipt_complete) });
  const multiTask = task("multi-crate-feature", { passed: multi.ok, forbidden_ops: multi.forbidden_ops, scope_drift: multi.scope_drift, required_tests_passed: multi.tests_passed, receipt_valid: validCompleted(multi.receipt) && multi.receipt_complete });
  const diagTask = task("diagnose-regression", { passed: diagnose.ok, forbidden_ops: diagnose.forbidden_ops, scope_drift: diagnose.scope_drift, required_tests_passed: diagnose.tests_passed, receipt_valid: validCompleted(diagnose.receipt) && diagnose.receipt_complete });
  const complianceOk = [screen?.committed, screen?.receipt_valid, screen?.receipt_complete, multi.committed, validCompleted(multi.receipt), multi.receipt_complete, diagnose.committed, validCompleted(diagnose.receipt), diagnose.receipt_complete].every(Boolean);
  const supervisorOk = cRun.code === 0 && noEarlyCommit && noEarlyChanges && Boolean(session) && validConsult(consultation) && diagnose.ok;
  const tasks: EvalTaskResult[] = [bug, multiTask, diagTask, task("skill-compliance", { passed: complianceOk, required_tests_passed: complianceOk, receipt_valid: complianceOk }), task("supervisor-resume", { passed: supervisorOk, forbidden_ops: forbiddenFromLog(cRun.text + dRun.text, ""), required_tests_passed: supervisorOk, receipt_valid: validConsult(consultation) && validCompleted(diagnose.receipt) && diagnose.receipt_complete })];
  const verdict = qualify(tasks);
  const infrastructureInconclusive = multiRun.infrastructureFailure || cRun.infrastructureFailure || dRun.infrastructureFailure;
  const result = { identity: candidate.identity, qualified: verdict.qualified, outcome: verdict.qualified ? "qualified" : (infrastructureInconclusive ? "inconclusive" : "rejected"), duration_ms: Date.now() - started, verdict, tasks, details: { multi, consultation: { valid: validConsult(consultation), no_early_commit: noEarlyCommit, no_early_changes: noEarlyChanges, session_id: session, infrastructure_failure: cRun.infrastructureFailure, attempts: cRun.attempts }, diagnose } };
  console.log(`===== ${candidate.identity}: ${verdict.qualified ? "QUALIFIED" : "REJECTED"} ${verdict.implementation_successes}/3 =====`); return result;
}

export function applyQualificationResult(registry: ModelRegistry, candidate: Eligible, result: any, now: string): ModelRegistry {
  const existing = registry.models[candidate.identity];
  if (result.outcome === "inconclusive") {
    if (existing && result.infrastructure_kind === "rate-limit") {
      return { ...recordObservation(registry, candidate.identity, "rate_limit", () => new Date(now)), suite_version: CURRENT_MODEL_SUITE_VERSION };
    }
    return { ...registry, suite_version: CURRENT_MODEL_SUITE_VERSION };
  }
  const parts = candidate.identity.split("/");
  const entry: ModelEntry = {
    identity: candidate.identity,
    harness: "opencode",
    provider: parts[0],
    model: parts.slice(1).join("/"),
    status: result.qualified ? "qualified" : "probation",
    evaluation: { suite_version: CURRENT_MODEL_SUITE_VERSION, passed: result.tasks.filter((t: EvalTaskResult) => t.passed).length, total: result.tasks.length, passed_classes: result.tasks.filter((t: EvalTaskResult) => t.passed).map((t: EvalTaskResult) => t.task_class), tested_at: now, duration_ms: result.duration_ms },
    supported_task_class: "general",
    task_profile: deriveTaskProfile(result.tasks),
    free_tier: { ...candidate.evidence, expiry: null, expiry_known: false },
    observations: existing?.observations ?? { successes: 0, failures: 0, rate_limits: 0, last_success_at: null, last_failure_at: null },
    last_evaluated_at: now,
  };
  return { ...upsertModel(registry, entry), suite_version: CURRENT_MODEL_SUITE_VERSION };
}

export function validateScreenReport(screen: any): void {
  if (screen?.version !== 4 || screen?.stage !== "screen" || !Array.isArray(screen?.results)) {
    throw new Error("qualification requires a version 4 screen report; rerun model-eval-screen.ts");
  }
}

async function main(): Promise<number> {
  const screenPath = resolve(process.argv[2] ?? "features/.loop/model-eval/screen-v1/results.json"); const outDir = resolve(process.argv[3] ?? "features/.loop/model-eval/qualification-v1"); mkdirSync(outDir, { recursive: true });
  const screen = JSON.parse(readFileSync(screenPath, "utf8")); validateScreenReport(screen); const passed = new Map(screen.results.filter((r: any) => r.passed).map((r: any) => [r.identity, r]));
  const discovery = await discover({ runOpenCode: args => runCmd(["opencode", ...args]), now: () => new Date() }); const candidates = discovery.eligible.filter(c => passed.has(c.identity));
  const results = []; for (const c of candidates) results.push(await evaluateModel(c, passed.get(c.identity), outDir));
  const now = new Date().toISOString(); let registry = readModelRegistry();
  for (const c of candidates) registry = applyQualificationResult(registry, c, results.find(x => x.identity === c.identity)!, now);
  writeModelRegistry(registry); const report = { version: 1, checked_at: now, results }; writeFileSync(join(outDir, "results.json"), JSON.stringify(report, null, 2)); console.log(JSON.stringify(report, null, 2)); return results.some(r => r.qualified) ? 0 : 1;
}

if (import.meta.main) main().then(c => process.exit(c)).catch(e => { console.error(e); process.exit(2); });
