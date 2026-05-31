#!/usr/bin/env bun
// get-review-config.ts — DELIVERABLE/REVIEW_INSTRUCTION/MAX_LOOPS/SCOPE_HINT を算出

import { detectDeliverable } from "./detect-deliverable.ts";
import type { Deliverable, ReviewMode, ReviewConfig } from "./types.ts";

const AGENTS_DIR = new URL("../agents", import.meta.url).pathname;

export async function getReviewConfig(
  issueNum: string,
  mode: ReviewMode,
  opts: { diffBase?: string; planFile?: string } = {}
): Promise<ReviewConfig> {
  const deliverable = await detectDeliverable(issueNum, {
    diffBase: opts.diffBase,
    planFile: opts.planFile,
  });

  let reviewInstructionPath: string;
  let maxLoops: number;

  if (mode === "design" && deliverable === "code") {
    reviewInstructionPath = `${AGENTS_DIR}/codex-design-reviewer.md`;
    maxLoops = 3;
  } else if (mode === "design" && deliverable === "docs") {
    reviewInstructionPath = `${AGENTS_DIR}/codex-design-reviewer-docs.md`;
    maxLoops = 2;
  } else if (mode === "final" && deliverable === "code") {
    reviewInstructionPath = `${AGENTS_DIR}/codex-final-reviewer.md`;
    maxLoops = 2;
  } else {
    reviewInstructionPath = `${AGENTS_DIR}/codex-final-reviewer-docs.md`;
    maxLoops = 1;
  }

  // SCOPE_HINT from issue labels
  const proc = Bun.spawn(
    [
      "gh",
      "issue",
      "view",
      issueNum,
      "--json",
      "labels",
      "-q",
      "[.labels[].name] | join(\",\")",
    ],
    { stdout: "pipe", stderr: "pipe" }
  );
  const labels = (await new Response(proc.stdout).text()).trim();
  await proc.exited;

  let scopeHint = "";
  if (/type: foundation/.test(labels)) {
    scopeHint =
      "foundation: 拡張・最適化・退化検出の追加は指摘しない。構造の入れ物が成立しているかだけ見ること。完成度ではなく『次の issue が継続できるか』が成否基準。";
  } else if (/type: refactor/.test(labels)) {
    scopeHint = "refactor: 影響範囲の最小性を最重視。新機能要求・gold plating はしない。";
  }

  return { deliverable, reviewInstructionPath, maxLoops, scopeHint };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const issueNum = args[0];
  const mode = args[1] as ReviewMode;
  let diffBase: string | undefined;
  let planFile: string | undefined;

  for (let i = 2; i < args.length; i++) {
    if (args[i] === "--diff-base") diffBase = args[++i];
    else if (args[i] === "--plan") planFile = args[++i];
    else {
      console.error(`Unknown arg: ${args[i]}`);
      process.exit(1);
    }
  }

  if (!issueNum || !["design", "final"].includes(mode)) {
    console.error(
      "Usage: get-review-config.ts <issue_num> <design|final> [--diff-base <branch>] [--plan <file>]"
    );
    process.exit(1);
  }

  const config = await getReviewConfig(issueNum, mode, { diffBase, planFile });
  // JSON output for debug; dispatch-codex-auto.ts imports this function directly
  console.log(JSON.stringify(config, null, 2));
}
