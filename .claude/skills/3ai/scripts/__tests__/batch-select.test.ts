// #210: foundation-batch tier の milestone gating ロジックの単体テスト
import { describe, expect, test } from "bun:test";
import { isFutureMilestoneTitle } from "../batch-select.ts";

describe("isFutureMilestoneTitle (#210)", () => {
  test("milestone なし → false (除外しない)", () => {
    expect(isFutureMilestoneTitle(null, 8)).toBe(false);
  });

  test("currentPhase が null → false (判定不能、除外しない)", () => {
    expect(isFutureMilestoneTitle("Phase 12: Quality Pass", null)).toBe(false);
  });

  test("将来 Phase (Phase 12, currentPhase 8) → true", () => {
    expect(isFutureMilestoneTitle("Phase 12: Quality + Refactor Pass 1", 8)).toBe(true);
  });

  test("将来 Phase (Phase 16) → true", () => {
    expect(isFutureMilestoneTitle("Phase 16: Quality + Refactor Pass 2", 8)).toBe(true);
  });

  test("将来 Phase (Phase 20) → true", () => {
    expect(isFutureMilestoneTitle("Phase 20: ...", 8)).toBe(true);
  });

  test("現 Phase (Phase 8) → false", () => {
    expect(isFutureMilestoneTitle("Phase 8: モデル面上のスケッチ", 8)).toBe(false);
  });

  test("過去 Phase (Phase 4) → false (振り返り作業 OK)", () => {
    expect(isFutureMilestoneTitle("Phase 4: Boolean", 8)).toBe(false);
  });

  test("非 Phase milestone title → false", () => {
    expect(isFutureMilestoneTitle("Backlog", 8)).toBe(false);
    expect(isFutureMilestoneTitle("Release 1.0", 8)).toBe(false);
  });

  test("境界: Phase 9 (currentPhase 8 の次) → true", () => {
    expect(isFutureMilestoneTitle("Phase 9: 履歴編集", 8)).toBe(true);
  });

  test("Phase の後ろに余計な文字が来るタイトルでも N で判定", () => {
    expect(isFutureMilestoneTitle("Phase 12: Quality + Refactor Pass 1", 11)).toBe(true);
    expect(isFutureMilestoneTitle("Phase 11: 拘束ソルバ 基礎", 11)).toBe(false);
  });
});

// 歪み #1: 親 ADR inactive 時に split 子 Issue を tier から降格
import { extractParentAdrNumbers, isParentAdrInactive } from "../batch-select.ts";

describe("extractParentAdrNumbers", () => {
  test("parent-adr:N ラベルから番号を抽出", () => {
    expect(extractParentAdrNumbers(["parent-adr:279", "bug"])).toEqual([279]);
  });
  test("複数 parent-adr 抽出", () => {
    expect(extractParentAdrNumbers(["parent-adr:279", "parent-adr:281"])).toEqual([279, 281]);
  });
  test("該当ラベル無しなら空配列", () => {
    expect(extractParentAdrNumbers(["bug", "batch:kernel"])).toEqual([]);
  });
});

describe("isParentAdrInactive (歪み #1)", () => {
  test("親 ADR が needs-human → true", () => {
    const lookup = (_: number) => ["gate:adr-review", "needs-human"];
    expect(isParentAdrInactive(["parent-adr:279"], lookup)).toBe(true);
  });
  test("親 ADR が gate:adr-review (まだ accept されていない) → true", () => {
    const lookup = (_: number) => ["gate:adr-review", "type: foundation"];
    expect(isParentAdrInactive(["parent-adr:279"], lookup)).toBe(true);
  });
  test("親 ADR が blocked-by-adr-retired → true", () => {
    const lookup = (_: number) => ["blocked-by-adr-retired"];
    expect(isParentAdrInactive(["parent-adr:279"], lookup)).toBe(true);
  });
  test("親 ADR が closed (lookup = null) → false (accept 確定済み扱い)", () => {
    const lookup = (_: number) => null;
    expect(isParentAdrInactive(["parent-adr:279"], lookup)).toBe(false);
  });
  test("親 ADR が active (gate/needs-human 無し) → false", () => {
    const lookup = (_: number) => ["type: foundation"];
    expect(isParentAdrInactive(["parent-adr:279"], lookup)).toBe(false);
  });
  test("parent-adr ラベル無し → false (lookup 呼ばれない)", () => {
    let called = false;
    const lookup = (_: number) => { called = true; return null; };
    expect(isParentAdrInactive(["bug"], lookup)).toBe(false);
    expect(called).toBe(false);
  });
  test("複数親のうち 1 つでも inactive なら true", () => {
    const lookup = (n: number) => n === 279 ? ["needs-human"] : ["type: foundation"];
    expect(isParentAdrInactive(["parent-adr:279", "parent-adr:281"], lookup)).toBe(true);
  });
});

// N=3 parallel worker 割り当て用 crate_groups 推定
import {
  estimateCrateGroup,
  computeCrateGroupsForBatch,
  type GhIssue,
} from "../batch-select.ts";

function makeIssue(overrides: Partial<GhIssue> = {}): GhIssue {
  return {
    number: 1,
    title: "",
    labels: [],
    body: "",
    milestone: null,
    ...overrides,
  };
}

describe("estimateCrateGroup (crate 推定ヒューリスティック)", () => {
  test("Sketch + Offset → engawa-format/src/sketch", () => {
    const r = estimateCrateGroup(makeIssue({
      title: "feat(format): Sketch Offset を追加",
      labels: [{ name: "batch:kernel" }],
    }));
    expect(r.touched).toEqual(["engawa-format/src/sketch"]);
    expect(r.path).toBe("engawa-format/src/sketch");
  });

  test("Sketch + Fillet → engawa-format/src/sketch (sketch 側優先、kernel/fillet に流さない)", () => {
    const r = estimateCrateGroup(makeIssue({ title: "feat: Sketch Fillet 実装" }));
    expect(r.touched).toEqual(["engawa-format/src/sketch"]);
  });

  test("Sketch + Chamfer → engawa-format/src/sketch", () => {
    const r = estimateCrateGroup(makeIssue({ title: "Sketch Chamfer" }));
    expect(r.touched).toEqual(["engawa-format/src/sketch"]);
  });

  test("Boolean X → engawa-kernel/src/boolean", () => {
    const r = estimateCrateGroup(makeIssue({ title: "feat(kernel): Boolean Union の CSG 実装" }));
    expect(r.touched).toEqual(["engawa-kernel/src/boolean"]);
  });

  test("Tessellation Y → engawa-kernel/src/tessellation", () => {
    const r = estimateCrateGroup(makeIssue({ title: "Tessellation の三角形化精度改善" }));
    expect(r.touched).toEqual(["engawa-kernel/src/tessellation"]);
  });

  test("拘束 → engawa-kernel/src/solver (Phase 11+)", () => {
    const r = estimateCrateGroup(makeIssue({ title: "feat: 拘束ソルバの初期実装" }));
    expect(r.touched).toEqual(["engawa-kernel/src/solver"]);
  });

  test("Solver 英語表記 → engawa-kernel/src/solver", () => {
    const r = estimateCrateGroup(makeIssue({ title: "feat: Solver 数値安定化" }));
    expect(r.touched).toEqual(["engawa-kernel/src/solver"]);
  });

  test("body-op Fillet (Sketch 修飾なし) → engawa-kernel/src/fillet", () => {
    const r = estimateCrateGroup(makeIssue({ title: "feat(kernel): Fillet エッジ丸め" }));
    expect(r.touched).toEqual(["engawa-kernel/src/fillet"]);
  });

  test("Extrude → engawa-format/src/feature + engawa-build/src/feature (multi-hit)", () => {
    const r = estimateCrateGroup(makeIssue({ title: "feat: ExtrudeCut を追加" }));
    expect(r.touched).toEqual([
      "engawa-build/src/feature",
      "engawa-format/src/feature",
    ]);
  });

  test("misc improvement (キーワード無し) → touched 空", () => {
    const r = estimateCrateGroup(makeIssue({ title: "misc improvement" }));
    expect(r.touched).toEqual([]);
    expect(r.path).toBe("");
  });

  test("Rectangle 単独 (Sketch 修飾なし) → engawa-format/src/sketch", () => {
    // Phase 10 の実 Issue #300 "Rectangle を engawa-format / engawa-build に実装" 相当
    const r = estimateCrateGroup(makeIssue({
      title: "feat(phase10): Rectangle を engawa-format / engawa-build に実装",
    }));
    expect(r.touched).toEqual(["engawa-format/src/sketch"]);
  });

  test("Polygon 単独 → engawa-format/src/sketch", () => {
    const r = estimateCrateGroup(makeIssue({ title: "feat: Polygon を engawa-format に実装" }));
    expect(r.touched).toEqual(["engawa-format/src/sketch"]);
  });

  test("Slot 単独 → engawa-format/src/sketch", () => {
    const r = estimateCrateGroup(makeIssue({ title: "Slot 実装" }));
    expect(r.touched).toEqual(["engawa-format/src/sketch"]);
  });

  test("batch:skill + 3ailoop タイトル → .claude/skills/3ailoop", () => {
    const r = estimateCrateGroup(makeIssue({
      title: "3ailoop watcher の polling 間隔調整",
      labels: [{ name: "batch:skill" }],
    }));
    expect(r.touched).toEqual([".claude/skills/3ailoop"]);
  });
});

describe("computeCrateGroupsForBatch (grouping algorithm)", () => {
  test("T01: 3 Sketch Issue (Offset/Fillet/Chamfer) → 1 crate_group serial", () => {
    const issues = [
      { number: 295, touched: ["engawa-format/src/sketch"] },
      { number: 296, touched: ["engawa-format/src/sketch"] },
      { number: 297, touched: ["engawa-format/src/sketch"] },
    ];
    const groups = computeCrateGroupsForBatch("batch:kernel", issues);
    expect(groups.length).toBe(1);
    expect(groups[0].id).toBe("engawa-format-sketch");
    expect(groups[0].issues).toEqual([295, 296, 297]);
    expect(groups[0].parallel_safe).toBe(false);
    expect(groups[0].reason).toContain("engawa-format/src/sketch");
  });

  test("T02: kernel Boolean と kernel Tessellation → 2 crate_groups, 各 parallel_safe: true", () => {
    const issues = [
      { number: 400, touched: ["engawa-kernel/src/boolean"] },
      { number: 401, touched: ["engawa-kernel/src/tessellation"] },
    ];
    const groups = computeCrateGroupsForBatch("batch:kernel", issues);
    expect(groups.length).toBe(2);
    expect(groups.every(g => g.parallel_safe)).toBe(true);
    expect(groups.map(g => g.id).sort()).toEqual(
      ["engawa-kernel-boolean", "engawa-kernel-tessellation"],
    );
  });

  test("T03: 5 Issue mixed (3 Sketch + 2 Boolean) → 2 crate_groups (sketch:3 / boolean:2)", () => {
    const issues = [
      { number: 500, touched: ["engawa-format/src/sketch"] },
      { number: 501, touched: ["engawa-format/src/sketch"] },
      { number: 502, touched: ["engawa-format/src/sketch"] },
      { number: 503, touched: ["engawa-kernel/src/boolean"] },
      { number: 504, touched: ["engawa-kernel/src/boolean"] },
    ];
    const groups = computeCrateGroupsForBatch("batch:kernel", issues);
    expect(groups.length).toBe(2);
    const byId = new Map(groups.map(g => [g.id, g]));
    expect(byId.get("engawa-format-sketch")?.issues).toEqual([500, 501, 502]);
    expect(byId.get("engawa-format-sketch")?.parallel_safe).toBe(false);
    expect(byId.get("engawa-kernel-boolean")?.issues).toEqual([503, 504]);
    expect(byId.get("engawa-kernel-boolean")?.parallel_safe).toBe(false);
  });

  test("T_DEG: ヒント無し Issue → fallback id <batch>-misc, singleton は parallel_safe: true", () => {
    const issues = [
      { number: 999, touched: [] },
    ];
    const groups = computeCrateGroupsForBatch("batch:kernel", issues);
    expect(groups.length).toBe(1);
    expect(groups[0].id).toBe("batch:kernel-misc");
    expect(groups[0].parallel_safe).toBe(true);
    expect(groups[0].reason).toContain("推定不能");
  });

  test("T_bonus_solver: 拘束 → engawa-kernel-solver singleton parallel_safe: true", () => {
    const issues = [
      { number: 700, touched: estimateCrateGroup(makeIssue({ title: "feat: 拘束ソルバ導入" })).touched },
    ];
    const groups = computeCrateGroupsForBatch("batch:kernel", issues);
    expect(groups.length).toBe(1);
    expect(groups[0].id).toBe("engawa-kernel-solver");
    expect(groups[0].parallel_safe).toBe(true);
  });

  test("空配列 → 空 crate_groups", () => {
    expect(computeCrateGroupsForBatch("batch:kernel", [])).toEqual([]);
  });

  test("multi-hit Extrude 2 件 → touched 交差で 1 crate_group にマージ", () => {
    const t = estimateCrateGroup(makeIssue({ title: "feat: Extrude 実装" })).touched;
    const issues = [
      { number: 800, touched: t },
      { number: 801, touched: t },
    ];
    const groups = computeCrateGroupsForBatch("batch:data", issues);
    expect(groups.length).toBe(1);
    expect(groups[0].issues).toEqual([800, 801]);
    expect(groups[0].parallel_safe).toBe(false);
    // touched 複数 → mixed id
    expect(groups[0].id).toBe("batch:data-mixed");
  });
});

// #254: refactor-batch tier が BatchTierType に追加されていることを型で確認
import type { BatchTierType } from "../types.ts";

describe("BatchTierType (#254)", () => {
  test("refactor-batch が型に含まれる (foundation と phase-feature の間)", () => {
    const tier: BatchTierType = "refactor-batch";
    expect(tier).toBe("refactor-batch");
  });

  test("既存 tier はそのまま使える", () => {
    const tiers: BatchTierType[] = [
      "split-batch", "bug-batch", "enh-batch",
      "foundation-batch", "refactor-batch", "phase-feature",
    ];
    expect(tiers.length).toBe(6);
  });
});
