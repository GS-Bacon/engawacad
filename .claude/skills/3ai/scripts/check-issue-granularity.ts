#!/usr/bin/env bun
// check-issue-granularity.ts — Issue 起票時の粒度 gate (Claude 内製、Codex 呼びなし)
// ADR-006 §1 の粒度チェックリストを決定的ルールで実装する。
//
// 出力 yaml フォーマットは dispatch-codex-intent.ts のそれと互換:
//   aligned: yes / no / skip
//   reason: |
//     ...
//   split_proposal:
//     - title: "..."
//       body: |
//         ...
//       labels: ["type: feature", "batch:kernel"]
//
// 使い方:
//   bun check-issue-granularity.ts (--issue-draft <file> | --issue N) --result <file> [--roadmap ROADMAP.md]

import { existsSync, readFileSync, writeFileSync } from "fs";

// split-detector 想定 Issue の skip マーカー (dispatch-codex-intent.ts から継承)
export const SPLIT_DETECTOR_MARKER = "loop-split-detector で分割される想定";
export const SPLIT_DETECTOR_TERMS = ["loop-split-detector", "分割される想定"] as const;
export const SPLIT_DETECTOR_LABEL = "splittable";
export const ALIGNED_SKIP_LINE = "aligned: skip (split-detector parent)";

export interface IssueMeta {
  title: string;
  body: string;
  labels: string[];
}

export interface CheckOpts {
  issueDraftFile?: string;
  issueNum?: string;
  resultFile: string;
  fetchOverride?: (issueNum: string) => Promise<IssueMeta>;
}

export interface SplitChild {
  title: string;
  body: string;
  labels: string[];
}

export interface CheckResult {
  aligned: "yes" | "no" | "skip";
  reason?: string;
  split_proposal?: SplitChild[];
}

async function fetchIssueMeta(issueNum: string): Promise<IssueMeta> {
  const proc = Bun.spawn(
    ["gh", "issue", "view", issueNum, "--json", "title,body,labels"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const stdoutText = (await new Response(proc.stdout).text()).trim();
  await proc.exited;
  if (!stdoutText) {
    throw new Error(`Issue #${issueNum} の本文を取得できませんでした`);
  }
  const json = JSON.parse(stdoutText) as {
    title?: string;
    body?: string;
    labels?: Array<{ name: string }>;
  };
  return {
    title: json.title ?? "",
    body: json.body ?? "",
    labels: (json.labels ?? []).map((l) => l.name),
  };
}

export function shouldSkipForSplitDetector(meta: IssueMeta): boolean {
  if (meta.labels.includes(SPLIT_DETECTOR_LABEL)) return true;
  if (meta.body.includes(SPLIT_DETECTOR_MARKER)) return true;
  if (SPLIT_DETECTOR_TERMS.every((t) => meta.body.includes(t))) return true;
  return false;
}

/**
 * タイトルから機能名候補を抽出する。
 * 対象パターン: "A + B + C", "A / B / C", "A、B、C", "A and B and C"
 * 括弧内の英語表記 alias "(Foo / Bar / Baz)" は除去してから split する。
 * "feat(phaseN): ..." 等の conventional-commit 風プレフィックスは除去する。
 */
export function extractFeatureNames(title: string): string[] {
  // 1. conventional-commit プレフィックス除去
  const withoutPrefix = title.replace(
    /^(feat|fix|chore|docs|refactor|test|perf|style)(\([^)]*\))?:\s*/i,
    "",
  );
  // 2. 括弧 (半角/全角) の中身を除去 (英語 alias は単独カウントしない)
  //    （ = '(', ） = ')' (全角括弧)
  const stripped = withoutPrefix
    .replace(/\([^)]*\)/g, "")
    .replace(/（[^）]*）/g, "")
    .trim();
  // 3. 主要な区切り: + / ／ 、 & と " and "
  //    「,」は装飾用途の可能性もあるが機能列挙で使われる場合も多いので採用
  const parts = stripped
    .split(/\s*(?:\+|\/|／|&|,|、| and )\s*/i)
    .map((p) => p.trim())
    .filter(Boolean);
  return parts;
}

/**
 * ラベル軸チェック: type: feature / type: refactor / type: foundation / bug / docs のいずれかが必須
 */
export function hasTypeLabel(labels: string[]): boolean {
  return labels.some(
    (l) =>
      /^type:\s*(feature|refactor|foundation)$/i.test(l) ||
      l === "bug" ||
      l === "docs",
  );
}

/**
 * In-Scope セクションが本文にあるか
 * 見出しレベル (##) は問わない。"In-Scope" or "In-scope" or "in scope" を許容
 */
export function hasInScopeSection(body: string): boolean {
  return /in[-\s]?scope/i.test(body);
}

/**
 * ADR-006 §1 の粒度チェック本体
 */
export function checkGranularity(meta: IssueMeta): CheckResult {
  // Rule 0: skip 経路 (split-detector 想定の親 Issue)
  if (shouldSkipForSplitDetector(meta)) {
    return { aligned: "skip" };
  }

  // Rule 1: type 軸ラベル欠如
  if (!hasTypeLabel(meta.labels)) {
    return {
      aligned: "no",
      reason:
        "type 軸ラベル (type: feature / type: refactor / type: foundation / bug / docs のいずれか) が付いていない。ADR-006 §1 粒度チェックリスト違反。",
    };
  }

  // Rule 2: enhancement ラベル使用禁止
  if (meta.labels.includes("enhancement")) {
    return {
      aligned: "no",
      reason:
        "enhancement ラベルは ADR-002 type 軸の正規ラベルでない。機能拡張は type: foundation を使うこと。",
    };
  }

  // Rule 3: In-Scope 欠如 (draft ファイルの場合は body 全体、Issue の場合は本文)
  if (!hasInScopeSection(meta.body)) {
    return {
      aligned: "no",
      reason:
        "Issue 本文に In-Scope / Out-of-Scope 表がない。ADR-006 §1 粒度チェックリスト・plan.md 必須セクション違反。",
    };
  }

  // Rule 4: 粒度違反 (タイトル内で機能列挙 3 個以上)
  const features = extractFeatureNames(meta.title);
  if (features.length >= 3) {
    const inheritLabels = meta.labels.filter(
      (l) =>
        /^type:\s*(feature|refactor|foundation)$/i.test(l) ||
        /^batch:/.test(l) ||
        l === "bug" ||
        l === "docs",
    );
    const split_proposal: SplitChild[] = features.map((f) => ({
      title: `${f} (single-feature split)`,
      body: `分割元: #<親番号>\n\n## In-Scope\n- ${f} の単独実装\n\n## Out-of-Scope\n- 他の機能 (別 Issue で扱う)\n\n## 完了条件\n- cargo test が pass\n- 退化/境界ケースの boundary test を含む`,
      labels: inheritLabels,
    }));
    return {
      aligned: "no",
      reason: `タイトル内で ${features.length} 個の機能が列挙されており粒度過大 (ADR-006 §1 "1 軸 × 1-2 op" 違反)。機能ごとに分割する。`,
      split_proposal,
    };
  }

  return { aligned: "yes" };
}

/**
 * CheckResult を yaml として resultFile に書き出す。
 * 出力形式は dispatch-codex-intent.ts の結果 yaml と互換で、
 * L-5.8 の grep '^aligned:' / grep '^split_proposal:' がそのまま消化できる。
 */
export function writeResult(resultFile: string, result: CheckResult): void {
  const lines: string[] = [];
  if (result.aligned === "skip") {
    lines.push(ALIGNED_SKIP_LINE);
  } else {
    lines.push(`aligned: ${result.aligned}`);
    if (result.reason) {
      lines.push("reason: |");
      for (const l of result.reason.split("\n")) {
        lines.push(`  ${l}`);
      }
    }
    if (result.split_proposal && result.split_proposal.length > 0) {
      lines.push("split_proposal:");
      for (const child of result.split_proposal) {
        lines.push(`  - title: "${child.title.replace(/"/g, '\\"')}"`);
        lines.push(`    body: |`);
        for (const l of child.body.split("\n")) {
          lines.push(`      ${l}`);
        }
        const labelsInline = child.labels.map((l) => `"${l}"`).join(", ");
        lines.push(`    labels: [${labelsInline}]`);
      }
    }
  }
  writeFileSync(resultFile, lines.join("\n") + "\n", "utf-8");
}

export async function runCheck(opts: CheckOpts): Promise<number> {
  const { issueDraftFile, issueNum, resultFile, fetchOverride } = opts;

  if ((!issueDraftFile && !issueNum) || !resultFile) {
    throw new Error("issueDraftFile/issueNum のいずれかと resultFile が必須です");
  }
  if (issueDraftFile && issueNum) {
    throw new Error("issueDraftFile と issueNum は排他的です");
  }

  let meta: IssueMeta;
  if (issueNum) {
    meta = await (fetchOverride ? fetchOverride(issueNum) : fetchIssueMeta(issueNum));
  } else {
    if (!existsSync(issueDraftFile!)) {
      throw new Error(`Issue draft not found: ${issueDraftFile}`);
    }
    const content = readFileSync(issueDraftFile!, "utf-8");
    // draft の 1 行目が "# Title" 形式なら title を抽出
    const firstLine = content.split("\n")[0];
    const title = firstLine.startsWith("# ") ? firstLine.slice(2).trim() : "";
    meta = { title, body: content, labels: [] };
  }

  const src = issueNum ? `#${issueNum}` : issueDraftFile;
  process.stderr.write(`=== check-issue-granularity: ${src} ===\n`);

  const result = checkGranularity(meta);
  writeResult(resultFile, result);

  if (result.aligned === "skip") {
    process.stdout.write(`${ALIGNED_SKIP_LINE}\n`);
    process.stderr.write(`\n✅ granularity SKIP: split-detector parent\n`);
    return 0;
  } else if (result.aligned === "yes") {
    process.stderr.write(`\n✅ granularity PASSED: aligned=yes\n`);
    return 0;
  } else {
    process.stderr.write(
      `\n❌ granularity FAILED: aligned=no\n  reason: ${result.reason ?? ""}\n`,
    );
    if (result.split_proposal) {
      process.stderr.write(
        `  split_proposal: ${result.split_proposal.length} children\n`,
      );
    }
    return 1;
  }
}

async function main() {
  const args = process.argv.slice(2);
  let issueDraftFile = "";
  let issueNum = "";
  let resultFile = "";

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--issue-draft":
        issueDraftFile = args[++i];
        break;
      case "--issue":
        issueNum = args[++i];
        break;
      case "--result":
        resultFile = args[++i];
        break;
      case "--roadmap":
        // 互換のため受け入れるが未使用 (Codex 版は ROADMAP コンテキストを注入していた)
        args[++i];
        break;
      default:
        console.error(`Unknown arg: ${args[i]}`);
        process.exit(1);
    }
  }

  if ((!issueDraftFile && !issueNum) || !resultFile) {
    console.error(
      "Usage: check-issue-granularity.ts (--issue-draft <file> | --issue N) --result <file>",
    );
    process.exit(1);
  }
  if (issueDraftFile && issueNum) {
    console.error("--issue-draft と --issue は排他的です");
    process.exit(1);
  }

  const code = await runCheck({
    issueDraftFile: issueDraftFile || undefined,
    issueNum: issueNum || undefined,
    resultFile,
  });
  process.exit(code);
}

if (import.meta.main) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
