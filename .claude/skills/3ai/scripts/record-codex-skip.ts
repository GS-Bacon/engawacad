#!/usr/bin/env bun
// record-codex-skip.ts — Codex gate スキップの後払いレビュー台帳への記録
//
// 背景: /3ai の Codex 独立レビュー gate (STEP 3.5 設計 / STEP 7.5 実装) は、Codex CLI が
//   usage/rate limit やエラーを返すと「state.ts set ... codex_{design,review} passed」で
//   無条件 pass に倒れる。つまり quota 枯渇中にマージされたコードは独立レビューなしで
//   main に入り、その事実がどこにも残らない。
//
// このスクリプトはスキップを台帳 (features/.loop/codex-skips.jsonl) に 1 行 append し、
//   後で loop-codex-skip-collector.ts が回復後にまとめて review する「後払い」に変える。
//
// 台帳が主・ラベルは補助: gh でラベル付与に失敗しても warn して続行する (台帳さえ残れば
//   collector が回収できる)。
//
// 使い方:
//   bun record-codex-skip.ts --issue N --slug S --step 3.5|7.5 --reason "..."
//   [--ledger <path>]   # 省略時 env CODEX_SKIPS_LEDGER or features/.loop/codex-skips.jsonl

import { appendFileSync, existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname } from "path";

export const DEFAULT_LEDGER = "features/.loop/codex-skips.jsonl";
export const DEFERRED_LABEL = "codex-review-deferred";

export type CodexSkipStep = "3.5" | "7.5";

export interface SkipEntry {
  issue: number;
  slug: string;
  step: CodexSkipStep;
  reason: string;
  recorded_at: string;
  /** null = 未回収。ISO = 回収済み。collector が書き込む。 */
  resolved_at: string | null;
  /** 回収結果 (reviewed-clean / blocking-raised:#N / no-merge-commit 等)。collector が書き込む。 */
  resolution?: string;
}

export function resolveLedgerPath(explicit?: string): string {
  return explicit || process.env.CODEX_SKIPS_LEDGER || DEFAULT_LEDGER;
}

/** ledger 全行を読む。破損行は warn して読み飛ばす (fail-safe: 1 行壊れても台帳全体を捨てない)。 */
export function readLedger(ledgerPath: string): SkipEntry[] {
  if (!existsSync(ledgerPath)) return [];
  const raw = readFileSync(ledgerPath, "utf-8");
  const out: SkipEntry[] = [];
  for (const line of raw.split("\n")) {
    const t = line.trim();
    if (!t) continue;
    try {
      out.push(JSON.parse(t) as SkipEntry);
    } catch {
      process.stderr.write(`WARN: codex-skips ledger の破損行を読み飛ばし: ${t.slice(0, 120)}\n`);
    }
  }
  return out;
}

export function appendSkip(entry: SkipEntry, ledgerPath: string): void {
  mkdirSync(dirname(ledgerPath), { recursive: true });
  appendFileSync(ledgerPath, JSON.stringify(entry) + "\n", "utf-8");
}

/** ledger 全体を atomic に書き換える (resolved_at 更新用)。tmp + rename。 */
export function writeLedger(entries: SkipEntry[], ledgerPath: string): void {
  mkdirSync(dirname(ledgerPath), { recursive: true });
  const body = entries.map((e) => JSON.stringify(e)).join("\n") + (entries.length ? "\n" : "");
  const tmp = `${ledgerPath}.tmp.${process.pid}.${Math.random().toString(36).slice(2, 8)}`;
  writeFileSync(tmp, body, "utf-8");
  renameSync(tmp, ledgerPath);
}

export function buildSkipEntry(
  issue: number,
  slug: string,
  step: CodexSkipStep,
  reason: string,
  now: Date = new Date(),
): SkipEntry {
  return { issue, slug, step, reason, recorded_at: now.toISOString(), resolved_at: null };
}

async function ghBestEffort(cmd: string[]): Promise<{ exitCode: number; stderr: string }> {
  // gh を best-effort で叩く。台帳が主なのでラベル操作が失敗しても throw しない。
  try {
    const proc = Bun.spawn(cmd, { stdout: "pipe", stderr: "pipe" });
    const stderr = await new Response(proc.stderr).text();
    await proc.exited;
    return { exitCode: proc.exitCode ?? -1, stderr };
  } catch (e) {
    return { exitCode: -1, stderr: String(e) };
  }
}

/** codex-review-deferred ラベルを付与。未存在なら create。gh 失敗は warn して続行 (台帳が主)。 */
export async function addDeferredLabel(issue: number): Promise<void> {
  await ghBestEffort([
    "gh", "label", "create", DEFERRED_LABEL,
    "--color", "D4C5F9",
    "--description", "Codex gate skipped (usage-limit 等); 後払いレビュー待ち",
  ]);
  const r = await ghBestEffort(["gh", "issue", "edit", String(issue), "--add-label", DEFERRED_LABEL]);
  if (r.exitCode !== 0) {
    process.stderr.write(
      `WARN: #${issue} への ${DEFERRED_LABEL} 付与に失敗 (exit=${r.exitCode})。` +
        `台帳記録は成功しているため続行。\n`,
    );
  }
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let issue = NaN;
  let slug = "";
  let step = "";
  let reason = "";
  let ledger = "";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--issue") issue = parseInt(args[++i]);
    else if (args[i] === "--slug") slug = args[++i];
    else if (args[i] === "--step") step = args[++i];
    else if (args[i] === "--reason") reason = args[++i];
    else if (args[i] === "--ledger") ledger = args[++i];
    else {
      process.stderr.write(`Unknown arg: ${args[i]}\n`);
      process.exit(2);
    }
  }

  if (!issue || isNaN(issue) || !slug || (step !== "3.5" && step !== "7.5") || !reason) {
    process.stderr.write(
      'Usage: record-codex-skip.ts --issue N --slug S --step 3.5|7.5 --reason "..." [--ledger <path>]\n',
    );
    process.exit(2);
  }

  const ledgerPath = resolveLedgerPath(ledger);
  const entry = buildSkipEntry(issue, slug, step as CodexSkipStep, reason);
  appendSkip(entry, ledgerPath);
  process.stdout.write(
    `OK: codex-skip 記録 → ${ledgerPath} (issue=#${issue} step=${step})\n`,
  );
  await addDeferredLabel(issue);
  process.exit(0);
}
