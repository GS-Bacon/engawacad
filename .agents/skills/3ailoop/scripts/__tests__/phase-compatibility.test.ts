import { describe, expect, test } from "bun:test";
import { buildDashboard, evaluatePhaseClose, extractPhaseConditions } from "../phase.ts";
import { decideAdr, parseSplitDrafts } from "../compatibility.ts";

describe("Phase compatibility", () => {
  test("close requires milestone, all feature issues closed, and no gates", () => {
    const condition = extractPhaseConditions("## Phase 10: Curves\n\n**完了条件**:\n- Rectangle works\n", 10)!;
    const issues = [{ number: 1, state: "closed", labels: ["type: feature"], milestone: "Phase 10: Curves" }];
    expect(evaluatePhaseClose(10, condition, issues, [{ title: "Phase 10: Curves", state: "open" }]).eligible).toBe(true);
    expect(evaluatePhaseClose(10, condition, [...issues, { number: 2, state: "open", labels: ["gate:adr-review"], milestone: null }], [{ title: "Phase 10: Curves", state: "open" }]).eligible).toBe(false);
  });

  test("dashboard is cumulative rather than a fixed skeleton", () => {
    const text = buildDashboard({ current_cycle: 9, paused: true, workers: { w1: {} }, recent_cycles: [{ cycle: 9, closed: [300], pause_reason: "gate" }] }, "decision one\n");
    expect(text).toContain("Cycle #9");
    expect(text).toContain("#300");
    expect(text).toContain("decision one");
  });
});

describe("split and ADR compatibility", () => {
  test("split drafts inherit parent and milestone without external writes", () => {
    const drafts = parseSplitDrafts("split_proposal:\n  - title: Child A\n    body: bounded\n    labels: [type: feature, batch:kernel]\n", 195, "Phase 10");
    expect(drafts[0]).toMatchObject({ title: "Child A", milestone: "Phase 10" });
    expect(drafts[0].labels).toContain("parent-blocked-by-split:195");
  });

  test("ADR requires all three independent Codex verdicts", () => {
    expect(decideAdr([{ persona: "architect", verdict: "approved" }]).outcome).toBe("needs-human");
    expect(decideAdr([
      { persona: "architect", verdict: "approved" }, { persona: "contrarian", verdict: "approved" },
      { persona: "migration", verdict: "approved" },
    ]).outcome).toBe("accepted");
  });
});
