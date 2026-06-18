// raise-issue-on-failure.ts の shouldSkipStep / findRecentSameStepIssue 単体テスト (#211, #229)
//
// intent-check 系の step は ADR-006 粒度違反シグナル (=人間判断項目) であり、
// raise-issue-on-failure による自動 bug 起票の対象外。本テストは判定の純関数を検証する。
//
// findRecentSameStepIssue (#229) は escalation 起票 dedup ガード:
// 同 step + 同 parent + 直近 24h で open Issue があれば dedup 対象として返す。

import { describe, expect, test } from "bun:test";
import { findRecentSameStepIssue, shouldSkipError, shouldSkipStep } from "../raise-issue-on-failure.ts";

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

describe("shouldSkipError (#250 Codex usage limit ガード)", () => {
  test("T_E01 (skip): 'Codex 3 ペルソナ r1: usage limit (You\\'ve hit your usage limit, reset 6:06 PM)' → skip:true", () => {
    const r = shouldSkipError("Codex 3 ペルソナ r1: usage limit (You've hit your usage limit, reset 6:06 PM)");
    expect(r.skip).toBe(true);
    expect(r.reason).toContain("usage|rate limit");
  });

  test("T_E02 (skip): 大文字 'USAGE LIMIT' → skip:true", () => {
    expect(shouldSkipError("USAGE LIMIT").skip).toBe(true);
  });

  test("T_E03 (skip): 'rate limit reached' → skip:true", () => {
    expect(shouldSkipError("rate limit reached").skip).toBe(true);
  });

  test("T_E04 (skip): HTTP 429 単体 → skip:true", () => {
    expect(shouldSkipError("Got HTTP 429 from upstream").skip).toBe(true);
  });

  test("T_E05 (pass): 通常の test FAILED → skip:false", () => {
    expect(shouldSkipError("test FAILED at brep::shell::tests::cuboid_shells").skip).toBe(false);
  });

  test("T_E06 (pass): 空文字列 → skip:false (誤マッチ防止)", () => {
    expect(shouldSkipError("").skip).toBe(false);
  });

  test("T_E07 (boundary): 数字 '1429' は 429 単独ではないので skip:false", () => {
    expect(shouldSkipError("error code 1429 (custom)").skip).toBe(false);
  });

  test("T_E08 (determinism): 同入力 2 回呼びで結果一致", () => {
    const a = shouldSkipError("usage limit");
    const b = shouldSkipError("usage limit");
    expect(a).toEqual(b);
  });
});

describe("findRecentSameStepIssue (#229 dedup ガード)", () => {
  const now = new Date("2026-06-18T12:00:00Z");
  // 過去事例: 親 Issue #220, step "STEP 6-D" で auto-raise された #221 (1h 前) と #222 (12h 前) を想定
  const candidates = [
    {
      number: 221,
      title: "fix(3ai): [自動起票] STEP 6-D でエラー — Issue #220 (phase8-partial-pit)",
      createdAt: "2026-06-18T11:00:00Z", // 1h 前
    },
    {
      number: 222,
      title: "fix(3ai): [自動起票] STEP 6-D でエラー — Issue #220 (phase8-partial-pit)",
      createdAt: "2026-06-18T00:00:00Z", // 12h 前
    },
    {
      number: 230,
      title: "fix(3ai): [自動起票] STEP 6-D でエラー — Issue #220 (phase8-partial-pit)",
      createdAt: "2026-06-15T12:00:00Z", // 72h 前 (24h 外)
    },
  ];

  test("T11 (trigger 24h 内 1h 前): 同 step + 同 parent + 1h 前 → dedupTo=221", () => {
    const r = findRecentSameStepIssue("STEP 6-D", 220, candidates, now, 24);
    expect(r.dedupTo).toBe(221);
  });

  test("T12 (trigger 24h 内 12h 前): 一番新しい候補が優先 (順序通り 221 を返す)", () => {
    const r = findRecentSameStepIssue("STEP 6-D", 220, candidates, now, 24);
    // candidates 配列の最初の match を返す。並びは呼び元が createdAt desc 想定。
    expect(r.dedupTo).toBe(221);
  });

  test("T13 (no trigger 24h 外): 72h 前のみだと dedupTo=null", () => {
    const onlyOld = [candidates[2]];
    const r = findRecentSameStepIssue("STEP 6-D", 220, onlyOld, now, 24);
    expect(r.dedupTo).toBeNull();
  });

  test("T14 (no trigger 親違い): 同 step だが parent が違う → dedupTo=null", () => {
    const r = findRecentSameStepIssue("STEP 6-D", 999, candidates, now, 24);
    expect(r.dedupTo).toBeNull();
  });

  test("T15 (no trigger step 違い): 同 parent だが step が違う → dedupTo=null", () => {
    const r = findRecentSameStepIssue("STEP 7.5", 220, candidates, now, 24);
    expect(r.dedupTo).toBeNull();
  });

  test("T16 (no trigger 空 candidates): dedupTo=null", () => {
    const r = findRecentSameStepIssue("STEP 6-D", 220, [], now, 24);
    expect(r.dedupTo).toBeNull();
  });

  test("T17 (boundary parent #2200 vs #220): 文字列部分一致で #2200 と #220 が混ざらない", () => {
    const mixed = [
      {
        number: 300,
        title: "fix(3ai): [自動起票] STEP 6-D でエラー — Issue #2200 (other-feature)",
        createdAt: "2026-06-18T11:00:00Z",
      },
    ];
    const r = findRecentSameStepIssue("STEP 6-D", 220, mixed, now, 24);
    expect(r.dedupTo).toBeNull(); // #220 で検索したのに #2200 を拾わない
  });

  test("T18 (boundary maxAge=0): maxAgeHours=0 ならどれもマッチしない", () => {
    const r = findRecentSameStepIssue("STEP 6-D", 220, candidates, now, 0);
    expect(r.dedupTo).toBeNull();
  });

  test("T19 (determinism): 同入力 2 回呼び出して結果一致", () => {
    const a = findRecentSameStepIssue("STEP 6-D", 220, candidates, now, 24);
    const b = findRecentSameStepIssue("STEP 6-D", 220, candidates, now, 24);
    expect(a).toEqual(b);
  });

  test("T20 (invalid createdAt): 不正な ISO は skip (dedupTo=null)", () => {
    const invalid = [{ number: 999, title: "STEP 6-D #220", createdAt: "not-a-date" }];
    const r = findRecentSameStepIssue("STEP 6-D", 220, invalid, now, 24);
    expect(r.dedupTo).toBeNull();
  });
});
