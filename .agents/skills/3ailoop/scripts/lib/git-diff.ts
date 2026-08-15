#!/usr/bin/env bun
// git-diff.ts — worktree の完全 delta を (tracked / staged / committed / untracked 含めて) 計算
//
// Open defect #330 (untracked files omitted from a review diff) を構造的に防ぐ。
// verification は receipt.changed_files をこの完全 delta と比較し、1 件でも
// receipt に載っていない file があれば fail させる (#330 の false-clean 防止)。
//
// DI 設計: runGit を injection するので test は real git 不要。
//
// 関連: contract §"State, receipts, Git, and landing" #3, #330

export interface RunGitDeps {
  /** run git in the given worktree cwd. Returns stdout (stderr ignored for diff callers). */
  runGit: (args: string[], cwd: string) => Promise<{ ok: boolean; stdout: string; stderr: string }>;
}

/** A complete view of what changed in a worktree vs its base:
 *  - tracked unstaged modifications
 *  - staged changes
 *  - committed-but-not-on-base changes (HEAD vs base)
 *  - untracked files (NOT ignored — see notes)
 *
 *  #330: untracked files are first-class. A receipt that omits an untracked file
 *  the worker actually created must fail verification.
 */
export interface CompleteDiff {
  /** base ref the worktree was branched from (e.g. origin/main). */
  base: string;
  /** names of files that differ between base and the worktree HEAD (committed). */
  committed: string[];
  /** names of files with staged (index) changes vs HEAD. */
  staged: string[];
  /** names of files with unstaged working-tree changes vs index. */
  unstaged: string[];
  /** names of untracked files (excluding gitignored). */
  untracked: string[];
  /** union of all the above, deduped + sorted — the canonical "everything changed". */
  all: string[];
}

/** Detect the default branch via origin/HEAD. Returns null on failure. */
export async function detectDefaultBranch(deps: RunGitDeps, cwd: string): Promise<string | null> {
  const r = await deps.runGit(
    ["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
    cwd,
  );
  if (!r.ok) return null;
  const raw = r.stdout.trim();
  if (!raw) return null;
  return raw.startsWith("origin/") ? raw.slice("origin/".length) : raw;
}

function splitNames(stdout: string): string[] {
  return stdout
    .split("\0")
    .map(s => s.trim())
    .filter(s => s.length > 0);
}

/** Compute the complete diff for a worktree. `base` defaults to origin/<default-branch>. */
export async function completeDiff(
  deps: RunGitDeps,
  worktree: string,
  base?: string,
): Promise<CompleteDiff> {
  const resolvedBase = base ?? (await detectDefaultBranch(deps, worktree)) ?? "main";

  // committed: names differing between base and HEAD (porcelain, NUL-separated)
  const committedRes = await deps.runGit(
    ["diff", "--name-only", "-z", `${resolvedBase}...HEAD`],
    worktree,
  );
  // diff may exit 1 when there are no common commits; treat empty stdout as no diff
  const committed = committedRes.ok ? splitNames(committedRes.stdout) : splitNames(committedRes.stdout);

  // staged: index vs HEAD
  const stagedRes = await deps.runGit(["diff", "--cached", "--name-only", "-z"], worktree);
  const staged = splitNames(stagedRes.stdout);

  // unstaged: working tree vs index
  const unstagedRes = await deps.runGit(["diff", "--name-only", "-z"], worktree);
  const unstaged = splitNames(unstagedRes.stdout);

  // untracked: NOT ignored (--exclude-standard drops .gitignored; we still see new files)
  const untrackedRes = await deps.runGit(
    ["ls-files", "--others", "--exclude-standard", "-z"],
    worktree,
  );
  const untracked = splitNames(untrackedRes.stdout);

  const allSet = new Set<string>([...committed, ...staged, ...unstaged, ...untracked]);
  const all = Array.from(allSet).sort();

  return { base: resolvedBase, committed, staged, unstaged, untracked, all };
}

export interface CompletenessResult {
  ok: boolean;
  /** files present in the diff but MISSING from the receipt (the #330 failure). */
  missingFromReceipt: string[];
  /** files claimed by the receipt but absent from the diff (over-claim). */
  extraInReceipt: string[];
}

/**
 * Compare a receipt's claimed changed_files against the actual complete diff.
 * #330: any file in the diff that the receipt omits → ok=false (missing untracked
 * must FAIL completeness rather than produce a false clean result).
 */
export function checkCompleteness(diff: CompleteDiff, claimedFiles: string[]): CompletenessResult {
  const actual = new Set(diff.all);
  const claimed = new Set(claimedFiles);
  const missingFromReceipt = Array.from(actual).filter(f => !claimed.has(f)).sort();
  const extraInReceipt = Array.from(claimed).filter(f => !actual.has(f)).sort();
  return {
    ok: missingFromReceipt.length === 0 && extraInReceipt.length === 0,
    missingFromReceipt,
    extraInReceipt,
  };
}
