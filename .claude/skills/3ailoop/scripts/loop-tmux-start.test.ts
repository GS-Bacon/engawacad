// loop-tmux-start.test.ts — Phase D-1 (2+N) pane 構成 bootstrap の単体テスト
//
// 検証観点:
// - N=1: 従来 2 pane 構成のプランを吐く (merge pane / worktree / register なし)
// - N=3: (2+N) 構成のプラン (worktree init + merge split/launch + worker×3 + layout)
// - --worktree-base で worktree パスを上書きできる
// - env LOOP_MAX_WORKERS が --n 未指定時に効く

import { describe, expect, test } from "bun:test";
import { buildPlan, resolveN } from "./loop-tmux-start.ts";

describe("resolveN", () => {
  test("CLI --n が最優先", () => {
    expect(resolveN(2, { LOOP_MAX_WORKERS: "5" })).toBe(2);
  });
  test("--n なしで LOOP_MAX_WORKERS=2 → 2", () => {
    expect(resolveN(null, { LOOP_MAX_WORKERS: "2" })).toBe(2);
  });
  test("両方なしで default 3", () => {
    expect(resolveN(null, {})).toBe(3);
  });
  test("不正な env は default 3", () => {
    expect(resolveN(null, { LOOP_MAX_WORKERS: "abc" })).toBe(3);
    expect(resolveN(null, { LOOP_MAX_WORKERS: "0" })).toBe(3);
    expect(resolveN(null, { LOOP_MAX_WORKERS: "-1" })).toBe(3);
  });
  test("CLI 0/負値は無視して env / default に落ちる", () => {
    expect(resolveN(0, { LOOP_MAX_WORKERS: "2" })).toBe(2);
    expect(resolveN(-3, {})).toBe(3);
  });
});

describe("T07_tmux_start_n1_backward", () => {
  test("N=1 は従来 2 pane 構成のみ (merge/worktree/register なし)", () => {
    const steps = buildPlan({ n: 1, claudeCmd: "claude --model sonnet", worktreeBase: "/x" });
    const phases = steps.map(s => s.phase);
    expect(phases).not.toContain("worktree-init");
    expect(phases).not.toContain("merge-split");
    expect(phases).not.toContain("merge-launch");
    expect(phases).not.toContain("worktree-add");
    expect(phases).not.toContain("worker-cd");
    expect(phases).not.toContain("worker-register");
    expect(phases).not.toContain("layout");
    // worker-split は 1 回だけ
    expect(phases.filter(p => p === "worker-split")).toHaveLength(1);
    // 初回 /3ailoop 投入は N=1 のみ (N>=2 では watcher が fan-out する)
    expect(phases).toContain("worker-initial-3ailoop");
    // panes.json と watcher daemon は必ずある
    expect(phases).toContain("panes-json");
    expect(phases).toContain("watcher-daemon");
  });
});

describe("T06_tmux_start_dry_run_n3", () => {
  const steps = buildPlan({ n: 3, claudeCmd: "claude --model sonnet", worktreeBase: "/home/bacon/worktrees" });
  const phases = steps.map(s => s.phase);

  test("worktree base 準備が先頭近く", () => {
    expect(phases).toContain("worktree-init");
    expect(phases.indexOf("worktree-init")).toBeLessThan(phases.indexOf("merge-split"));
  });

  test("merge pane split + launch が 1 回ずつ", () => {
    expect(phases.filter(p => p === "merge-split")).toHaveLength(1);
    expect(phases.filter(p => p === "merge-launch")).toHaveLength(1);
    expect(phases.indexOf("merge-split")).toBeLessThan(phases.indexOf("merge-launch"));
  });

  test("worker split が N=3 回", () => {
    expect(phases.filter(p => p === "worker-split")).toHaveLength(3);
  });

  test("worktree add が N=3 回で worktree base 配下のパス", () => {
    const wtAdds = steps.filter(s => s.phase === "worktree-add");
    expect(wtAdds).toHaveLength(3);
    for (let i = 0; i < 3; i++) {
      const args = wtAdds[i].args!;
      // git -C REPO_ROOT worktree add <path> HEAD
      expect(args).toContain("worktree");
      expect(args).toContain("add");
      const wtPath = args[args.length - 2];
      expect(wtPath).toBe(`/home/bacon/worktrees/w${i + 1}`);
    }
  });

  test("worker-registry register が N=3 回で worker-1..3", () => {
    const regs = steps.filter(s => s.phase === "worker-register");
    expect(regs).toHaveLength(3);
    const ids = regs.map(r => {
      const args = r.args!;
      const idIdx = args.indexOf("--worker-id");
      return args[idIdx + 1];
    });
    expect(ids).toEqual(["worker-1", "worker-2", "worker-3"]);
  });

  test("layout / panes-json / watcher-daemon が最後付近", () => {
    expect(phases).toContain("layout");
    expect(phases).toContain("panes-json");
    expect(phases).toContain("watcher-daemon");
    // 順序: layout → panes-json → watcher-daemon
    expect(phases.indexOf("layout")).toBeLessThan(phases.indexOf("panes-json"));
    expect(phases.indexOf("panes-json")).toBeLessThan(phases.indexOf("watcher-daemon"));
  });

  test("初回 /3ailoop 投入は N>=2 では登場しない (fan-out で watcher が assign)", () => {
    expect(phases).not.toContain("worker-initial-3ailoop");
  });

  test("N=3 で total pane 操作 = 1 (merge split) + 3 (worker split) = 4 split-window", () => {
    const splitOps = steps.filter(s =>
      Array.isArray(s.args) && s.args[0] === "tmux" && s.args[1] === "split-window",
    );
    expect(splitOps).toHaveLength(4);
  });
});

describe("T_bonus_dry_run_custom_worktree_base", () => {
  test("--worktree-base が worktree add パスに反映される", () => {
    const steps = buildPlan({ n: 2, claudeCmd: "claude", worktreeBase: "/tmp/wt" });
    const wtAdds = steps.filter(s => s.phase === "worktree-add");
    expect(wtAdds).toHaveLength(2);
    const paths = wtAdds.map(s => s.args![s.args!.length - 2]);
    expect(paths).toEqual(["/tmp/wt/w1", "/tmp/wt/w2"]);
  });
});

describe("T_bonus_dry_run_env_override", () => {
  test("env LOOP_MAX_WORKERS=2 (CLI --n なし) で N=2 プラン", () => {
    // resolveN + buildPlan の組み合わせで確認
    const n = resolveN(null, { LOOP_MAX_WORKERS: "2" });
    expect(n).toBe(2);
    const steps = buildPlan({ n, claudeCmd: "claude", worktreeBase: "/x" });
    expect(steps.filter(s => s.phase === "worker-split")).toHaveLength(2);
    expect(steps.filter(s => s.phase === "worktree-add")).toHaveLength(2);
    expect(steps.filter(s => s.phase === "worker-register")).toHaveLength(2);
  });
});
