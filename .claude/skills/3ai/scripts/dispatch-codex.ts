#!/usr/bin/env bun
// dispatch-codex.ts — Codex (gpt-5.4) レビュー起動スクリプト
// CODEX_DRY_RUN=1 のとき Codex を呼ばず stdin prefix をダンプして exit 0 (テスト用)

import { readFileSync, writeFileSync } from "fs";

function detectBase(): Promise<string> {
  const proc = Bun.spawn(["git", "symbolic-ref", "refs/remotes/origin/HEAD"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  return new Response(proc.stdout)
    .text()
    .then((s) => s.trim().replace("refs/remotes/origin/", ""));
}

function buildPrefix(scopeHint: string, extraInputFile: string): string {
  let prefix = "";
  if (scopeHint) {
    prefix += `===== SCOPE PROFILE =====\n${scopeHint}\n===== END SCOPE PROFILE =====\n\n`;
  }
  if (extraInputFile) {
    try {
      prefix += readFileSync(extraInputFile, "utf-8") + "\n\n";
    } catch {}
  }
  return prefix;
}

function parseVerdict(text: string): {
  verdict: string;
  severity_counts: Record<string, number>;
  blocking: number;
} {
  const verdicts = [...text.matchAll(/verdict:\s*(pass|fail)/g)].map((m) => m[1]);
  const verdict = verdicts[verdicts.length - 1] ?? "unknown";
  const sevs = [...text.matchAll(/severity:\s*(critical|high|medium|low)/g)].map((m) => m[1]);
  const counts = { critical: 0, high: 0, medium: 0, low: 0 };
  for (const s of sevs) counts[s as keyof typeof counts]++;
  return { verdict, severity_counts: counts, blocking: counts.critical + counts.high };
}

export interface DispatchCodexOpts {
  mode: "design" | "review";
  instructionFile: string;
  resultFile: string;
  inputFile?: string;
  baseBranch?: string;
  extraInputFile?: string;
  scopeHint?: string;
}

export async function dispatchCodex(opts: DispatchCodexOpts): Promise<void> {
  const { mode, instructionFile, resultFile } = opts;
  const extraInputFile = opts.extraInputFile ?? "";
  const scopeHint = opts.scopeHint ?? "";
  let { inputFile, baseBranch } = opts;

  const instr = readFileSync(instructionFile, "utf-8");

  if (mode === "review" && !baseBranch) {
    baseBranch = await detectBase();
    if (!baseBranch) throw new Error("--base unset and origin/HEAD undetectable");
  }

  process.stderr.write(`=== dispatch-codex: mode=${mode} ===\n`);
  const prefix = buildPrefix(scopeHint, extraInputFile);
  let stdinContent: string;

  if (mode === "design") {
    if (!inputFile) throw new Error("--input required for mode=design");
    process.stderr.write(`  input: ${inputFile}\n`);
    stdinContent = prefix + readFileSync(inputFile, "utf-8");
  } else {
    process.stderr.write(`  base: ${baseBranch}\n`);
    const proc = Bun.spawn(["git", "diff", `${baseBranch}...HEAD`], { stdout: "pipe" });
    stdinContent = prefix + (await new Response(proc.stdout).text());
    await proc.exited;
  }

  if (process.env.CODEX_DRY_RUN === "1") {
    process.stdout.write(stdinContent);
    return;
  }

  const logFile = `${resultFile}.log`;
  const proc = Bun.spawn(
    ["codex", "exec", "-c", "sandbox_mode=read-only", "--output-last-message", resultFile, instr],
    {
      stdin: Buffer.from(stdinContent, "utf-8"),
      stdout: "pipe",
      stderr: "pipe",
    }
  );
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  writeFileSync(logFile, stdout + stderr);

  try {
    const text = await Bun.file(resultFile).text().catch(() => "");
    writeFileSync(`${resultFile}.verdict.json`, JSON.stringify(parseVerdict(text)));
  } catch {
    writeFileSync(
      `${resultFile}.verdict.json`,
      JSON.stringify({ verdict: "unknown", blocking: -1 })
    );
  }

  process.stderr.write(`=== dispatch-codex: done (exit ${exitCode}), result → ${resultFile} ===\n`);
  process.exit(exitCode);
}

if (import.meta.main) {
  let mode: "design" | "review" | undefined;
  let inputFile: string | undefined;
  let baseBranch: string | undefined;
  let instructionFile = "";
  let resultFile = "";
  let extraInputFile = "";
  let scopeHint = "";

  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--mode") mode = args[++i] as "design" | "review";
    else if (args[i] === "--input") inputFile = args[++i];
    else if (args[i] === "--base") baseBranch = args[++i];
    else if (args[i] === "--instruction") instructionFile = args[++i];
    else if (args[i] === "--result") resultFile = args[++i];
    else if (args[i] === "--extra-input") extraInputFile = args[++i];
    else if (args[i] === "--scope-hint") scopeHint = args[++i];
    else { console.error(`Unknown arg: ${args[i]}`); process.exit(1); }
  }

  if (!mode || !instructionFile || !resultFile) {
    console.error("ERROR: --mode, --instruction, and --result are required");
    process.exit(1);
  }

  await dispatchCodex({ mode, instructionFile, resultFile, inputFile, baseBranch, extraInputFile, scopeHint });
}
