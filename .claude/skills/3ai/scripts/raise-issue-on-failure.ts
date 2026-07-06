#!/usr/bin/env bun
// raise-issue-on-failure.ts — 3ai フロー中のトラブルを GitHub Issue として自動起票する
//
// Usage: bun raise-issue-on-failure.ts \
//   --step <STEP名> \
//   --feature-dir <features/N-slug> \
//   --error-summary <エラー概要テキスト> \
//   [--result-file <dispatch result JSON>] \
//   [--dry-run] \
//   [--new-on-block]   # 他者ブロック時のみ新規起票 (default は dedup → 親 Issue にコメント)
//
// dedup ガード (#229):
//   - default: 同 step + 同 parent + 直近 24h で auto-raised Issue が open なら新規起票せず親 Issue にコメント
//   - --new-on-block 指定時のみ強制新規起票 (他者ブロック / 別 Issue が止まっている等で明示的に分けたい時)
//
// exit 0: Issue 起票成功 or dedup 経由でコメント追記 or dry-run
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

/** #250: Codex/GLM の usage limit / rate limit は infra 一時障害であり、
 *  実装側で修正できる "fault" ではない。auto-raise すると "loop が回るたびに
 *  同じ infra Issue が湧く" だけになるため、errorSummary 側で検出して skip する。
 *
 *  検出対象: "You've hit your usage limit", "usage limit", "rate limit", "429"
 *  (大文字小文字無視) */
export function shouldSkipError(errorSummary: string): { skip: boolean; reason: string } {
  const text = errorSummary ?? "";
  if (/usage\s*limit|rate\s*limit|hit\s+your\s+(usage|rate)|\b429\b/i.test(text)) {
    return {
      skip: true,
      reason: "Codex/GLM usage|rate limit (infra 制約) — auto-raise 対象外",
    };
  }
  return { skip: false, reason: "" };
}

/** Codex 削減改修 Task 7: 親 Issue が `needs-human` かどうかを判定する。
 *  親 Issue が needs-human の場合、子 auto-raise は無駄なので skip する経路に使う。
 *  fetchLabels は gh CLI を呼ぶ default 実装があるが、テストで mock 差し替え可能。 */
export async function checkParentNeedsHuman(
  issueNum: string,
  fetchLabels?: (n: string) => Promise<string[]>,
): Promise<{ hasNeedsHuman: boolean }> {
  if (issueNum === "unknown" || !issueNum) return { hasNeedsHuman: false };
  const fetch = fetchLabels ?? (async (n: string) => {
    const proc = Bun.spawn(
      ["gh", "issue", "view", n, "--json", "labels", "-q", ".labels[].name"],
      { stdout: "pipe", stderr: "pipe" },
    );
    const out = await new Response(proc.stdout).text();
    await proc.exited;
    return out
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean);
  });
  try {
    const labels = await fetch(issueNum);
    return { hasNeedsHuman: labels.includes("needs-human") };
  } catch {
    return { hasNeedsHuman: false };
  }
}

/** #229: 既 open の auto-raised Issue 候補から「同 step + 同 parent + 直近 24h」を探す。
 *  見つかれば dedupTo にその Issue 番号を返す。呼び元はコメント追記して新規起票を skip する。
 *  parent 番号は `#NNN` の後ろに非数字 (or 末尾) があることを要求し、#220 が #2200 にマッチしないようにする。 */
export function findRecentSameStepIssue(
  step: string,
  parentIssue: number,
  candidates: Array<{ title: string; number: number; createdAt: string }>,
  now: Date,
  maxAgeHours: number = 24,
): { dedupTo: number | null } {
  const cutoffMs = now.getTime() - maxAgeHours * 3600 * 1000;
  // `#220` の次が非数字 or 末尾。`#2200` 等の数字続きをマッチさせない。
  const parentRe = new RegExp(`#${parentIssue}(?!\\d)`);
  for (const c of candidates) {
    const createdMs = Date.parse(c.createdAt);
    if (!Number.isFinite(createdMs) || createdMs < cutoffMs) continue;
    // title 形式: `fix(3ai): [自動起票] <step> でエラー — Issue #<parentIssue> (<slug>)`
    // step と parent 両方含むものだけ dedup 対象
    if (c.title.includes(step) && parentRe.test(c.title)) {
      return { dedupTo: c.number };
    }
  }
  return { dedupTo: null };
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
let newOnBlock = false;

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--step") step = args[++i];
  else if (args[i] === "--feature-dir") featureDir = args[++i];
  else if (args[i] === "--error-summary") errorSummary = args[++i];
  else if (args[i] === "--result-file") resultFile = args[++i];
  else if (args[i] === "--dry-run") dryRun = true;
  else if (args[i] === "--new-on-block") newOnBlock = true;
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

// Codex/GLM usage limit 等の infra 一時障害は自動起票対象外 (#250)
{
  const skip = shouldSkipError(errorSummary);
  if (skip.skip) {
    process.stderr.write(
      `SKIP: errorSummary に usage/rate limit シグナル検出 (${skip.reason}). ` +
        `次サイクルの retry で復旧する想定のため新規 Issue は起票しません。\n`,
    );
    process.exit(0);
  }
}

// featureDir から issue 番号・slug を取得
const dirMatch = featureDir.match(/features\/(\d+)-(.+)$/);
const issueNum = dirMatch ? dirMatch[1] : "unknown";
const slug = dirMatch ? dirMatch[2] : featureDir;

// Codex 削減改修 Task 7: 親 Issue が `needs-human` なら子 auto-raise は無駄なので skip。
// 親 Issue にコメント追記して観測可能に。RAISE_SKIP_PARENT_NEEDS_HUMAN_DISABLE=1 で無効化 (テスト用)。
if (issueNum !== "unknown" && process.env.RAISE_SKIP_PARENT_NEEDS_HUMAN_DISABLE !== "1") {
  const parentCheck = await checkParentNeedsHuman(issueNum);
  if (parentCheck.hasNeedsHuman) {
    const commentBody = `## 子 auto-raise skip (parent needs-human)

**Step**: ${step}
**Time**: ${new Date().toISOString()}

## エラー概要
${errorSummary}

---
*このコメントは raise-issue-on-failure.ts により親 Issue の \`needs-human\` 検出時に追記されました。*
*新規 Issue 起票は skip されました (親解除まで無駄な auto-raise を防ぐため)。*`;
    if (!dryRun) {
      try {
        const proc = Bun.spawn(
          ["gh", "issue", "comment", issueNum, "--body", commentBody],
          { stdout: "pipe", stderr: "pipe" },
        );
        await proc.exited;
      } catch {}
    }
    process.stdout.write(
      `SKIP: 親 Issue #${issueNum} が needs-human. 子 auto-raise を skip し親にコメント追記しました\n`,
    );
    process.exit(0);
  }
}

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

// 重複チェック (#229 拡張):
//   - default: 同 step + 同 parent + 直近 24h の open Issue があれば、新規起票せず親 Issue にコメント追記
//   - --new-on-block: dedup を skip して強制新規起票 (他者ブロック / 別 Issue が止まる時の明示フラグ)
const searchProc = Bun.spawn(
  ["gh", "issue", "list", "--state", "open", "--search", `"${step}" "${issueNum}"`, "--json", "title,number,createdAt", "--limit", "10"],
  { stdout: "pipe", stderr: "pipe" }
);
const searchOut = await new Response(searchProc.stdout).text();
await searchProc.exited;

if (!newOnBlock && issueNum !== "unknown") {
  try {
    const existing = JSON.parse(searchOut) as Array<{ title: string; number: number; createdAt: string }>;
    const dedupResult = findRecentSameStepIssue(step, parseInt(issueNum), existing, new Date(), 24);
    if (dedupResult.dedupTo !== null) {
      // 親 Issue (#issueNum) にコメント追記して新規起票を skip
      const commentBody = `## エラー再発 (auto-raised dedup, 24h 以内)

**Step**: ${step}
**Time**: ${new Date().toISOString()}
**Existing auto-raised Issue**: #${dedupResult.dedupTo}

## エラー概要
${errorSummary}
${ciExcerpt}

---
*このコメントは raise-issue-on-failure.ts により dedup 経由で追記されました。* \
*直近 24h 以内に同 step (#${dedupResult.dedupTo}) で起票済みのため新規 Issue は作成していません。* \
*\`--new-on-block\` フラグで強制新規起票が必要なケース (他者ブロック / 別 Issue が止まる) は明示してください。*`;
    const commentProc = Bun.spawn(
      ["gh", "issue", "comment", issueNum, "--body", commentBody],
      { stdout: "pipe", stderr: "pipe" }
    );
    const commentErr = await new Response(commentProc.stderr).text();
    await commentProc.exited;
    if (commentProc.exitCode !== 0) {
      process.stderr.write(`WARN: 親 Issue #${issueNum} へのコメント追記失敗: ${commentErr}\n`);
      // コメント失敗時は新規起票にフォールバックする
    } else {
      process.stdout.write(`OK: dedup 経由で親 Issue #${issueNum} にコメント追記しました (既存: #${dedupResult.dedupTo})\n`);
      process.exit(0);
    }
    }
  } catch {}
}

// 旧 dedup 経路 (--new-on-block or 24h 外で見つかった場合に同タイトルがあれば silent skip)
try {
  const existing = JSON.parse(searchOut) as Array<{ title: string; number: number; createdAt: string }>;
  const dup = existing.find((i) => i.title.includes(issueNum) && i.title.includes(step));
  if (dup && !newOnBlock) {
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
