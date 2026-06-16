// loop-tmux-watcher.test.ts — pure 関数の単体テスト
// 副作用 (tmux/gh/fs) は除外し、差分検知・アクション決定ロジックのみ検証。

import { describe, expect, it } from "bun:test";
import {
  decideAction,
  detectCycleCompleted,
  extractLastEndedAt,
} from "./loop-tmux-watcher.ts";

describe("extractLastEndedAt", () => {
  it("returns null for null state", () => {
    expect(extractLastEndedAt(null)).toBeNull();
  });
  it("returns null for empty recent_cycles", () => {
    expect(extractLastEndedAt({ recent_cycles: [] })).toBeNull();
  });
  it("returns null when recent_cycles is missing", () => {
    expect(extractLastEndedAt({})).toBeNull();
  });
  it("returns ended_at of last cycle", () => {
    const state = {
      recent_cycles: [
        { ended_at: "2026-06-16T00:00:00.000Z" },
        { ended_at: "2026-06-16T01:00:00.000Z" },
        { ended_at: "2026-06-16T02:00:00.000Z" },
      ],
    };
    expect(extractLastEndedAt(state)).toBe("2026-06-16T02:00:00.000Z");
  });
  it("returns null if last cycle has no ended_at", () => {
    const state = { recent_cycles: [{}] };
    expect(extractLastEndedAt(state)).toBeNull();
  });
});

describe("detectCycleCompleted", () => {
  it("returns false when curr is null", () => {
    expect(detectCycleCompleted(null, null)).toBe(false);
    expect(detectCycleCompleted("2026-06-16T00:00:00.000Z", null)).toBe(false);
  });
  it("returns false when prev is null (baseline must be established first; #181 F01)", () => {
    expect(detectCycleCompleted(null, "2026-06-16T00:00:00.000Z")).toBe(false);
  });
  it("returns true when prev and curr differ", () => {
    expect(
      detectCycleCompleted("2026-06-16T00:00:00.000Z", "2026-06-16T01:00:00.000Z"),
    ).toBe(true);
  });
  it("returns false when prev and curr are equal", () => {
    expect(
      detectCycleCompleted("2026-06-16T00:00:00.000Z", "2026-06-16T00:00:00.000Z"),
    ).toBe(false);
  });
});

describe("decideAction", () => {
  const baseInput = {
    prevEndedAt: null as string | null,
    currEndedAt: null as string | null,
    shouldStopRc: null as number | null,
    minutesSinceLastUpdate: 0,
    stuckThresholdMin: 45,
    workerPaneAlive: true,
  };

  it("worker-gone overrides every other condition", () => {
    const action = decideAction({
      ...baseInput,
      workerPaneAlive: false,
      prevEndedAt: "2026-06-16T00:00:00.000Z",
      currEndedAt: "2026-06-16T01:00:00.000Z",
      shouldStopRc: 0,
    });
    expect(action.kind).toBe("worker-gone");
  });

  it("returns wait when no cycle completion and not stuck", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: "2026-06-16T00:00:00.000Z",
      currEndedAt: "2026-06-16T00:00:00.000Z",
      minutesSinceLastUpdate: 5,
    });
    expect(action.kind).toBe("wait");
  });

  it("returns stuck when no diff and minutes exceed threshold", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: "2026-06-16T00:00:00.000Z",
      currEndedAt: "2026-06-16T00:00:00.000Z",
      minutesSinceLastUpdate: 50,
      stuckThresholdMin: 45,
    });
    expect(action.kind).toBe("stuck");
    if (action.kind === "stuck") expect(action.minutesIdle).toBe(50);
  });

  it("does NOT return stuck when currEndedAt is null (state.json not yet written)", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: null,
      currEndedAt: null,
      minutesSinceLastUpdate: 100,
      stuckThresholdMin: 45,
    });
    expect(action.kind).toBe("wait");
  });

  it("returns send-clear-and-restart on cycle completion + shouldStopRc=0", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: "2026-06-16T00:00:00.000Z",
      currEndedAt: "2026-06-16T01:00:00.000Z",
      shouldStopRc: 0,
    });
    expect(action.kind).toBe("send-clear-and-restart");
  });

  it("returns stop on cycle completion + shouldStopRc=1", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: "2026-06-16T00:00:00.000Z",
      currEndedAt: "2026-06-16T01:00:00.000Z",
      shouldStopRc: 1,
    });
    expect(action.kind).toBe("stop");
  });

  it("first poll with prev=null + curr=value → init-baseline (#181 F01)", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: null,
      currEndedAt: "2026-06-16T00:00:00.000Z",
      shouldStopRc: null,
    });
    expect(action.kind).toBe("init-baseline");
    if (action.kind === "init-baseline") expect(action.baseline).toBe("2026-06-16T00:00:00.000Z");
  });

  it("first poll with prev=null + curr=null → wait (baseline cannot be set yet)", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: null,
      currEndedAt: null,
      shouldStopRc: null,
    });
    expect(action.kind).toBe("wait");
  });

  it("returns should-stop-error when shouldStopRc is 2 (#181 F02)", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: "2026-06-16T00:00:00.000Z",
      currEndedAt: "2026-06-16T01:00:00.000Z",
      shouldStopRc: 2,
    });
    expect(action.kind).toBe("should-stop-error");
    if (action.kind === "should-stop-error") expect(action.rc).toBe(2);
  });

  it("returns should-stop-error when shouldStopRc is -1 (script missing; #181 F02)", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: "2026-06-16T00:00:00.000Z",
      currEndedAt: "2026-06-16T01:00:00.000Z",
      shouldStopRc: -1,
    });
    expect(action.kind).toBe("should-stop-error");
    if (action.kind === "should-stop-error") expect(action.rc).toBe(-1);
  });
});
