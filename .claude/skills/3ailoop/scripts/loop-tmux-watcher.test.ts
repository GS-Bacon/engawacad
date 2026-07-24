// loop-tmux-watcher.test.ts — pure 関数の単体テスト
// 副作用 (tmux/gh/fs) は除外し、差分検知・アクション決定ロジックのみ検証。

import { describe, expect, it, test } from "bun:test";
import {
  decideAction,
  decideWatcherPauseWarning,
  detectCycleCompleted,
  emptyFanoutState,
  extractLastEndedAt,
  flattenPlan,
  isPaneActive,
  nextEnqueuable,
  planFanout,
  selectWatcherMode,
  updateFanoutStateFromRegistry,
  WATCHER_PAUSE_WARNING_THRESHOLDS,
  type FanoutPlan,
  type PanesJson,
  type WatcherRegistryEntry,
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

describe("isPaneActive (#209)", () => {
  // 想定外コマンドも全部 active 扱いに倒すため、idle は既知 shell のみで判定する。
  const cases: Array<[string | null, boolean]> = [
    ["claude", true],
    ["bun", true],
    ["node", true],
    ["cargo", true],
    ["rustc", true],
    ["codex", true],
    ["gh", true],
    ["git", true],
    ["make", true],
    ["vim", true],
    ["python", true],
    ["htop", true],
    ["sleep", true],
    ["bash", false],
    ["zsh", false],
    ["sh", false],
    ["fish", false],
    ["dash", false],
    ["  bash  ", false],
    ["", false],
    [null, false],
  ];
  for (const [cmd, expected] of cases) {
    it(`isPaneActive(${JSON.stringify(cmd)}) === ${expected}`, () => {
      expect(isPaneActive(cmd)).toBe(expected);
    });
  }
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

  it("returns pause on cycle completion + shouldStopRc=1 (#253 watcher 不死身化)", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: "2026-06-16T00:00:00.000Z",
      currEndedAt: "2026-06-16T01:00:00.000Z",
      shouldStopRc: 1,
    });
    expect(action.kind).toBe("pause");
    if (action.kind === "pause") expect(action.reason).toContain("RC=1");
  });

  it("returns halt on cycle completion + shouldStopRc=2 (#253 UI 期 / kill switch)", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: "2026-06-16T00:00:00.000Z",
      currEndedAt: "2026-06-16T01:00:00.000Z",
      shouldStopRc: 2,
    });
    expect(action.kind).toBe("halt");
    if (action.kind === "halt") expect(action.reason).toContain("RC=2");
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

  it("returns should-stop-error when shouldStopRc is 99 (想定外 RC; #253)", () => {
    const action = decideAction({
      ...baseInput,
      prevEndedAt: "2026-06-16T00:00:00.000Z",
      currEndedAt: "2026-06-16T01:00:00.000Z",
      shouldStopRc: 99,
    });
    expect(action.kind).toBe("should-stop-error");
    if (action.kind === "should-stop-error") expect(action.rc).toBe(99);
  });
});

// --- #285: pause mode 中の段階通知 ---
// pauseStreak が WATCHER_PAUSE_WARNING_THRESHOLDS (24, 72) 境界にちょうど到達した瞬間だけ
// 該当閾値を返す。境界以外は null。halt (144) はこの関数の責務外 (decideAction で扱う)。

describe("decideWatcherPauseWarning (#285)", () => {
  it("既定閾値は [24, 72]", () => {
    expect(WATCHER_PAUSE_WARNING_THRESHOLDS).toEqual([24, 72]);
  });

  it("streak=0 → null", () => {
    expect(decideWatcherPauseWarning(0)).toBeNull();
  });

  it("streak=23 (24 未満) → null", () => {
    expect(decideWatcherPauseWarning(23)).toBeNull();
  });

  it("streak=24 (ちょうど境界) → 24", () => {
    expect(decideWatcherPauseWarning(24)).toBe(24);
  });

  it("streak=25 (24 を超えたが 72 未満) → null", () => {
    expect(decideWatcherPauseWarning(25)).toBeNull();
  });

  it("streak=72 (ちょうど境界) → 72", () => {
    expect(decideWatcherPauseWarning(72)).toBe(72);
  });

  it("streak=143 (halt 直前) → null (72 以降は閾値なし)", () => {
    expect(decideWatcherPauseWarning(143)).toBeNull();
  });

  it("カスタム閾値 [10, 50] を渡せる", () => {
    expect(decideWatcherPauseWarning(10, [10, 50])).toBe(10);
    expect(decideWatcherPauseWarning(11, [10, 50])).toBeNull();
    expect(decideWatcherPauseWarning(50, [10, 50])).toBe(50);
  });
});

// --- Phase D-1: fan-out mode の pure logic ---

describe("selectWatcherMode (Phase D-1)", () => {
  test("panes.json 不在 → legacy", () => {
    expect(selectWatcherMode(null)).toBe("legacy");
  });

  test("workers=1 で merge なし → legacy (N=1 backward compat)", () => {
    const panes: PanesJson = {
      workers: [{ id: "worker-1", pane_id: "%10", worktree: "" }],
      max_workers: 1,
      saved_at: "2026-07-24T00:00:00Z",
    };
    expect(selectWatcherMode(panes)).toBe("legacy");
  });

  test("workers>=2 + merge あり → fanout", () => {
    const panes: PanesJson = {
      merge: { pane_id: "%11" },
      workers: [
        { id: "worker-1", pane_id: "%12", worktree: "/w1" },
        { id: "worker-2", pane_id: "%13", worktree: "/w2" },
      ],
      max_workers: 2,
      saved_at: "2026-07-24T00:00:00Z",
    };
    expect(selectWatcherMode(panes)).toBe("fanout");
  });

  test("workers>=2 だが merge 欠落 → legacy (保険的にフォールバック)", () => {
    const panes: PanesJson = {
      workers: [
        { id: "worker-1", pane_id: "%12", worktree: "/w1" },
        { id: "worker-2", pane_id: "%13", worktree: "/w2" },
      ],
      max_workers: 2,
      saved_at: "2026-07-24T00:00:00Z",
    };
    expect(selectWatcherMode(panes)).toBe("legacy");
  });
});

describe("flattenPlan (Phase D-1)", () => {
  test("null / undefined → 空", () => {
    expect(flattenPlan(null).crate_groups).toHaveLength(0);
    expect(flattenPlan(undefined).crate_groups).toHaveLength(0);
    expect(flattenPlan({}).crate_groups).toHaveLength(0);
  });

  test("batch group をまたいで順序を保持", () => {
    const raw = {
      groups: [
        {
          crate_groups: [
            { id: "cg-a", issues: [100], parallel_safe: true, reason: "" },
            { id: "cg-b", issues: [101, 102], parallel_safe: false, reason: "" },
          ],
        },
        {
          crate_groups: [
            { id: "cg-c", issues: [200], parallel_safe: true, reason: "" },
          ],
        },
      ],
    };
    const p = flattenPlan(raw);
    expect(p.crate_groups.map(g => g.id)).toEqual(["cg-a", "cg-b", "cg-c"]);
    expect(p.crate_groups[1].parallel_safe).toBe(false);
    expect(p.crate_groups[1].issues).toEqual([101, 102]);
  });

  test("破損 entry (issues 非配列 / id なし) は skip", () => {
    const raw = {
      groups: [
        { crate_groups: [{ id: "cg-x", issues: "bad" }, { issues: [1] }] },
      ],
    };
    expect(flattenPlan(raw).crate_groups).toHaveLength(0);
  });
});

describe("nextEnqueuable (Phase D-1)", () => {
  test("plan 空 → null", () => {
    expect(nextEnqueuable({ crate_groups: [] }, new Map())).toBeNull();
  });

  test("最初の空き group から最小番号の Issue を返す", () => {
    const plan: FanoutPlan = {
      crate_groups: [
        { id: "cg-a", parallel_safe: true, issues: [100] },
        { id: "cg-b", parallel_safe: false, issues: [200, 201] },
      ],
    };
    expect(nextEnqueuable(plan, new Map())).toEqual({ issue: 100, groupId: "cg-a" });
  });

  test("cg-a in-flight → 次は cg-b から", () => {
    const plan: FanoutPlan = {
      crate_groups: [
        { id: "cg-a", parallel_safe: true, issues: [100] },
        { id: "cg-b", parallel_safe: false, issues: [200, 201] },
      ],
    };
    expect(nextEnqueuable(plan, new Map([["cg-a", 100]]))).toEqual({ issue: 200, groupId: "cg-b" });
  });

  test("全 group in-flight → null (2 idle worker あっても割り当てできない)", () => {
    const plan: FanoutPlan = {
      crate_groups: [
        { id: "cg-a", parallel_safe: false, issues: [100, 101] },
      ],
    };
    expect(nextEnqueuable(plan, new Map([["cg-a", 100]]))).toBeNull();
  });

  test("completed の Issue は skip", () => {
    const plan: FanoutPlan = {
      crate_groups: [
        { id: "cg-a", parallel_safe: false, issues: [100, 101, 102] },
      ],
    };
    expect(nextEnqueuable(plan, new Map(), new Set([100]))).toEqual({ issue: 101, groupId: "cg-a" });
  });

  test("group 内全 Issue completed → 次 group へ", () => {
    const plan: FanoutPlan = {
      crate_groups: [
        { id: "cg-a", parallel_safe: false, issues: [100, 101] },
        { id: "cg-b", parallel_safe: true, issues: [200] },
      ],
    };
    expect(nextEnqueuable(plan, new Map(), new Set([100, 101]))).toEqual({ issue: 200, groupId: "cg-b" });
  });
});

describe("T_bonus_watcher_fanout_idle_worker (Phase D-1)", () => {
  test("registry 2 idle + 1 busy、plan 3 parallel-safe → 2 assign", () => {
    const registry: Record<string, WatcherRegistryEntry> = {
      "worker-1": { state: "idle", current_issue: null },
      "worker-2": { state: "idle", current_issue: null },
      "worker-3": { state: "busy", current_issue: 300 },
    };
    const plan: FanoutPlan = {
      crate_groups: [
        { id: "cg-a", parallel_safe: true, issues: [100] },
        { id: "cg-b", parallel_safe: true, issues: [101] },
        { id: "cg-c", parallel_safe: true, issues: [102] },
      ],
    };
    const assignments = planFanout(registry, plan, emptyFanoutState());
    expect(assignments).toHaveLength(2);
    expect(assignments[0].workerId).toBe("worker-1");
    expect(assignments[0].issue).toBe(100);
    expect(assignments[1].workerId).toBe("worker-2");
    expect(assignments[1].issue).toBe(101);
    // worker-3 は busy なのでスキップ、cg-c は未割当
  });
});

describe("T_bonus_watcher_fanout_serial_group (Phase D-1)", () => {
  test("2 idle worker、serial group 1 個 (3 Issue) → 1 assign のみ (rest wait)", () => {
    const registry: Record<string, WatcherRegistryEntry> = {
      "worker-1": { state: "idle", current_issue: null },
      "worker-2": { state: "idle", current_issue: null },
    };
    const plan: FanoutPlan = {
      crate_groups: [
        { id: "cg-serial", parallel_safe: false, issues: [500, 501, 502] },
      ],
    };
    const assignments = planFanout(registry, plan, emptyFanoutState());
    expect(assignments).toHaveLength(1);
    expect(assignments[0].workerId).toBe("worker-1");
    expect(assignments[0].issue).toBe(500);
    // worker-2 は 2 番目の idle だが cg-serial が in-flight になったので待機
  });

  test("serial group の 1 件目完了 (busy→idle) 後、次 Issue が pick される", () => {
    const registry: Record<string, WatcherRegistryEntry> = {
      "worker-1": { state: "idle", current_issue: null },
    };
    const plan: FanoutPlan = {
      crate_groups: [
        { id: "cg-serial", parallel_safe: false, issues: [500, 501] },
      ],
    };
    // 1 件目 assign
    let state = emptyFanoutState();
    const a1 = planFanout(registry, plan, state);
    expect(a1[0].issue).toBe(500);
    // 疑似: registry で 500 が busy 状態になり、その後 idle に戻る (worker released)
    state.inflight.set(a1[0].groupId, a1[0].issue);
    const registryAfterMerge: Record<string, WatcherRegistryEntry> = {
      "worker-1": { state: "idle", current_issue: null }, // released 後
    };
    state = updateFanoutStateFromRegistry(state, registryAfterMerge);
    // 500 が completed に移り、次に 501 が pick される
    expect(state.completed.has(500)).toBe(true);
    const a2 = planFanout(registryAfterMerge, plan, state);
    expect(a2).toHaveLength(1);
    expect(a2[0].issue).toBe(501);
  });
});

describe("updateFanoutStateFromRegistry (Phase D-1)", () => {
  test("in-flight worker が引き続き busy → 状態変化なし", () => {
    const state = emptyFanoutState();
    state.inflight.set("cg-a", 100);
    const registry: Record<string, WatcherRegistryEntry> = {
      "worker-1": { state: "busy", current_issue: 100 },
    };
    const next = updateFanoutStateFromRegistry(state, registry);
    expect(next.inflight.get("cg-a")).toBe(100);
    expect(next.completed.has(100)).toBe(false);
  });

  test("in-flight worker が idle に戻った → completed へ移動", () => {
    const state = emptyFanoutState();
    state.inflight.set("cg-a", 100);
    const registry: Record<string, WatcherRegistryEntry> = {
      "worker-1": { state: "idle", current_issue: null },
    };
    const next = updateFanoutStateFromRegistry(state, registry);
    expect(next.inflight.has("cg-a")).toBe(false);
    expect(next.completed.has(100)).toBe(true);
  });

  test("merging 状態も継続 in-flight 扱い (merge 完了までは占有)", () => {
    const state = emptyFanoutState();
    state.inflight.set("cg-a", 100);
    const registry: Record<string, WatcherRegistryEntry> = {
      "worker-1": { state: "merging", current_issue: 100 },
    };
    const next = updateFanoutStateFromRegistry(state, registry);
    expect(next.inflight.get("cg-a")).toBe(100);
    expect(next.completed.has(100)).toBe(false);
  });
});

describe("T_bonus_watcher_n1_backward (Phase D-1)", () => {
  test("N=1 panes.json → legacy モード判定 (single /clear /3ailoop 経路)", () => {
    const panes: PanesJson = {
      workers: [{ id: "worker-1", pane_id: "%10", worktree: "" }],
      max_workers: 1,
      saved_at: "2026-07-24T00:00:00Z",
    };
    expect(selectWatcherMode(panes)).toBe("legacy");
    // fan-out ルートに乗らないので、既存 legacy コードパス (decideAction ベース) が動く。
    // 実際の /clear /3ailoop 送信は runDaemon の legacy 経路で扱われる。
  });
});

