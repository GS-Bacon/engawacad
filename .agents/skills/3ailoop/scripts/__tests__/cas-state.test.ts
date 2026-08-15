// cas-state.test.ts — CAS state machine: stale generation, closed issue, duplicate assign,
// worker terminal cleanup (#333 defenses)
//
// 検証観点 (contract acceptance):
//   - stale generation (CAS mismatch → reject)
//   - closed Issue cannot be reassigned
//   - duplicate assignment rejected
//   - terminal outcome frees the worker slot (no stuck busy)
//   - moveToVerifying / markClosed transitions

import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  tryAssign,
  markClosed,
  moveToVerifying,
  recordTerminal,
  getIssue,
  mutateState,
  type AssignState,
} from "../lib/cas-state.ts";

const now = () => new Date("2026-08-13T00:00:00Z");

test("state mutation creates a previously missing parent directory", () => {
  const root = mkdtempSync(join(tmpdir(), "cas-parent-"));
  const path = join(root, "nested", "assign-state.json");
  mutateState(path, state => state);
  expect(JSON.parse(readFileSync(path, "utf-8"))).toEqual({ issues: {} });
  rmSync(root, { recursive: true, force: true });
});

function empty(): AssignState {
  return { issues: {} };
}

describe("stale generation (#333)", () => {
  test("assign with wrong expectedGeneration → stale-generation", () => {
    let s = empty();
    // first assign at gen 0 → bumps to 1
    const a1 = tryAssign(s, { issue: 1, workerId: "w1", expectedGeneration: 0, now });
    expect(a1.ok).toBe(true);
    if (a1.ok) s = a1.state;
    // stale plan read gen=0 again and tries to re-assign → must be rejected
    const a2 = tryAssign(s, { issue: 1, workerId: "w1", expectedGeneration: 0, now });
    expect(a2.ok).toBe(false);
    if (!a2.ok) expect(a2.error).toBe("stale-generation");
  });

  test("moveToVerifying with stale gen → rejected", () => {
    let s = empty();
    const a = tryAssign(s, { issue: 2, workerId: "w1", expectedGeneration: 0, now });
    if (a.ok) s = a.state;
    const v = moveToVerifying(s, 2, "w1", 999, now);
    expect(v.ok).toBe(false);
    if (!v.ok) expect(v.error).toBe("stale-generation");
  });
});

describe("closed Issue cannot be reassigned (#333)", () => {
  test("assign after markClosed → closed failure", () => {
    let s = empty();
    s = markClosed(s, 5, now);
    const a = tryAssign(s, { issue: 5, workerId: "w1", expectedGeneration: getIssue(s, 5).generation, now });
    expect(a.ok).toBe(false);
    if (!a.ok) expect(a.error).toBe("closed");
  });
});

describe("duplicate assignment rejected", () => {
  test("assign to w2 while w1 owns it → already-assigned", () => {
    let s = empty();
    const a1 = tryAssign(s, { issue: 7, workerId: "w1", expectedGeneration: 0, now });
    if (a1.ok) s = a1.state;
    const gen = getIssue(s, 7).generation;
    const a2 = tryAssign(s, { issue: 7, workerId: "w2", expectedGeneration: gen, now });
    expect(a2.ok).toBe(false);
    if (!a2.ok) expect(a2.error).toBe("already-assigned");
  });

  test("idempotent re-assign to SAME worker is allowed", () => {
    let s = empty();
    const a1 = tryAssign(s, { issue: 8, workerId: "w1", expectedGeneration: 0, now });
    if (a1.ok) s = a1.state;
    const gen = getIssue(s, 8).generation;
    const a2 = tryAssign(s, { issue: 8, workerId: "w1", expectedGeneration: gen, now });
    expect(a2.ok).toBe(true);
  });
});

describe("worker terminal cleanup (no stuck busy, #333)", () => {
  test("completed keeps ownership for landing", () => {
    let s = empty();
    const a = tryAssign(s, { issue: 9, workerId: "w1", expectedGeneration: 0, now });
    if (a.ok) s = a.state;
    const gen = getIssue(s, 9).generation;
    const t = recordTerminal(s, 9, "w1", "completed", gen, now);
    expect(t.ok).toBe(true);
    if (t.ok) {
      expect(t.state.issues["9"].last_outcome).toBe("completed");
      expect(t.state.issues["9"].worker_id).toBe("w1"); // still owned, for landing
    }
  });

  test("failed → unassigned (free for re-pick, no double-assign)", () => {
    let s = empty();
    const a = tryAssign(s, { issue: 10, workerId: "w1", expectedGeneration: 0, now });
    if (a.ok) s = a.state;
    const gen = getIssue(s, 10).generation;
    const t = recordTerminal(s, 10, "w1", "failed", gen, now);
    expect(t.ok).toBe(true);
    if (t.ok) {
      expect(t.state.issues["10"].status).toBe("unassigned");
      expect(t.state.issues["10"].worker_id).toBeNull();
      // re-assign to a different worker now works
      const a2 = tryAssign(t.state, { issue: 10, workerId: "w2", expectedGeneration: t.state.issues["10"].generation, now });
      expect(a2.ok).toBe(true);
    }
  });

  test("rate_limited → unassigned", () => {
    let s = empty();
    const a = tryAssign(s, { issue: 11, workerId: "w1", expectedGeneration: 0, now });
    if (a.ok) s = a.state;
    const gen = getIssue(s, 11).generation;
    const t = recordTerminal(s, 11, "w1", "rate_limited", gen, now);
    expect(t.ok).toBe(true);
    if (t.ok) expect(t.state.issues["11"].status).toBe("unassigned");
  });
});
