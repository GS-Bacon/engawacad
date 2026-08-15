import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "fs";
import { join, resolve } from "path";
import { tmpdir } from "os";
import { lifecycleForLog } from "../evaluation-pane-follow.ts";

describe("evaluation-pane-follow", () => {
  test("derives safe lifecycle labels from evaluation artifact names", () => {
    expect(lifecycleForLog("opencode_hy3-free-attempt2.jsonl")).toBe("=== SCREEN opencode/hy3-free attempt 2 ===");
    expect(lifecycleForLog("opencode_hy3-free-consult.jsonl")).toBe("===== QUALIFY opencode/hy3-free =====");
  });

  test("renders appended events and exits from terminal state without polling", async () => {
    const root = mkdtempSync(join(tmpdir(), "eval-follow-"));
    const screen = join(root, "screen");
    const qualify = join(root, "qualify");
    const state = join(root, "visible.json");
    mkdirSync(screen);
    writeFileSync(state, JSON.stringify({ stage: "running" }));
    const script = resolve(import.meta.dir, "../evaluation-pane-follow.ts");
    const proc = Bun.spawn([
      "bun", script, "--label", "live", "--state-file", state,
      "--screen-dir", screen, "--qualification-dir", qualify, "--no-color",
    ], { stdout: "pipe", stderr: "pipe" });
    const reader = proc.stdout.getReader();
    let output = "";
    const until = async (needle: string) => {
      while (!output.includes(needle)) {
        const item = await reader.read();
        if (item.done) break;
        output += new TextDecoder().decode(item.value);
      }
    };
    await Promise.race([until("LIVE SCORE"), new Promise((_, reject) => setTimeout(() => reject(new Error("follower did not start")), 1_000))]);
    writeFileSync(join(screen, "opencode_hy3-free-attempt1.jsonl"), JSON.stringify({ type: "step_start" }) + "\n");
    await Promise.race([until("1/5"), new Promise((_, reject) => setTimeout(() => reject(new Error("follower did not render append")), 1_000))]);
    writeFileSync(state, JSON.stringify({ stage: "completed" }));
    const exit = await proc.exited;
    if (exit !== 0) throw new Error(`follower exited ${exit}: ${await new Response(proc.stderr).text()}`);
    expect(exit).toBe(0);
    expect(output).toContain("CURRENT  hy3");
  });
});
