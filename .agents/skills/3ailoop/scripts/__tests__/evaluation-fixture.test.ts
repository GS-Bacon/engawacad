import { describe, expect, test } from "bun:test";
import { mkdtempSync, writeFileSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import { completeFixtureChanges, receiptMatchesChanges } from "../lib/evaluation-fixture.ts";

function git(root: string, args: string[]) {
  const run = Bun.spawnSync(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  if (run.exitCode !== 0) throw new Error(run.stderr.toString());
  return run.stdout.toString().trim();
}

describe("evaluation fixture completeness", () => {
  test("includes committed, staged, unstaged, and untracked files", () => {
    const root = mkdtempSync(join(tmpdir(), "eval-diff-"));
    git(root, ["init", "-q"]);
    git(root, ["config", "user.name", "Eval"]);
    git(root, ["config", "user.email", "eval@invalid"]);
    for (const file of ["committed.rs", "staged.rs", "unstaged.rs"]) writeFileSync(join(root, file), "before\n");
    git(root, ["add", "."]); git(root, ["commit", "-qm", "base"]);
    const base = git(root, ["rev-parse", "HEAD"]);
    writeFileSync(join(root, "committed.rs"), "after\n"); git(root, ["add", "committed.rs"]); git(root, ["commit", "-qm", "change"]);
    writeFileSync(join(root, "staged.rs"), "after\n"); git(root, ["add", "staged.rs"]);
    writeFileSync(join(root, "unstaged.rs"), "after\n");
    writeFileSync(join(root, "untracked.rs"), "new\n");
    expect(completeFixtureChanges(root, base)).toEqual(["committed.rs", "staged.rs", "unstaged.rs", "untracked.rs"]);
  });

  test("receipt must neither omit nor overclaim files", () => {
    expect(receiptMatchesChanges({ changed_files: ["src/lib.rs"] }, ["src/lib.rs"])).toBe(true);
    expect(receiptMatchesChanges({ changed_files: [] }, ["src/lib.rs"])).toBe(false);
    expect(receiptMatchesChanges({ changed_files: ["src/lib.rs", "ghost.rs"] }, ["src/lib.rs"])).toBe(false);
  });
});
