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
