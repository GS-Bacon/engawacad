// maybe-commit-generated-ts.test.ts (#248)

import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import { spawnSync } from "child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { buildCommitMessage, gitStatusGenerated, run } from "./maybe-commit-generated-ts";

let workDir: string;

function git(args: string[], cwd: string = workDir) {
  return spawnSync("git", args, { cwd, encoding: "utf-8" });
}

function initRepo(): string {
  const dir = mkdtempSync(join(tmpdir(), "maybe-commit-gen-ts-test-"));
  git(["init", "-q"], dir);
  git(["config", "user.email", "test@example.com"], dir);
  git(["config", "user.name", "Test"], dir);
  git(["config", "commit.gpgsign", "false"], dir);
  mkdirSync(join(dir, "web/src/generated"), { recursive: true });
  writeFileSync(join(dir, "web/src/generated/Document.ts"), "export type Document = {};\n");
  git(["add", "."], dir);
  git(["commit", "-q", "-m", "init"], dir);
  return dir;
}

function countCommits(cwd: string = workDir): number {
  const r = git(["rev-list", "--count", "HEAD"], cwd);
  return parseInt((r.stdout ?? "0").trim(), 10);
}

function headMessage(cwd: string = workDir): string {
  return (git(["log", "-1", "--pretty=%B"], cwd).stdout ?? "").trim();
}

beforeEach(() => {
  workDir = initRepo();
});

afterEach(() => {
  if (existsSync(workDir)) rmSync(workDir, { recursive: true, force: true });
});

describe("buildCommitMessage", () => {
  test("T05_issue_msg: --issue N → includes #N", () => {
    expect(buildCommitMessage("248")).toContain("#248");
  });
  test("T_BOUNDARY_no_issue: no issue → no #", () => {
    expect(buildCommitMessage()).not.toContain("#");
  });
});

describe("maybe-commit-generated-ts run()", () => {
  test("T02_clean_noop: clean → exit 0, no commit added", () => {
    const before = countCommits();
    const rc = run({ cwd: workDir });
    expect(rc).toBe(0);
    expect(countCommits()).toBe(before);
  });

  test("T03_modified_commit: modified tracked TS → commit 1 added", () => {
    writeFileSync(
      join(workDir, "web/src/generated/Document.ts"),
      "export type Document = { v: number };\n",
    );
    const before = countCommits();
    const rc = run({ cwd: workDir, issueNum: "248" });
    expect(rc).toBe(0);
    expect(countCommits()).toBe(before + 1);
    expect(headMessage()).toContain("#248");
    expect(headMessage()).toContain("gen-ts intermediate");
  });

  test("T04_untracked_commit: new untracked TS → commit 1 added + tracked", () => {
    writeFileSync(
      join(workDir, "web/src/generated/Variable.ts"),
      "export type Variable = { name: string; expr: string };\n",
    );
    const before = countCommits();
    const rc = run({ cwd: workDir, issueNum: "248" });
    expect(rc).toBe(0);
    expect(countCommits()).toBe(before + 1);
    // file should now be tracked (status clean)
    expect(gitStatusGenerated(workDir)).toBe("");
  });

  test("T01_determinism: 2 連続実行 → 2 回目は no-op", () => {
    writeFileSync(
      join(workDir, "web/src/generated/Feature.ts"),
      "export type Feature = string;\n",
    );
    const before = countCommits();
    expect(run({ cwd: workDir })).toBe(0);
    const afterFirst = countCommits();
    expect(afterFirst).toBe(before + 1);
    // 2nd run: clean, no-op
    expect(run({ cwd: workDir })).toBe(0);
    expect(countCommits()).toBe(afterFirst);
  });

  test("T_DEG_outside_gen: web/src/main.ts の dirty は無視", () => {
    mkdirSync(join(workDir, "web/src"), { recursive: true });
    writeFileSync(join(workDir, "web/src/main.ts"), "console.log('app');\n");
    // generated/ は clean のまま、main.ts は untracked
    const before = countCommits();
    expect(run({ cwd: workDir })).toBe(0);
    expect(countCommits()).toBe(before); // no commit
    // main.ts should still be untracked
    const status = spawnSync("git", ["status", "--porcelain", "--", "web/src/main.ts"], {
      cwd: workDir,
      encoding: "utf-8",
    }).stdout;
    expect(status).toContain("web/src/main.ts");
  });
});
