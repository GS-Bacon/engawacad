// escalate-glm-adversarial.ts: STEP 6-D 自律 escalation テスト (#225)
//
// 検証対象:
//   T01 (determinism)         : tracker が 2 回 escalate() で deterministic に inc
//   T02 (mock pass)           : --mock-mode pass → kind=continue、全 approved
//   T03 (mock refute)         : --mock-mode refute → kind=needs_human, reason=refute
//   T04_boundary_token_cap    : token_used を cap 直前で初期化 → next round で token_cap 越え
//   T05_degen_missing_dir     : feature-dir が存在しない → throw

import { describe, expect, test, beforeEach } from "bun:test";
import { mkdirSync, rmSync, writeFileSync, readFileSync, existsSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  escalate,
  ESCALATION_TOKEN_CAP,
  initTracker,
  incRegen,
  addTokens,
} from "../escalate-glm-adversarial.ts";

const TMP_BASE = join(tmpdir(), `esc-glm-adv-test-${process.pid}`);

function freshTmpRoot(label: string): { trackerRoot: string; featureDir: string } {
  const dir = join(TMP_BASE, label);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const trackerRoot = join(dir, "tracker");
  const featureDir = join(dir, "feature");
  mkdirSync(trackerRoot, { recursive: true });
  mkdirSync(featureDir, { recursive: true });
  writeFileSync(join(featureDir, "ci.log"), "test: failed\nerror: assertion failed", "utf-8");
  writeFileSync(join(featureDir, "debug-spec.md"), "## 仮説\nテスト用 debug-spec\n", "utf-8");
  return { trackerRoot, featureDir };
}

beforeEach(() => {
  process.env.ESC_RAISE_ISSUE = "0";
  process.env.ESC_DISABLE_GH = "1";
  delete process.env.ESC_GLM_ADV_MOCK;
  delete process.env.CODEX_DRY_RUN;
});

describe("escalate-glm-adversarial", () => {
  test("T01 (determinism): 2 回 escalate() で tracker の regen_count が deterministic に inc", async () => {
    const { trackerRoot, featureDir } = freshTmpRoot("t01");
    process.env.ESC_TRACKER_ROOT = trackerRoot;
    const r1 = await escalate({ issueNum: 9001, featureDir, mockMode: "pass" });
    expect(r1.kind).toBe("continue");
    expect(r1.regen_count).toBe(1);
    const r2 = await escalate({ issueNum: 9001, featureDir, mockMode: "pass" });
    expect(r2.kind).toBe("continue");
    expect(r2.regen_count).toBe(2);
    // token_used は 1 round 推定 90k なので 2 round で 180k (< cap 200k)
    expect(r2.token_used).toBe(r1.token_used * 2);
    expect(r2.token_used).toBeLessThan(ESCALATION_TOKEN_CAP);
  });

  test("T02 (mock pass): 全ペルソナ approved → kind=continue", async () => {
    const { trackerRoot, featureDir } = freshTmpRoot("t02");
    process.env.ESC_TRACKER_ROOT = trackerRoot;
    const r = await escalate({ issueNum: 9002, featureDir, mockMode: "pass" });
    expect(r.kind).toBe("continue");
    if (r.kind === "continue") {
      expect(r.verdicts).toHaveLength(3);
      expect(r.verdicts.every(v => v.approved)).toBe(true);
      expect(r.verdicts.map(v => v.persona).sort()).toEqual(["architect", "contrarian", "migration"]);
    }
  });

  test("T03 (mock refute): 全ペルソナ refute → kind=needs_human, reason=refute", async () => {
    const { trackerRoot, featureDir } = freshTmpRoot("t03");
    process.env.ESC_TRACKER_ROOT = trackerRoot;
    const r = await escalate({ issueNum: 9003, featureDir, mockMode: "refute" });
    expect(r.kind).toBe("needs_human");
    if (r.kind === "needs_human") {
      expect(r.reason).toBe("refute");
      expect(r.details.length).toBeGreaterThan(0);
    }
  });

  test("T04_boundary_token_cap: token_used を cap 越え状態に初期化 → reason=token_cap", async () => {
    const { trackerRoot, featureDir } = freshTmpRoot("t04");
    process.env.ESC_TRACKER_ROOT = trackerRoot;
    // 初回 escalate() の token 計上 (推定 90k) で token_used > cap になるように
    // 事前に cap 直前まで token_used を積み上げる
    initTracker(9004);
    addTokens(9004, ESCALATION_TOKEN_CAP - 1_000); // = 199_000
    const r = await escalate({ issueNum: 9004, featureDir, mockMode: "pass" });
    expect(r.kind).toBe("needs_human");
    if (r.kind === "needs_human") {
      expect(r.reason).toBe("token_cap");
      expect(r.token_used).toBeGreaterThan(ESCALATION_TOKEN_CAP);
    }
  });

  test("T05_degen_missing_dir: feature-dir が存在しない → throw", async () => {
    const { trackerRoot } = freshTmpRoot("t05");
    process.env.ESC_TRACKER_ROOT = trackerRoot;
    const missing = join(TMP_BASE, "t05", "does-not-exist");
    await expect(
      escalate({ issueNum: 9005, featureDir: missing, mockMode: "pass" }),
    ).rejects.toThrow(/feature-dir not found/);
  });

  test("T06 (retire writes tracker): refute 経路で tracker に retired メタが付く", async () => {
    const { trackerRoot, featureDir } = freshTmpRoot("t06");
    process.env.ESC_TRACKER_ROOT = trackerRoot;
    await escalate({ issueNum: 9006, featureDir, mockMode: "refute" });
    const trackerPath = join(trackerRoot, "9006.json");
    expect(existsSync(trackerPath)).toBe(true);
    const state = JSON.parse(readFileSync(trackerPath, "utf-8"));
    expect(state.retired?.reason).toBe("refute");
  });
});

describe("escalate-glm-adversarial tracker helpers (純関数)", () => {
  test("T07 incRegen 単発: 0 → 1", () => {
    const { trackerRoot } = freshTmpRoot("t07");
    process.env.ESC_TRACKER_ROOT = trackerRoot;
    const s = incRegen(9007);
    expect(s.regen_count).toBe(1);
  });

  test("T08 addTokens 単発: 0 → n", () => {
    const { trackerRoot } = freshTmpRoot("t08");
    process.env.ESC_TRACKER_ROOT = trackerRoot;
    const s = addTokens(9008, 12345);
    expect(s.token_used).toBe(12345);
  });

  test("T09_degen addTokens 負値 → throw", () => {
    const { trackerRoot } = freshTmpRoot("t09");
    process.env.ESC_TRACKER_ROOT = trackerRoot;
    expect(() => addTokens(9009, -1)).toThrow(/n must be >= 0/);
  });
});
