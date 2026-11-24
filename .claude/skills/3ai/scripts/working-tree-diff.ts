// Review the working tree relative to the branch point, including untracked
// (non-ignored) files, without staging files in the user's index.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

export function workingTreeDiff(base: string, args: string[] = [], cwd = process.cwd()): string {
  function git(command: string[], env = process.env): string {
    return execFileSync("git", command, {
      cwd, env, encoding: "utf8", maxBuffer: 64 * 1024 * 1024,
      stdio: ["ignore", "pipe", "pipe"],
    });
  }

  const mergeBase = git(["merge-base", "--", base, "HEAD"]).trim();
  const index = git(["rev-parse", "--path-format=absolute", "--git-path", "index"]).trimEnd();
  const dir = mkdtempSync(join(tmpdir(), "3ai-review-index-"));
  try {
    const scratch = join(dir, "index");
    copyFileSync(index, scratch);
    const env = { ...process.env, GIT_INDEX_FILE: scratch };
    git(["add", "-N", "--", ":/"], env);
    return git(["diff", mergeBase, ...args], env);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
