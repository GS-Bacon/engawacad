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
