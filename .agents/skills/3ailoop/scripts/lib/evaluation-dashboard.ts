import { basename } from "path";

export const SCREEN_GATES = ["response", "edit", "test", "commit", "receipt"] as const;
export type ScreenGate = typeof SCREEN_GATES[number];
export type QualificationStage = "multi" | "diagnose" | "ask-codex" | "resume";

export type EvaluationModelProgress = {
  identity: string;
  gates: Set<ScreenGate>;
  screenOutcome: "passed" | "failed" | "inconclusive" | null;
  qualificationScore: { passed: number; total: number; outcome: string } | null;
  qualificationStage: QualificationStage | null;
  workFinished: boolean;
  toolErrors: number;
  providerRetries: number;
  lastAction: string;
};

export type EvaluationDashboardState = {
  models: string[];
  progress: Map<string, EvaluationModelProgress>;
  currentModel: string | null;
  startedAt: number;
  lastEventAt: number;
  terminalStage: "completed" | "failed" | null;
};

function identity(safe: string): string {
  const separator = safe.indexOf("_");
  return separator < 0 ? safe : `${safe.slice(0, separator)}/${safe.slice(separator + 1)}`;
}

export function modelAndStageForLog(file: string): { identity: string; stage: "screen" | QualificationStage } | null {
  const name = basename(file);
  const screen = name.match(/^(.*)-attempt\d+\.jsonl$/);
  if (screen) return { identity: identity(screen[1]), stage: "screen" };
  const qualification = name.match(/^(.*)-(multi|diagnose)(-resume)?\.jsonl$/);
  if (qualification) return { identity: identity(qualification[1]), stage: qualification[3] ? "resume" : qualification[2] as "multi" | "diagnose" };
  return null;
}

export function createEvaluationDashboard(models: string[], now = Date.now()): EvaluationDashboardState {
  return { models, progress: new Map(), currentModel: null, startedAt: now, lastEventAt: now, terminalStage: null };
}

function model(state: EvaluationDashboardState, identity: string): EvaluationModelProgress {
  let value = state.progress.get(identity);
  if (!value) {
    value = {
      identity, gates: new Set(), screenOutcome: null, qualificationScore: null,
      qualificationStage: null, workFinished: false, toolErrors: 0,
      providerRetries: 0, lastAction: "開始待ち",
    };
    state.progress.set(identity, value);
    if (!state.models.includes(identity)) state.models.push(identity);
  }
  return value;
}

export function beginEvaluationStage(
  state: EvaluationDashboardState,
  identity: string,
  stage: "screen" | QualificationStage,
  observedAt = Date.now(),
): void {
  if (state.currentModel && state.currentModel !== identity) {
    const previous = model(state, state.currentModel);
    if (!previous.workFinished && previous.qualificationScore === null) {
      previous.workFinished = true;
      previous.lastAction = previous.qualificationStage ? "正式判定待ち" : "足切り判定待ち";
    }
  }
  const current = model(state, identity);
  state.currentModel = identity;
  state.lastEventAt = observedAt;
  current.workFinished = false;
  current.lastAction = stage === "screen" ? "モデル応答待ち" : `${stage}開始`;
  if (stage !== "screen") current.qualificationStage = stage;
}

export function observeEvaluationLine(state: EvaluationDashboardState, file: string, line: string, observedAt = Date.now()): void {
  const source = modelAndStageForLog(file);
  if (!source) return;
  if (state.currentModel !== source.identity) beginEvaluationStage(state, source.identity, source.stage, observedAt);
  const current = model(state, source.identity);
  state.currentModel = source.identity;
  state.lastEventAt = observedAt;
  if (source.stage !== "screen") current.qualificationStage = source.stage;
  try {
    const event = JSON.parse(line);
    if (event?.type === "step_start") {
      current.gates.add("response");
      current.lastAction = source.stage === "screen" ? "課題を調査中" : `${source.stage}課題を実行中`;
    }
    if (event?.type === "retry" || (event?.type === "session_status" && event?.status?.type === "retry")) {
      current.providerRetries += 1;
      current.lastAction = "provider再試行中";
    }
    if (event?.type === "session_idle") {
      current.workFinished = true;
      current.lastAction = source.stage === "screen" ? "検証器の判定待ち" : `${source.stage}の判定待ち`;
    }
    if (event?.type !== "tool_use") return;
    const tool = String(event?.part?.tool ?? "tool");
    const toolState = event?.part?.state ?? {};
    const status = toolState.status;
    const input = toolState.input ?? {};
    if (status === "error") {
      current.toolErrors += 1;
      current.lastAction = "許可外または無効な操作を拒否";
      return;
    }
    if (status === "running") {
      current.lastAction = tool === "bash" && /^cargo test\b/.test(String(input.command ?? ""))
        ? "テスト実行中"
        : tool === "edit" || tool === "write" ? "編集実行中" : "調査実行中";
      return;
    }
    if (status !== "completed") return;
    const filePath = String(input.filePath ?? "");
    const command = String(input.command ?? "");
    if ((tool === "edit" || tool === "write") && /\.eval\/receipt\.json$/.test(filePath)) current.gates.add("receipt");
    else if (tool === "edit" || tool === "write") current.gates.add("edit");
    if (tool === "bash" && /^cargo test\b/.test(command) && toolState.metadata?.exit === 0) current.gates.add("test");
    if (tool === "bash" && /(?:^|\s)git commit\b/.test(command) && toolState.metadata?.exit === 0) current.gates.add("commit");
  } catch { /* only normalized JSONL contributes to score */ }
}

export function applyScreenReport(state: EvaluationDashboardState, report: any): void {
  for (const result of report?.results ?? []) {
    const current = model(state, result.identity);
    current.screenOutcome = result.outcome ?? (result.passed ? "passed" : "failed");
    if (result.passed) for (const gate of SCREEN_GATES) current.gates.add(gate);
    current.workFinished = true;
  }
}

export function applyQualificationReport(state: EvaluationDashboardState, report: any): void {
  for (const result of report?.results ?? []) {
    const current = model(state, result.identity);
    const tasks = Array.isArray(result.tasks) ? result.tasks : [];
    current.qualificationScore = {
      passed: tasks.filter((task: any) => task?.passed).length,
      total: tasks.length || 4,
      outcome: result.outcome ?? (result.qualified ? "qualified" : "rejected"),
    };
    current.workFinished = true;
  }
}

function bar(value: number, total: number, width = 12): string {
  const filled = total === 0 ? 0 : Math.round(width * value / total);
  return `${"━".repeat(filled)}${"─".repeat(width - filled)}`;
}

function short(identity: string): string {
  const value = identity.replace(/^opencode\//, "").replace(/-free$/, "");
  return value.length > 24 ? `${value.slice(0, 23)}…` : value;
}

type DashboardRenderOptions = { color?: boolean; now?: number };

const COLOR = {
  reset: "\x1b[0m", bold: "\x1b[1m", cyan: "\x1b[38;5;45m", green: "\x1b[38;5;84m",
  yellow: "\x1b[38;5;220m", red: "\x1b[38;5;203m", violet: "\x1b[38;5;141m", dim: "\x1b[2m",
};

function paint(text: string, code: string, enabled: boolean): string {
  return enabled ? `${code}${text}${COLOR.reset}` : text;
}

export function renderEvaluationDashboard(state: EvaluationDashboardState, options: DashboardRenderOptions = {}): string[] {
  const color = options.color ?? false;
  const now = options.now ?? Date.now();
  const known = state.models.map(identity => model(state, identity));
  const screenDone = known.filter(item => item.screenOutcome !== null).length;
  const screenWorked = known.filter(item => item.workFinished || item.screenOutcome !== null).length;
  const started = known.filter(item => item.identity === state.currentModel || item.gates.size > 0 || item.providerRetries > 0 || item.toolErrors > 0).length;
  const active = Math.max(0, started - screenWorked);
  const qualificationDone = known.filter(item => item.qualificationScore !== null).length;
  const qualificationActive = known.filter(item => item.qualificationStage !== null && item.qualificationScore === null).length;
  const current = state.currentModel ? model(state, state.currentModel) : null;
  const provisional = current?.gates.size ?? 0;
  const pulse = ["◐", "◓", "◑", "◒"][Math.floor((now - state.startedAt) / 500) % 4];
  const runningSeconds = Math.max(0, Math.floor((now - state.startedAt) / 1_000));
  const secondsSinceEvent = Math.max(0, Math.floor((now - state.lastEventAt) / 1_000));
  const scanPosition = Math.floor((now - state.startedAt) / 500) % 12;
  const scan = `[${Array.from({ length: 12 }, (_, index) => index === scanPosition ? "◆" : "─").join("")}]`;
  const signal = [">  ", ">> ", ">>>", " >>"][Math.floor((now - state.startedAt) / 500) % 4];
  const lifecycle = state.terminalStage === "completed"
    ? paint("◆ COMPLETE", COLOR.bold + COLOR.green, color)
    : state.terminalStage === "failed"
      ? paint("■ STOPPED", COLOR.bold + COLOR.red, color)
      : paint(`${signal} RUNNING ${String(runningSeconds).padStart(5, "0")}s`, COLOR.bold + COLOR.green, color);
  const lines = [
    paint("╭────────────────────────────────────────────────────────╮", COLOR.cyan, color),
    `│  ${paint("◈  3AI LOOP", COLOR.bold + COLOR.cyan, color)}  /  ${paint("MODEL LAB · LIVE SCORE", COLOR.bold + COLOR.violet, color)}`,
    `│  ${lifecycle}  ${paint(scan, COLOR.cyan, color)}`,
    `│  ${paint(`${pulse} EVENT STREAM`, COLOR.cyan, color)}  last event ${secondsSinceEvent}s ago`,
    paint("╰────────────────────────────────────────────────────────╯", COLOR.cyan, color),
    `  ${paint("SCREEN", COLOR.bold + COLOR.cyan, color)}   ${paint(bar(screenWorked, known.length), COLOR.cyan, color)}  work ${screenWorked}/${known.length}`,
    `           active ${active} · wait ${Math.max(0, known.length - started)} · verdict ${screenDone}/${known.length}`,
    `  ${paint("QUALIFY", COLOR.bold + COLOR.violet, color)}  ${paint(bar(qualificationDone, Math.max(1, known.length)), COLOR.violet, color)}  done ${qualificationDone} · active ${qualificationActive}`,
    `  ${paint("PASS", COLOR.bold + COLOR.yellow, color)}     implementation 2/3 + production contract 1/1`,
    "",
  ];
  if (current) {
    lines.push(`  ${paint("▶ CURRENT", COLOR.bold + COLOR.green, color)}  ${paint(short(current.identity), COLOR.bold, color)}`);
    if (current.qualificationScore) {
      lines.push(`    正式スコア  ${current.qualificationScore.passed}/${current.qualificationScore.total}  ${current.qualificationScore.outcome}`);
    } else if (current.qualificationStage) {
      lines.push(`    本評価中    正式点はgrader確定待ち · stage=${current.qualificationStage}`);
    } else {
      lines.push(`    暫定スコア  ${provisional}/5`);
      lines.push("    gates       response · edit · test · commit · receipt");
    }
    lines.push(`    状態        ${current.lastAction}`);
    lines.push(`    errors      policy ${current.toolErrors} · provider retry ${current.providerRetries}`);
    lines.push("                ※ scoreとは別集計");
    lines.push("");
  }
  lines.push("  CANDIDATES");
  for (const item of known) {
    const outcome = item.qualificationScore
      ? `${item.qualificationScore.passed}/${item.qualificationScore.total} ${item.qualificationScore.outcome}`
      : item.screenOutcome ? `screen ${item.screenOutcome}`
      : `${item.gates.size}/5 暫定`;
    const marker = item.identity === state.currentModel ? paint("◆", COLOR.green, color) : item.screenOutcome === "passed" ? paint("✓", COLOR.green, color) : item.workFinished ? paint("◇", COLOR.yellow, color) : paint("·", COLOR.dim, color);
    lines.push(`  ${marker} ${short(item.identity).padEnd(25)} ${outcome.padEnd(16)} E${item.toolErrors} R${item.providerRetries}`);
  }
  return lines;
}
