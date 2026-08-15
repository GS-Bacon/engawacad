#!/usr/bin/env bun
// helpers.ts — CLI 共通 util (arg parse, subprocess, paths)
//
// new runtime の CLI entrypoints が共通で使う小さな helper 集。

import { spawn } from "child_process";
import { resolve } from "path";

// this file lives at .agents/skills/3ailoop/scripts/cli/helpers.ts → 5 ".." to reach the
// repository root (matches the lib/ path constants; a shallow count resolves to .agents,
// which would misplace features/.loop/*, ROADMAP.md, and the .agents/skills discovery root).
export const REPO_ROOT = resolve(import.meta.dir, "../../../../..");

/** --name value 形式の arg を取る。無ければ undefined。 */
export function arg(args: string[], name: string): string | undefined {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
}
export function hasFlag(args: string[], name: string): boolean {
  return args.includes(name);
}
/** --name=val 形式と --name val 形式の両方を受け付ける。 */
export function argLoose(args: string[], name: string): string | undefined {
  const eq = `${name}=`;
  for (const a of args) {
    if (a.startsWith(eq)) return a.slice(eq.length);
  }
  return arg(args, name);
}

export interface CmdResult {
  ok: boolean;
  stdout: string;
  stderr: string;
  exitCode: number | null;
}

/** Spawn a subprocess, capture stdout/stderr. cwd optional. */
export function runCmd(args: string[], cwd?: string, env?: Record<string, string>): Promise<CmdResult> {
  return new Promise(res => {
    const proc = spawn(args[0], args.slice(1), {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, ...env },
    });
    let stdout = "";
    let stderr = "";
    proc.stdout?.on("data", d => (stdout += d.toString()));
    proc.stderr?.on("data", d => (stderr += d.toString()));
    proc.on("error", e => {
      res({ ok: false, stdout, stderr: stderr + String(e), exitCode: null });
    });
    proc.on("close", code => {
      res({ ok: code === 0, stdout, stderr, exitCode: code });
    });
  });
}

/** Print JSON to stdout (the contract's machine-readable convention). */
export function emitJson(obj: unknown): void {
  process.stdout.write(JSON.stringify(obj, null, 2) + "\n");
}

/** Print a one-line human status to stderr (separate from machine stdout). */
export function emitStatus(msg: string): void {
  process.stderr.write(msg + "\n");
}
