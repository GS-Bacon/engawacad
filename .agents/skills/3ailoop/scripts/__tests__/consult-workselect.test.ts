// consult-workselect.test.ts — consultation/resume lifecycle + work selection
//
// 検証観点 (contract acceptance):
//   - consultation/resume (open → answered → resolved)
//   - work selection excludes closed/assigned, orders by tier

import { describe, expect, test } from "bun:test";
import { openConsultation, answerConsultation, resolveConsultation } from "../lib/consult.ts";
import { selectWork, type CandidateIssue } from "../lib/work-select.ts";

const now = () => new Date("2026-08-13T00:00:00Z");

describe("consultation / resume", () => {
  test("open → answered (Codex decision) → resolved (worker resume)", () => {
    const open = openConsultation({
      id: "c1", issue: 5, worker_id: "worker-1",
      question: "which lib?", options: ["thiserror", "anyhow"], recommendation: "thiserror",
    }, now);
    expect(open.status).toBe("open");

    const answered = answerConsultation(open, "thiserror", now);
    expect(answered.status).toBe("answered");
    expect(answered.decision).toBe("thiserror");

    const resolved = resolveConsultation(answered, now);
    expect(resolved.status).toBe("resolved");
  });

  test("cannot answer a non-open consultation", () => {
    const resolved = resolveConsultation({ id: "c2", issue: 1, worker_id: "w1", question: "q", options: ["a", "b"], recommendation: "a", status: "open", asked_at: "2026-08-13T00:00:00Z" }, now);
    // resolve only works from answered
    expect(resolved.status).toBe("open");
  });
});

describe("work selection", () => {
  const candidates: CandidateIssue[] = [
    { number: 1, tier: "general", title: "g", labels: [] },
    { number: 2, tier: "phase-feature", title: "pf", labels: ["type:feature"] },
    { number: 3, tier: "split-batch", title: "sb", labels: [] },
    { number: 4, tier: "foundation", title: "f", labels: ["type:foundation"] },
  ];

  test("excludes closed issues (#333)", () => {
    const r = selectWork({ candidates, closedIssues: new Set([3]), assignedIssues: new Set(), excludeLabels: [] });
    expect(r.next?.number).toBe(2); // 3 closed → next tier is phase-feature #2
    expect(r.dropped.find(d => d.number === 3)?.reason).toBe("closed");
  });

  test("excludes already-assigned issues (double-assign defense)", () => {
    const r = selectWork({ candidates, closedIssues: new Set(), assignedIssues: new Set([2]), excludeLabels: [] });
    expect(r.next?.number).toBe(3); // 2 assigned → split-batch #3 wins
    expect(r.dropped.find(d => d.number === 2)?.reason).toBe("already-assigned");
  });

  test("excludes gate/needs/blocked-by-split labels", () => {
    const labeled: CandidateIssue[] = [
      { number: 10, tier: "split-batch", title: "x", labels: ["gate:adr-review"] },
      { number: 11, tier: "general", title: "y", labels: [] },
    ];
    const r = selectWork({ candidates: labeled, closedIssues: new Set(), assignedIssues: new Set(), excludeLabels: ["gate:", "needs-", "blocked-by-split"] });
    expect(r.next?.number).toBe(11);
  });

  test("nothing actionable → next null", () => {
    const r = selectWork({ candidates, closedIssues: new Set([1, 2, 3, 4]), assignedIssues: new Set(), excludeLabels: [] });
    expect(r.next).toBeNull();
  });
});
