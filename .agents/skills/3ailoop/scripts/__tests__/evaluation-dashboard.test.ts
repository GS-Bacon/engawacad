import { describe, expect, test } from "bun:test";
import {
  applyQualificationReport,
  applyScreenReport,
  createEvaluationDashboard,
  observeEvaluationLine,
  renderEvaluationDashboard,
} from "../lib/evaluation-dashboard.ts";

const file = "opencode_hy3-free-attempt1.jsonl";

describe("evaluation dashboard", () => {
  test("scores only completed screening gates and separates errors from retries", () => {
    const state = createEvaluationDashboard(["opencode/hy3-free", "opencode/next-free"]);
    observeEvaluationLine(state, file, JSON.stringify({ type: "step_start" }));
    observeEvaluationLine(state, file, JSON.stringify({ type: "tool_use", part: { tool: "edit", state: { status: "pending", input: { filePath: "/tmp/src/lib.rs" } } } }));
    observeEvaluationLine(state, file, JSON.stringify({ type: "tool_use", part: { tool: "edit", state: { status: "completed", input: { filePath: "/tmp/src/lib.rs" } } } }));
    observeEvaluationLine(state, file, JSON.stringify({ type: "tool_use", part: { tool: "bash", state: { status: "error", input: { command: "ls" } } } }));
    observeEvaluationLine(state, file, JSON.stringify({ type: "session_status", status: { type: "retry" } }));
    const output = renderEvaluationDashboard(state).join("\n");
    expect(output).toContain("2/5");
    expect(output).toContain("policy 1");
    expect(output).toContain("provider retry 1");
    expect(output).toContain("active 1");
  });

  test("shows formal screen and qualification results when graders publish reports", () => {
    const state = createEvaluationDashboard(["opencode/hy3-free"]);
    applyScreenReport(state, { results: [{ identity: "opencode/hy3-free", passed: true, outcome: "passed" }] });
    applyQualificationReport(state, { results: [{ identity: "opencode/hy3-free", outcome: "qualified", tasks: [{ passed: true }, { passed: true }, { passed: false }, { passed: true }] }] });
    state.currentModel = "opencode/hy3-free";
    const output = renderEvaluationDashboard(state).join("\n");
    expect(output).toContain("正式スコア  3/4  qualified");
    expect(output).toContain("verdict 1/1");
  });

  test("counts the previous candidate as work-finished when the next one starts", () => {
    const state = createEvaluationDashboard(["opencode/first-free", "opencode/second-free"]);
    observeEvaluationLine(state, "opencode_first-free-attempt1.jsonl", JSON.stringify({ type: "step_start" }));
    observeEvaluationLine(state, "opencode_second-free-attempt1.jsonl", JSON.stringify({ type: "step_start" }));
    const output = renderEvaluationDashboard(state).join("\n");
    expect(output).toContain("work 1/2");
    expect(output).toContain("active 1 · wait 0");
  });

  test("shows an animated LIVE heartbeat and terminal STOPPED state", () => {
    const state = createEvaluationDashboard([], 1_000);
    expect(renderEvaluationDashboard(state, { now: 1_000 }).join("\n")).toContain(">   RUNNING 00000s");
    expect(renderEvaluationDashboard(state, { now: 1_500 }).join("\n")).toContain(">>  RUNNING 00000s");
    expect(renderEvaluationDashboard(state, { now: 2_000 }).join("\n")).toContain("RUNNING 00001s");
    state.terminalStage = "failed";
    expect(renderEvaluationDashboard(state, { now: 2_000 }).join("\n")).toContain("■ STOPPED");
  });
});
