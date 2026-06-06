#!/usr/bin/env bun
// extract-test-summary.ts — CI ログからテスト結果を構造化 JSON に変換

import { writeFileSync } from "fs";

interface TestEntry {
  name: string;
  kind: string;
}

interface TestSummary {
  totals: { passed: number; failed: number; ignored: number };
  by_crate: Record<string, { passed: number; failed: number }>;
  added_in_round: TestEntry[];
  coverage_hints: Record<string, number>;
}

export async function extractTestSummary(
  ciLogPath: string,
  outputPath: string,
  baseBranch: string
): Promise<void> {
  const log = await Bun.file(ciLogPath).text();
  const lines = log.split("\n");

  // totals
  const totals = { passed: 0, failed: 0, ignored: 0 };
  for (const m of log.matchAll(
    /test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored/g
  )) {
    totals.passed += parseInt(m[1]);
    totals.failed += parseInt(m[2]);
    totals.ignored += parseInt(m[3]);
  }

  // by_crate (heuristic: track current crate from "Running" lines)
  const byCrate: Record<string, { passed: number; failed: number }> = {};
  let currentCrate = "";
  for (const line of lines) {
    const runMatch = line.match(/Running (?:unittests |tests\/)?[^\s]+\s+\(([^)]+)\)/);
    if (runMatch) {
      const path = runMatch[1];
      const m = path.match(/\/([^/]+)\/target\//);
      currentCrate = m ? m[1] : (path.split("/").pop() ?? path);
    }
    const resultMatch = line.match(/test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed/);
    if (resultMatch && currentCrate) {
      const entry = byCrate[currentCrate] ?? { passed: 0, failed: 0 };
      entry.passed += parseInt(resultMatch[1]);
      entry.failed += parseInt(resultMatch[2]);
      byCrate[currentCrate] = entry;
    }
  }

  // added_in_round via git diff
  const addedInRound: TestEntry[] = [];
  if (baseBranch) {
    try {
      const proc = Bun.spawn(
        ["git", "diff", `${baseBranch}...HEAD`, "--", "*.rs"],
        { stdout: "pipe", stderr: "pipe" }
      );
      const diff = await new Response(proc.stdout).text();
      await proc.exited;
      for (const m of diff.matchAll(/^\+\s*(?:async\s+)?fn\s+((?:test_|t\d+_)[A-Za-z0-9_]+)/gm)) {
        const name = m[1];
        let kind = "other";
        if (/determin|repeat|twice|idempotent/.test(name)) kind = "determinism";
        else if (/degen|zero_|empty_|boundary/.test(name)) kind = "degenerate";
        else if (/max|min|inf|nan|overflow|extreme/.test(name)) kind = "boundary";
        else if (/golden|roundtrip|yaml|serialize/.test(name)) kind = "golden";
        else if (/edge|case/.test(name)) kind = "edge_case";
        addedInRound.push({ name, kind });
      }
    } catch {}
  }

  const KINDS = ["determinism", "degenerate", "boundary", "golden", "edge_case"];
  const coverageHints: Record<string, number> = { total_added: addedInRound.length };
  for (const k of KINDS) {
    coverageHints[k] = addedInRound.filter((t) => t.kind === k).length;
  }

  const result: TestSummary = { totals, by_crate: byCrate, added_in_round: addedInRound, coverage_hints: coverageHints };
  writeFileSync(outputPath, JSON.stringify(result, null, 2));
  process.stderr.write(
    `=== extract-test-summary: passed=${totals.passed} failed=${totals.failed} added=${addedInRound.length} ===\n`
  );
}

if (import.meta.main) {
  let ciLog = "";
  let output = "";
  let base = "";
  const args = process.argv.slice(2);

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--ci-log") ciLog = args[++i];
    else if (args[i] === "--output") output = args[++i];
    else if (args[i] === "--base") base = args[++i];
    else { console.error(`Unknown arg: ${args[i]}`); process.exit(1); }
  }

  if (!ciLog || !output) {
    console.error("Usage: extract-test-summary.ts --ci-log <file> --output <file> [--base <branch>]");
    process.exit(1);
  }

  if (!base) {
    const proc = Bun.spawn(["git", "symbolic-ref", "refs/remotes/origin/HEAD"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    base = (await new Response(proc.stdout).text()).trim().replace("refs/remotes/origin/", "");
    await proc.exited;
  }

  await extractTestSummary(ciLog, output, base);
}
