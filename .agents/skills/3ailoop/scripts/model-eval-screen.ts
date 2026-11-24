#!/usr/bin/env bun
// Cheap first-stage evaluation for every currently-free OpenCode model.
// Each model receives the same isolated Rust bug-fix task. Only passing models proceed
// to the more expensive three-task qualification stage.

import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { join, resolve } from "path";
import { tmpdir } from "os";
import { discover, type Eligible } from "./lib/discover.ts";
import { arg, runCmd } from "./cli/helpers.ts";
import {
  DEFAULT_EVALUATION_TIMEOUTS,
  type EvaluationTimeoutConfig,
  type EvaluationTimeoutKind,
} from "./lib/evaluation-timeout.ts";
import { runOpenCodeEvaluation, type InfrastructureKind } from "./lib/opencode-evaluation-run.ts";
import { completeFixtureChanges, receiptMatchesChanges } from "./lib/evaluation-fixture.ts";
import { receiptInstructions, validateHarnessReceipt } from "./lib/worker-contract.ts";

type ScreenOutcome = "passed" | "failed" | "inconclusive";

type ScreenAttempt = {
  identity: string;
  passed: boolean;
  outcome: ScreenOutcome;
  elapsed_ms: number;
  exit_code: number | null;
  tests_passed: boolean;
  committed: boolean;
  receipt_valid: boolean;
  receipt_complete: boolean;
  scope_drift: boolean;
  timeout_kind: EvaluationTimeoutKind | null;
  infrastructure_failure: boolean;
  infrastructure_kind: InfrastructureKind | null;
  retry_after_ms: number | null;
  first_output_ms: number | null;
  last_output_ms: number | null;
  progress: {
    event_count: number;
    edit_observed: boolean;
    agent_test_pass_observed: boolean;
  };
  reason: string;
  session_log: string;
};

type ScreenResult = ScreenAttempt & { attempts: ScreenAttempt[] };

const RECEIPT = ".eval/receipt.json";

export function createScreenFixture(root: string): void {
  mkdirSync(join(root, "src"), { recursive: true });
  mkdirSync(join(root, ".eval"), { recursive: true });
  writeFileSync(join(root, ".gitignore"), ".eval/\ntarget/\n");
  writeFileSync(join(root, "Cargo.toml"), `[package]\nname = "engawa-model-screen"\nversion = "0.1.0"\nedition = "2021"\n`);
  writeFileSync(join(root, "src/lib.rs"), `#[derive(Debug, Clone)]
pub struct IdGenerator { namespace: u32, next: u32 }

impl IdGenerator {
    pub fn new(namespace: u32) -> Self { Self { namespace, next: 0 } }

    pub fn next_id(&mut self) -> u64 {
        self.next += 1;
        ((self.namespace as u64) << 32) | self.next as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_zero_based_and_deterministic() {
        let mut a = IdGenerator::new(7);
        let mut b = IdGenerator::new(7);
        let expected = [(7_u64 << 32), (7_u64 << 32) | 1, (7_u64 << 32) | 2];
        for id in expected {
            assert_eq!(a.next_id(), id);
            assert_eq!(b.next_id(), id);
        }
    }
}
`);
  // Evaluation-local policy: no network/GitHub/subagents/external directories. OpenCode
  // may edit this fixture, run focused Rust checks, and make one local commit.
  writeFileSync(join(root, "opencode.json"), JSON.stringify({
    $schema: "https://opencode.ai/config.json",
    permission: {
      "*": "deny",
      read: "allow",
      edit: "allow",
      glob: "allow",
      grep: "allow",
      list: "allow",
      lsp: "allow",
      task: "deny",
      skill: "deny",
      webfetch: "deny",
      websearch: "deny",
      external_directory: "deny",
      bash: {
        "*": "deny",
        "cargo test*": "allow",
        "cargo fmt*": "allow",
        "git status*": "allow",
        "git diff*": "allow",
        "git add *": "allow",
        "git commit *": "allow",
        "git log*": "allow",
        "git push*": "deny",
        "gh *": "deny",
        "curl *": "deny",
        "wget *": "deny"
      }
    }
  }, null, 2));
}

function git(root: string, args: string[]): { exitCode: number; stdout: string; stderr: string } {
  const r = Bun.spawnSync(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  return { exitCode: r.exitCode, stdout: r.stdout.toString(), stderr: r.stderr.toString() };
}

export function baseline(root: string): string {
  git(root, ["init", "-q"]);
  git(root, ["config", "user.name", "Engawa Eval"]);
  git(root, ["config", "user.email", "eval@invalid"]);
  // Cargo creates Cargo.lock on the first test run. Commit it in the fixture
  // baseline so grader output is not misclassified as worker scope drift.
  const lock = Bun.spawnSync(["cargo", "generate-lockfile"], { cwd: root, stdout: "pipe", stderr: "pipe" });
  if (lock.exitCode !== 0) throw new Error(lock.stderr.toString());
  git(root, ["add", "."]);
  const c = git(root, ["commit", "-qm", "evaluation baseline"]);
  if (c.exitCode !== 0) throw new Error(c.stderr);
  return git(root, ["rev-parse", "HEAD"]).stdout.trim();
}

function positiveInt(value: string | undefined, fallback: number): number {
  if (value === undefined) return fallback;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`expected a positive integer, got ${value}`);
  return parsed;
}

function nonNegativeInt(value: string | undefined, fallback: number): number {
  if (value === undefined) return fallback;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 0) throw new Error(`expected a non-negative integer, got ${value}`);
  return parsed;
}

export function shouldRetryScreenAttempt(attempt: Pick<ScreenAttempt, "infrastructure_failure" | "infrastructure_kind">): boolean {
  return attempt.infrastructure_failure && attempt.infrastructure_kind !== "rate-limit";
}

function providerCooldownResult(candidate: Eligible): ScreenResult {
  const attempt: ScreenAttempt = {
    identity: candidate.identity,
    passed: false,
    outcome: "inconclusive",
    elapsed_ms: 0,
    exit_code: null,
    tests_passed: false,
    committed: false,
    receipt_valid: false,
    receipt_complete: false,
    scope_drift: false,
    timeout_kind: null,
    infrastructure_failure: true,
    infrastructure_kind: "rate-limit",
    retry_after_ms: null,
    first_output_ms: null,
    last_output_ms: null,
    progress: { event_count: 0, edit_observed: false, agent_test_pass_observed: false },
    reason: "provider-cooldown",
    session_log: "",
  };
  return { ...attempt, attempts: [] };
}

function progressFromLog(text: string): ScreenAttempt["progress"] {
  let eventCount = 0;
  let editObserved = false;
  let agentTestPassObserved = false;
  for (const line of text.split("\n")) {
    try {
      const event = JSON.parse(line);
      eventCount += 1;
      if (event?.type !== "tool_use" || event?.part?.state?.status !== "completed") continue;
      const tool = event?.part?.tool;
      if (tool === "edit" || tool === "write") editObserved = true;
      const command = event?.part?.state?.input?.command;
      const exit = event?.part?.state?.metadata?.exit;
      if (typeof command === "string" && /^cargo test\b/.test(command) && exit === 0) agentTestPassObserved = true;
    } catch { /* stderr and partial lines are not JSON events */ }
  }
  return { event_count: eventCount, edit_observed: editObserved, agent_test_pass_observed: agentTestPassObserved };
}

async function runAttempt(candidate: Eligible, outputDir: string, attempt: number, timeouts: EvaluationTimeoutConfig): Promise<ScreenAttempt> {
  const safe = candidate.identity.replaceAll(/[^a-zA-Z0-9._-]/g, "_");
  const root = mkdtempSync(join(tmpdir(), `engawa-model-screen-${safe}-`));
  const log = resolve(outputDir, `${safe}-attempt${attempt}.jsonl`);
  createScreenFixture(root);
  const before = baseline(root);
  const prompt = [
    `Fix the failing Rust test in this isolated EngawaCAD-style fixture.`,
    `Preserve the public API, run cargo test, make one local git commit, and keep the change minimal.`,
    `Do not access the network or paths outside this directory.`,
    ...receiptInstructions(RECEIPT),
  ].join("\n");
  const started = Date.now();
  console.log(`\n=== SCREEN ${candidate.identity} attempt ${attempt} ===`);
  writeFileSync(log, "");
  const run = await runOpenCodeEvaluation({
    model: candidate.identity,
    root,
    prompt,
    title: `Engawa screen ${candidate.identity}`,
    timeouts,
    logPath: log,
    onLine: line => process.stdout.write(line + "\n"),
  });
  const combined = run.text;
  const timeoutKind = run.timeoutKind;
  const exitCode = run.code;

  const tests = Bun.spawnSync(["cargo", "test", "--quiet"], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const after = git(root, ["rev-parse", "HEAD"]).stdout.trim();
  const committed = before !== after;
  let receiptValid = false;
  let receipt: any = null;
  if (existsSync(join(root, RECEIPT))) {
    try {
      const raw = JSON.parse(readFileSync(join(root, RECEIPT), "utf8"));
      const bound = validateHarnessReceipt(raw, candidate.identity, run.sessionId);
      receipt = bound.receipt;
      receiptValid = bound.validation.ok && receipt?.status === "completed";
    } catch { receiptValid = false; }
  }
  const changed = completeFixtureChanges(root, before);
  const scopeDrift = changed.some(file => file !== "src/lib.rs");
  const receiptComplete = receiptValid && receiptMatchesChanges(receipt, changed);
  const passed = exitCode === 0 && tests.exitCode === 0 && committed && receiptValid && receiptComplete && !scopeDrift;
  const infrastructureFailure = run.infrastructureFailure;
  const outcome: ScreenOutcome = passed
    ? "passed"
    : (infrastructureFailure || (timeoutKind !== null && tests.exitCode === 0) ? "inconclusive" : "failed");
  const reasons = [
    timeoutKind ? `${timeoutKind}-timeout` : (exitCode === 0 ? null : `opencode-exit=${exitCode}`),
    infrastructureFailure ? "infrastructure-unavailable" : null,
    tests.exitCode === 0 ? null : "tests-failed",
    committed ? null : "no-local-commit",
    receiptValid ? null : "invalid-receipt",
    receiptComplete ? null : "receipt-diff-mismatch",
    scopeDrift ? "scope-drift" : null,
  ].filter(Boolean);
  const result: ScreenAttempt = {
    identity: candidate.identity,
    passed,
    outcome,
    elapsed_ms: Date.now() - started,
    exit_code: exitCode,
    tests_passed: tests.exitCode === 0,
    committed,
    receipt_valid: receiptValid,
    receipt_complete: receiptComplete,
    scope_drift: scopeDrift,
    timeout_kind: timeoutKind,
    infrastructure_failure: infrastructureFailure,
    infrastructure_kind: run.infrastructureKind,
    retry_after_ms: run.retryAfterMs,
    first_output_ms: run.firstOutputMs,
    last_output_ms: run.lastOutputMs,
    progress: progressFromLog(combined),
    reason: reasons.join(",") || "passed",
    session_log: log,
  };
  console.log(`\n=== RESULT ${candidate.identity}: ${outcome.toUpperCase()} (${result.reason}) ===`);
  rmSync(root, { recursive: true, force: true });
  return result;
}

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const valuedOptions = ["--models", "--output", "--first-output-timeout-ms", "--idle-timeout-ms", "--hard-timeout-ms", "--infra-retries"];
  const optionValues = new Set(valuedOptions.map(name => arg(args, name)).filter((value): value is string => value !== undefined));
  const positional = args.find(a => !a.startsWith("--") && !optionValues.has(a));
  const outputDir = resolve(arg(args, "--output") ?? positional ?? "features/.loop/model-eval/screen-v1");
  const requested = new Set((arg(args, "--models") ?? "").split(",").map(s => s.trim()).filter(Boolean));
  const timeouts: EvaluationTimeoutConfig = {
    firstOutputMs: positiveInt(arg(args, "--first-output-timeout-ms"), DEFAULT_EVALUATION_TIMEOUTS.firstOutputMs),
    idleMs: positiveInt(arg(args, "--idle-timeout-ms"), DEFAULT_EVALUATION_TIMEOUTS.idleMs),
    hardMs: positiveInt(arg(args, "--hard-timeout-ms"), DEFAULT_EVALUATION_TIMEOUTS.hardMs),
  };
  const infraRetries = nonNegativeInt(arg(args, "--infra-retries"), 1);
  mkdirSync(outputDir, { recursive: true });
  const found = await discover({ runOpenCode: args => runCmd(["opencode", ...args]), now: () => new Date() });
  // Never evaluate routers: selection must pin a real provider/model.
  const candidates = found.eligible.filter(c =>
    !c.identity.includes("/auto") &&
    !c.model.id.includes("router") &&
    (requested.size === 0 || requested.has(c.identity))
  );
  const candidateIdentities = new Set(candidates.map(candidate => candidate.identity));
  const requestedMissing = [...requested].filter(identity => !candidateIdentities.has(identity));
  const results: ScreenResult[] = [];
  const rateLimitedModels = new Set<string>();
  let providerCooldown = false;
  const skippedByCooldown: string[] = [];
  for (const candidate of candidates) {
    if (providerCooldown) {
      skippedByCooldown.push(candidate.identity);
      results.push(providerCooldownResult(candidate));
      console.log(`\n=== RESULT ${candidate.identity}: INCONCLUSIVE (provider-cooldown) ===`);
      continue;
    }
    const attempts: ScreenAttempt[] = [];
    let current = await runAttempt(candidate, outputDir, 1, timeouts);
    attempts.push(current);
    for (let retry = 0; retry < infraRetries && shouldRetryScreenAttempt(current); retry += 1) {
      current = await runAttempt(candidate, outputDir, retry + 2, timeouts);
      attempts.push(current);
    }
    results.push({ ...current, attempts });
    if (current.infrastructure_kind === "rate-limit") {
      rateLimitedModels.add(candidate.identity);
      providerCooldown = rateLimitedModels.size >= 2;
    }
  }
  const report = {
    version: 5,
    stage: "screen",
    checked_at: new Date().toISOString(),
    timeout_config: timeouts,
    infra_retries: infraRetries,
    candidates: candidates.map(c => c.identity),
    requested_missing_or_ineligible: requestedMissing,
    provider_cooldown: providerCooldown ? { provider: "opencode", reason: "rate-limit-confirmed-on-two-models", evidence_models: [...rateLimitedModels], skipped_models: skippedByCooldown } : null,
    results,
  };
  writeFileSync(join(outputDir, "results.json"), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  return results.some(r => r.passed) ? 0 : 1;
}

if (import.meta.main) main().then(code => process.exit(code)).catch(error => { console.error(error); process.exit(2); });
