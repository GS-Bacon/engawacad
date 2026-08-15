import { describe, expect, test } from "bun:test";
import { mkdtempSync } from "fs";
import { join, resolve } from "path";
import { tmpdir } from "os";
import { renderBanner, renderPaneLine, type PaneRenderState } from "../pane-render.ts";

function state(): PaneRenderState {
  return { label: "model evaluation", startedAt: 1_000, step: 0, model: null };
}

const options = { color: false, now: 6_000 };

describe("pane-render", () => {
  test("renders a line from a shell pipeline before stdin reaches EOF", async () => {
    const log = join(mkdtempSync(join(tmpdir(), "pane-live-")), "raw.log");
    const script = resolve(import.meta.dir, "../pane-render.ts");
    const proc = Bun.spawn([
      "bash", "-lc",
      `{ printf '[live] started\\n'; read -r _; } | bun '${script}' --label live --log '${log}' --no-color`,
    ], { stdin: "pipe", stdout: "pipe", stderr: "pipe" });
    const reader = proc.stdout.getReader();
    let output = "";
    const readUntilRunning = (async () => {
      while (!output.includes("RUNNING")) {
        const item = await reader.read();
        if (item.done) break;
        output += new TextDecoder().decode(item.value);
      }
    })();
    proc.stdin.write("[live] started\n");
    await proc.stdin.flush();
    await Promise.race([readUntilRunning, new Promise((_, reject) => setTimeout(() => reject(new Error("renderer buffered until EOF")), 1_000))]);
    expect(output).toContain("RUNNING");
    proc.stdin.write("release\n");
    await proc.stdin.flush();
    proc.stdin.end();
    await proc.exited;
  });

  test("renders a compact branded header", () => {
    expect(renderBanner("model evaluation", false).join("\n")).toContain("3AI LOOP   / MODEL LAB");
  });

  test("renders the animated live-score dashboard for evaluation mode", async () => {
    const log = join(mkdtempSync(join(tmpdir(), "pane-eval-")), "raw.log");
    const script = resolve(import.meta.dir, "../pane-render.ts");
    const proc = Bun.spawn(["bun", script, "--label", "eval", "--log", log, "--evaluation-models", "opencode/a-free,opencode/b-free", "--no-color"], { stdin: "pipe", stdout: "pipe", stderr: "pipe" });
    const reader = proc.stdout.getReader();
    let output = "";
    const until = async (needle: string) => {
      while (!output.includes(needle)) {
        const item = await reader.read();
        if (item.done) break;
        output += new TextDecoder().decode(item.value);
      }
    };
    proc.stdin.write('=== SCREEN opencode/a-free attempt 1 ===\n{"type":"step_start"}\n');
    await proc.stdin.flush();
    await Promise.race([until("1/5"), new Promise((_, reject) => setTimeout(() => reject(new Error("live score did not update")), 1_000))]);
    expect(output).toContain("LIVE SCORE");
    expect(output).toContain("CURRENT  a");
    proc.stdin.write("[eval] finished exit=0\n");
    proc.stdin.end();
    expect(await proc.exited).toBe(0);
  });

  test("shows screen lifecycle and waiting state", () => {
    const output = renderPaneLine("=== SCREEN opencode/hyne-3 attempt 2 ===", state(), options).join("\n");
    expect(output).toContain("SCREEN");
    expect(output).toContain("opencode/hyne-3");
    expect(output).toContain("attempt 2");
    expect(output).toContain("WAIT");
  });

  test("summarizes tools without exposing full model output", () => {
    const edit = JSON.stringify({
      type: "tool_use",
      part: { tool: "edit", state: { status: "completed", input: { filePath: "/repo/src/shape.rs", patch: "SECRET PATCH" } } },
    });
    const output = renderPaneLine(edit, state(), options).join("\n");
    expect(output).toContain("EDIT");
    expect(output).toContain("src/shape.rs");
    expect(output).not.toContain("SECRET PATCH");

    const text = JSON.stringify({ type: "text", part: { text: "private chain of thought" } });
    expect(renderPaneLine(text, state(), options)).toEqual([]);
  });

  test("does not mislabel pending or running tools as blocked", () => {
    const pending = JSON.stringify({ type: "tool_use", part: { tool: "read", state: { status: "pending", input: { filePath: "/tmp/a" } } } });
    const running = JSON.stringify({ type: "tool_use", part: { tool: "read", state: { status: "running", input: { filePath: "/tmp/a" } } } });
    const failed = JSON.stringify({ type: "tool_use", part: { tool: "read", state: { status: "error", input: { filePath: "/tmp/a" } } } });
    expect(renderPaneLine(pending, state(), options)).toEqual([]);
    expect(renderPaneLine(running, state(), options).join("\n")).toContain("active");
    expect(renderPaneLine(running, state(), options).join("\n")).not.toContain("blocked");
    expect(renderPaneLine(failed, state(), options).join("\n")).toContain("blocked");
  });

  test("recognizes test commands but hides arbitrary shell details", () => {
    const testEvent = JSON.stringify({
      type: "tool_use",
      part: { tool: "bash", state: { status: "completed", input: { command: "cargo test --workspace" } } },
    });
    expect(renderPaneLine(testEvent, state(), options).join("\n")).toContain("TEST");

    const shellEvent = JSON.stringify({
      type: "tool_use",
      part: { tool: "bash", state: { status: "completed", input: { command: "curl https://secret.invalid/token" } } },
    });
    const output = renderPaneLine(shellEvent, state(), options).join("\n");
    expect(output).toContain("SHELL");
    expect(output).not.toContain("secret.invalid");
  });

  test("redacts credentials in errors", () => {
    const event = JSON.stringify({ type: "error", error: { message: "\u001b[2Jprovider rejected sk-secret1234567890" } });
    const output = renderPaneLine(event, state(), options).join("\n");
    expect(output).toContain("ERROR");
    expect(output).not.toContain("\u001b[2J");
    expect(output).not.toContain("sk-secret1234567890");
  });

  test("renders final verdicts and suppresses JSON reports", () => {
    expect(renderPaneLine("=== RESULT model-x: PASSED (all gates) ===", state(), options).join("\n")).toContain("PASSED");
    expect(renderPaneLine("=== RESULT model-x: INCONCLUSIVE (provider timeout) ===", state(), options).join("\n")).toContain("INCONCLUSIVE");
    expect(renderPaneLine(JSON.stringify({ report: "verbose report" }), state(), options)).toEqual([]);
  });

  test("shows provider retry events without exposing error bodies", () => {
    const event = JSON.stringify({ type: "retry", error: { data: { message: "sensitive provider body" } } });
    const output = renderPaneLine(event, state(), options).join("\n");
    expect(output).toContain("RETRY");
    expect(output).not.toContain("sensitive provider body");
  });

  test("distinguishes an inconclusive qualification from rejection", () => {
    const output = renderPaneLine("===== opencode/model: INCONCLUSIVE multi-crate/rate-limit =====", state(), options).join("\n");
    expect(output).toContain("INCONCLUSIVE");
    expect(output).not.toContain("REJECTED");
  });
});
