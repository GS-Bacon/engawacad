// worker-registry.test.ts — worker terminal cleanup + reconcileBusy (#333)
//
// 検証観点 (contract acceptance): worker terminal cleanup

import { describe, expect, test } from "bun:test";
import {
  registerWorker,
  assignIssue,
  releaseTerminal,
  reconcileBusy,
  firstIdle,
  type Registry,
} from "../lib/worker-registry.ts";

function reg3(): Registry {
  let r: Registry = { workers: {}, max_workers: 3 };
  r = registerWorker(r, "worker-1", "%1", "/wt/w1");
  r = registerWorker(r, "worker-2", "%2", "/wt/w2");
  r = registerWorker(r, "worker-3", "%3", "/wt/w3");
  return r;
}

describe("worker terminal cleanup (#333)", () => {
  test("releaseTerminal forces busy worker back to idle", () => {
    let r = reg3();
    r = assignIssue(r, "worker-1", 100);
    expect(r.workers["worker-1"].state).toBe("busy");
    r = releaseTerminal(r, "worker-1");
    expect(r.workers["worker-1"].state).toBe("idle");
    expect(r.workers["worker-1"].current_issue).toBeNull();
  });

  test("releaseTerminal on merging worker (stuck) also frees it", () => {
    let r = reg3();
    r = assignIssue(r, "worker-2", 200);
    // simulate stuck merging
    r = { workers: { ...r.workers, "worker-2": { ...r.workers["worker-2"], state: "merging" } }, max_workers: 3 };
    r = releaseTerminal(r, "worker-2");
    expect(r.workers["worker-2"].state).toBe("idle");
  });

  test("reconcileBusy frees workers whose issue reached terminal state", () => {
    let r = reg3();
    r = assignIssue(r, "worker-1", 300);
    r = assignIssue(r, "worker-2", 301);
    // issue 300 is terminal (closed), 301 is not
    const r2 = reconcileBusy(r, new Set([300]));
    expect(r2.workers["worker-1"].state).toBe("idle");
    expect(r2.workers["worker-2"].state).toBe("busy");
  });

  test("firstIdle picks the lowest-id idle worker", () => {
    let r = reg3();
    r = assignIssue(r, "worker-1", 1);
    expect(firstIdle(r)).toBe("worker-2");
  });

  test("max_workers enforced", () => {
    let r: Registry = { workers: {}, max_workers: 2 };
    r = registerWorker(r, "worker-1", "%1", "/wt/w1");
    r = registerWorker(r, "worker-2", "%2", "/wt/w2");
    expect(() => registerWorker(r, "worker-3", "%3", "/wt/w3")).toThrow();
  });
});
