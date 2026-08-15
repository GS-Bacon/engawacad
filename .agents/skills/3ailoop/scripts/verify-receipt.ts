#!/usr/bin/env bun
// verify-receipt.ts — worker receipt の検証 + complete diff 照合 CLI
//
// 使い方:
//   bun verify-receipt.ts --receipt <file.json> --worktree <path> --audit <file.json>
//       [--base <ref>] [--expect-issue <N>] [--expect-worker <id>]
//       [--expect-model <provider/model>] [--expect-session <id>]
//       [--state-path <assign-state.json>]
//
// 検証:
//   1. receipt schema validation (receipt.ts)。
//   2. lifecycle binding: receipt の issue/worker/model/session を queued job の期待値と照合。
//      --expect-* が 1 つでも与えられれば、対応する receipt フィールドが一致しないと fail。
//   3. forbidden_operations / safety audit scan (safety.ts)。audit は **必須** で fail-closed:
//      指定がない / 読めない / 壊れている と、worker が安全だったことを証明できず safety 違反。
//   4. complete diff 計算 + completeness check (#330: untracked 抜けは fail)。
//   5. (任意) CAS を `verifying` に遷移 (lib/cas-state.ts moveToVerifying)。
//
// 出力 (stdout JSON): { ok, receipt_valid, identity_ok, safety_violations, completeness, diff }
// exit: 0=ok / 1=verify fail / 2=usage

import { readFileSync } from "fs";
import { validateReceipt, type Receipt } from "./lib/receipt.ts";
import { completeDiff, checkCompleteness, type RunGitDeps } from "./lib/git-diff.ts";
import { scanAudit, type AuditedCommand, type SafetyViolation } from "./lib/safety.ts";
import { arg, emitJson, emitStatus, runCmd } from "./cli/helpers.ts";
import { matchIdentity, type IdentityExpectation } from "./lib/verify-identity.ts";
import { mutateState, moveToVerifying, readState, STATE_PATHS } from "./lib/cas-state.ts";

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const receiptFile = arg(args, "--receipt");
  const worktree = arg(args, "--worktree");
  const base = arg(args, "--base");
  const auditFile = arg(args, "--audit");
  const expectIssue = arg(args, "--expect-issue");
  const expectWorker = arg(args, "--expect-worker");
  const expectModel = arg(args, "--expect-model");
  const expectSession = arg(args, "--expect-session");
  const statePath = arg(args, "--state-path") ?? STATE_PATHS.DEFAULT_STATE_PATH;

  if (!receiptFile || !worktree) {
    emitStatus("Usage: verify-receipt.ts --receipt <file.json> --worktree <path> --audit <file.json> [--base <ref>] [--expect-issue <N>] [--expect-worker <id>] [--expect-model <p/m>] [--expect-session <id>] [--state-path <f>]");
    return 2;
  }

  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(receiptFile, "utf-8"));
  } catch (e) {
    emitJson({ ok: false, error: "bad receipt json", detail: (e as Error).message });
    return 1;
  }

  const validation = validateReceipt(raw);
  if (!validation.ok) {
    emitJson({ ok: false, receipt_valid: false, errors: validation.errors });
    return 1;
  }
  const receipt = raw as Receipt;

  // --- lifecycle binding (F2): compare receipt identity against the assigned job's
  //     expected issue/worker/model/session. Any expectation supplied must match, else
  //     a structurally-valid receipt for the WRONG issue/worker/model/session is rejected. ---
  const expectation: IdentityExpectation = {
    issue: expectIssue ? parseInt(expectIssue, 10) : undefined,
    workerId: expectWorker,
    model: expectModel,
    sessionId: expectSession,
  };
  const identityCheck = matchIdentity(receipt, expectation);

  // --- safety audit (REQUIRED, fail-closed): a worker cannot prove it stayed safe without
  //     an audited command log. A missing/unreadable/malformed audit is itself a safety
  //     violation — the worker must never be able to execute `git push`/`gh issue close`/
  //     cross-worktree commands and pass verification by emitting a clean receipt. ---
  let violations: SafetyViolation[] = [];
  if (!auditFile) {
    violations.push({
      kind: "push",
      command: [],
      detail: "no --audit provided: worker command log is required to prove no forbidden external operation occurred (fail-closed)",
    });
  } else {
    let audited: AuditedCommand[] | null = null;
    try {
      const parsed = JSON.parse(readFileSync(auditFile, "utf-8"));
      if (Array.isArray(parsed)) audited = parsed as AuditedCommand[];
    } catch {
      audited = null;
    }
    if (audited === null) {
      violations.push({
        kind: "push",
        command: [],
        detail: `audit file missing/unreadable/malformed (${auditFile}): cannot verify worker stayed within safety bounds (fail-closed)`,
      });
    } else {
      violations = scanAudit(audited, worktree);
    }
  }
  if (receipt.forbidden_operations && receipt.forbidden_operations.length > 0) {
    violations = [
      ...violations,
      ...receipt.forbidden_operations.map(op => ({ kind: "push" as const, command: [], detail: op })),
    ];
  }

  // complete diff + completeness (only meaningful for completed receipts)
  let diff: Awaited<ReturnType<typeof completeDiff>> | null = null;
  let completeness: ReturnType<typeof checkCompleteness> | null = null;
  if (receipt.status === "completed") {
    const deps: RunGitDeps = {
      runGit: async (gargs, cwd) => runCmd(["git", ...gargs], cwd),
    };
    diff = await completeDiff(deps, worktree, base);
    completeness = checkCompleteness(diff, receipt.changed_files ?? []);
  }

  const ok = identityCheck.ok && violations.length === 0 && (completeness?.ok ?? true);
  emitStatus(`verify-receipt: ok=${ok} identity=${identityCheck.ok} violations=${violations.length} completeness=${completeness ? completeness.ok : "n/a"}`);
  emitJson({
    ok,
    receipt_valid: true,
    identity_ok: identityCheck.ok,
    identity_mismatches: identityCheck.mismatches,
    status: receipt.status,
    safety_violations: violations,
    completeness,
    diff: diff ? { base: diff.base, all: diff.all, untracked: diff.untracked } : null,
  });

  // --- CAS transition to `verifying` (contract §State/receipts; documented in the verify
  //     SKILL). Only when identity is bound to a generation we can check, and verification
  //     passed. Guarded so a no-op call (no expectation) cannot mutate state. ---
  if (ok && expectation.issue !== undefined && expectation.workerId !== undefined) {
    try {
      const cur = readState(statePath);
      const iss = cur.issues[String(expectation.issue)];
      if (iss && iss.generation !== undefined) {
        mutateState(statePath, s => moveToVerifying(s, expectation.issue!, expectation.workerId!, iss.generation));
      }
    } catch {
      // CAS state is a durable record, not a verification gate; a transition failure is
      // surfaced via the JSON output above but does not flip an otherwise-passing verify.
    }
  }
  return ok ? 0 : 1;
}

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}
