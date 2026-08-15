import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { bindReceipt, classifyEvent, finalize, newAccumulator } from "../opencode-worker-terminal.ts";
import { scanAudit } from "../lib/safety.ts";
import { runCmd } from "../cli/helpers.ts";

const RUNNER = join(import.meta.dir, "..", "opencode-worker-terminal.ts");

describe("OpenCode terminal evidence", () => {
  test("error events and missing sessions fail closed", () => {
    const acc = newAccumulator();
    classifyEvent(acc, JSON.stringify({ type: "error", message: "quota" }), "/work");
    expect(finalize(acc, 0, true).error_type).toBe("error-event");
    const clean = newAccumulator();
    expect(finalize(clean, 0, true).error_type).toBe("missing-session");
  });

  test("compound forbidden commands remain detectable in the audit", () => {
    const acc = newAccumulator();
    classifyEvent(acc, JSON.stringify({ type: "tool_use", part: { state: { input: { command: "cd src && git push origin main" } } } }), "/work");
    expect(scanAudit(acc.audit, "/work").map(x => x.kind)).toContain("push");
  });

  test("runner binds observed identity and writes terminal/audit files", async () => {
    const root = mkdtempSync(join(tmpdir(), "worker-terminal-"));
    const state = join(root, "state.json"), audit = join(root, "audit.json"), receipt = join(root, "receipt.json");
    writeFileSync(receipt, JSON.stringify({ schema_version: 1, status: "failed", summary: "bounded", forbidden_operations: [], test_commands: [], unverified_items: [], emitted_at: "2026-08-13T00:00:00Z" }));
    const child = `console.log(JSON.stringify({sessionID:"real-session"}));console.log(JSON.stringify({type:"tool_use",part:{state:{input:{command:"git status"}}}}));`;
    const result = await runCmd(["bun", RUNNER, "--out", state, "--audit", audit, "--receipt", receipt, "--issue", "7", "--worker", "w1", "--model", "opencode/hy3-free", "--", "bun", "-e", child], root);
    expect(result.exitCode).toBe(0);
    expect(JSON.parse(readFileSync(state, "utf-8")).session_id).toBe("real-session");
    expect(JSON.parse(readFileSync(audit, "utf-8"))).toHaveLength(1);
    const bound = JSON.parse(readFileSync(receipt, "utf-8"));
    expect(bound).toMatchObject({ issue: 7, worker_id: "w1", session_id: "real-session", model: { provider: "opencode", model: "hy3-free" } });
    expect(new Date(bound.emitted_at).getTime()).toBeGreaterThan(new Date("2026-08-13T00:00:00Z").getTime());
    rmSync(root, { recursive: true, force: true });
  });

  test("binding rejects a receipt that reports forbidden operations", () => {
    const root = mkdtempSync(join(tmpdir(), "worker-terminal-invalid-"));
    const receipt = join(root, "receipt.json");
    writeFileSync(receipt, JSON.stringify({
      schema_version: 1, status: "failed", summary: "bad contract",
      forbidden_operations: ["git push"], test_commands: [], unverified_items: [],
      emitted_at: "2026-08-13T00:00:00Z",
    }));
    expect(() => bindReceipt(receipt, { issue: 7, worker: "w1", model: "opencode/hy3-free", session: "s1" })).toThrow("invalid receipt");
    rmSync(root, { recursive: true, force: true });
  });
});
