#!/usr/bin/env bun
// git-real.ts — real git subprocess adapters (shared by land.ts / verify-receipt.ts / run-worker.ts)
//
// run-worker は assigned isolated worktree 内での git 操作しか許さないため、
// ここでは worktree scoped の安全な helper のみ提供する (push は含まない)。

import { runCmd } from "./helpers.ts";
import { detectDefaultBranch as gitDetect, headSha as gitHead, addWorktree as gitAdd, removeWorktree as gitRemove, type GitOpsDeps } from "../lib/git-ops.ts";

export { runCmd };

const realGitOps: GitOpsDeps = {
  runGit: async (args, cwd) => runCmd(["git", ...args], cwd),
};

export async function detectDefaultBranch(cwd: string): Promise<string | null> {
  return gitDetect(realGitOps, cwd);
}

export async function headSha(worktree: string): Promise<string> {
  return gitHead(realGitOps, worktree);
}

export async function addWorktree(repoRoot: string, path: string, detachAt?: string) {
  return gitAdd(realGitOps, repoRoot, path, detachAt);
}

export async function removeWorktree(repoRoot: string, path: string) {
  return gitRemove(realGitOps, repoRoot, path);
}
