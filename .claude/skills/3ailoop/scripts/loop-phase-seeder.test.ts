// #315 Phase B: loop-phase-seeder テスト
//
// 検証範囲:
//   - ROADMAP 完了条件パーサ (parseCompletionConditions)
//   - batch:* 推定 (guessBatchLabel)
//   - Issue draft 生成 (renderIssueDraft)
//   - 粒度チェック + split 展開 (expandDrafts) と needs-human 退避
//   - runSeeder エントリの exit code / dry-run 出力 / gh 起票の依存注入経路

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  expandDrafts,
  guessBatchLabel,
  parseCompletionConditions,
  renderIssueDraft,
  runSeeder,
  type Checker,
  type IssueDraft,
  type PhaseContext,
} from "./loop-phase-seeder";
import type { CheckResult } from "../../3ai/scripts/check-issue-granularity";

// --- テスト用の一時 ROADMAP を作るユーティリティ ---

function mkTempDir(prefix: string): string {
  const dir = join(tmpdir(), `${prefix}-${process.pid}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`);
  mkdirSync(dir, { recursive: true });
  return dir;
}

const PHASE_11_STUB = `# EngawaCAD Roadmap

## Phase 10: 先行 ✅

**外から見た成果**: 済

**完了条件**:
- 既済

---

## Phase 11: 拘束ソルバ 基礎

**外から見た成果**: スケッチに幾何拘束・寸法拘束を付与してパラメトリックに駆動できる

**完了条件**:
- 幾何拘束 (Horizontal / Vertical / Coincident) が動作
- 寸法拘束 (Linear / Aligned) が動作
- Over/Under-constrained 検出が動作
- ソルバの決定性テスト緑

**前提 ADR**: 新規 (ADR-004 tolerant model との整合)

---

## Phase 12: あと

**外から見た成果**: TBD

**完了条件**:
- TBD
`;

const PHASE_COMPLETED_STUB = `# EngawaCAD Roadmap

## ✅ Phase 4: Boolean 演算ができる

**外から見た成果**: 形状の足し引き

**完了条件**:
- Cut / Fuse / Intersect が動作

---
`;

const PHASE_EMPTY_CONDITIONS_STUB = `# EngawaCAD Roadmap

## Phase 42: 空条件

**外から見た成果**: 何か

**完了条件**:

---

## Phase 43: 次
`;

// --- parseCompletionConditions ---

describe("parseCompletionConditions", () => {
  test("Phase 11 完了条件 4 件を抽出", () => {
    const p = parseCompletionConditions(PHASE_11_STUB, 11);
    expect(p.found).toBe(true);
    expect(p.completed).toBe(false);
    expect(p.title).toBe("拘束ソルバ 基礎");
    expect(p.visibleOutcome).toContain("パラメトリック");
    expect(p.conditions.length).toBe(4);
    expect(p.conditions[0]).toContain("幾何拘束");
    expect(p.conditions[3]).toContain("ソルバの決定性テスト緑");
    expect(p.adrRefs).toContain("ADR-004");
  });

  test("Phase 見つからず → found=false", () => {
    const p = parseCompletionConditions(PHASE_11_STUB, 99);
    expect(p.found).toBe(false);
    expect(p.conditions).toEqual([]);
  });

  test("✅ 付き Phase → completed=true", () => {
    const p = parseCompletionConditions(PHASE_COMPLETED_STUB, 4);
    expect(p.found).toBe(true);
    expect(p.completed).toBe(true);
    expect(p.conditions.length).toBe(1);
  });

  test("完了条件ブロックが空 → conditions=[]", () => {
    const p = parseCompletionConditions(PHASE_EMPTY_CONDITIONS_STUB, 42);
    expect(p.found).toBe(true);
    expect(p.completed).toBe(false);
    expect(p.conditions).toEqual([]);
  });

  test("番号一致は境界厳密 (11 で 1 が hit しない、10 も別)", () => {
    const p1 = parseCompletionConditions(PHASE_11_STUB, 1);
    expect(p1.found).toBe(false);
    const p10 = parseCompletionConditions(PHASE_11_STUB, 10);
    expect(p10.found).toBe(true);
    expect(p10.completed).toBe(true);
  });
});

// --- guessBatchLabel ---

describe("guessBatchLabel", () => {
  test("T_bonus_batch_kernel", () => {
    expect(guessBatchLabel("Extrude が動作")).toBe("kernel");
  });
  test("T_bonus_batch_viewer", () => {
    expect(guessBatchLabel("ブラウザ上で回転できる")).toBe("viewer");
  });
  test("T_bonus_batch_data", () => {
    expect(guessBatchLabel(".engawa スキーマに X フィールド追加")).toBe("data");
  });
  test("T_bonus_batch_skill", () => {
    expect(guessBatchLabel("/3ailoop の loop-X.ts")).toBe("skill");
  });
  test("kernel default (無関係語)", () => {
    expect(guessBatchLabel("Fillet と Chamfer が動作")).toBe("kernel");
  });
});

// --- renderIssueDraft ---

describe("renderIssueDraft", () => {
  const ctx: PhaseContext = {
    phase: 11,
    title: "拘束ソルバ 基礎",
    visibleOutcome: "スケッチに拘束を付与できる",
    adrRefs: ["ADR-004"],
  };

  test("タイトルに feat(phase11): プレフィックス", () => {
    const d = renderIssueDraft("幾何拘束 (Horizontal / Vertical) が動作", ctx);
    expect(d.title.startsWith("feat(phase11):")).toBe(true);
    // 括弧内は削除される
    expect(d.title).not.toContain("Horizontal");
  });

  test("body に In-Scope / Out-of-Scope が入っている (粒度 gate を通す)", () => {
    const d = renderIssueDraft("Extrude が動作", ctx);
    expect(d.body).toContain("In-Scope");
    expect(d.body).toContain("Out-of-Scope");
  });

  test("labels は 'type: feature' + batch:*", () => {
    const d = renderIssueDraft("Extrude が動作", ctx);
    expect(d.labels).toContain("type: feature");
    expect(d.labels.some(l => l.startsWith("batch:"))).toBe(true);
  });

  test("adrRefs があれば body に前提 ADR 行が付く", () => {
    const d = renderIssueDraft("Extrude が動作", ctx);
    expect(d.body).toContain("前提 ADR");
    expect(d.body).toContain("ADR-004");
  });
});

// --- expandDrafts ---

describe("expandDrafts", () => {
  const alwaysYes: Checker = () => ({ aligned: "yes" });

  const noSplitReturn: Checker = () => ({ aligned: "no", reason: "test" });

  test("全部 yes → そのまま passthrough", () => {
    const drafts: IssueDraft[] = [
      { title: "a", body: "b", labels: ["type: feature", "batch:kernel"], sourceCondition: "a" },
    ];
    const r = expandDrafts(drafts, alwaysYes);
    expect(r.finalDrafts.length).toBe(1);
    expect(r.needsHuman.length).toBe(0);
    expect(r.splitCount).toBe(0);
  });

  test("no + split_proposal あり → children 展開 (T03_seeder_split)", () => {
    const drafts: IssueDraft[] = [
      { title: "big", body: "b", labels: ["type: feature", "batch:kernel"], sourceCondition: "big feature" },
    ];
    // 親: no + 2 children を返す。子: 常に yes。
    let call = 0;
    const checker: Checker = () => {
      call += 1;
      if (call === 1) {
        return {
          aligned: "no",
          reason: "too big",
          split_proposal: [
            { title: "child-1", body: "c1", labels: ["type: feature", "batch:kernel"] },
            { title: "child-2", body: "c2", labels: ["type: feature", "batch:kernel"] },
          ],
        };
      }
      return { aligned: "yes" };
    };
    const r = expandDrafts(drafts, checker);
    expect(r.finalDrafts.length).toBe(2);
    expect(r.finalDrafts.map(d => d.title)).toEqual(["child-1", "child-2"]);
    expect(r.needsHuman.length).toBe(0);
    expect(r.splitCount).toBe(1);
  });

  test("no + split_proposal なし → needs-human", () => {
    const drafts: IssueDraft[] = [
      { title: "x", body: "b", labels: ["type: feature", "batch:kernel"], sourceCondition: "src" },
    ];
    const r = expandDrafts(drafts, noSplitReturn);
    expect(r.finalDrafts.length).toBe(0);
    expect(r.needsHuman.length).toBe(1);
    expect(r.needsHuman[0].sourceCondition).toBe("src");
  });

  test("children も no → needs-human (再帰なし = 1 回のみ split)", () => {
    let call = 0;
    const checker: Checker = () => {
      call += 1;
      if (call === 1) {
        return {
          aligned: "no",
          reason: "big",
          split_proposal: [
            { title: "still-big", body: "b", labels: ["type: feature", "batch:kernel"] },
          ],
        };
      }
      return { aligned: "no", reason: "still big" };
    };
    const drafts: IssueDraft[] = [
      { title: "parent", body: "b", labels: ["type: feature", "batch:kernel"], sourceCondition: "src" },
    ];
    const r = expandDrafts(drafts, checker);
    expect(r.finalDrafts.length).toBe(0);
    expect(r.needsHuman.length).toBe(1);
    expect(r.needsHuman[0].reason).toContain("still aligned:no");
    expect(r.splitCount).toBe(1);
  });
});

// --- runSeeder (with dependency injection) ---

describe("runSeeder", () => {
  let tmpDir: string;
  let roadmapPath: string;
  let logPath: string;

  beforeEach(() => {
    tmpDir = mkTempDir("phase-seeder-test");
    roadmapPath = join(tmpDir, "ROADMAP.md");
    logPath = join(tmpDir, "phase-seeder-log.jsonl");
  });

  afterEach(() => {
    if (existsSync(tmpDir)) rmSync(tmpDir, { recursive: true, force: true });
  });

  test("T01_seeder_dry_run: Phase 11 stub で >=3 drafts", async () => {
    writeFileSync(roadmapPath, PHASE_11_STUB, "utf-8");
    const r = await runSeeder(
      { phase: 11, dryRun: true, roadmapPath, logPath },
      {},
    );
    expect(r.exitCode).toBe(0);
    expect(r.plannedDrafts?.length ?? 0).toBeGreaterThanOrEqual(3);
    // dry-run はログを書かない
    expect(existsSync(logPath)).toBe(false);
  });

  test("T02_seeder_empty_phase: ROADMAP に Phase 99 なし → exit 2", async () => {
    writeFileSync(roadmapPath, PHASE_11_STUB, "utf-8");
    const r = await runSeeder(
      { phase: 99, dryRun: true, roadmapPath, logPath },
      {},
    );
    expect(r.exitCode).toBe(2);
    expect(r.message).toContain("Phase 99");
  });

  test("T_DEG_seeder_no_conditions: 完了条件ブロック空 → exit 0 + 'no seed'", async () => {
    writeFileSync(roadmapPath, PHASE_EMPTY_CONDITIONS_STUB, "utf-8");
    const r = await runSeeder(
      { phase: 42, dryRun: true, roadmapPath, logPath },
      {},
    );
    expect(r.exitCode).toBe(0);
    expect(r.message).toContain("no seed");
    expect(r.plannedDrafts).toEqual([]);
  });

  test("T_bonus_completed_phase: ✅ 付き Phase を seed 拒否 → exit 2", async () => {
    writeFileSync(roadmapPath, PHASE_COMPLETED_STUB, "utf-8");
    const r = await runSeeder(
      { phase: 4, dryRun: true, roadmapPath, logPath },
      {},
    );
    expect(r.exitCode).toBe(2);
    expect(r.message).toContain("already marked");
  });

  test("T03_seeder_split: 粒度違反で split_proposal 展開 → children 起票 (dry-run で確認)", async () => {
    writeFileSync(roadmapPath, PHASE_11_STUB, "utf-8");
    // 最初の 1 draft (幾何拘束) だけ no+split を返し、それ以外は yes
    let firstCall = true;
    const checker: Checker = (meta): CheckResult => {
      if (firstCall && meta.title.includes("幾何拘束")) {
        firstCall = false;
        return {
          aligned: "no",
          reason: "too big",
          split_proposal: [
            { title: "feat(phase11): Horizontal 拘束", body: "In-Scope\n- foo", labels: ["type: feature", "batch:kernel"] },
            { title: "feat(phase11): Vertical 拘束", body: "In-Scope\n- bar", labels: ["type: feature", "batch:kernel"] },
          ],
        };
      }
      return { aligned: "yes" };
    };
    const r = await runSeeder(
      { phase: 11, dryRun: true, roadmapPath, logPath },
      { checker },
    );
    expect(r.exitCode).toBe(0);
    expect(r.splitCount).toBe(1);
    // 幾何拘束 1 → 2 に、その他 3 は passthrough で計 5
    expect(r.plannedDrafts?.length).toBe(5);
    const titles = r.plannedDrafts!.map(d => d.title);
    expect(titles).toContain("feat(phase11): Horizontal 拘束");
    expect(titles).toContain("feat(phase11): Vertical 拘束");
  });

  test("dry-run でない → createIssue が全 draft ぶん呼ばれ、log 追記", async () => {
    writeFileSync(roadmapPath, PHASE_11_STUB, "utf-8");
    const calls: Array<{ title: string; milestone: string }> = [];
    let next = 1000;
    const createIssue = async (title: string, _body: string, _labels: string[], milestone: string) => {
      calls.push({ title, milestone });
      return { ok: true, number: next++ };
    };
    const logs: string[] = [];
    const writeLog = (_path: string, line: string) => { logs.push(line); };
    const r = await runSeeder(
      { phase: 11, dryRun: false, roadmapPath, logPath },
      { createIssue, writeLog, now: () => "2026-07-24T00:00:00Z" },
    );
    expect(r.exitCode).toBe(0);
    expect(r.createdIssues?.length).toBe(4);
    expect(calls.length).toBe(4);
    expect(calls[0].milestone).toBe("Phase 11: 拘束ソルバ 基礎");
    expect(logs.length).toBe(1);
    const logEntry = JSON.parse(logs[0]);
    expect(logEntry.phase).toBe(11);
    expect(logEntry.created_issues.length).toBe(4);
    expect(logEntry.timestamp).toBe("2026-07-24T00:00:00Z");
  });

  test("--milestone-title 明示指定でオーバライドされる", async () => {
    writeFileSync(roadmapPath, PHASE_11_STUB, "utf-8");
    const seen: string[] = [];
    const createIssue = async (_t: string, _b: string, _l: string[], ms: string) => {
      seen.push(ms);
      return { ok: true, number: 42 };
    };
    const r = await runSeeder(
      { phase: 11, dryRun: false, roadmapPath, logPath, milestoneTitle: "Phase 11: alt title" },
      { createIssue, writeLog: () => {} },
    );
    expect(r.exitCode).toBe(0);
    for (const ms of seen) expect(ms).toBe("Phase 11: alt title");
  });

  test("createIssue 失敗 → exit 1、それ以外の draft は続行", async () => {
    writeFileSync(roadmapPath, PHASE_11_STUB, "utf-8");
    let idx = 0;
    const createIssue = async (title: string) => {
      idx += 1;
      if (idx === 2) return { ok: false, error: "gh boom" };
      return { ok: true, number: 900 + idx };
    };
    const r = await runSeeder(
      { phase: 11, dryRun: false, roadmapPath, logPath },
      { createIssue, writeLog: () => {} },
    );
    expect(r.exitCode).toBe(1);
    expect(r.createdIssues?.length).toBe(3);
  });
});
