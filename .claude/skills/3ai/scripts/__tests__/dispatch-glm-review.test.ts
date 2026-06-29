// dispatch-glm-review.ts: failure detection + error verdict テスト (#262)
//
// 元事象: #255 で GLM final review が max-turns 5 / 15 共に
// 'Error: Reached max turns (15)' で終了 → verdict.json は
// {verdict:"unknown", blocking:0} となり vacuous pass を許してしまった。
// 本テストは dispatch 失敗を明示的に検出し
// {verdict:"error", dispatch_error:true, blocking:-1} を書く挙動を固める。

import { describe, expect, test } from "bun:test";
import {
  parseVerdict,
  detectDispatchFailure,
  makeErrorVerdict,
  isValidReviewYaml,
  classifyDispatch,
  demoteIssuesToMedium,
} from "../dispatch-glm-review.ts";

describe("parseVerdict (既存挙動の固定)", () => {
  test("T01: verdict:pass + critical 1 + high 1 → blocking=2", () => {
    const yaml = `verdict: pass
issues:
  - id: I1
    severity: critical
  - id: I2
    severity: high
`;
    const v = parseVerdict(yaml);
    expect(v.verdict).toBe("pass");
    expect(v.severity_counts.critical).toBe(1);
    expect(v.severity_counts.high).toBe(1);
    expect(v.blocking).toBe(2);
  });

  test("T02: verdict:fail を拾う", () => {
    const yaml = "verdict: fail\n";
    expect(parseVerdict(yaml).verdict).toBe("fail");
  });

  test("T03_boundary_empty: 空文字 → verdict=unknown, blocking=0", () => {
    const v = parseVerdict("");
    expect(v.verdict).toBe("unknown");
    expect(v.blocking).toBe(0);
    expect(v.severity_counts.critical).toBe(0);
    expect(v.severity_counts.high).toBe(0);
    expect(v.severity_counts.medium).toBe(0);
    expect(v.severity_counts.low).toBe(0);
  });
});

describe("detectDispatchFailure (#262 新規)", () => {
  test("T04_degen_max_turns: 'Error: Reached max turns (15)' → failed reason=max-turns", () => {
    const out = "Error: Reached max turns (15)\n";
    const r = detectDispatchFailure(out, 0, out.trim());
    expect(r.failed).toBe(true);
    expect(r.reason).toBe("max-turns");
  });

  test("T05_degen_non_zero_exit: claude CLI exit=137 → failed reason=claude-exit-137", () => {
    const r = detectDispatchFailure("anything", 137, "anything");
    expect(r.failed).toBe(true);
    expect(r.reason).toBe("claude-exit-137");
  });

  test("T06: 有効 YAML + exit 0 → failed=false", () => {
    const yaml = "verdict: pass\nissues: []\n";
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(false);
  });

  test("T07_boundary_error_prefix: 'Error: api timeout' + verdict 行なし → reason=claude-error", () => {
    const out = "Error: api timeout\n";
    const r = detectDispatchFailure(out, 0, out.trim());
    expect(r.failed).toBe(true);
    expect(r.reason).toBe("claude-error");
  });

  test("T08_boundary_no_verdict: YAML 風だが verdict 行不在 → reason=no-verdict-line", () => {
    const yaml = "issues:\n  - id: I1\n    severity: medium\n";
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(true);
    expect(r.reason).toBe("no-verdict-line");
  });

  test("T08b: max-turns 検出は exit code に優先しない (exit 0 でも検出する)", () => {
    // claude CLI は max-turns 到達時に exit 0 で stdout に Error 文字列を吐くケースがある
    const out = "Error: Reached max turns (5)";
    const r = detectDispatchFailure(out, 0, out);
    expect(r.failed).toBe(true);
    expect(r.reason).toBe("max-turns");
  });

  // #262 r2 F01 (Codex 3 persona 一致): 有効な YAML レビューが "Error: Reached max turns"
  // を引用しているケースで偽陽性を出さないこと。本 Issue 自身のような自己言及 review が
  // 該当する典型例。
  // 有効 verdict + max-turns 文字列を本文中で引用しているケース。
  // 偽陽性を出さないこと (r3 contract も満たすため severity を低めに振って blocking=0)。
  test("T08c_boundary_quote_max_turns: 有効 verdict (pass + medium 引用 max-turns) → failed=false", () => {
    const yaml = `verdict: pass
issues:
  - id: A-N01
    severity: medium
    finding: "dispatcher が 'Error: Reached max turns' を誤検出する可能性がある"
`;
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(false);
  });

  // 有効 verdict + Error: プレフィックスを本文中で引用しているケース。
  test("T08d_boundary_quote_error_prefix: 有効 verdict (fail + high 引用 Error:) → failed=false", () => {
    const yaml = `verdict: fail
issues:
  - id: B-F01
    severity: high
    finding: "log に 'Error: api timeout' と出る箇所がある"
`;
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(false);
  });

  // #262 r2 F01 (Codex 3 persona 一致): top-level に verdict だけあって
  // issues: が欠落しているケース。「verdict: pass で blocking=0」だけが書かれた
  // 不完全出力を vacuous pass にしない。
  test("T08e_degen_verdict_only: verdict のみ + issues 欠落 → reason=missing-issues", () => {
    const yaml = "verdict: pass\n";
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(true);
    expect(r.reason).toBe("missing-issues");
  });

  test("T08f_degen_issues_only: issues のみ + verdict 欠落 → reason=no-verdict-line", () => {
    const yaml = "issues:\n  - id: I1\n    severity: high\n";
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(true);
    expect(r.reason).toBe("no-verdict-line");
  });

  // diff に含まれる verdict 行を GLM が引用しただけのケース。引用は top-level の
  // 列に来ないため失敗判定になる (rawOut では `verdict:` を含むが indented で yamlText 上は top-level でない想定)。
  test("T08g_boundary_indented_verdict: 字下げされた verdict (top-level でない) → reason=no-verdict-line", () => {
    const yaml = `here is the review context:
  verdict: pass
  issues:
    - id: I1
`;
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(true);
    // verdict は字下げで top-level マッチしない → no-verdict-line
    expect(r.reason).toBe("no-verdict-line");
  });
});

describe("detectDispatchFailure verdict-blocking contract (#262 r3)", () => {
  // GLM が verdict:fail と言いつつ critical/high が 0 件 → 自己矛盾。
  // 旧実装ではこれが parseVerdict 経由で {verdict:'fail', blocking:0} となり
  // STEP 7 の pass / blocking>=1 分岐どちらにも乗らない orphan を作っていた。
  test("C01: verdict:fail + blocking=0 (issues empty) → reason=fail-without-blockers", () => {
    const yaml = "verdict: fail\nissues: []\n";
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(true);
    expect(r.reason).toBe("fail-without-blockers");
  });

  test("C02: verdict:fail + medium only (blocking=0) → reason=fail-without-blockers", () => {
    const yaml = `verdict: fail
issues:
  - id: M1
    severity: medium
  - id: L1
    severity: low
`;
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(true);
    expect(r.reason).toBe("fail-without-blockers");
  });

  // #293: pass-with-blockers は dispatch_error にせず demote 経路で吸収する
  // (verdict=pass を尊重し issues を medium にデモート、caller の pass 分岐に乗る)。
  // 後方互換 wrapper detectDispatchFailure では demote = failed:false に倒している。
  test("C03 (#293): verdict:pass + critical 1 → failed=false (demote 経路、wrapper では ok 扱い)", () => {
    const yaml = `verdict: pass
issues:
  - id: C1
    severity: critical
`;
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(false);
  });

  test("C04: verdict:pass + issues empty (blocking=0) → failed=false (legitimate)", () => {
    const yaml = "verdict: pass\nissues: []\n";
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(false);
  });

  test("C05: verdict:fail + high 1 (blocking=1) → failed=false (legitimate)", () => {
    const yaml = `verdict: fail
issues:
  - id: H1
    severity: high
`;
    const r = detectDispatchFailure(yaml, 0, yaml);
    expect(r.failed).toBe(false);
  });
});

describe("isValidReviewYaml (#262 r2)", () => {
  test("V01: verdict + issues 両方 top-level → true", () => {
    expect(isValidReviewYaml("verdict: pass\nissues: []\n")).toBe(true);
  });

  test("V02: verdict のみ → false", () => {
    expect(isValidReviewYaml("verdict: pass\n")).toBe(false);
  });

  test("V03: issues のみ → false", () => {
    expect(isValidReviewYaml("issues:\n  - id: I1\n")).toBe(false);
  });

  test("V04: 空文字 → false", () => {
    expect(isValidReviewYaml("")).toBe(false);
  });

  test("V05: verdict が字下げ (top-level でない) → false", () => {
    expect(isValidReviewYaml("  verdict: pass\nissues: []\n")).toBe(false);
  });
});

describe("makeErrorVerdict (#262 新規)", () => {
  test("T09: 形状 — verdict='error', dispatch_error=true, reason 反映, severity_counts 全 0", () => {
    const v = makeErrorVerdict("max-turns");
    expect(v.verdict).toBe("error");
    expect(v.dispatch_error).toBe(true);
    expect(v.reason).toBe("max-turns");
    expect(v.severity_counts).toEqual({ critical: 0, high: 0, medium: 0, low: 0 });
  });

  test("T10: blocking は負数 (no-findings の 0 と区別)", () => {
    const v = makeErrorVerdict("claude-exit-1");
    expect(v.blocking).toBeLessThan(0);
  });
});

// =====================================================================
// #293: pass-with-blockers を dispatch_error にせず demote 経路で吸収する。
// classifyDispatch (新規 3 値 API) と demoteIssuesToMedium のテスト。
// =====================================================================

const PASS_WITH_HIGH_293 = `issues:
  - id: FN01
    severity: high
    file: "crates/engawa-format/tests/golden_examples.rs"
    line_hint: 247
    finding: "In-Scope の golden round-trip テストが未実装。"
    suggestion: "別 Issue または follow-up commit で追加すること。"
verdict: pass
`;

const PASS_NO_ISSUES_293 = `issues: []
verdict: pass
`;

const FAIL_WITH_HIGH_293 = `issues:
  - id: F01
    severity: high
    finding: "x"
verdict: fail
`;

const FAIL_WITHOUT_BLOCKERS_293 = `issues: []
verdict: fail
`;

describe("classifyDispatch (#293)", () => {
  test("T01 pass-with-blockers → kind=demote (was fail/dispatch_error)", () => {
    const r = classifyDispatch("", 0, PASS_WITH_HIGH_293);
    expect(r.kind).toBe("demote");
    expect(r.reason).toBe("pass-with-blockers");
  });

  test("T02_boundary_pass_no_issues → kind=ok", () => {
    const r = classifyDispatch("", 0, PASS_NO_ISSUES_293);
    expect(r.kind).toBe("ok");
  });

  test("T03_boundary_fail_with_blockers → kind=ok (caller 側で fail 処理)", () => {
    const r = classifyDispatch("", 0, FAIL_WITH_HIGH_293);
    expect(r.kind).toBe("ok");
  });

  test("T04_degen_fail_without_blockers → kind=fail (非対称、本物の矛盾)", () => {
    const r = classifyDispatch("", 0, FAIL_WITHOUT_BLOCKERS_293);
    expect(r.kind).toBe("fail");
    expect(r.reason).toBe("fail-without-blockers");
  });

  test("T05_degen_exit_nonzero → kind=fail", () => {
    const r = classifyDispatch("", 2, PASS_NO_ISSUES_293);
    expect(r.kind).toBe("fail");
    expect(r.reason).toBe("claude-exit-2");
  });

  test("T06_determinism: 同一入力で 2 回呼び出して同一結果", () => {
    const a = classifyDispatch("", 0, PASS_WITH_HIGH_293);
    const b = classifyDispatch("", 0, PASS_WITH_HIGH_293);
    expect(a).toEqual(b);
  });
});

describe("demoteIssuesToMedium (#293)", () => {
  test("severity: high → severity: medium", () => {
    const out = demoteIssuesToMedium(PASS_WITH_HIGH_293);
    expect(out).toContain("severity: medium");
    expect(out).not.toContain("severity: high");
  });

  test("severity: critical も medium にデモート", () => {
    const yaml = `issues:
  - id: X
    severity: critical
verdict: pass
`;
    const out = demoteIssuesToMedium(yaml);
    expect(out).toContain("severity: medium");
    expect(out).not.toContain("severity: critical");
  });

  test("severity: low も medium に整合化 (cap)", () => {
    const yaml = `issues:
  - id: X
    severity: low
verdict: pass
`;
    const out = demoteIssuesToMedium(yaml);
    expect(out).toContain("severity: medium");
    expect(out).not.toContain("severity: low");
  });

  test("デモート後の parseVerdict は blocking=0", () => {
    const v = parseVerdict(demoteIssuesToMedium(PASS_WITH_HIGH_293));
    expect(v.verdict).toBe("pass");
    expect(v.blocking).toBe(0);
    expect(v.severity_counts.high).toBe(0);
    expect(v.severity_counts.medium).toBe(1);
  });

  test("issues 空のケースは no-op", () => {
    expect(demoteIssuesToMedium(PASS_NO_ISSUES_293)).toBe(PASS_NO_ISSUES_293);
  });

  test("T_determinism: 同一入力で同一出力", () => {
    const a = demoteIssuesToMedium(PASS_WITH_HIGH_293);
    const b = demoteIssuesToMedium(PASS_WITH_HIGH_293);
    expect(a).toBe(b);
  });
});
