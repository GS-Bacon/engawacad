import { describe, expect, test } from "bun:test";
import { classifyInfrastructureEvent, decodeSseChunk, normalizeOpenCodeEvent } from "../lib/opencode-evaluation-run.ts";

describe("OpenCode evaluation events", () => {
  test("decodes fragmented SSE without polling", () => {
    const first = decodeSseChunk('data: {"type":"session.status","properties":');
    expect(first.events).toEqual([]);
    const second = decodeSseChunk(first.rest + '{"sessionID":"s1","status":{"type":"busy"}}}\n\n');
    expect((second.events[0] as any).type).toBe("session.status");
    expect(second.rest).toBe("");
  });

  test("classifies rate limits and Retry-After immediately", () => {
    const event = { type: "session.error", properties: { error: { data: { statusCode: 429, message: "FreeUsageLimitError", responseHeaders: { "retry-after": "60" } } } } };
    expect(classifyInfrastructureEvent(event)).toEqual({ kind: "rate-limit", retryAfterMs: 60_000 });
  });

  test("classifies OpenCode free allowance retry text as a rate limit", () => {
    const event = {
      type: "session.status",
      properties: {
        sessionID: "s1",
        status: { type: "retry", attempt: 1, message: "Free usage exceeded, subscribe to Go https://opencode.ai/go" },
      },
    };
    expect(classifyInfrastructureEvent(event)?.kind).toBe("rate-limit");
  });

  test("classifies upstream and connection failures separately", () => {
    expect(classifyInfrastructureEvent({ type: "session.error", properties: { error: { data: { statusCode: 502 } } } })?.kind).toBe("upstream");
    expect(classifyInfrastructureEvent({ type: "session.error", properties: { error: { message: "socket connection was closed" } } })?.kind).toBe("connection");
    expect(classifyInfrastructureEvent({ type: "session.error", properties: { error: { message: "assertion mismatch" } } })).toBeNull();
    expect(classifyInfrastructureEvent({ type: "message.part.updated", properties: { part: { type: "text", text: "a test returned 429" } } })).toBeNull();
  });

  test("normalizes only the selected session and hides reasoning", () => {
    const tool = normalizeOpenCodeEvent({ type: "message.part.updated", properties: { part: { sessionID: "s1", type: "tool", tool: "edit", state: { status: "completed", input: { filePath: "src/lib.rs" } } } } }, "s1");
    expect(JSON.parse(tool.line!).type).toBe("tool_use");
    expect(tool.modelActivity).toBe(true);
    expect(normalizeOpenCodeEvent({ type: "message.part.updated", properties: { part: { sessionID: "s1", type: "reasoning", text: "secret" } } }, "s1").line).toBeNull();
    expect(normalizeOpenCodeEvent({ type: "session.idle", properties: { sessionID: "other" } }, "s1").terminal).toBeNull();
    expect(normalizeOpenCodeEvent({ type: "session.status", properties: { sessionID: "s1", status: { type: "idle" } } }, "s1").terminal).toBe("idle");
  });
});
