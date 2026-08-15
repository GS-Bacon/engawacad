// git-diff.test.ts — complete diff + completeness (#330 untracked inclusion)
//
// 検証観点 (contract acceptance):
//   - untracked files included in the complete delta
//   - a receipt omitting an untracked file → completeness-fail (no false clean)
//   - tracked/staged/committed all counted

import { describe, expect, test } from "bun:test";
import { completeDiff, checkCompleteness, type RunGitDeps } from "../lib/git-diff.ts";

function fakeGit(out: Record<string, string>): RunGitDeps {
  return {
    runGit: async (args) => {
      const key = args.join(" ");
      // match by distinctive arg combos
      if (key.includes("symbolic-ref")) return { ok: true, stdout: "origin/main\n", stderr: "" };
      if (key.includes("diff") && key.includes("-z") && key.includes("--cached")) return { ok: true, stdout: out.staged ?? "", stderr: "" };
      if (key.includes("diff") && key.includes("-z") && !key.includes("--cached") && !key.includes("...")) return { ok: true, stdout: out.unstaged ?? "", stderr: "" };
      if (key.includes("diff") && key.includes("...HEAD")) return { ok: true, stdout: out.committed ?? "", stderr: "" };
      if (key.includes("ls-files")) return { ok: true, stdout: out.untracked ?? "", stderr: "" };
      return { ok: true, stdout: "", stderr: "" };
    },
  };
}

function nulSep(files: string[]): string {
  return files.join("\0");
}

describe("complete diff (#330)", () => {
  test("untracked files appear in .all", async () => {
    const deps = fakeGit({
      committed: nulSep(["crates/a.rs"]),
      staged: nulSep(["crates/b.rs"]),
      unstaged: nulSep([]),
      untracked: nulSep(["features/123-foo/note.md"]),
    });
    const d = await completeDiff(deps, "/wt");
    expect(d.committed).toEqual(["crates/a.rs"]);
    expect(d.staged).toEqual(["crates/b.rs"]);
    expect(d.untracked).toEqual(["features/123-foo/note.md"]);
    expect(d.all).toEqual(["crates/a.rs", "crates/b.rs", "features/123-foo/note.md"]);
  });

  test("receipt covering all files → completeness ok", () => {
    const d = { base: "origin/main", committed: [], staged: [], unstaged: [], untracked: [], all: ["a.rs", "b.rs"] };
    const c = checkCompleteness(d, ["a.rs", "b.rs"]);
    expect(c.ok).toBe(true);
  });

  test("receipt OMITTING an untracked file → completeness-fail (#330 false-clean defense)", () => {
    const d = { base: "origin/main", committed: [], staged: [], unstaged: [], untracked: ["new.md"], all: ["a.rs", "new.md"] };
    const c = checkCompleteness(d, ["a.rs"]); // forgot new.md
    expect(c.ok).toBe(false);
    expect(c.missingFromReceipt).toEqual(["new.md"]);
  });

  test("receipt OVER-claiming a file → completeness-fail", () => {
    const d = { base: "origin/main", committed: [], staged: [], unstaged: [], untracked: [], all: ["a.rs"] };
    const c = checkCompleteness(d, ["a.rs", "ghost.rs"]);
    expect(c.ok).toBe(false);
    expect(c.extraInReceipt).toEqual(["ghost.rs"]);
  });
});
