#!/usr/bin/env bun
// land.ts — serialized landing CLI (default-branch mutation 前に全面再検証)
//
// 使い方:
//   bun land.ts --receipt <file.json> --worktree <path> --worker-id <id> --issue <N>
//               --generation <gen> [--base <ref>] [--dry-run]
//               [--state-path <assign-state.json>] [--registry-path <worker-registry.json>]
//
// --dry-run: revalidate まで走らせ、git/cargo/gh を呼ばない (fake deps)。exit 0=ok / 1=fail。
//
// 直列化: land-serial file lock で同時 1 件のみ。
// 再検証: receipt / generation / closed / scope / complete diff / tests (land-serial.ts)。
// 失敗時: bounded state に戻す (破壊的 recovery しない)。
//
// real path (--dry-run 無し): readCas は実際の CAS state を読み、checkScope は worktree の
// scope manifest で検証し、markClosed / releaseWorker は実際の CAS state / worker registry を
// 更新する。dry-run.ts で証明済みのパターンと同じ。
//
// 出力 (stdout JSON): LandResult
// exit: 0=success / 1=bounded-fail / 2=usage

import { readFileSync } from "fs";
import { resolve } from "path";
import { validateReceipt, type Receipt } from "./lib/receipt.ts";
import { landOnce, type LandDeps, type LandRequest } from "./lib/land-serial.ts";
import { completeDiff, type RunGitDeps } from "./lib/git-diff.ts";
import { readState, markClosed, mutateState, STATE_PATHS } from "./lib/cas-state.ts";
import { mutateRegistry, releaseTerminal, REGISTRY_DEFAULTS } from "./lib/worker-registry.ts";
import { checkScope as realCheckScope } from "./lib/scope-check.ts";
import { runCmd, detectDefaultBranch, headSha as gitHeadSha } from "./cli/git-real.ts";
import { arg, emitJson, emitStatus } from "./cli/helpers.ts";

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const receiptFile = arg(args, "--receipt");
  const worktree = arg(args, "--worktree");
  const workerId = arg(args, "--worker-id");
  const issueStr = arg(args, "--issue");
  const genStr = arg(args, "--generation");
  const dryRun = args.includes("--dry-run");
  const statePath = arg(args, "--state-path") ?? STATE_PATHS.DEFAULT_STATE_PATH;
  const registryPath = arg(args, "--registry-path") ?? REGISTRY_DEFAULTS.DEFAULT_REGISTRY_PATH;

  if (!receiptFile || !worktree || !workerId || !issueStr || !genStr) {
    emitStatus("Usage: land.ts --receipt <f> --worktree <p> --worker-id <id> --issue <N> --generation <gen> [--dry-run]");
    return 2;
  }

  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(receiptFile, "utf-8"));
  } catch (e) {
    emitJson({ outcome: "receipt-invalid", detail: (e as Error).message });
    return 1;
  }
  const v = validateReceipt(raw);
  if (!v.ok) {
    emitJson({ outcome: "receipt-invalid", errors: v.errors });
    return 1;
  }
  const receipt = raw as Receipt;

  const issue = parseInt(issueStr, 10);
  const expectedGeneration = parseInt(genStr, 10);
  const req: LandRequest = {
    issue,
    workerId,
    worktree,
    expectedGeneration,
    receipt: {
      status: receipt.status,
      model: receipt.model,
      commit_sha: receipt.commit_sha,
      changed_files: receipt.changed_files,
    },
  };

  const deps: LandDeps = dryRun
    ? dryRunDeps(req)
    : realDeps(worktree, statePath, registryPath);

  const result = await landOnce(req, deps);
  emitStatus(`land: #${result.issue} → ${result.outcome} (${result.durationMs}ms)`);
  emitJson(result);
  return result.outcome === "success" ? 0 : 1;
}

/** Real dependencies: read/mutate actual CAS state + worker registry; real scope check;
 *  real git/cargo. Mirrors the proven dry-run.ts pattern. */
function realDeps(worktree: string, statePath: string, registryPath: string): LandDeps {
  return {
    // read actual CAS state for this issue (generation / closed / ownership). An absent
    // issue entry is reported as generation 0 with NO owner (worker_id "") — never null —
    // so land-serial's worker-ownership check refuses a receipt for an issue that was never
    // assigned instead of treating the absent record as "landable by anyone" (F2).
    readCas: async (issue) => {
      const st = readState(statePath);
      const s = st.issues[String(issue)];
      if (!s) {
        return { generation: 0, closed: false, worker_id: "" };
      }
      return {
        generation: s.generation,
        closed: s.closed,
        worker_id: s.worker_id ?? "",
      };
    },
    // real scope check against the worktree's declared scope manifest (fail-closed)
    checkScope: async (issue, wt) => {
      void issue;
      const diff = await completeDiff(
        { runGit: async (gargs, cwd) => runCmd(["git", ...gargs], cwd) },
        wt,
      );
      return realCheckScope(wt, diff.all);
    },
    completeDiff: async (wt) => {
      const deps: RunGitDeps = { runGit: async (a, cwd) => runCmd(["git", ...a], cwd) };
      const d = await completeDiff(deps, wt);
      return { all: d.all };
    },
    rebase: async (wt) => {
      const fetch = await runCmd(["git", "-C", wt, "fetch", "origin"]);
      if (!fetch.ok) return { ok: false, stderr: `fetch: ${fetch.stderr}` };
      const branch = (await detectDefaultBranch(wt)) ?? "main";
      const r = await runCmd(["git", "-C", wt, "rebase", `origin/${branch}`]);
      return { ok: r.ok, stderr: r.stderr };
    },
    abortRebase: async (wt) => { await runCmd(["git", "-C", wt, "rebase", "--abort"]); },
    runCi: async (wt) => {
      const r = await runCmd(["cargo", "xtask", "ci"], wt);
      return { ok: r.ok, stderr: r.stderr };
    },
    push: async (wt) => {
      const defBr = (await detectDefaultBranch(wt)) ?? "main";
      const r = await runCmd(["git", "-C", wt, "push", "--force-with-lease", "origin", `HEAD:refs/heads/${defBr}`]);
      return { ok: r.ok, stderr: r.stderr };
    },
    // real CAS mutation: mark the issue closed (bumps generation, blocks reassignment #333)
    markClosed: async (issue) => {
      mutateState(statePath, cur => markClosed(cur, issue, () => new Date()));
    },
    // real worker-registry mutation: release the worker slot back to idle (no stuck-busy #333)
    releaseWorker: async (workerId) => {
      mutateRegistry(registryPath, reg => releaseTerminal(reg, workerId));
    },
    now: () => Date.now(),
  };
}

function dryRunDeps(req: LandRequest): LandDeps {
  // dry-run echo: report the caller's own worker as the owner and the caller's generation
  // as current, so the smoke check exercises the full revalidate path (ownership now binds)
  // instead of faking an unassigned issue as landable.
  return {
    readCas: async (issue) => ({
      generation: issue === req.issue ? req.expectedGeneration : 0,
      closed: false,
      worker_id: issue === req.issue ? req.workerId : null,
    }),
    checkScope: async () => ({ ok: true, detail: "dry-run" }),
    completeDiff: async () => ({ all: req.receipt.changed_files ?? [] }),
    rebase: async () => ({ ok: true, stderr: "dry-run" }),
    abortRebase: async () => {},
    runCi: async () => ({ ok: true, stderr: "dry-run" }),
    push: async () => ({ ok: true, stderr: "dry-run" }),
    markClosed: async () => {},
    releaseWorker: async () => {},
    now: () => Date.now(),
  };
}

void gitHeadSha;
void resolve;

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}
