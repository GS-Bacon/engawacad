#!/usr/bin/env bun
// detect-deliverable.ts — issue の deliverable を "docs" または "code" で判定
// 判定順: 1. --diff-base (final モード) 2. --plan (design モード) 3. ラベル fallback

import type { Deliverable } from "./types.ts";

export async function detectDeliverable(
  issueNum: string,
  opts: { diffBase?: string; planFile?: string } = {}
): Promise<Deliverable> {
  // 1. diff-base (authoritative for final mode)
  if (opts.diffBase) {
    const proc = Bun.spawn(
      ["git", "diff", "--name-only", `${opts.diffBase}...HEAD`],
      { stdout: "pipe", stderr: "pipe" }
    );
    const changed = (await new Response(proc.stdout).text()).trim();
    await proc.exited;
    if (!changed) return "docs";
    const lines = changed.split("\n").filter(Boolean);
    if (lines.some((l) => !/^docs\//.test(l) && !l.endsWith(".md"))) return "code";
    return "docs";
  }

  // 2. plan file paths
  if (opts.planFile) {
    const planFile = Bun.file(opts.planFile);
    if (await planFile.exists()) {
      const text = await planFile.text();
      const paths = text.match(/(?:docs\/\S+|crates\/\S+|\S+\.md|\S+\.rs)/g) ?? [];
      if (paths.some((p) => p.startsWith("crates/") || p.endsWith(".rs"))) return "code";
      if (paths.some((p) => p.startsWith("docs/") || p.endsWith(".md"))) return "docs";
    }
  }

  // 3. labels fallback
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
  if (/\b(kernel|format|cli|viewer)\b/.test(labels)) return "code";
  if (/\bdocs\b/.test(labels)) return "docs";
  return "code";
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let issueNum = "";
  let diffBase: string | undefined;
  let planFile: string | undefined;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--diff-base") diffBase = args[++i];
    else if (args[i] === "--plan") planFile = args[++i];
    else if (!issueNum) issueNum = args[i];
    else {
      console.error(`Unknown arg: ${args[i]}`);
      process.exit(1);
    }
  }

  if (!issueNum) {
    console.error(
      "Usage: detect-deliverable.ts <issue_num> [--diff-base <branch>] [--plan <file>]"
    );
    process.exit(1);
  }

  const result = await detectDeliverable(issueNum, { diffBase, planFile });
  console.log(result);
}
