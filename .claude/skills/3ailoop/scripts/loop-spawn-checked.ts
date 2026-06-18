#!/usr/bin/env bun
// loop-spawn-checked.ts — dispatch 系の spawn を exit code 検証 + stderr 保持で実行 (#227)
//
// 背景:
// - loop / 3ai の多くの dispatch スクリプトが Bun.spawn の exit code を見ずに stdout だけ読んでいた
//   (loop-failure-tracker, loop-intent-guard, loop-adr-auto-accept, dispatch-codex-auto 等)
// - gh auth error / 404 / codex crash / git symbolic-ref 失敗が silent fail し、
//   needs-human ラベルが付かない / 空 result file を review する等の経路が露呈
//
// API:
//   const { stdout, stderr, exitCode } = await runChecked(["gh", "issue", "edit", ...]);
//   // 非ゼロ exit で LoopCommandError を throw
//
//   const r = await runChecked(["gh", ...], { allowFailure: true });
//   // exit code を見て分岐したい場合 (例: remove-label でラベル不在を許容)
//
// 関連: #227

/** Spawn が非ゼロ exit したことを示す error。stderr / stdout / exitCode を保持する。 */
export class LoopCommandError extends Error {
  public readonly cmd: string[];
  public readonly exitCode: number;
  public readonly stderr: string;
  public readonly stdout: string;

  constructor(cmd: string[], exitCode: number, stderr: string, stdout: string) {
    const stderrSnippet = stderr.length > 400 ? stderr.slice(0, 400) + "...(truncated)" : stderr;
    super(
      `Command failed (exit ${exitCode}): ${cmd.join(" ")}\nstderr: ${stderrSnippet.trim() || "(empty)"}`,
    );
    this.name = "LoopCommandError";
    this.cmd = cmd;
    this.exitCode = exitCode;
    this.stderr = stderr;
    this.stdout = stdout;
  }
}

export interface RunCheckedOptions {
  /** Allow non-zero exit (returns result instead of throwing). Default false. */
  allowFailure?: boolean;
  /** Stdin handling. Default "ignore". */
  stdin?: "ignore" | "inherit";
  /** Working directory for the spawn. */
  cwd?: string;
  /** Additional env vars merged on top of process.env. */
  env?: Record<string, string>;
}

export interface RunCheckedResult {
  stdout: string;
  stderr: string;
  exitCode: number;
}

/** Spawn `cmd` with stdio piped, await exit, and verify the exit code.
 *
 *  Returns { stdout, stderr, exitCode } when:
 *  - exit code is 0, OR
 *  - exit code is non-zero AND opts.allowFailure is true.
 *
 *  Throws LoopCommandError when exit code is non-zero AND opts.allowFailure is falsy.
 *  Always preserves stderr in the error for debugging.
 */
export async function runChecked(
  cmd: string[],
  opts: RunCheckedOptions = {},
): Promise<RunCheckedResult> {
  if (!cmd || cmd.length === 0) {
    throw new Error("runChecked: cmd must be a non-empty array");
  }
  const spawnOpts: Parameters<typeof Bun.spawn>[1] = {
    stdout: "pipe",
    stderr: "pipe",
    stdin: opts.stdin ?? "ignore",
  };
  if (opts.cwd) spawnOpts.cwd = opts.cwd;
  if (opts.env) spawnOpts.env = { ...process.env, ...opts.env } as Record<string, string>;

  const proc = Bun.spawn(cmd, spawnOpts);
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  await proc.exited;
  const exitCode = proc.exitCode ?? -1;
  if (exitCode !== 0 && !opts.allowFailure) {
    throw new LoopCommandError(cmd, exitCode, stderr, stdout);
  }
  return { stdout, stderr, exitCode };
}
