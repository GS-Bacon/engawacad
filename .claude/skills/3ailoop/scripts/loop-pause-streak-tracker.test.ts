// loop-pause-streak-tracker.test.ts — #284 欠陥 B: Issue × Category 2 軸の連続 pause カウンタ

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { PAUSE_STREAK_THRESHOLD, getStreak, incStreak, resetStreak, resetAllForIssue } from "./loop-pause-streak-tracker";

let workDir: string;
let originalCwd: string;

beforeEach(() => {
  originalCwd = process.cwd();
  workDir = mkdtempSync(join(tmpdir(), "pause-streak-test-"));
  process.chdir(workDir);
});

afterEach(() => {
  process.chdir(originalCwd);
  rmSync(workDir, { recursive: true, force: true });
});

describe("loop-pause-streak-tracker", () => {
  test("初回 inc は count=1 + capped=false", () => {
    const r = incStreak(274, "intent-aligned-no", "test reason");
    expect(r.count).toBe(1);
    expect(r.capped).toBe(false);
  });

  test("PAUSE_STREAK_THRESHOLD 到達で capped=true", () => {
    let last;
    for (let i = 0; i < PAUSE_STREAK_THRESHOLD; i++) {
      last = incStreak(274, "intent-aligned-no", "test reason");
    }
    expect(last!.count).toBe(PAUSE_STREAK_THRESHOLD);
    expect(last!.capped).toBe(true);
  });

  test("THRESHOLD 未満は capped=false", () => {
    incStreak(274, "intent-aligned-no", "r1");
    const r = incStreak(274, "intent-aligned-no", "r2");
    expect(r.count).toBe(2);
    expect(r.capped).toBe(false);
  });

  test("異なる Issue は独立カウント", () => {
    incStreak(274, "intent-aligned-no", "r1");
    incStreak(274, "intent-aligned-no", "r2");
    const r = incStreak(275, "intent-aligned-no", "r1");
    expect(r.count).toBe(1);
    expect(getStreak(274, "intent-aligned-no")?.count).toBe(2);
  });

  test("同じ Issue でも異なる Category は独立カウント", () => {
    incStreak(274, "intent-aligned-no", "r1");
    incStreak(274, "intent-aligned-no", "r2");
    const r = incStreak(274, "codex-usage-limit", "r1");
    expect(r.count).toBe(1);
    expect(getStreak(274, "intent-aligned-no")?.count).toBe(2);
  });

  test("getStreak 未作成は null", () => {
    expect(getStreak(999, "unknown-cat")).toBeNull();
  });

  test("resetStreak で特定 Issue × Category のみ削除", () => {
    incStreak(274, "intent-aligned-no", "r1");
    incStreak(274, "codex-usage-limit", "r1");
    resetStreak(274, "intent-aligned-no");
    expect(getStreak(274, "intent-aligned-no")).toBeNull();
    expect(getStreak(274, "codex-usage-limit")?.count).toBe(1);
  });

  test("resetAllForIssue で Issue の全 Category を一括削除", () => {
    incStreak(274, "intent-aligned-no", "r1");
    incStreak(274, "codex-usage-limit", "r1");
    incStreak(275, "intent-aligned-no", "r1");
    const removed = resetAllForIssue(274);
    expect(new Set(removed)).toEqual(new Set(["intent-aligned-no", "codex-usage-limit"]));
    expect(getStreak(274, "intent-aligned-no")).toBeNull();
    expect(getStreak(274, "codex-usage-limit")).toBeNull();
    // 別 Issue は影響受けない
    expect(getStreak(275, "intent-aligned-no")?.count).toBe(1);
  });

  test("resetAllForIssue で Issue が無いとき空配列", () => {
    const removed = resetAllForIssue(999);
    expect(removed).toEqual([]);
  });

  test("inc 時に reason / last_at が記録される", () => {
    incStreak(274, "intent-aligned-no", "ADR-017 滞留により aligned=no");
    const s = getStreak(274, "intent-aligned-no");
    expect(s?.last_reason).toBe("ADR-017 滞留により aligned=no");
    expect(s?.last_at).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    expect(s?.issue).toBe(274);
    expect(s?.category).toBe("intent-aligned-no");
  });

  test("カテゴリ名の特殊文字は state path で sanitize される (パストラバーサル防止)", () => {
    // category に / が混入してもファイルが安全な path に書き出される
    // (例: "foo/bar" が "foo-bar" など、または弾く)
    expect(() => incStreak(274, "foo/bar", "r1")).toThrow();
    expect(() => incStreak(274, "foo bar", "r1")).toThrow();
    expect(() => incStreak(274, "", "r1")).toThrow();
    expect(() => incStreak(274, "../escape", "r1")).toThrow();
  });
});
