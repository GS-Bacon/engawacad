// lint-regression-preserved.test.ts — parse/filter ロジックの単体テスト (Issue #313)
//
// 実際の git subprocess は起動せず、`git diff --name-status --diff-filter=DR` の
// 出力文字列を直接パーサに食わせて検証する。

import { describe, expect, test } from "bun:test";
import {
  filterRegressionDeletions,
  formatViolations,
  isRegressionPath,
  parseGitDiffOutput,
} from "./lint-regression-preserved.ts";

describe("isRegressionPath", () => {
  test("crates/foo/tests/regression_issue_100.rs → true", () => {
    expect(isRegressionPath("crates/foo/tests/regression_issue_100.rs")).toBe(true);
  });
  test("regression_100.rs without regression_ prefix → false", () => {
    expect(isRegressionPath("crates/foo/tests/other_test.rs")).toBe(false);
  });
  test("outside crates/ → false", () => {
    expect(isRegressionPath("tests/regression_100.rs")).toBe(false);
  });
  test("regression in src/ (not tests) → false", () => {
    expect(isRegressionPath("crates/foo/src/regression_100.rs")).toBe(false);
  });
});

describe("parseGitDiffOutput", () => {
  test("mix of D/M/A/R lines", () => {
    const raw = [
      "D\tcrates/foo/tests/regression_issue_100.rs",
      "M\tcrates/foo/src/lib.rs",
      "A\tcrates/foo/tests/new_test.rs",
      "R100\tcrates/foo/tests/regression_issue_200.rs\tcrates/foo/tests/misc_200.rs",
    ].join("\n");
    const entries = parseGitDiffOutput(raw);
    // filter は DR に絞られている前提だが、パーサは何でも受け付ける
    const del = entries.find((e) => e.kind === "delete");
    const ren = entries.find((e) => e.kind === "rename");
    expect(del).toEqual({ kind: "delete", path: "crates/foo/tests/regression_issue_100.rs" });
    expect(ren).toEqual({
      kind: "rename",
      score: 100,
      oldPath: "crates/foo/tests/regression_issue_200.rs",
      newPath: "crates/foo/tests/misc_200.rs",
    });
  });

  test("empty input → []", () => {
    expect(parseGitDiffOutput("")).toEqual([]);
    expect(parseGitDiffOutput("\n\n")).toEqual([]);
  });

  test("R without score digits", () => {
    const entries = parseGitDiffOutput("R\ta.rs\tb.rs");
    expect(entries[0]).toMatchObject({ kind: "rename", oldPath: "a.rs", newPath: "b.rs" });
  });
});

describe("filterRegressionDeletions", () => {
  test("T03_regression_preserved_ok: additions/modifications only → 0 violations", () => {
    const raw = [
      "M\tcrates/foo/src/lib.rs",
      "A\tcrates/foo/tests/new_test.rs",
      "A\tcrates/foo/tests/regression_issue_new.rs",
    ].join("\n");
    const entries = parseGitDiffOutput(raw);
    expect(filterRegressionDeletions(entries)).toEqual([]);
  });

  test("T04_regression_preserved_ng_deleted: D 行 1 件 → 1 violation", () => {
    const raw = "D\tcrates/foo/tests/regression_issue_100.rs";
    const violations = filterRegressionDeletions(parseGitDiffOutput(raw));
    expect(violations).toEqual([
      { kind: "delete", oldPath: "crates/foo/tests/regression_issue_100.rs" },
    ]);
  });

  test("T04_regression_preserved_ng_renamed_away: R (regression_ → misc_) → 1 violation", () => {
    const raw =
      "R100\tcrates/foo/tests/regression_issue_100.rs\tcrates/foo/tests/misc_100.rs";
    const violations = filterRegressionDeletions(parseGitDiffOutput(raw));
    expect(violations).toEqual([
      {
        kind: "rename",
        oldPath: "crates/foo/tests/regression_issue_100.rs",
        newPath: "crates/foo/tests/misc_100.rs",
      },
    ]);
  });

  test("T_bonus_renamed_within: regression_100 → regression_100_v2 → 0 violations", () => {
    const raw =
      "R100\tcrates/foo/tests/regression_100.rs\tcrates/foo/tests/regression_100_v2.rs";
    const violations = filterRegressionDeletions(parseGitDiffOutput(raw));
    expect(violations).toEqual([]);
  });

  test("T_bonus_non_regression_deleted: D 行が regression_ 以外 → 0 violations", () => {
    const raw = "D\tcrates/foo/tests/other_test.rs";
    expect(filterRegressionDeletions(parseGitDiffOutput(raw))).toEqual([]);
  });

  test("T_bonus_multiple: D 行 2 件 → 2 violations", () => {
    const raw = [
      "D\tcrates/foo/tests/regression_issue_100.rs",
      "D\tcrates/bar/tests/regression_issue_200.rs",
    ].join("\n");
    const violations = filterRegressionDeletions(parseGitDiffOutput(raw));
    expect(violations).toHaveLength(2);
    expect(violations.map((v) => v.oldPath)).toEqual([
      "crates/foo/tests/regression_issue_100.rs",
      "crates/bar/tests/regression_issue_200.rs",
    ]);
  });

  test("delete outside crates/ → 0 violations (誤検出防止)", () => {
    const raw = "D\ttests/regression_100.rs";
    expect(filterRegressionDeletions(parseGitDiffOutput(raw))).toEqual([]);
  });

  test("rename outside crates/tests → 0 violations", () => {
    const raw = "R100\tdocs/regression_100.md\tdocs/moved.md";
    expect(filterRegressionDeletions(parseGitDiffOutput(raw))).toEqual([]);
  });
});

describe("formatViolations", () => {
  test("D と R が両方含まれる出力", () => {
    const out = formatViolations([
      { kind: "delete", oldPath: "crates/foo/tests/regression_issue_123.rs" },
      {
        kind: "rename",
        oldPath: "crates/foo/tests/regression_issue_456.rs",
        newPath: "crates/foo/tests/misc_456.rs",
      },
    ]);
    expect(out).toContain("ERROR: 2 件");
    expect(out).toContain("D crates/foo/tests/regression_issue_123.rs");
    expect(out).toContain(
      "R crates/foo/tests/regression_issue_456.rs -> crates/foo/tests/misc_456.rs",
    );
    expect(out).toContain("plan.md");
    expect(out.endsWith("\n")).toBe(true);
  });
});
