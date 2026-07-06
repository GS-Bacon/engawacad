// dispatch-codex-design.ts: buildDesignInput の unit test
//
// Codex 呼び本体 (dispatchCodex) は既存の dispatch-codex.ts のテストで担保されるので、
// ここでは入力連結ロジックのみをテストする。

import { describe, expect, test } from "bun:test";
import { mkdirSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { buildDesignInput } from "../dispatch-codex-design.ts";

const TMP_BASE = join(tmpdir(), `dispatch-codex-design-test-${process.pid}`);

function freshDir(label: string): string {
  const dir = join(TMP_BASE, label);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  return dir;
}

describe("buildDesignInput", () => {
  test("plan.md のみ (他ファイルなし) → PLAN ブロックのみ", () => {
    const dir = freshDir("plan-only");
    const planFile = join(dir, "plan.md");
    writeFileSync(planFile, "## In-Scope\n- foo", "utf-8");

    const output = buildDesignInput(planFile, dir);
    expect(output).toContain("===== PLAN =====");
    expect(output).toContain("## In-Scope");
    expect(output).toContain("===== END PLAN =====");
    expect(output).not.toContain("===== ADR EXCERPT =====");
    expect(output).not.toContain("===== PRIOR JUDGMENTS =====");
    expect(output).not.toContain("===== PRIOR REJECTIONS =====");
  });

  test("全 optional ファイル存在 → 全 4 ブロック", () => {
    const dir = freshDir("all");
    const planFile = join(dir, "plan.md");
    writeFileSync(planFile, "PLAN BODY", "utf-8");
    writeFileSync(join(dir, "adr-context.md"), "ADR BODY", "utf-8");
    writeFileSync(join(dir, "judgment-summary.md"), "JUDGMENT BODY", "utf-8");
    writeFileSync(join(dir, "rejection.md"), "REJECTION BODY", "utf-8");

    const output = buildDesignInput(planFile, dir);
    // 順序: ADR → JUDGMENTS → REJECTIONS → PLAN
    const adrIdx = output.indexOf("===== ADR EXCERPT =====");
    const judgIdx = output.indexOf("===== PRIOR JUDGMENTS =====");
    const rejIdx = output.indexOf("===== PRIOR REJECTIONS =====");
    const planIdx = output.indexOf("===== PLAN =====");

    expect(adrIdx).toBeGreaterThanOrEqual(0);
    expect(judgIdx).toBeGreaterThan(adrIdx);
    expect(rejIdx).toBeGreaterThan(judgIdx);
    expect(planIdx).toBeGreaterThan(rejIdx);

    expect(output).toContain("ADR BODY");
    expect(output).toContain("JUDGMENT BODY");
    expect(output).toContain("REJECTION BODY");
    expect(output).toContain("PLAN BODY");
  });

  test("adr-context.md のみ存在 → ADR + PLAN", () => {
    const dir = freshDir("adr-only");
    const planFile = join(dir, "plan.md");
    writeFileSync(planFile, "PLAN", "utf-8");
    writeFileSync(join(dir, "adr-context.md"), "ADR", "utf-8");

    const output = buildDesignInput(planFile, dir);
    expect(output).toContain("===== ADR EXCERPT =====");
    expect(output).toContain("===== PLAN =====");
    expect(output).not.toContain("===== PRIOR JUDGMENTS =====");
    expect(output).not.toContain("===== PRIOR REJECTIONS =====");
  });

  test("空 rejection.md → ブロック追加 (空でもマーカー付与)", () => {
    const dir = freshDir("empty-rej");
    const planFile = join(dir, "plan.md");
    writeFileSync(planFile, "PLAN", "utf-8");
    writeFileSync(join(dir, "rejection.md"), "", "utf-8");

    const output = buildDesignInput(planFile, dir);
    expect(output).toContain("===== PRIOR REJECTIONS =====");
    expect(output).toContain("===== END PRIOR REJECTIONS =====");
  });

  test("plan.md 存在しない → throw", () => {
    const dir = freshDir("no-plan");
    expect(() => buildDesignInput(join(dir, "no-plan.md"), dir)).toThrow();
  });
});
