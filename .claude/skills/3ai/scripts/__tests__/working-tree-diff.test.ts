import { expect, test } from "bun:test";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { workingTreeDiff } from "../working-tree-diff.ts";

test("review consumers inspect the worktree and preserve the index, including when HEAD diverged", () => {
  const temp = mkdtempSync(join(tmpdir(), "3ai-diff-test-"));
  const repo = join(temp, "repo");
  mkdirSync(repo);
  const git = (...args: string[]) => execFileSync("git", args, { cwd: repo, encoding: "utf8" });
  const run = (name: string, ...args: string[]) =>
    spawnSync(process.execPath, [resolve(import.meta.dir, "..", name + ".ts"), ...args], {
      cwd: repo, encoding: "utf8", env: { ...process.env, PATH: temp + ":" + process.env.PATH },
    });
  try {
    git("init", "-q", "-b", "main");
    git("config", "user.name", "tester");
    git("config", "user.email", "tester@localhost");
    writeFileSync(join(repo, ".gitignore"), "ignored.rs\n");
    git("add", ".");
    git("commit", "-qm", "base");
    git("switch", "-qc", "work");
    writeFileSync(join(repo, "notes.md"), "committed\n");
    git("add", "notes.md");
    git("commit", "-qm", "document feature");
    git("switch", "-q", "main");
    writeFileSync(join(repo, "upstream.md"), "upstream only\n");
    git("add", ".");
    git("commit", "-qm", "advance main");
    git("switch", "-q", "work");
    git("update-ref", "refs/remotes/origin/main", git("rev-parse", "main").trim());
    git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main");

    writeFileSync(join(repo, "notes.md"), "staged\n");
    git("add", "notes.md");
    writeFileSync(join(repo, "notes.md"), "working version\n");
    writeFileSync(join(repo, "ignored.rs"), "ignored\n");
    mkdirSync(join(repo, "crates", "demo"), { recursive: true });
    const file = "crates/demo/new test.rs";
    writeFileSync(join(repo, file), "fn t01_golden() { assert!(true); }\n");
    const status = git("status", "--short");
    const index = readFileSync(join(repo, ".git", "index"));
    const patch = workingTreeDiff("main", [], join(repo, "crates"));
    expect(patch).toContain("working version");
    expect(patch).toContain("t01_golden");
    expect(patch).not.toContain("upstream.md");
    expect(patch).not.toContain("ignored.rs");

    expect(run("detect-deliverable", "1", "--diff-base", "main").stdout.trim()).toBe("code");
    const plan = join(temp, "plan.md");
    const summary = join(temp, "summary.json");
    writeFileSync(plan, "## テスト計画\n| T01 | expected 1 |\n");
    expect(run("extract-test-summary", "--ci-log", plan, "--output", summary).status).toBe(0);
    expect(JSON.parse(readFileSync(summary, "utf8")).added_in_round).toEqual([
      { name: "t01_golden", kind: "golden" },
    ]);
    expect(run("check-spec-divergence", "--feature-dir", repo, "--plan-file", plan).stdout)
      .toContain("fn t01_golden()");

    writeFileSync(join(temp, "cargo"), "#!/bin/sh\nexit 0\n", { mode: 0o755 });
    const lcov = join(temp, "lcov.info");
    writeFileSync(lcov, `SF:${file}\nDA:1,0\nend_of_record\n`);
    const gate = ["--base", "main", "--phase", "11", "--skip-generate", "--lcov-path", lcov];
    const working = run("check-diff-coverage", ...gate);
    expect(working.stderr).toContain(file);
    expect(working.status).toBe(1);
    expect(run("check-diff-coverage", ...gate, "--head", "HEAD").status).toBe(0);

    expect(readFileSync(join(repo, ".git", "index"))).toEqual(index);
    expect(git("status", "--short")).toBe(status);
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
});
