// raise-issue-on-failure.ts の shouldSkipStep ガード単体テスト (#211)
//
// intent-check 系の step は ADR-006 粒度違反シグナル (=人間判断項目) であり、
// raise-issue-on-failure による自動 bug 起票の対象外。本テストは判定の純関数を検証する。

import { describe, expect, test } from "bun:test";
import { shouldSkipStep } from "../raise-issue-on-failure.ts";

describe("shouldSkipStep", () => {
  test("T01 (determinism): 同一入力を 2 回呼んで結果が一致", () => {
    const a = shouldSkipStep("STEP B-3 intent-check failed");
    const b = shouldSkipStep("STEP B-3 intent-check failed");
    expect(a).toEqual(b);
    expect(a.skip).toBe(true);
  });

  test("T02 (skip): 'STEP B-3 intent-check failed' → skip:true", () => {
    const r = shouldSkipStep("STEP B-3 intent-check failed");
    expect(r.skip).toBe(true);
    expect(r.reason).toContain("intent-check");
  });

  test("T03 (skip): snake_case 'intent_check' → skip:true", () => {
    expect(shouldSkipStep("intent_check aligned no").skip).toBe(true);
  });

  test("T04 (skip): 大文字 'INTENT-CHECK aligned no' → skip:true", () => {
    expect(shouldSkipStep("INTENT-CHECK aligned no").skip).toBe(true);
  });

  test("T05 (pass): 'STEP 6-D' → skip:false (通常の GLM 失敗系)", () => {
    expect(shouldSkipStep("STEP 6-D").skip).toBe(false);
  });

  test("T06 (pass): 'STEP 7.5 codex_review' → skip:false (Codex 独立ゲート)", () => {
    expect(shouldSkipStep("STEP 7.5 codex_review").skip).toBe(false);
  });

  test("T07_boundary_empty: 空文字列 → skip:false (誤マッチ防止)", () => {
    expect(shouldSkipStep("").skip).toBe(false);
  });

  test("T08_boundary_intent_only: 'intent' 単体 → skip:false (check が無いと判定不可)", () => {
    expect(shouldSkipStep("intent").skip).toBe(false);
  });

  test("T09_boundary_unrelated: 'check' 単体 → skip:false (intent が無いと判定不可)", () => {
    expect(shouldSkipStep("check status").skip).toBe(false);
  });
});
