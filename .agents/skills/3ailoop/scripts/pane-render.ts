#!/usr/bin/env bun
// Operator-only renderer for 3ailoop right-side panes. It turns OpenCode JSONL and
// evaluation lifecycle lines into a compact terminal dashboard while preserving the
// exact raw stream in the durable log supplied by visible-command.ts.

import { appendFileSync, mkdirSync } from "fs";
import { dirname } from "path";
import { arg } from "./cli/helpers.ts";
import { redact } from "./lib/worker-pane.ts";
import {
  applyScreenReport,
  beginEvaluationStage,
  createEvaluationDashboard,
  observeEvaluationLine,
  renderEvaluationDashboard,
} from "./lib/evaluation-dashboard.ts";

export type PaneRenderState = {
  label: string;
  startedAt: number;
  step: number;
  model: string | null;
};

type RenderOptions = { color: boolean; now: number };

const ANSI = {
  reset: "\x1b[0m",
  dim: "\x1b[2m",
  bold: "\x1b[1m",
  cyan: "\x1b[38;5;45m",
  blue: "\x1b[38;5;75m",
  green: "\x1b[38;5;84m",
  yellow: "\x1b[38;5;220m",
  red: "\x1b[38;5;203m",
  violet: "\x1b[38;5;141m",
};

function paint(text: string, code: string, enabled: boolean): string {
  return enabled ? `${code}${text}${ANSI.reset}` : text;
}

function compact(value: unknown, max = 82): string {
  const safe = redact(String(value ?? ""))
    .replace(/\x1B(?:[@-_]|\[[0-?]*[ -/]*[@-~])/g, "")
    .replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F-\u009F]/g, "")
    .replace(/\s+/g, " ")
    .trim();
  return safe.length > max ? `${safe.slice(0, max - 1)}…` : safe;
}

function shortPath(value: unknown): string {
  const parts = String(value ?? "").split("/").filter(Boolean);
  return compact(parts.slice(-3).join("/"), 58) || "workspace";
}

function elapsed(startedAt: number, now: number): string {
  const total = Math.max(0, Math.floor((now - startedAt) / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return minutes > 0 ? `${minutes}m${String(seconds).padStart(2, "0")}s` : `${seconds}s`;
}

function toolSummary(event: any): { tag: string; detail: string; ok: boolean } {
  const tool = String(event?.part?.tool ?? "tool");
  const state = event?.part?.state ?? {};
  const input = state.input ?? {};
  const ok = state.status === "completed";
  if (tool === "read" || tool === "glob" || tool === "grep" || tool === "list") {
    return { tag: "LOOK", detail: shortPath(input.filePath ?? input.path ?? input.pattern), ok };
  }
  if (tool === "edit" || tool === "write") {
    return { tag: "EDIT", detail: shortPath(input.filePath), ok };
  }
  if (tool === "bash") {
    const command = String(input.command ?? "");
    if (/^cargo\s+(?:test|xtask\s+ci)\b/.test(command)) return { tag: "TEST", detail: compact(command, 58), ok };
    if (/\bgit\s+commit\b/.test(command)) return { tag: "COMMIT", detail: "local commit", ok };
    if (/\bgit\s+(?:status|diff|log)\b/.test(command)) return { tag: "CHECK", detail: compact(command, 58), ok };
    return { tag: "SHELL", detail: "workspace command", ok };
  }
  return { tag: tool.toUpperCase().slice(0, 8), detail: "tool call", ok };
}

export function renderBanner(label: string, color = true): string[] {
  const title = paint("◈  3AI LOOP", ANSI.bold + ANSI.cyan, color);
  return [
    "╭────────────────────────────────────────────────────────────────────╮",
    `│  ${title}   ${paint("/ MODEL LAB", ANSI.violet, color)}`,
    `│  ${paint(compact(label, 58), ANSI.dim, color)}`,
    "╰────────────────────────────────────────────────────────────────────╯",
  ];
}

export function renderPaneLine(line: string, state: PaneRenderState, options: RenderOptions): string[] {
  const color = options.color;
  const age = paint(elapsed(state.startedAt, options.now).padStart(7), ANSI.dim, color);
  const screen = line.match(/^=== SCREEN (\S+) attempt (\d+) ===$/);
  if (screen) {
    state.model = screen[1];
    state.step = 0;
    return [
      "",
      `${paint("◆ SCREEN", ANSI.bold + ANSI.cyan, color)}  ${paint(screen[1], ANSI.bold, color)}  ${paint(`attempt ${screen[2]}`, ANSI.violet, color)}  ${age}`,
      `${paint("  ◌ WAIT", ANSI.yellow, color)}   first model event · deadline guarded`,
    ];
  }

  const result = line.match(/^=== RESULT (\S+): (PASSED|FAILED|INCONCLUSIVE) \((.*)\) ===$/);
  if (result) {
    const verdictColor = result[2] === "PASSED" ? ANSI.green : result[2] === "FAILED" ? ANSI.red : ANSI.yellow;
    const icon = result[2] === "PASSED" ? "✓" : result[2] === "FAILED" ? "✕" : "◇";
    return [`${paint(`  ${icon} ${result[2]}`, ANSI.bold + verdictColor, color)}  ${paint(compact(result[3], 70), ANSI.dim, color)}  ${age}`];
  }

  const qualify = line.match(/^===== QUALIFY (\S+) =====$/);
  if (qualify) {
    state.model = qualify[1];
    state.step = 0;
    return ["", `${paint("◆ QUALIFY", ANSI.bold + ANSI.violet, color)} ${paint(qualify[1], ANSI.bold, color)}  ${age}`];
  }
  const qualified = line.match(/^===== (\S+): (QUALIFIED|REJECTED|INCONCLUSIVE) (.*) =====$/);
  if (qualified) {
    const ok = qualified[2] === "QUALIFIED";
    const inconclusive = qualified[2] === "INCONCLUSIVE";
    return [`${paint(ok ? "  ★ QUALIFIED" : inconclusive ? "  ◇ INCONCLUSIVE" : "  ✕ REJECTED", ANSI.bold + (ok ? ANSI.green : inconclusive ? ANSI.yellow : ANSI.red), color)}  ${paint(compact(qualified[3]), ANSI.dim, color)}  ${age}`];
  }

  if (/^\[(?:multi-crate|diagnose|ask-codex|resume)/.test(line)) {
    return [`${paint("  ▸ STAGE", ANSI.blue, color)}  ${compact(line.replace(/^\[|\]$/g, ""), 64)}  ${age}`];
  }
  if (/infrastructure unavailable; retrying/i.test(line)) {
    return [`${paint("  ↻ RETRY", ANSI.yellow, color)}  transient provider failure · fresh attempt  ${age}`];
  }
  if (/^\[.+\] started$/.test(line)) return [`${paint("● RUNNING", ANSI.green, color)}  ${age}`];
  const finished = line.match(/^\[.+\] finished exit=(\d+)$/);
  if (finished) {
    const ok = finished[1] === "0";
    return ["", `${paint(ok ? "● COMPLETE" : "● STOPPED", ANSI.bold + (ok ? ANSI.green : ANSI.red), color)}  exit=${finished[1]}  ${age}`];
  }

  try {
    const event = JSON.parse(line);
    if (event?.type === "step_start") {
      state.step += 1;
      return [`${paint(`  ◇ STEP ${String(state.step).padStart(2, "0")}`, ANSI.dim, color)}  model active  ${age}`];
    }
  if (event?.type === "tool_use") {
      const summary = toolSummary(event);
      const status = event?.part?.state?.status;
      if (status === "pending") return [];
      const failed = status === "error";
      const icon = summary.ok ? "├─" : failed ? "!─" : "…─";
      const tone = summary.ok ? ANSI.blue : failed ? ANSI.red : ANSI.yellow;
      const suffix = summary.ok ? paint("✓", ANSI.green, color) : failed ? paint("blocked", ANSI.red, color) : paint("active", ANSI.yellow, color);
      return [`${paint(`  ${icon} ${summary.tag.padEnd(6)}`, tone, color)} ${summary.detail} ${suffix}  ${age}`];
    }
    if (event?.type === "error") {
      const message = event?.error?.data?.message ?? event?.error?.message ?? event?.error ?? "model error";
      return [`${paint("  ✕ ERROR", ANSI.bold + ANSI.red, color)}  ${compact(message, 78)}  ${age}`];
    }
    if (event?.type === "retry" || (event?.type === "session_status" && event?.status?.type === "retry")) {
      return [`${paint("  ↻ RETRY", ANSI.yellow, color)}  provider backoff detected  ${age}`];
    }
    // Never display model text/reasoning or pretty JSON reports in the operator pane.
    return [];
  } catch { /* ordinary command output */ }

  if (line.trim() === "") return [];
  return [`${paint("  │", ANSI.dim, color)} ${compact(line, 86)}  ${age}`];
}

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const label = arg(args, "--label") ?? "3ailoop worker";
  const logPath = arg(args, "--log");
  const color = !args.includes("--no-color") && process.env.NO_COLOR === undefined;
  const evaluationModels = (arg(args, "--evaluation-models") ?? "").split(",").filter(Boolean);
  if (!logPath) throw new Error("pane-render requires --log <raw-log-path>");
  mkdirSync(dirname(logPath), { recursive: true });
  if (evaluationModels.length > 0) {
    const dashboard = createEvaluationDashboard(evaluationModels);
    let currentFile: string | null = null;
    const draw = () => {
      if (color) process.stdout.write("\x1b[2J\x1b[H");
      process.stdout.write(renderEvaluationDashboard(dashboard, { color }).join("\n") + "\n");
    };
    const safe = (identity: string) => identity.replaceAll(/[^a-zA-Z0-9._-]/g, "_");
    const renderEvaluation = (line: string) => {
      appendFileSync(logPath, line + "\n");
      const screen = line.match(/^=== SCREEN (\S+) attempt (\d+) ===$/);
      if (screen) {
        beginEvaluationStage(dashboard, screen[1], "screen");
        currentFile = `${safe(screen[1])}-attempt${screen[2]}.jsonl`;
      }
      const screenResult = line.match(/^=== RESULT (\S+): (PASSED|FAILED|INCONCLUSIVE)/);
      if (screenResult) applyScreenReport(dashboard, { results: [{ identity: screenResult[1], outcome: screenResult[2].toLowerCase(), passed: screenResult[2] === "PASSED" }] });
      const qualify = line.match(/^===== QUALIFY (\S+) =====$/);
      if (qualify) {
        beginEvaluationStage(dashboard, qualify[1], "multi");
        currentFile = `${safe(qualify[1])}-multi.jsonl`;
      }
      if (/^\[(multi-crate|diagnose|ask-codex|resume)/.test(line) && dashboard.currentModel) {
        const stage = line.startsWith("[ask-codex") ? "ask-codex" : line.startsWith("[resume") ? "resume" : line.startsWith("[diagnose") ? "diagnose" : "multi";
        beginEvaluationStage(dashboard, dashboard.currentModel, stage);
        currentFile = `${safe(dashboard.currentModel)}-${stage === "resume" ? "diagnose-resume" : stage}.jsonl`;
      }
      if (currentFile && line.startsWith("{")) observeEvaluationLine(dashboard, currentFile, line);
      const finished = line.match(/^\[.+\] finished exit=(\d+)$/);
      if (finished) dashboard.terminalStage = finished[1] === "0" ? "completed" : "failed";
      draw();
    };
    draw();
    // Animation only; it redraws held state and never queries the worker.
    const heartbeat = setInterval(draw, 500);
    let pending = "";
    const consume = (chunk: string) => {
      pending += chunk.replaceAll("\r\n", "\n");
      const lines = pending.split("\n");
      pending = lines.pop() ?? "";
      for (const line of lines) renderEvaluation(line);
    };
    await new Promise<void>((resolve, reject) => {
      process.stdin.setEncoding("utf8");
      process.stdin.on("data", consume);
      process.stdin.once("end", resolve);
      process.stdin.once("error", reject);
      process.stdin.resume();
    });
    clearInterval(heartbeat);
    if (pending) renderEvaluation(pending);
    return 0;
  }
  const state: PaneRenderState = { label, startedAt: Date.now(), step: 0, model: null };
  if (color) process.stdout.write("\x1b[2J\x1b[H");
  process.stdout.write(renderBanner(label, color).join("\n") + "\n");
  const render = (line: string) => {
    appendFileSync(logPath, line + "\n");
    const rendered = renderPaneLine(line, state, { color, now: Date.now() });
    if (rendered.length > 0) process.stdout.write(rendered.join("\n") + "\n");
  };
  let pending = "";
  const consume = (chunk: string) => {
    pending += chunk.replaceAll("\r\n", "\n");
    const lines = pending.split("\n");
    pending = lines.pop() ?? "";
    for (const line of lines) render(line);
  };
  // Bun.stdin.stream() works for a direct subprocess pipe but can remain silent when
  // this renderer is the final process in a bash pipeline inside tmux. Node's data
  // events cover both cases and deliver chunks before EOF.
  await new Promise<void>((resolve, reject) => {
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", consume);
    process.stdin.once("end", resolve);
    process.stdin.once("error", reject);
    process.stdin.resume();
  });
  if (pending) render(pending);
  return 0;
}

if (import.meta.main) main().then(code => process.exit(code)).catch(error => { console.error(error); process.exit(2); });
