#!/usr/bin/env bun
// Event-driven operator display for an evaluation that is already running.
// It follows durable JSONL artifacts with fs.watch; it never polls or controls
// the evaluation process.

import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  watch,
  type FSWatcher,
} from "fs";
import { basename, dirname, join, resolve } from "path";
import { arg } from "./cli/helpers.ts";
import {
  applyQualificationReport,
  applyScreenReport,
  createEvaluationDashboard,
  observeEvaluationLine,
  renderEvaluationDashboard,
} from "./lib/evaluation-dashboard.ts";

function identity(safe: string): string {
  const separator = safe.indexOf("_");
  return separator < 0 ? safe : `${safe.slice(0, separator)}/${safe.slice(separator + 1)}`;
}

export function lifecycleForLog(file: string): string | null {
  const name = basename(file);
  const screen = name.match(/^(.*)-attempt(\d+)\.jsonl$/);
  if (screen) return `=== SCREEN ${identity(screen[1])} attempt ${screen[2]} ===`;
  const qualification = name.match(/^(.*)-(?:multi|diagnose)(?:-resume)?\.jsonl$/);
  if (qualification) return `===== QUALIFY ${identity(qualification[1])} =====`;
  return null;
}

export function expectedModelsFromState(state: any): string[] {
  const command = Array.isArray(state?.command) ? state.command.join(" ") : "";
  const match = command.match(/--models\s+([^\s'"&]+)/);
  return match ? match[1].split(",").filter(Boolean) : [];
}

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const label = arg(args, "--label") ?? "model evaluation live view";
  const statePath = resolve(arg(args, "--state-file") ?? "features/.loop/model-eval/visible.json");
  const directories = [arg(args, "--screen-dir"), arg(args, "--qualification-dir")]
    .filter((value): value is string => Boolean(value))
    .map(value => resolve(value));
  if (directories.length === 0) throw new Error("evaluation-pane-follow requires --screen-dir and/or --qualification-dir");

  const color = !args.includes("--no-color") && process.env.NO_COLOR === undefined;
  let visibleState: any = {};
  try { visibleState = JSON.parse(readFileSync(statePath, "utf8")); } catch {}
  const dashboard = createEvaluationDashboard(expectedModelsFromState(visibleState));
  const draw = () => {
    if (color) process.stdout.write("\x1b[2J\x1b[H");
    process.stdout.write(renderEvaluationDashboard(dashboard, { color }).join("\n") + "\n");
  };

  const offsets = new Map<string, number>();
  const pending = new Map<string, string>();
  const announced = new Set<string>();
  const watchers = new Map<string, FSWatcher>();
  let finished = false;
  // Display-only heartbeat: redraw held state so a live pane visibly moves even
  // between model events. This timer never reads files or queries the worker.
  const heartbeat = setInterval(draw, 500);

  const readNew = (file: string) => {
    if (!existsSync(file) || !file.endsWith(".jsonl")) return;
    const bytes = readFileSync(file);
    let offset = offsets.get(file) ?? 0;
    if (bytes.length < offset) offset = 0;
    if (!announced.has(file)) {
      const lifecycle = lifecycleForLog(file);
      void lifecycle;
      announced.add(file);
    }
    if (bytes.length === offset) return;
    let text = (pending.get(file) ?? "") + bytes.subarray(offset).toString("utf8").replaceAll("\r\n", "\n");
    offsets.set(file, bytes.length);
    const lines = text.split("\n");
    text = lines.pop() ?? "";
    pending.set(file, text);
    for (const line of lines) observeEvaluationLine(dashboard, file, line);
  };

  const refreshReports = () => {
    const screen = directories.find(directory => /screen/i.test(basename(directory)));
    const qualification = directories.find(directory => /qualification/i.test(basename(directory)));
    try { if (screen && existsSync(join(screen, "results.json"))) applyScreenReport(dashboard, JSON.parse(readFileSync(join(screen, "results.json"), "utf8"))); } catch {}
    try { if (qualification && existsSync(join(qualification, "results.json"))) applyQualificationReport(dashboard, JSON.parse(readFileSync(join(qualification, "results.json"), "utf8"))); } catch {}
  };

  const scan = (directory: string) => {
    if (!existsSync(directory)) return;
    for (const file of readdirSync(directory)
      .filter(name => name.endsWith(".jsonl"))
      .map(name => join(directory, name))
      .sort((a, b) => statSync(a).mtimeMs - statSync(b).mtimeMs)) readNew(file);
  };

  const ensureWatcher = (directory: string) => {
    if (!existsSync(directory) || watchers.has(directory)) return;
    watchers.set(directory, watch(directory, (_event, filename) => {
      if (filename?.toString().endsWith(".jsonl")) readNew(join(directory, filename.toString()));
      refreshReports();
      draw();
    }));
  };

  const close = (stage: string) => {
    if (finished) return;
    finished = true;
    dashboard.terminalStage = stage === "completed" ? "completed" : "failed";
    clearInterval(heartbeat);
    for (const directory of directories) scan(directory);
    draw();
    for (const watcher of watchers.values()) watcher.close();
  };
  const checkState = () => {
    if (!existsSync(statePath)) return;
    try {
      const stage = JSON.parse(readFileSync(statePath, "utf8"))?.stage;
      if (["completed", "failed"].includes(stage)) close(stage);
    } catch { /* atomic rewrite in progress; the directory emits another event */ }
  };

  const parent = dirname(statePath);
  ensureWatcher(parent);
  const parentWatcher = watchers.get(parent);
  if (parentWatcher) {
    parentWatcher.on("change", () => {
      for (const directory of directories) ensureWatcher(directory);
      for (const directory of directories) scan(directory);
      refreshReports();
      draw();
      checkState();
    });
  }
  for (const directory of directories) ensureWatcher(directory);
  for (const directory of directories) scan(directory);
  refreshReports();
  draw();
  checkState();
  if (!finished) await new Promise<void>(resolve => {
    const intervalFreeExit = () => { if (finished) resolve(); };
    for (const watcher of watchers.values()) watcher.on("close", intervalFreeExit);
  });
  return 0;
}

if (import.meta.main) main().then(code => process.exit(code)).catch(error => { console.error(error); process.exit(2); });
