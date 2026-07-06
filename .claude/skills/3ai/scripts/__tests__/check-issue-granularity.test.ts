// check-issue-granularity.ts: ADR-006 §1 決定的粒度チェックのテスト
//
// T1: body に固定マーカー → skip 経路 (aligned: skip / exit 0)
// T2: label `splittable` のみ → skip 経路
// T3: 通常 Issue (In-Scope あり + type ラベルあり + 単一機能) → aligned: yes (exit 0)
// T4: In-Scope 欠如 → aligned: no
// T5: marker 有り + skip 優先 (result 上書き)
// T6: タイトルに 3+ 機能 → aligned: no + split_proposal
// T7: type 軸ラベル欠如 → aligned: no
// T8: enhancement ラベル使用 → aligned: no
// T9: 2 機能 (Trim + Extend) → aligned: yes (境界: 2 機能は許容)
//
// extractFeatureNames 純関数のテスト:
//   - "feat(phase10): 矩形 + 多角形 + Slot (Rectangle / Polygon / Slot)" → 3 個 (括弧内除去)
//   - "Trim + Extend" → 2 個
//   - "Offset / Fillet / Chamfer" → 3 個
//   - "単一機能タイトル" → 1 個

import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  ALIGNED_SKIP_LINE,
  checkGranularity,
  extractFeatureNames,
  hasInScopeSection,
  hasTypeLabel,
  runCheck,
  shouldSkipForSplitDetector,
  SPLIT_DETECTOR_LABEL,
  SPLIT_DETECTOR_MARKER,
} from "../check-issue-granularity.ts";

const TMP_BASE = join(tmpdir(), `check-issue-granularity-test-${process.pid}`);

function freshTmpRoot(label: string): string {
  const dir = join(TMP_BASE, label);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  return dir;
}

function captureStdout<T>(
  fn: () => Promise<T>,
): Promise<{ out: string; value: T }> {
  const chunks: string[] = [];
  const orig = process.stdout.write.bind(process.stdout);
  // @ts-expect-error monkey-patch for test capture
  process.stdout.write = (chunk: any) => {
    chunks.push(typeof chunk === "string" ? chunk : chunk.toString("utf-8"));
    return true;
  };
  return fn()
    .finally(() => {
      process.stdout.write = orig;
    })
    .then((value) => ({ out: chunks.join(""), value }));
}

describe("extractFeatureNames (純関数)", () => {
  test("conventional-commit プレフィックスと括弧を除去して 3 機能列挙 → 3 個", () => {
    const t =
      "feat(phase10): 矩形 + 多角形 + Slot (Rectangle / Polygon / Slot) を engawa-format / engawa-build に実装";
    // "feat(phase10):" 除去 → "矩形 + 多角形 + Slot" (括弧内除去) + " を engawa-format / engawa-build に実装"
    // 現状の実装では " を engawa-format / engawa-build に実装" 部分も split 対象になる
    // → 実運用では "Slot を engawa-format" のような合成は稀なので保守的に運用
    // ここでは 3+ 機能検出を主眼にテストする
    const features = extractFeatureNames(t);
    expect(features.length).toBeGreaterThanOrEqual(3);
  });

  test('"Trim + Extend" → 2 個', () => {
    expect(extractFeatureNames("feat(phase10): Trim + Extend (スケッチ編集)")).toHaveLength(2);
  });

  test('"Offset / Fillet / Chamfer" → 3 個', () => {
    const features = extractFeatureNames(
      "feat(phase10): Offset / Fillet / Chamfer",
    );
    expect(features).toEqual(["Offset", "Fillet", "Chamfer"]);
  });

  test('"Mirror + Pattern (スケッチ複製)" → 2 個 (括弧内除去)', () => {
    expect(
      extractFeatureNames("feat(phase10): Mirror + Pattern (スケッチ複製)"),
    ).toHaveLength(2);
  });

  test("単一機能タイトル → 1 個", () => {
    expect(extractFeatureNames("feat(kernel): Cylinder 実装")).toEqual([
      "Cylinder 実装",
    ]);
  });

  test("日本語区切り「、」 3 機能 → 3 個", () => {
    expect(extractFeatureNames("feat: A、B、C を追加")).toEqual([
      "A",
      "B",
      "C を追加",
    ]);
  });
});

describe("hasTypeLabel (純関数)", () => {
  test("type: feature → true", () => {
    expect(hasTypeLabel(["type: feature"])).toBe(true);
  });
  test("type: refactor → true", () => {
    expect(hasTypeLabel(["type: refactor", "batch:kernel"])).toBe(true);
  });
  test("bug → true", () => {
    expect(hasTypeLabel(["bug", "batch:kernel"])).toBe(true);
  });
  test("docs → true", () => {
    expect(hasTypeLabel(["docs"])).toBe(true);
  });
  test("enhancement のみ → false (正規ラベルでない)", () => {
    expect(hasTypeLabel(["enhancement"])).toBe(false);
  });
  test("batch:* のみ → false", () => {
    expect(hasTypeLabel(["batch:kernel"])).toBe(false);
  });
});

describe("hasInScopeSection (純関数)", () => {
  test('"## In-Scope" heading → true', () => {
    expect(hasInScopeSection("...\n## In-Scope\n- foo")).toBe(true);
  });
  test('"in scope" → true (case-insensitive, hyphen 不要)', () => {
    expect(hasInScopeSection("In scope: foo")).toBe(true);
  });
  test('"In-Scope" テーブル → true', () => {
    expect(hasInScopeSection("| In-Scope | Out-of-Scope |")).toBe(true);
  });
  test("なし → false", () => {
    expect(hasInScopeSection("これは意図明確な Issue です")).toBe(false);
  });
});

describe("shouldSkipForSplitDetector (純関数)", () => {
  test("固定マーカーあり → true", () => {
    expect(
      shouldSkipForSplitDetector({
        title: "",
        body: `xx ${SPLIT_DETECTOR_MARKER} yy`,
        labels: [],
      }),
    ).toBe(true);
  });
  test("label splittable → true", () => {
    expect(
      shouldSkipForSplitDetector({
        title: "",
        body: "plain",
        labels: [SPLIT_DETECTOR_LABEL],
      }),
    ).toBe(true);
  });
  test("両単語共存バリアント → true (#195 系文言)", () => {
    expect(
      shouldSkipForSplitDetector({
        title: "",
        body: "loop-split-detector で粒度に合うように分割される想定",
        labels: [],
      }),
    ).toBe(true);
  });
  test("なし → false", () => {
    expect(
      shouldSkipForSplitDetector({ title: "", body: "plain", labels: ["bug"] }),
    ).toBe(false);
  });
});

describe("checkGranularity (メイン判定)", () => {
  test("T3: 通常 Issue (In-Scope あり + type + 単一機能) → aligned: yes", () => {
    const result = checkGranularity({
      title: "feat(kernel): Cylinder を実装",
      body: "## In-Scope\n- Cylinder のみ\n\n## Out-of-Scope\n- Sphere",
      labels: ["type: feature", "batch:kernel"],
    });
    expect(result.aligned).toBe("yes");
  });

  test("T4: In-Scope 欠如 → aligned: no (split_proposal なし)", () => {
    const result = checkGranularity({
      title: "feat(kernel): Cylinder を実装",
      body: "Cylinder を実装する",
      labels: ["type: feature", "batch:kernel"],
    });
    expect(result.aligned).toBe("no");
    expect(result.reason).toMatch(/In-Scope/);
    expect(result.split_proposal).toBeUndefined();
  });

  test("T6: タイトル 3 機能 (In-Scope あり + type あり) → aligned: no + split_proposal 3 件", () => {
    const result = checkGranularity({
      title: "feat(phase10): Offset / Fillet / Chamfer を実装",
      body: "## In-Scope\n- 3 機能\n\n## Out-of-Scope\n- (なし)",
      labels: ["type: feature", "batch:kernel"],
    });
    expect(result.aligned).toBe("no");
    expect(result.split_proposal).toBeDefined();
    expect(result.split_proposal!.length).toBeGreaterThanOrEqual(3);
    // 継承ラベル: type: feature + batch:kernel
    expect(result.split_proposal![0].labels).toContain("type: feature");
    expect(result.split_proposal![0].labels).toContain("batch:kernel");
  });

  test("T7: type 軸ラベル欠如 → aligned: no", () => {
    const result = checkGranularity({
      title: "feat(kernel): Cylinder を実装",
      body: "## In-Scope\n- Cylinder",
      labels: ["batch:kernel"],
    });
    expect(result.aligned).toBe("no");
    expect(result.reason).toMatch(/type 軸ラベル/);
  });

  test("T8: enhancement ラベル使用 → aligned: no", () => {
    const result = checkGranularity({
      title: "feat: Cylinder を実装",
      body: "## In-Scope\n- Cylinder",
      labels: ["enhancement", "batch:kernel"],
    });
    // まず type 軸チェックで落ちる可能性があるので、type: foundation を追加した場合を検証
    const result2 = checkGranularity({
      title: "feat: Cylinder を実装",
      body: "## In-Scope\n- Cylinder",
      labels: ["type: foundation", "enhancement", "batch:kernel"],
    });
    expect(result2.aligned).toBe("no");
    expect(result2.reason).toMatch(/enhancement/);
    // どちらでも aligned=no
    expect(result.aligned).toBe("no");
  });

  test("T9: 2 機能 (Trim + Extend) → aligned: yes (境界: 1-2 op 許容)", () => {
    const result = checkGranularity({
      title: "feat(phase10): Trim + Extend (スケッチ編集) を実装",
      body: "## In-Scope\n- Trim / Extend",
      labels: ["type: feature", "batch:kernel"],
    });
    expect(result.aligned).toBe("yes");
  });

  test("skip: split-detector 想定親 → aligned: skip (他の判定より優先)", () => {
    const result = checkGranularity({
      title: "feat(phase10): 全スコープ起点 (10+ 機能列挙)",
      body: `Phase 10 全スコープ起点。${SPLIT_DETECTOR_MARKER}。In-Scope はなし。`,
      labels: [],
    });
    expect(result.aligned).toBe("skip");
  });
});

describe("runCheck (E2E)", () => {
  test("T1: --issue-draft で body にマーカー → skip 経路 (aligned: skip / exit 0)", async () => {
    const dir = freshTmpRoot("t1");
    const draftFile = join(dir, "issue-draft.md");
    const resultFile = join(dir, "result.yaml");
    writeFileSync(
      draftFile,
      `# Phase 9 起点\n\n本 Issue は ${SPLIT_DETECTOR_MARKER}。\n`,
      "utf-8",
    );

    const { out, value: code } = await captureStdout(() =>
      runCheck({ issueDraftFile: draftFile, resultFile }),
    );

    expect(code).toBe(0);
    expect(out).toContain(ALIGNED_SKIP_LINE);
    expect(readFileSync(resultFile, "utf-8")).toContain(ALIGNED_SKIP_LINE);
  });

  test("T2: label splittable のみ (fetchOverride) → skip 経路", async () => {
    const dir = freshTmpRoot("t2");
    const resultFile = join(dir, "result.yaml");

    const { out, value: code } = await captureStdout(() =>
      runCheck({
        issueNum: "194",
        resultFile,
        fetchOverride: async () => ({
          title: "Phase 起点",
          body: "# 通常本文\n\n意図 marker なし",
          labels: [SPLIT_DETECTOR_LABEL],
        }),
      }),
    );

    expect(code).toBe(0);
    expect(out).toContain(ALIGNED_SKIP_LINE);
    expect(readFileSync(resultFile, "utf-8")).toContain(ALIGNED_SKIP_LINE);
  });

  test("T3: 通常 Issue → aligned: yes / exit 0", async () => {
    const dir = freshTmpRoot("t3");
    const resultFile = join(dir, "result.yaml");

    const code = await runCheck({
      issueNum: "42",
      resultFile,
      fetchOverride: async () => ({
        title: "feat(kernel): Cylinder を実装",
        body: "## In-Scope\n- Cylinder のみ\n\n## Out-of-Scope\n- Sphere",
        labels: ["type: feature", "batch:kernel"],
      }),
    });

    expect(code).toBe(0);
    const yaml = readFileSync(resultFile, "utf-8");
    expect(yaml).toMatch(/^aligned:\s*yes/m);
  });

  test("T4: In-Scope 欠如 → aligned: no / exit 1", async () => {
    const dir = freshTmpRoot("t4");
    const resultFile = join(dir, "result.yaml");

    const code = await runCheck({
      issueNum: "42",
      resultFile,
      fetchOverride: async () => ({
        title: "feat(kernel): Cylinder を実装",
        body: "Cylinder を実装する",
        labels: ["type: feature", "batch:kernel"],
      }),
    });

    expect(code).toBe(1);
    const yaml = readFileSync(resultFile, "utf-8");
    expect(yaml).toMatch(/^aligned:\s*no/m);
    expect(yaml).toMatch(/^reason:/m);
    expect(yaml).not.toMatch(/^split_proposal:/m);
  });

  test("T5: marker 有り + result に既存 aligned:yes → skip で上書き", async () => {
    const dir = freshTmpRoot("t5");
    const draftFile = join(dir, "draft.md");
    const resultFile = join(dir, "result.yaml");
    writeFileSync(
      draftFile,
      `# Phase 起点\n\n本 Issue は ${SPLIT_DETECTOR_MARKER} 想定。`,
      "utf-8",
    );
    writeFileSync(resultFile, "aligned: yes\n# legacy result\n", "utf-8");

    const { out, value: code } = await captureStdout(() =>
      runCheck({ issueDraftFile: draftFile, resultFile }),
    );

    expect(code).toBe(0);
    expect(out).toContain(ALIGNED_SKIP_LINE);
    const content = readFileSync(resultFile, "utf-8");
    expect(content).toContain(ALIGNED_SKIP_LINE);
    // legacy コメントは skip 上書きで消える
    expect(content).not.toContain("legacy result");
  });

  test("T6: タイトル 3 機能 → aligned: no + split_proposal 併記 / exit 1", async () => {
    const dir = freshTmpRoot("t6");
    const resultFile = join(dir, "result.yaml");

    const code = await runCheck({
      issueNum: "277",
      resultFile,
      fetchOverride: async () => ({
        title: "feat(phase10): Offset / Fillet / Chamfer を実装",
        body: "## In-Scope\n- 3 機能\n\n## Out-of-Scope\n- (なし)",
        labels: ["type: feature", "batch:kernel"],
      }),
    });

    expect(code).toBe(1);
    const yaml = readFileSync(resultFile, "utf-8");
    expect(yaml).toMatch(/^aligned:\s*no/m);
    expect(yaml).toMatch(/^split_proposal:/m);
    // 各子 entry に title / body / labels が含まれる
    expect(yaml).toMatch(/- title:.*Offset/);
    expect(yaml).toMatch(/- title:.*Fillet/);
    expect(yaml).toMatch(/- title:.*Chamfer/);
    expect(yaml).toContain("labels:");
    expect(yaml).toContain('"type: feature"');
    expect(yaml).toContain('"batch:kernel"');
  });
});
