#!/usr/bin/env bun
// git-ops.ts — worktree 作成・HEAD sha 取得などの git 操作 (DI 設計)
//
// worker は assigned isolated worktree 内でのみ動く。本モジュールは
// worktree の作成/取得と sha 読み取りを提供する。実コマンドは DI で注入。
//
// 関連: contract §"Orchestration and authority" #2

export interface GitOpsDeps {
  runGit: (args: string[], cwd?: string) => Promise<{ ok: boolean; stdout: string; stderr: string }>;
}

/** Create an isolated worktree at HEAD. Returns the worktree path. */
export async function addWorktree(
  deps: GitOpsDeps,
  repoRoot: string,
  path: string,
  detachAt?: string,
): Promise<{ ok: boolean; path: string; stderr: string }> {
  const args = ["worktree", "add", "--detach", path];
  if (detachAt) args.push(detachAt);
  const r = await deps.runGit(args, repoRoot);
  return { ok: r.ok, path, stderr: r.stderr };
}

/** Remove a worktree (force). */
export async function removeWorktree(
  deps: GitOpsDeps,
  repoRoot: string,
  path: string,
): Promise<{ ok: boolean; stderr: string }> {
  const r = await deps.runGit(["worktree", "remove", "--force", path], repoRoot);
  return { ok: r.ok, stderr: r.stderr };
}

/** Read HEAD sha of a worktree. Throws on failure. */
export async function headSha(deps: GitOpsDeps, worktree: string): Promise<string> {
  const r = await deps.runGit(["rev-parse", "HEAD"], worktree);
  if (!r.ok) throw new Error(`rev-parse HEAD failed: ${r.stderr}`);
  return r.stdout.trim();
}

/** Detect the default branch via origin/HEAD. */
export async function detectDefaultBranch(deps: GitOpsDeps, cwd: string): Promise<string | null> {
  const r = await deps.runGit(
    ["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
    cwd,
  );
  if (!r.ok) return null;
  const raw = r.stdout.trim();
  if (!raw) return null;
  return raw.startsWith("origin/") ? raw.slice("origin/".length) : raw;
}
