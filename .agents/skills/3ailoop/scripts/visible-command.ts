#!/usr/bin/env bun
// Run one long command in a dedicated right-side tmux pane and wait for its terminal
// event. Lifecycle uses tmux wait-for + an atomic exit-code file, never pane scraping.
// Finished panes are released by default; --retain-pane is a debugging override.

import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { arg, emitJson, runCmd } from "./cli/helpers.ts";

export function shouldRetainPane(
  optionArgs: string[],
  env: Record<string, string | undefined> = process.env,
): boolean {
  return optionArgs.includes("--retain-pane") || env.LOOP_RETAIN_WORKER_PANE === "1";
}

function quote(value: string): string {
  return `'${value.replaceAll("'", `'\"'\"'`)}'`;
}

export function buildPaneScript(options: {
  cwd: string;
  label: string;
  command: string[];
  logPath: string;
  exitPath: string;
  channel: string;
  rendererPath: string;
  retainPane: boolean;
  evaluationModels?: string;
}): string {
  const { cwd, label, command, logPath, exitPath, channel, rendererPath, retainPane, evaluationModels } = options;
  return `#!/usr/bin/env bash
set +e
set -o pipefail
cd ${quote(cwd)}
(
  echo ${quote(`[${label}] started`)}
  ${command.map(quote).join(" ")}
  rc=$?
  echo ${quote(`[${label}] finished`)} "exit=$rc"
  exit "$rc"
) 2>&1 | bun ${quote(rendererPath)} --label ${quote(label)} --log ${quote(logPath)}${evaluationModels ? ` --evaluation-models ${quote(evaluationModels)}` : ""}
rc=\${PIPESTATUS[0]}
printf '%s\\n' "$rc" > ${quote(`${exitPath}.tmp`)}
mv ${quote(`${exitPath}.tmp`)} ${quote(exitPath)}
tmux wait-for -S ${quote(channel)}
${retainPane ? `exec bash --noprofile --norc` : `exit "$rc"`}
`;
}

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const separator = args.indexOf("--");
  const optionArgs = separator >= 0 ? args.slice(0, separator) : args;
  const label = arg(optionArgs, "--label") ?? "3ailoop-worker";
  const target = arg(optionArgs, "--target") ?? process.env.TMUX_PANE ?? "";
  const statePath = resolve(arg(optionArgs, "--state-file") ?? "features/.loop/visible-command.json");
  const cwd = resolve(arg(optionArgs, "--cwd") ?? process.cwd());
  const retainPane = shouldRetainPane(optionArgs);
  const evaluationModels = arg(optionArgs, "--evaluation-models");
  const command = separator >= 0 ? args.slice(separator + 1) : [];
  if (command.length === 0) throw new Error("visible-command requires -- <command> [args]");

  mkdirSync(dirname(statePath), { recursive: true });
  const runId = `${Date.now()}-${process.pid}`;
  const exitPath = `${statePath}.${runId}.exit`;
  const logPath = `${statePath}.${runId}.log`;
  const scriptPath = `${statePath}.${runId}.sh`;
  const channel = `3ailoop-visible-${runId}`;
  const rendererPath = resolve(import.meta.dir, "pane-render.ts");

  const state = {
    version: 1,
    label,
    command,
    cwd,
    stage: "queued",
    pane_id: null as string | null,
    log_path: logPath,
    exit_code: null as number | null,
    display: { mode: evaluationModels ? "evaluation-dashboard" : "dashboard", renderer: rendererPath, raw_log: logPath },
    pane_cleanup: "not-allocated" as "not-allocated" | "pending" | "retained" | "released" | "release-failed",
  };
  writeFileSync(statePath, JSON.stringify(state, null, 2));

  const inTmux = Boolean(process.env.TMUX) && (await runCmd(["tmux", "display-message", "-p", "#{pane_id}"])).ok;
  if (!inTmux) {
    state.stage = "running";
    writeFileSync(statePath, JSON.stringify(state, null, 2));
    const proc = Bun.spawn(command, { cwd, stdin: "ignore", stdout: "inherit", stderr: "inherit", env: process.env });
    const rc = await proc.exited;
    state.stage = rc === 0 ? "completed" : "failed";
    state.exit_code = rc;
    writeFileSync(statePath, JSON.stringify(state, null, 2));
    emitJson(state);
    return rc;
  }

  const script = buildPaneScript({
    cwd,
    label,
    command,
    logPath,
    exitPath,
    channel,
    rendererPath,
    retainPane,
    evaluationModels,
  });
  writeFileSync(scriptPath, script);
  chmodSync(scriptPath, 0o700);

  const splitArgs = ["tmux", "split-window", "-h", "-P", "-F", "#{pane_id}", "-l", "50%"];
  if (target) splitArgs.push("-t", target);
  splitArgs.push(scriptPath);
  const split = await runCmd(splitArgs);
  if (!split.ok) throw new Error(`tmux split failed: ${split.stderr}`);
  const paneId = split.stdout.trim();
  state.pane_id = paneId;
  state.pane_cleanup = "pending";
  state.stage = "running";
  writeFileSync(statePath, JSON.stringify(state, null, 2));
  await runCmd(["tmux", "select-pane", "-t", state.pane_id, "-T", label]);

  // A single event-driven wait. Pane output is operator UI only and is never scraped.
  const waited = await runCmd(["tmux", "wait-for", channel]);
  if (!waited.ok || !existsSync(exitPath)) throw new Error("visible command ended without an exit record");
  const rc = Number.parseInt(readFileSync(exitPath, "utf8").trim(), 10);
  state.stage = rc === 0 ? "completed" : "failed";
  state.exit_code = rc;
  if (retainPane) {
    state.pane_cleanup = "retained";
  } else {
    const released = await runCmd(["tmux", "kill-pane", "-t", paneId]);
    state.pane_cleanup = released.ok || /can't find pane|unknown pane/i.test(released.stderr)
      ? "released"
      : "release-failed";
  }
  writeFileSync(statePath, JSON.stringify(state, null, 2));
  emitJson(state);
  return rc;
}

if (import.meta.main) {
  main().then(code => process.exit(code)).catch(error => {
    console.error(error);
    process.exit(2);
  });
}
