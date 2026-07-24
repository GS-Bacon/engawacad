// check-proptest-required.test.ts — #318 Phase C の T01-T03 + bonus
//
// 実 CLI プロセスは起動せず、runCheck を直接呼んで exit code / 出力を検証する。

import { describe, expect, test } from "bun:test";
import { mkdtempSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  inferPhaseFromDir,
  isProptestRequired,
  parseProptestIds,
  runCheck,
} from "./check-proptest-required.ts";

function makeFeatureDir(spec: string | null): string {
  const dir = mkdtempSync(join(tmpdir(), "proptest-check-"));
  if (spec !== null) writeFileSync(join(dir, "test-spec.md"), spec);
  return dir;
}

describe("parseProptestIds", () => {
  test("T_bonus_parse_no_match: T_PROP_ を含まない文字列は []", () => {
    expect(parseProptestIds("no T_PROP here")).toEqual([]);
    expect(parseProptestIds("T_PROP (末尾に _ 無し) は不一致")).toEqual([]);
  });

  test("T_bonus_parse_multi: 3 件の T_PROP_* ID を全て返す", () => {
    const content = [
      "## proptest",
      "- T_PROP_manifold_after_fuse: 融合後の manifold 性",
      "- T_PROP_euler_invariant: Euler-Poincaré",
      "- T_PROP_determinism_seed42",
    ].join("\n");
    expect(parseProptestIds(content)).toEqual([
      "T_PROP_manifold_after_fuse",
      "T_PROP_euler_invariant",
      "T_PROP_determinism_seed42",
    ]);
  });

  test("英数字と _ のみを ID とみなす (ハイフンなどは境界)", () => {
    expect(parseProptestIds("T_PROP_abc-def は abc 部分のみ")).toEqual([
      "T_PROP_abc",
    ]);
  });
});

describe("isProptestRequired", () => {
  test("Phase 11 → required (タイトル問わず)", () => {
    expect(isProptestRequired(11, "feat: whatever")).toBe(true);
    expect(isProptestRequired(12, "")).toBe(true);
  });

  test("Phase 10 + boolean タイトル (大文字混在) → required", () => {
    expect(
      isProptestRequired(10, "feat(phase10): Boolean fuse robustness"),
    ).toBe(true);
    expect(isProptestRequired(10, "boolean cut edge case")).toBe(true);
  });

  test("Phase 10 + Tessellation / tessellate タイトル → required", () => {
    expect(isProptestRequired(10, "Tessellation adaptive edge split")).toBe(
      true,
    );
    expect(isProptestRequired(10, "tessellate 密度検証")).toBe(true);
  });

  test("Phase 10 + 曲面 / 自由曲面 (日本語) → required", () => {
    expect(isProptestRequired(10, "曲面フィレット導入")).toBe(true);
    expect(isProptestRequired(10, "自由曲面のトリム")).toBe(true);
  });

  test("Phase 10 + 汎用タイトル → not required", () => {
    expect(
      isProptestRequired(10, "feat(phase10): 汎用ユーティリティ整理"),
    ).toBe(false);
  });
});

describe("inferPhaseFromDir", () => {
  test("features/295-phase10-sketch-offset-engawa → 10", () => {
    expect(inferPhaseFromDir("features/295-phase10-sketch-offset-engawa")).toBe(
      10,
    );
  });

  test("features/318-phase11-quality-gate → 11", () => {
    expect(inferPhaseFromDir("/abs/features/318-phase11-quality-gate")).toBe(
      11,
    );
  });

  test("phase を含まない slug → undefined", () => {
    expect(inferPhaseFromDir("features/12-cylinder")).toBeUndefined();
  });
});

describe("runCheck", () => {
  test("T01_proptest_required_phase11: T_PROP_solver_convergence 有 → exit 0", () => {
    const dir = makeFeatureDir(
      "- T_PROP_solver_convergence: ソルバ収束性",
    );
    const res = runCheck({
      featureDir: dir,
      phase: 11,
      issueTitle: "feat: solver 実装",
    });
    expect(res.exitCode).toBe(0);
    expect(res.stdout).toContain("OK: proptest ID 1 件確認");
    expect(res.stdout).toContain("Phase 11");
  });

  test("T02_proptest_missing_phase11: T_PROP_ 無し → exit 1 + 明確なメッセージ", () => {
    const dir = makeFeatureDir("- T01_unit_only: 単体テストのみ");
    const res = runCheck({
      featureDir: dir,
      phase: 11,
      issueTitle: "feat: solver 実装",
    });
    expect(res.exitCode).toBe(1);
    expect(res.stderr).toContain("T_PROP_* ID が最低 1 件必要");
    expect(res.stderr).toContain("Phase 11");
    expect(res.stderr).toContain("T_PROP_manifold_after_fuse");
  });

  test("T03_proptest_not_required_phase10: 非必須 → exit 0", () => {
    const dir = makeFeatureDir("- T01_x: unit only");
    const res = runCheck({
      featureDir: dir,
      phase: 10,
      issueTitle: "feat(phase10): 汎用ユーティリティ整理",
    });
    expect(res.exitCode).toBe(0);
    expect(res.stdout).toContain("非必須");
    expect(res.stdout).toContain("Phase 10");
  });

  test("T_bonus_proptest_boolean_issue: phase=10 + Boolean タイトル → required, exit 1", () => {
    const dir = makeFeatureDir("- T01_manual_case: 手動確認のみ");
    const res = runCheck({
      featureDir: dir,
      phase: 10,
      issueTitle: "feat(phase10): Boolean fuse edge case",
    });
    expect(res.exitCode).toBe(1);
    expect(res.stderr).toContain("T_PROP_*");
    expect(res.stderr).toContain("Phase 10");
  });

  test("T_bonus_proptest_tessellation: phase=10 + Tessellation タイトル → required", () => {
    const dir = makeFeatureDir("- T01: unit");
    const res = runCheck({
      featureDir: dir,
      phase: 10,
      issueTitle: "Tessellation triangle-count invariant",
    });
    expect(res.exitCode).toBe(1);
    expect(res.stderr).toContain("T_PROP_*");
  });

  test("T_bonus_missing_spec: test-spec.md 無し → exit 2", () => {
    const dir = makeFeatureDir(null);
    const res = runCheck({
      featureDir: dir,
      phase: 11,
      issueTitle: "feat: solver",
    });
    expect(res.exitCode).toBe(2);
    expect(res.stderr).toContain("test-spec.md");
  });

  test("必須 + T_PROP_ 複数件 → exit 0 + 件数を stdout に反映", () => {
    const dir = makeFeatureDir(
      [
        "- T_PROP_manifold_after_fuse",
        "- T_PROP_euler_invariant",
        "- T_PROP_determinism_seed42",
      ].join("\n"),
    );
    const res = runCheck({
      featureDir: dir,
      phase: 11,
      issueTitle: "feat: solver",
    });
    expect(res.exitCode).toBe(0);
    expect(res.stdout).toContain("OK: proptest ID 3 件確認");
  });

  test("phase 未指定 & feature-dir から Phase 推論 (phase11 slug) → required 判定", () => {
    const dir = mkdtempSync(join(tmpdir(), "295-phase11-foo-"));
    writeFileSync(join(dir, "test-spec.md"), "- T01_x: unit only");
    const res = runCheck({ featureDir: dir, issueTitle: "feat: foo" });
    expect(res.exitCode).toBe(1);
    expect(res.stderr).toContain("Phase 11");
  });
});
