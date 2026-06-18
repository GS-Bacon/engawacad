#!/usr/bin/env bun
// maybe-commit-generated-ts.ts (#248)
// web/src/generated/ が dirty (modified or untracked) なら intermediate commit する。
// STEP 8 の `git merge --squash` で全 intermediate commit は最終 1 commit に集約される前提。
//
// 使い方:
//   bun .claude/skills/3ai/scripts/maybe-commit-generated-ts.ts [--issue N]
//
// 挙動:
//   - clean (`git status --porcelain web/src/generated/` が空) → no-op, exit 0
//   - dirty → `git add web/src/generated/` + commit, exit 0
//   - git add / commit が失敗 → exit 1
//
// 例外: 3ai 規約「git commit/push は STEP 8 以外で行わない」の唯一の許容例外
// (web/src/generated/ は auto-generated で gen-ts 出力。Rust 型と TS 型の commit 必須)。

import { spawnSync } from "child_process";

export function gitStatusGenerated(cwd?: string): string {
  const r = spawnSync(
    "git",
    ["status", "--porcelain", "--untracked-files=all", "--", "web/src/generated/"],
    { encoding: "utf-8", cwd },
  );
  return (r.stdout ?? "").trim();
}

export function buildCommitMessage(issueNum?: string): string {
  return issueNum
    ? `chore(3ai): #${issueNum} gen-ts intermediate (squashed in STEP 8)`
    : `chore(3ai): gen-ts intermediate (squashed in STEP 8)`;
}

function parseIssueArg(argv: string[]): string | undefined {
  const i = argv.indexOf("--issue");
  if (i < 0) return undefined;
  const v = argv[i + 1];
  if (!v || v.startsWith("--")) return undefined;
  return v;
}

export function run(opts: { issueNum?: string; cwd?: string } = {}): number {
  const status = gitStatusGenerated(opts.cwd);
  if (!status) {
    console.log("OK: web/src/generated/ is clean — no intermediate commit needed");
    return 0;
  }
  console.log(`Detected drift in web/src/generated/:\n${status}`);

  const addRes = spawnSync("git", ["add", "web/src/generated/"], {
    stdio: "inherit",
    cwd: opts.cwd,
  });
  if (addRes.status !== 0) {
    console.error("ERROR: git add web/src/generated/ failed");
    return 1;
  }

  const msg = buildCommitMessage(opts.issueNum);
  const commitRes = spawnSync("git", ["commit", "-m", msg], {
    stdio: "inherit",
    cwd: opts.cwd,
  });
  if (commitRes.status !== 0) {
    console.error("ERROR: git commit failed");
    return 1;
  }

  console.log(`OK: intermediate commit created: ${msg}`);
  return 0;
}

if (import.meta.main) {
  const issueNum = parseIssueArg(process.argv);
  process.exit(run({ issueNum }));
}
