#!/usr/bin/env bun
// loop-codex-skip-collector.ts — Codex gate スキップの後払いレビュー回収
//
// features/.loop/codex-skips.jsonl の resolved_at:null エントリを古い順に最大 2 件/サイクル
// review する。record-codex-skip.ts が積んだ「Codex gate (STEP 3.5/7.5) が usage-limit 等で
// スキップされた状態で main にマージ済み」エントリを、Codex 回復後にまとめて後追いレビューする。
//
// 各エントリ:
//   1. merge 済み squash commit を `git log --grep "Closes #<issue>"` で特定
//      (commit message に必ず含まれる規約。#293 が #2934 に誤マッチしないよう JS 側で厳密判定)
//   2. `git show <sha>` の diff + Issue title/body で Codex レビュー入力を構築 → Codex 呼び出し
//      (dispatch-codex.ts mode=design を再利用。usage-limit は detectCodexUsageLimit で判定)
//   3. blocking (critical/high) findings が出たら bug Issue を起票 (元 batch 継承 or batch:kernel)
//   4. 成功エントリは resolved_at を書き込み + codex-review-deferred ラベル除去
//
// 中断規約:
//   - usage-limit 検出 → その場で処理中断 (残エントリは次サイクルへ)
//   - 他のエラーは 1 エントリ失敗で全体を止めない (resolved にせず次サイクルで再試行)
//   - merge commit 無し → no-merge-commit として close 扱い (レビュー対象が main に無い)
//
// fail-safe: main() は必ず exit 0 (collector 自身の例外がループを止めないため)。

import { existsSync, mkdirSync, readFileSync, unlinkSync, writeFileSync } from "fs";
import { runChecked } from "./loop-spawn-checked.ts";
import {
  dispatchCodex,
  detectCodexUsageLimit,
  parseVerdict,
} from "../../3ai/scripts/dispatch-codex.ts";
import {
  DEFERRED_LABEL,
  readLedger,
  resolveLedgerPath,
  writeLedger,
  type SkipEntry,
} from "../../3ai/scripts/record-codex-skip.ts";

export const MAX_PER_CYCLE = 2;
const REVIEWER_INSTRUCTION = ".claude/skills/3ai/agents/codex-final-reviewer.md";
const REVIEW_WORKDIR = "features/.loop/codex-skip-review";

export interface CodexReviewResult {
  /** codex が verdict を出せたか (usage-limit / crash 時は false)。 */
  ok: boolean;
  /** usage/rate limit を検出したか。true なら回収サイクルを中断する。 */
  usageLimit: boolean;
  /** critical + high の件数。 */
  blocking: number;
  /** レビュー本文 (blocking Issue の body に転記)。 */
  findings: string;
  error?: string;
}

export interface IssueMeta {
  title: string;
  body: string;
  /** 元 Issue の batch:* ラベル (継承用)。無ければ null。 */
  batch: string | null;
}

/** 副作用を持つ操作の注入口。テストは mock を差し込む。 */
export interface CollectorDeps {
  findMergeSha(issue: number): Promise<string | null>;
  getCommitDiff(sha: string): Promise<string>;
  getIssueMeta(issue: number): Promise<IssueMeta>;
  reviewCodex(input: { entry: SkipEntry; sha: string; diff: string; meta: IssueMeta }): Promise<CodexReviewResult>;
  /** blocking Issue を起票して新 Issue 番号を返す。gh 失敗時は throw (呼び元が retry)。 */
  raiseBlockingIssue(input: { entry: SkipEntry; sha: string; meta: IssueMeta; findings: string }): Promise<number | null>;
  removeDeferredLabel(issue: number): Promise<void>;
  now(): Date;
}

export interface CollectResult {
  processed: number;
  resolved: number;
  raised: number;
  /** usage-limit で途中中断したか。 */
  aborted: boolean;
  /** 回収後に残る未回収エントリ数。 */
  remaining: number;
  details: Array<{ issue: number; step: string; outcome: string }>;
}

/** ledger を読み、古い順に最大 maxPerCycle 件を回収する純粋寄りのコア。副作用は deps 経由。 */
export async function collectCodexSkips(opts: {
  ledgerPath: string;
  maxPerCycle?: number;
  deps: CollectorDeps;
}): Promise<CollectResult> {
  const { ledgerPath, deps } = opts;
  const maxPerCycle = opts.maxPerCycle ?? MAX_PER_CYCLE;

  const all = readLedger(ledgerPath);
  const unresolved = all
    .filter((e) => e.resolved_at == null)
    .sort((a, b) =>
      a.recorded_at < b.recorded_at ? -1 : a.recorded_at > b.recorded_at ? 1 : 0,
    );
  const batch = unresolved.slice(0, maxPerCycle);

  const details: CollectResult["details"] = [];
  let processed = 0;
  let resolved = 0;
  let raised = 0;
  let aborted = false;
  let dirty = false;

  for (const entry of batch) {
    processed++;

    // 1. merge commit 特定
    let sha: string | null = null;
    try {
      sha = await deps.findMergeSha(entry.issue);
    } catch (e) {
      process.stderr.write(`WARN: #${entry.issue} merge sha 検索失敗: ${(e as Error).message}\n`);
    }

    if (!sha) {
      // レビュー対象が main に無い (WIP 凍結等) → close 扱いにして後払いを畳む
      process.stderr.write(
        `WARN: #${entry.issue} の merge commit (Closes #${entry.issue}) が見つからない → no-merge-commit として close\n`,
      );
      entry.resolved_at = deps.now().toISOString();
      entry.resolution = "no-merge-commit";
      dirty = true;
      resolved++;
      details.push({ issue: entry.issue, step: entry.step, outcome: "no-merge-commit" });
      continue;
    }

    // 2. diff + Issue meta 取得
    let diff = "";
    let meta: IssueMeta = { title: `#${entry.issue}`, body: "", batch: null };
    try {
      diff = await deps.getCommitDiff(sha);
      meta = await deps.getIssueMeta(entry.issue);
    } catch (e) {
      process.stderr.write(
        `WARN: #${entry.issue} diff/meta 取得失敗: ${(e as Error).message} — 次サイクルへ持ち越し\n`,
      );
      details.push({ issue: entry.issue, step: entry.step, outcome: "fetch-error" });
      continue;
    }

    // 3. Codex review
    let review: CodexReviewResult;
    try {
      review = await deps.reviewCodex({ entry, sha, diff, meta });
    } catch (e) {
      process.stderr.write(
        `WARN: #${entry.issue} Codex review 例外: ${(e as Error).message} — 次サイクルへ持ち越し\n`,
      );
      details.push({ issue: entry.issue, step: entry.step, outcome: "review-exception" });
      continue;
    }

    if (review.usageLimit) {
      // usage-limit → その場で中断。このエントリも次エントリも resolved にしない。
      process.stderr.write(
        `[CODEX_USAGE_LIMIT] #${entry.issue} 回収中に usage/rate limit 検出 → 中断 (残エントリは次サイクル)\n`,
      );
      aborted = true;
      details.push({ issue: entry.issue, step: entry.step, outcome: "usage-limit-abort" });
      break;
    }
    if (!review.ok) {
      process.stderr.write(
        `WARN: #${entry.issue} Codex review 失敗 (${review.error ?? "unknown"}) — 次サイクルへ持ち越し\n`,
      );
      details.push({ issue: entry.issue, step: entry.step, outcome: "review-failed" });
      continue;
    }

    // 4. blocking なら bug 起票
    let outcome = "reviewed-clean";
    if (review.blocking >= 1) {
      let newIssue: number | null = null;
      try {
        newIssue = await deps.raiseBlockingIssue({ entry, sha, meta, findings: review.findings });
      } catch (e) {
        // 起票失敗時は resolved にしない (blocking が記録されないまま台帳から消えるのを防ぐ)
        process.stderr.write(
          `WARN: #${entry.issue} blocking Issue 起票失敗: ${(e as Error).message} — 次サイクルへ持ち越し\n`,
        );
        details.push({ issue: entry.issue, step: entry.step, outcome: "raise-failed" });
        continue;
      }
      raised++;
      outcome = newIssue ? `blocking-raised:#${newIssue}` : "blocking-raised";
    }

    // 5. resolved 化 + ラベル除去
    entry.resolved_at = deps.now().toISOString();
    entry.resolution = outcome;
    dirty = true;
    resolved++;
    details.push({ issue: entry.issue, step: entry.step, outcome });
    try {
      await deps.removeDeferredLabel(entry.issue);
    } catch (e) {
      process.stderr.write(`WARN: #${entry.issue} ${DEFERRED_LABEL} 除去失敗: ${(e as Error).message}\n`);
    }
  }

  if (dirty) writeLedger(all, ledgerPath);

  const remaining = all.filter((e) => e.resolved_at == null).length;
  return { processed, resolved, raised, aborted, remaining, details };
}

// ---- 実 side-effect 実装 (デフォルト deps) ----

const US = "\x1f"; // unit separator (sha ↔ body)
const RS = "\x1e"; // record separator (commit ↔ commit)

async function realFindMergeSha(issue: number): Promise<string | null> {
  // git --grep は基本正規表現の substring 一致なので superset を取り、JS 側で
  // `#<issue>(?!\d)` により #293 が #2934 に誤マッチしないよう厳密判定する。
  const r = await runChecked(
    ["git", "log", "--grep", `Closes #${issue}`, `--format=%H${US}%B${RS}`, "-n", "50"],
    { allowFailure: true },
  );
  if (r.exitCode !== 0) return null;
  const re = new RegExp(`Closes #${issue}(?!\\d)`);
  // git log は新しい順で出力するため、最初の一致が最新の squash commit。
  for (const rec of r.stdout.split(RS)) {
    const t = rec.trim();
    if (!t) continue;
    const idx = t.indexOf(US);
    if (idx < 0) continue;
    const sha = t.slice(0, idx).trim();
    const body = t.slice(idx + 1);
    if (re.test(body)) return sha;
  }
  return null;
}

async function realGetCommitDiff(sha: string): Promise<string> {
  const r = await runChecked(["git", "show", "--format=medium", sha], { allowFailure: true });
  return r.stdout;
}

async function realGetIssueMeta(issue: number): Promise<IssueMeta> {
  const r = await runChecked(
    ["gh", "issue", "view", String(issue), "--json", "title,body,labels"],
    { allowFailure: true },
  );
  if (r.exitCode !== 0) return { title: `#${issue}`, body: "", batch: null };
  try {
    const j = JSON.parse(r.stdout) as {
      title?: string;
      body?: string;
      labels?: Array<{ name: string }>;
    };
    const batch = (j.labels ?? []).map((l) => l.name).find((n) => n.startsWith("batch:")) ?? null;
    return { title: j.title ?? `#${issue}`, body: j.body ?? "", batch };
  } catch {
    return { title: `#${issue}`, body: "", batch: null };
  }
}

async function realReviewCodex(input: {
  entry: SkipEntry;
  sha: string;
  diff: string;
  meta: IssueMeta;
}): Promise<CodexReviewResult> {
  const { entry, sha, diff, meta } = input;
  mkdirSync(REVIEW_WORKDIR, { recursive: true });
  const inputFile = `${REVIEW_WORKDIR}/input-${entry.issue}.md`;
  const resultFile = `${REVIEW_WORKDIR}/result-${entry.issue}.md`;
  const content =
    `# 後払い Codex レビュー: Issue #${entry.issue} (${entry.slug})\n\n` +
    `このコードは Codex gate (STEP ${entry.step}) が usage-limit 等でスキップされた状態で main にマージ済み。\n` +
    `後追いで独立レビューする。blocking (critical/high) があれば verdict: fail と severity を明記せよ。\n\n` +
    `## Issue タイトル\n${meta.title}\n\n` +
    `## Issue 本文\n${meta.body || "(なし)"}\n\n` +
    `## マージ済みコミット ${sha}\n\`\`\`diff\n${diff}\n\`\`\`\n`;
  writeFileSync(inputFile, content, "utf-8");

  try {
    // dispatch-codex.ts mode=design を再利用: inputFile を stdin に流し verdict.json / log を生成。
    const code = await dispatchCodex({
      mode: "design",
      instructionFile: REVIEWER_INSTRUCTION,
      inputFile,
      resultFile,
    });

    // usage-limit 判定 (#250 と同じ detectCodexUsageLimit を dispatch-codex が書く log に適用)
    let logText = "";
    try {
      logText = readFileSync(`${resultFile}.log`, "utf-8");
    } catch {}
    if (detectCodexUsageLimit(logText)) {
      return { ok: false, usageLimit: true, blocking: 0, findings: "", error: "usage-limit" };
    }
    if (code !== 0) {
      return { ok: false, usageLimit: false, blocking: 0, findings: "", error: `codex exit=${code}` };
    }

    let resultText = "";
    try {
      resultText = readFileSync(resultFile, "utf-8");
    } catch {}
    const verdict = parseVerdict(resultText);
    return { ok: true, usageLimit: false, blocking: verdict.blocking, findings: resultText };
  } catch (e) {
    return { ok: false, usageLimit: false, blocking: 0, findings: "", error: (e as Error).message };
  } finally {
    try {
      if (existsSync(inputFile)) unlinkSync(inputFile);
    } catch {}
  }
}

async function realRaiseBlockingIssue(input: {
  entry: SkipEntry;
  sha: string;
  meta: IssueMeta;
  findings: string;
}): Promise<number | null> {
  const { entry, sha, meta, findings } = input;
  const batch = meta.batch ?? "batch:kernel";
  const title = `fix(3ai): [後払いレビュー] Codex が #${entry.issue} (${entry.slug}) に blocking 指摘`;
  const body =
    `## 後払い Codex レビューで blocking 検出\n\n` +
    `Issue #${entry.issue} は Codex gate (STEP ${entry.step}) が usage-limit 等でスキップされた状態で main にマージされた。\n` +
    `回復後の後追いレビュー (loop-codex-skip-collector) で critical/high 指摘が出た。\n\n` +
    `- 元 Issue: #${entry.issue} (${entry.slug})\n` +
    `- gate: STEP ${entry.step}\n` +
    `- マージ済みコミット: ${sha}\n` +
    `- スキップ理由: ${entry.reason}\n\n` +
    `## Codex 指摘 (findings)\n\n${findings.slice(0, 8000)}\n\n` +
    `---\n*このIssueは loop-codex-skip-collector.ts により後払いレビューの blocking 指摘を受けて自動起票されました。*`;
  const labels = `bug,${batch}`;

  const r = await runChecked(
    ["gh", "issue", "create", "--title", title, "--body", body, "--label", labels],
    { allowFailure: true },
  );
  if (r.exitCode !== 0) {
    // throw して呼び元に retry させる (起票できないまま resolved にしない)
    throw new Error(`gh issue create 失敗 (exit=${r.exitCode}): ${r.stderr.trim().slice(0, 200)}`);
  }
  const url = r.stdout.trim();
  const m = url.match(/\/(\d+)\s*$/);
  const newNum = m ? parseInt(m[1]) : null;

  if (newNum) {
    // 起票直後に label lint (exit 0 期待、非0 は warn のみ — 起票自体は成功扱い)
    const lint = await runChecked(
      ["bun", ".claude/skills/3ai/scripts/lint-issue-labels.ts", "--issue", String(newNum)],
      { allowFailure: true },
    );
    if (lint.exitCode !== 0) {
      process.stderr.write(
        `WARN: 起票した #${newNum} の label lint 非0 (exit=${lint.exitCode}): ${lint.stderr.trim()}\n`,
      );
    }
  }
  return newNum;
}

async function realRemoveDeferredLabel(issue: number): Promise<void> {
  // closed Issue でも gh issue edit --remove-label は動く。不在ラベルでも allowFailure で握る。
  await runChecked(
    ["gh", "issue", "edit", String(issue), "--remove-label", DEFERRED_LABEL],
    { allowFailure: true },
  );
}

export function defaultDeps(): CollectorDeps {
  return {
    findMergeSha: realFindMergeSha,
    getCommitDiff: realGetCommitDiff,
    getIssueMeta: realGetIssueMeta,
    reviewCodex: realReviewCodex,
    raiseBlockingIssue: realRaiseBlockingIssue,
    removeDeferredLabel: realRemoveDeferredLabel,
    now: () => new Date(),
  };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let ledger = "";
  let maxPerCycle = MAX_PER_CYCLE;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--ledger") ledger = args[++i];
    else if (args[i] === "--max") maxPerCycle = parseInt(args[++i]) || MAX_PER_CYCLE;
    else process.stderr.write(`Unknown arg: ${args[i]}\n`);
  }
  const ledgerPath = resolveLedgerPath(ledger);

  try {
    const res = await collectCodexSkips({ ledgerPath, maxPerCycle, deps: defaultDeps() });
    process.stdout.write(JSON.stringify(res) + "\n");
    process.stdout.write(
      `codex-skip-collector: processed=${res.processed} resolved=${res.resolved} ` +
        `raised=${res.raised} aborted=${res.aborted} remaining=${res.remaining}\n`,
    );
  } catch (e) {
    // fail-safe: collector 自身の例外がループを止めないよう握りつぶして exit 0
    process.stderr.write(`WARN: collector 例外 (握りつぶして exit 0): ${(e as Error).message}\n`);
  }
  process.exit(0);
}
