// loop-cycle-record.ts の pause_reason 同状態判定 単体テスト (#229)
//
// extractRootCauseHash / isSameStatePause は cycle 反復 (#20→#21 のような同 step / 同 Issue
// での再 pause) を 1 cycle で打ち切るための純関数。本テストはハッシュ抽出と判定を検証する。

import { describe, expect, test } from "bun:test";
import { extractRootCauseHash, isSameStatePause } from "./loop-cycle-record.ts";

describe("extractRootCauseHash", () => {
  test("T01: undefined → null", () => {
    expect(extractRootCauseHash(undefined)).toBeNull();
  });

  test("T02: 空文字列 → null (#NN なし)", () => {
    expect(extractRootCauseHash("")).toBeNull();
  });

  test("T03: 通常の #220 STEP 6-D pause → issue=220 / step=STEP 6-D", () => {
    const r = extractRootCauseHash("#220 STEP 6-D escalation 3 サイクル目: ESC_MAX_LOOPS=1 消費済み");
    expect(r).not.toBeNull();
    expect(r?.issueNum).toBe(220);
    expect(r?.step).toBe("STEP 6-D");
    expect(r?.hash).toBe("220:STEP 6-D");
  });

  test("T04: #220 STEP 6-D + 別の説明 → 同 hash (再現性)", () => {
    // Note: 先頭の #NN を root cause issue として採用するので、prefix に #19 を混ぜると
    // そちらを拾ってしまう。pause_reason は先頭で対象 Issue を明示する想定。
    const a = extractRootCauseHash("#220 STEP 6-D escalation continuation r3");
    const b = extractRootCauseHash("#220 STEP 6-D escalation 3 サイクル目: 別の説明");
    expect(a?.hash).toBe(b?.hash); // 説明が違っても hash 一致
    expect(a?.issueNum).toBe(220);
  });

  test("T05: #220 のみ (step なし) → step='_' / hash='220:_'", () => {
    const r = extractRootCauseHash("#220 WIP uncommitted changes");
    expect(r?.issueNum).toBe(220);
    expect(r?.step).toBe("_");
    expect(r?.hash).toBe("220:_");
  });

  test("T06: 先頭 #N が複数あれば最初の Issue 番号を採用", () => {
    const r = extractRootCauseHash("Pause #220 needs-human, #221 already raised");
    expect(r?.issueNum).toBe(220);
  });

  test("T07: STEP 7.5 codex review → step='STEP 7.5'", () => {
    const r = extractRootCauseHash("#206 STEP 7.5 codex review needs-review");
    expect(r?.issueNum).toBe(206);
    expect(r?.step).toBe("STEP 7.5");
  });

  test("T08: STEP B-3 intent-check → step='STEP B-3'", () => {
    const r = extractRootCauseHash("#206 STEP B-3 intent-check aligned no");
    expect(r?.issueNum).toBe(206);
    expect(r?.step).toBe("STEP B-3");
  });

  test("T09: step 表記の case 差異 (step 6-D) → 大文字化で同 hash", () => {
    const a = extractRootCauseHash("#220 STEP 6-D foo");
    const b = extractRootCauseHash("#220 step 6-D bar");
    expect(a?.hash).toBe(b?.hash);
  });

  test("T10 (determinism): 同入力 2 回呼び出して結果一致", () => {
    const reason = "#220 STEP 6-D escalation 3 サイクル目";
    expect(extractRootCauseHash(reason)).toEqual(extractRootCauseHash(reason));
  });

  test("T10b (Cycle prefix): 'Cycle #22: #220 WIP...' → cycle 番号は無視して #220 を採用", () => {
    const r = extractRootCauseHash("Cycle #22: #220 WIP (uncommitted assemble.rs partial-pit detect) leaves cargo xtask ci red");
    expect(r?.issueNum).toBe(220);
    expect(r?.step).toBe("_");
    expect(r?.hash).toBe("220:_");
  });

  test("T10c (Cycle prefix + STEP): 'Cycle #21: #220 STEP 6-D escalation' → issue=220 / step=STEP 6-D", () => {
    const r = extractRootCauseHash("Cycle #21: #220 STEP 6-D escalation 3 サイクル目");
    expect(r?.issueNum).toBe(220);
    expect(r?.step).toBe("STEP 6-D");
  });
});

describe("isSameStatePause (#229 short-circuit)", () => {
  test("T11 (trigger 反復): cycle #20→#21 と同じ Issue + 同じ step → sameState=true", () => {
    const prev = "#220 STEP 6-D escalation continuation: ESC_MAX_LOOPS=1 既消費";
    const cur = "#220 STEP 6-D escalation 3 サイクル目: ESC_MAX_LOOPS=1 消費済み";
    const r = isSameStatePause(cur, prev);
    expect(r.sameState).toBe(true);
    expect(r.issueNum).toBe(220);
    expect(r.hash).toBe("220:STEP 6-D");
  });

  test("T12 (no trigger 別 Issue): 同 step でも別 Issue → sameState=false", () => {
    const prev = "#220 STEP 6-D foo";
    const cur = "#221 STEP 6-D bar";
    expect(isSameStatePause(cur, prev).sameState).toBe(false);
  });

  test("T13 (no trigger 別 step): 同 Issue でも別 step → sameState=false", () => {
    const prev = "#206 STEP B-3 intent-check aligned no";
    const cur = "#206 STEP 6-D escalation";
    expect(isSameStatePause(cur, prev).sameState).toBe(false);
  });

  test("T14 (no trigger 前回 pause なし): prev=undefined → sameState=false", () => {
    expect(isSameStatePause("#220 STEP 6-D", undefined).sameState).toBe(false);
  });

  test("T15 (no trigger 今回 pause なし): cur=undefined → sameState=false", () => {
    expect(isSameStatePause(undefined, "#220 STEP 6-D").sameState).toBe(false);
  });

  test("T16 (no trigger 両方 pause なし): 両方 undefined → sameState=false", () => {
    expect(isSameStatePause(undefined, undefined).sameState).toBe(false);
  });

  test("T17 (no trigger Issue 番号なし): hash 抽出不能 → sameState=false", () => {
    expect(isSameStatePause("STEP 6-D escalation", "STEP 6-D escalation").sameState).toBe(false);
  });

  test("T18 (trigger step なし両側): 同 Issue + step なし → sameState=true (hash=NN:_)", () => {
    const prev = "#220 WIP uncommitted changes";
    const cur = "#220 WIP still uncommitted";
    const r = isSameStatePause(cur, prev);
    expect(r.sameState).toBe(true);
    expect(r.issueNum).toBe(220);
  });

  test("T19 (no trigger step 片側のみ): 一方 step あり / 一方 step なし → hash 一致せず", () => {
    const prev = "#220 STEP 6-D escalation";
    const cur = "#220 WIP";
    expect(isSameStatePause(cur, prev).sameState).toBe(false);
  });

  test("T20 (determinism): 同入力 2 回呼び出して結果一致", () => {
    const prev = "#220 STEP 6-D foo";
    const cur = "#220 STEP 6-D bar";
    expect(isSameStatePause(cur, prev)).toEqual(isSameStatePause(cur, prev));
  });
});
