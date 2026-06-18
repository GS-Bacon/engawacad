// dispatch-codex-intent.ts: #235 split-detector parent skip ガード テスト
//
// T1: body に固定文字列マーカー → skip 経路 (Codex 未呼出 / aligned: skip / exit 0)
// T2: label `splittable` のみ → skip 経路
// T3: marker 無し + aligned:yes (pre-written) → 従来通り exit 0
// T4: marker 無し + aligned:no  (pre-written) → 従来通り exit 1
// T5: marker 有り + aligned:yes 同居 → skip 経路優先

import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  ALIGNED_SKIP_LINE,
  runIntentCheck,
  shouldSkipForSplitDetector,
  SPLIT_DETECTOR_MARKER,
  SPLIT_DETECTOR_LABEL,
} from "../dispatch-codex-intent.ts";

const TMP_BASE = join(tmpdir(), `dispatch-codex-intent-test-${process.pid}`);

function freshTmpRoot(label: string): string {
  const dir = join(TMP_BASE, label);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  return dir;
}

/** process.stdout.write を一時的に差し替えて書き込みをキャプチャ */
function captureStdout<T>(fn: () => Promise<T>): Promise<{ out: string; value: T }> {
  const chunks: string[] = [];
  const orig = process.stdout.write.bind(process.stdout);
  // @ts-expect-error monkey-patch for test capture
  process.stdout.write = (chunk: any) => {
    chunks.push(typeof chunk === "string" ? chunk : chunk.toString("utf-8"));
    return true;
  };
  return fn().finally(() => {
    process.stdout.write = orig;
  }).then((value) => ({ out: chunks.join(""), value }));
}

beforeEach(() => {
  delete process.env.CODEX_DRY_RUN;
});

afterEach(() => {
  delete process.env.CODEX_DRY_RUN;
});

describe("shouldSkipForSplitDetector (純関数)", () => {
  test("canonical 固定文字列 → true", () => {
    expect(shouldSkipForSplitDetector({ body: `xxx ${SPLIT_DETECTOR_MARKER} yyy`, labels: [] })).toBe(true);
  });
  test("label splittable のみ → true", () => {
    expect(shouldSkipForSplitDetector({ body: "plain body", labels: [SPLIT_DETECTOR_LABEL] })).toBe(true);
  });
  test("real-world variant: loop-split-detector で粒度に合うように分割される想定 → true (#195-#205 文言)", () => {
    expect(shouldSkipForSplitDetector({
      body: "Phase 10 全スコープの起点 Issue。loop-split-detector で粒度に合うように分割される想定。",
      labels: [],
    })).toBe(true);
  });
  test("real-world variant: 順序反転 (#194 文言) → true", () => {
    expect(shouldSkipForSplitDetector({
      body: "Codex review の split_proposal で粒度ガード (ADR-006 §1) に合うように分割される想定 (loop-split-detector が子 Issue を派生)。",
      labels: [],
    })).toBe(true);
  });
  test("両方なし → false", () => {
    expect(shouldSkipForSplitDetector({ body: "plain body", labels: ["bug"] })).toBe(false);
  });
  test("片方の単語のみ → false (両単語 AND 条件)", () => {
    expect(shouldSkipForSplitDetector({
      body: "loop-split-detector を将来使う予定だがこの Issue 自体は分割対象ではない",
      labels: [],
    })).toBe(false);
    expect(shouldSkipForSplitDetector({
      body: "この Issue は手動で分割される想定だが split-detector は使わない",
      labels: [],
    })).toBe(false);
  });
});

describe("runIntentCheck #235 split-detector skip ガード", () => {
  test("T1: --issue-draft で body に固定マーカーを含む → skip 経路 (aligned: skip / exit 0 / Codex 未呼出)", async () => {
    const dir = freshTmpRoot("t1");
    const draftFile = join(dir, "issue-draft.md");
    const resultFile = join(dir, "result.yaml");
    writeFileSync(
      draftFile,
      `# Phase 9 起点\n\n本 Issue は ${SPLIT_DETECTOR_MARKER}。\n`,
      "utf-8",
    );

    // Codex が呼ばれたら絶対に失敗する状態 (CODEX_DRY_RUN を立てない)
    // skip 経路で Codex 未呼出を保証するため
    const { out, value: code } = await captureStdout(() =>
      runIntentCheck({ issueDraftFile: draftFile, resultFile }),
    );

    expect(code).toBe(0);
    expect(out).toContain(ALIGNED_SKIP_LINE);
    expect(readFileSync(resultFile, "utf-8")).toContain(ALIGNED_SKIP_LINE);
  });

  test("T2: label splittable のみ → skip 経路 (fetchOverride でラベル擬似)", async () => {
    const dir = freshTmpRoot("t2");
    const resultFile = join(dir, "result.yaml");

    const { out, value: code } = await captureStdout(() =>
      runIntentCheck({
        issueNum: "194",
        resultFile,
        fetchOverride: async () => ({
          body: "# 通常の本文 (marker 無し)\n\nこの Issue は marker 無しだが label で skip 対象",
          labels: [SPLIT_DETECTOR_LABEL],
        }),
      }),
    );

    expect(code).toBe(0);
    expect(out).toContain(ALIGNED_SKIP_LINE);
    expect(readFileSync(resultFile, "utf-8")).toContain(ALIGNED_SKIP_LINE);
  });

  test("T3: marker 無し + pre-written aligned:yes → 従来通り exit 0", async () => {
    const dir = freshTmpRoot("t3");
    const draftFile = join(dir, "draft.md");
    const resultFile = join(dir, "result.yaml");
    writeFileSync(draftFile, "# 通常の Issue\n\n意図明確、スコープ明確", "utf-8");
    // DRY_RUN は resultFile を消さないので pre-write が残る
    writeFileSync(resultFile, "aligned: yes\nreason: scope clear\n", "utf-8");

    process.env.CODEX_DRY_RUN = "1";
    const code = await runIntentCheck({
      issueDraftFile: draftFile,
      resultFile,
      roadmapFile: join(dir, "nonexistent-roadmap.md"),
    });
    expect(code).toBe(0);
  });

  test("T4: marker 無し + pre-written aligned:no → 従来通り exit 1 (regression なし)", async () => {
    const dir = freshTmpRoot("t4");
    const draftFile = join(dir, "draft.md");
    const resultFile = join(dir, "result.yaml");
    writeFileSync(draftFile, "# 曖昧な Issue", "utf-8");
    writeFileSync(resultFile, "aligned: no\nreason: scope unclear\n", "utf-8");

    process.env.CODEX_DRY_RUN = "1";
    const code = await runIntentCheck({
      issueDraftFile: draftFile,
      resultFile,
      roadmapFile: join(dir, "nonexistent-roadmap.md"),
    });
    expect(code).toBe(1);
  });

  test("T5: marker 有り + 仮の aligned:yes 同居 → skip 経路優先 (Codex 未呼出)", async () => {
    const dir = freshTmpRoot("t5");
    const draftFile = join(dir, "draft.md");
    const resultFile = join(dir, "result.yaml");
    writeFileSync(
      draftFile,
      `# Phase 起点\n\n本 Issue は ${SPLIT_DETECTOR_MARKER} 想定です。`,
      "utf-8",
    );
    // 万一 Codex 経路を辿った場合に紛れ込みうる aligned: yes — skip 経路ならこれが上書きされる
    writeFileSync(resultFile, "aligned: yes\n# legacy result\n", "utf-8");

    // CODEX_DRY_RUN を立てない: skip 経路なら Codex に到達しないので問題なし、
    // 仮に skip 経路を通らず Codex が起動すれば即座に失敗 (テスト環境に codex 無し)
    const { out, value: code } = await captureStdout(() =>
      runIntentCheck({ issueDraftFile: draftFile, resultFile }),
    );

    expect(code).toBe(0);
    expect(out).toContain(ALIGNED_SKIP_LINE);
    // skip 経路が aligned: yes を上書きしたことを確認
    const content = readFileSync(resultFile, "utf-8");
    expect(content).toContain(ALIGNED_SKIP_LINE);
    expect(content).not.toContain("legacy result");
  });
});
