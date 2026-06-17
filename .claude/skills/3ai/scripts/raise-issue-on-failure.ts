#!/usr/bin/env bun
// raise-issue-on-failure.ts — 3ai フロー中のトラブルを GitHub Issue として自動起票する
//
// Usage: bun raise-issue-on-failure.ts \
//   --step <STEP名> \
//   --feature-dir <features/N-slug> \
//   --error-summary <エラー概要テキスト> \
//   [--result-file <dispatch result JSON>] \
//   [--dry-run]
//
// 既に同じ STEP+feature の Issue が open なら二重起票しない
// exit 0: Issue 起票成功 or dry-run
// exit 1: 起票失敗

import { readFileSync, writeFileSync, existsSync } from "fs";
import type { StateData } from "./types.ts";
import { extractCiFailureContext } from "./extract-ci-failure-context.ts";

export function shouldSkipStep(step: string): { skip: boolean; reason: string } {
  if (/intent[-_ ]?check/i.test(step)) {
    return {
      skip: true,
      reason: "intent-check 系は ADR-006 粒度違反シグナル — bug 起票対象外",
    };
  }
  return { skip: false, reason: "" };
}

if (import.meta.main) {
  await main();
}

async function main() {
const args = process.argv.slice(2);
let step = "";
let featureDir = "";
let errorSummary = "";
let resultFile = "";
let dryRun = false;

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--step") step = args[++i];
  else if (args[i] === "--feature-dir") featureDir = args[++i];
  else if (args[i] === "--error-summary") errorSummary = args[++i];
  else if (args[i] === "--result-file") resultFile = args[++i];
  else if (args[i] === "--dry-run") dryRun = true;
}

if (!step || !featureDir || !errorSummary) {
  process.stderr.write("Usage: raise-issue-on-failure.ts --step <step> --feature-dir <dir> --error-summary <text> [--result-file <json>] [--dry-run]\n");
  process.exit(2);
}

// intent-check 系 (人間判断シグナル) は自動起票対象外 (#211)
{
  const skip = shouldSkipStep(step);
  if (skip.skip) {
    process.stderr.write(
      `SKIP: step="${step}" は自動起票対象外 (${skip.reason}). ` +
        `人間通知には loop-notify.ts / loop-intent-guard.ts を使ってください。\n`,
    );
    process.exit(0);
  }
}

// featureDir から issue 番号・slug を取得
const dirMatch = featureDir.match(/features\/(\d+)-(.+)$/);
const issueNum = dirMatch ? dirMatch[1] : "unknown";
const slug = dirMatch ? dirMatch[2] : featureDir;

// state.json を読む
let stateSummary = "";
const stateFile = `${featureDir}/state.json`;
if (existsSync(stateFile)) {
  try {
    const state = JSON.parse(readFileSync(stateFile, "utf-8"));
    stateSummary = JSON.stringify(state, null, 2);
  } catch {}
}

// result file の内容（あれば）
let resultDetail = "";
if (resultFile && existsSync(resultFile)) {
  try {
    const r = JSON.parse(readFileSync(resultFile, "utf-8"));
    resultDetail = `\n\n## dispatch result\n\`\`\`json\n${JSON.stringify(r, null, 2)}\n\`\`\``;
  } catch {}
}

// ci.log の失敗周辺抜粋 (#150 — 自動起票 Issue 受領側が真因を読みやすくする)
let ciExcerpt = "";
const ciLogPath = `${featureDir}/ci.log`;
if (existsSync(ciLogPath)) {
  try {
    const ciLog = readFileSync(ciLogPath, "utf-8");
    const excerpt = extractCiFailureContext(ciLog, 40);
    if (excerpt) {
      ciExcerpt = `\n\n## ci.log 失敗周辺 (抜粋)\n\`\`\`\n${excerpt}\n\`\`\``;
    }
  } catch {}
}

const title = `fix(3ai): [自動起票] ${step} でエラー — Issue #${issueNum} (${slug})`;
const body = `## 発生ステップ
${step}

## 対象 feature
- Issue: #${issueNum}
- slug: ${slug}
- feature-dir: \`${featureDir}\`

## エラー概要
${errorSummary}
${ciExcerpt}

## state.json
\`\`\`json
${stateSummary || "(取得不可)"}
\`\`\`
${resultDetail}

---
*このIssueは \`raise-issue-on-failure.ts\` により自動起票されました。*`;

// 元 Issue から batch:* ラベルを継承する処理を dry-run 前に実施し、表示に含める
async function lookupInheritedBatch(): Promise<string | null> {
  if (issueNum === "unknown") return null;
  try {
    const labelProc = Bun.spawn(
      ["gh", "issue", "view", issueNum, "--json", "labels", "-q", ".labels[].name"],
      { stdout: "pipe", stderr: "pipe" }
    );
    const labelOut = await new Response(labelProc.stdout).text();
    await labelProc.exited;
    const batchLabels = labelOut.split("\n").map((l) => l.trim()).filter((l) => l.startsWith("batch:"));
    return batchLabels[0] ?? null;
  } catch {
    return null;
  }
}

const preInheritedBatch = await lookupInheritedBatch();
const preLabels = preInheritedBatch ? `bug,${preInheritedBatch}` : "bug";

if (dryRun) {
  process.stdout.write(
    `[dry-run] Would create issue:\n  title: ${title}\n  labels: ${preLabels}\n`,
  );
  if (!preInheritedBatch && issueNum !== "unknown") {
    process.stderr.write(
      `WARN: 元 Issue #${issueNum} から batch:* を継承できませんでした (auto 選定ラダーから漏れます)\n`,
    );
  }
  process.exit(0);
}

// 重複チェック: 同タイトルの open issue があれば skip
const searchProc = Bun.spawn(
  ["gh", "issue", "list", "--state", "open", "--search", `"${step}" "${issueNum}"`, "--json", "title,number", "--limit", "5"],
  { stdout: "pipe", stderr: "pipe" }
);
const searchOut = await new Response(searchProc.stdout).text();
await searchProc.exited;

try {
  const existing = JSON.parse(searchOut) as Array<{ title: string; number: number }>;
  const dup = existing.find((i) => i.title.includes(issueNum) && i.title.includes(step));
  if (dup) {
    process.stdout.write(`SKIP: 既に同じトラブルの Issue #${dup.number} が open です\n`);
    process.exit(0);
  }
} catch {}

// 元 Issue から継承した batch:* と合わせて gh 起票時のラベルを構築 (dry-run 前と同じ判定を再利用)
const labels = preLabels;
if (!preInheritedBatch && issueNum !== "unknown") {
  process.stderr.write(
    `WARN: 元 Issue #${issueNum} から batch:* を継承できませんでした (auto 選定ラダーから漏れます)\n`,
  );
}

// 起票
const createProc = Bun.spawn(
  ["gh", "issue", "create", "--title", title, "--body", body, "--label", labels],
  { stdout: "pipe", stderr: "pipe" }
);
const createOut = await new Response(createProc.stdout).text();
const createErr = await new Response(createProc.stderr).text();
await createProc.exited;

if (createProc.exitCode !== 0) {
  process.stderr.write(`ERROR: Issue 起票失敗:\n${createErr}\n`);
  process.exit(1);
}

const issueUrl = createOut.trim();
process.stdout.write(`OK: Issue を起票しました: ${issueUrl}\n`);

// 起票した Issue 番号を state.json の raised_issues[] に記録する（ベストエフォート）
const numMatch = issueUrl.match(/\/(\d+)$/);
if (numMatch && existsSync(stateFile)) {
  try {
    const stateData = JSON.parse(readFileSync(stateFile, "utf-8")) as StateData;
    if (!stateData.raised_issues) stateData.raised_issues = [];
    const raisedNum = parseInt(numMatch[1], 10);
    // 重複登録しない
    if (!stateData.raised_issues.some((r) => r.number === raisedNum)) {
      stateData.raised_issues.push({ number: raisedNum, step });
      writeFileSync(stateFile, JSON.stringify(stateData), "utf-8");
      process.stdout.write(`  → state.json に raised_issues[${raisedNum}] を記録しました\n`);
    }
  } catch (e) {
    // ベストエフォート: 記録失敗しても起票自体は成功扱い
    process.stderr.write(`WARN: state.json への raised_issues 記録に失敗: ${e}\n`);
  }
}
}
