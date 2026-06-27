#!/usr/bin/env bun
// loop-adr-pause-detector.ts — 新規 ADR draft を検出して gate:adr-review Issue を起票
//
// 直近 N commit (デフォルト 5) の git diff で新規追加された docs/decisions/*.md を検出。
// 該当ファイルがあれば、各 ADR について以下を実施:
// - ADR 内容の冒頭 80 行を抜粋
// - gh issue create --label "gate:adr-review,docs" で gate Issue を起票
// - --auto-accept フラグが付いていれば、起票後に loop-adr-auto-accept.ts を呼び出して
//   Decision Matrix lint + Multi-LLM Review を走らせる (ADR-013)
//
// 使い方:
//   bun loop-adr-pause-detector.ts scan [--depth N] [--auto-accept] [--dry-run]
//
// 関連 plan: dashboard の Decision Log に「ADR-NNN draft 起票」として追加される想定

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { basename, dirname } from "path";
import { autoAccept } from "./loop-adr-auto-accept";
import { clearRetiredIfHumanReleased } from "./loop-adr-regen-tracker";

const DEFAULT_MARKER_PATH = "features/.loop/last-adr-scan-sha";

async function runGit(args: string[]): Promise<string> {
  const proc = Bun.spawn(["git", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return out;
}

async function runGh(args: string[]): Promise<{ stdout: string; exit: number }> {
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return { stdout: out.trim(), exit: proc.exitCode ?? 0 };
}

/** #252: open gate:adr-review Issue の title から ADR path を抽出する。
 *  起票 title 形式: "[gate:adr-review] ADR-XXX: タイトル (docs/decisions/XXX.md)" */
export function extractAdrPathFromTitle(title: string): string | null {
  const m = title.match(/\((docs\/decisions\/[^)]+\.md)\)\s*$/);
  return m ? m[1] : null;
}

export type RescanRecord = {
  issue: number;
  adr: string | null;
  outcome: string;
  retired_cleared?: boolean;
};

/** ADR-013 運用: docs/decisions/<slug>.md → <slug> */
export function adrSlugFromPath(adrPath: string): string {
  return basename(adrPath, ".md");
}

/** #252: 既存の open gate:adr-review Issue を rescan して autoAccept に再投入する。
 *  両 LLM 不在 / regen 中断 / 過去サイクル取り残しで滞留した gate Issue を救済する。
 *  ADR-013 運用拡張: autoAccept 実行前に clearRetiredIfHumanReleased を呼び、
 *  人間が `needs-human` ラベルを外した retired ADR を auto-accept ルートに戻す。 */
export async function rescanStaleGateIssues(
  opts: {
    dryRun?: boolean;
    autoAcceptFn?: typeof autoAccept;
    ghFn?: typeof runGh;
    clearRetiredFn?: typeof clearRetiredIfHumanReleased;
  } = {},
): Promise<RescanRecord[]> {
  const gh = opts.ghFn ?? runGh;
  const aa = opts.autoAcceptFn ?? autoAccept;
  const clr = opts.clearRetiredFn ?? clearRetiredIfHumanReleased;
  const r = await gh([
    "issue", "list",
    "--label", "gate:adr-review",
    "--state", "open",
    "--json", "number,title",
    "--limit", "50",
  ]);
  if (r.exit !== 0) {
    process.stderr.write(`WARN: rescan: gh issue list failed (exit=${r.exit})\n`);
    return [];
  }
  let issues: Array<{ number: number; title: string }>;
  try {
    issues = JSON.parse(r.stdout);
  } catch {
    process.stderr.write(`WARN: rescan: gh output not JSON\n`);
    return [];
  }

  const records: RescanRecord[] = [];
  for (const i of issues) {
    const adrPath = extractAdrPathFromTitle(i.title);
    if (!adrPath) {
      records.push({ issue: i.number, adr: null, outcome: "skip: no ADR path in title" });
      continue;
    }
    if (!existsSync(adrPath)) {
      records.push({ issue: i.number, adr: adrPath, outcome: "skip: ADR file missing" });
      continue;
    }
    if (opts.dryRun) {
      records.push({ issue: i.number, adr: adrPath, outcome: "dry-run" });
      continue;
    }
    // ADR-013: 人間が needs-human を外していれば retired 解除して auto-accept ルートに戻す
    let retiredCleared = false;
    try {
      const slug = adrSlugFromPath(adrPath);
      const c = await clr(slug, i.number, gh);
      retiredCleared = c.cleared;
      if (c.cleared) {
        process.stderr.write(`[rescan] retired state cleared for ${slug} (issue #${i.number})\n`);
      }
    } catch (e) {
      process.stderr.write(`WARN: clearRetiredIfHumanReleased failed for issue #${i.number}: ${(e as Error).message}\n`);
    }
    try {
      const outcome = await aa({ adrPath, issueNum: i.number });
      records.push({ issue: i.number, adr: adrPath, outcome: outcome.kind, retired_cleared: retiredCleared });
    } catch (e) {
      records.push({ issue: i.number, adr: adrPath, outcome: `error:${(e as Error).message}`, retired_cleared: retiredCleared });
    }
  }
  return records;
}

async function existingGateIssueFor(adrPath: string): Promise<number | null> {
  // タイトル一致で既存 gate Issue を検出 (重複起票防止)
  const r = await runGh([
    "issue", "list", "--state", "all", "--label", "gate:adr-review",
    "--search", `${adrPath} in:title`,
    "--json", "number,title",
  ]);
  if (r.exit !== 0) return null;
  try {
    const arr = JSON.parse(r.stdout) as Array<{ number: number; title: string }>;
    return arr.find(i => i.title.includes(adrPath))?.number ?? null;
  } catch { return null; }
}

async function findNewAdrs(depth: number, sinceSha?: string): Promise<string[]> {
  // pathspec を渡すと「ADR を含む最新 N commit」になり古い ADR まで拾われるため、
  // pathspec なしで commit を選び、後で ADR pattern で filter する。
  // #177 指摘 7 対応: --since-sha があればその範囲、なければ depth で fallback
  let out: string;
  if (sinceSha) {
    out = await runGit([
      "log", `${sinceSha}..HEAD`, "--diff-filter=A", "--name-only",
      "--pretty=format:",
    ]);
  } else {
    out = await runGit([
      "log", `-${depth}`, "--diff-filter=A", "--name-only",
      "--pretty=format:",
    ]);
  }
  const lines = out.split("\n").map(l => l.trim()).filter(l => l.length > 0);
  const adrs = lines.filter(l => /^docs\/decisions\/.+\.md$/.test(l));
  return [...new Set(adrs)];
}

function readMarker(path: string): string | null {
  if (!existsSync(path)) return null;
  try { return readFileSync(path, "utf-8").trim() || null; } catch { return null; }
}

function writeMarker(path: string, sha: string): void {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, sha, "utf-8");
  renameSync(tmp, path);
}

function adrTitle(path: string): string {
  if (!existsSync(path)) return path;
  try {
    const text = readFileSync(path, "utf-8");
    const m = text.match(/^#\s+(.+)$/m);
    return m?.[1] ?? path;
  } catch { return path; }
}

function adrExcerpt(path: string): string {
  if (!existsSync(path)) return "*(ADR file missing)*";
  try {
    return readFileSync(path, "utf-8").split("\n").slice(0, 80).join("\n");
  } catch { return "*(read failed)*"; }
}

async function raiseGateIssue(path: string, dryRun: boolean): Promise<number | null> {
  const existing = await existingGateIssueFor(path);
  if (existing !== null) {
    process.stderr.write(`SKIP: ${path} already has gate Issue #${existing}\n`);
    return existing;
  }
  const title = `[gate:adr-review] ${adrTitle(path)} (${path})`;
  const body = [
    `## ADR review required`,
    "",
    `**File**: \`${path}\``,
    "",
    `loop が新規 ADR draft を検出した。人間が accepted/rejected を判断するまで loop は本 Issue を skip する。`,
    "",
    `## Review 後のアクション`,
    "",
    "- accepted: ADR の Status を Accepted に書き換えて commit、本 Issue を close、`gate:adr-review` ラベルを削除",
    "- rejected: ADR を revert する commit を作成し本 Issue を close",
    "",
    `## ADR Excerpt (first 80 lines)`,
    "",
    "```markdown",
    adrExcerpt(path),
    "```",
  ].join("\n");

  if (dryRun) {
    console.log(`[dry-run] would create issue: ${title}`);
    return null;
  }
  const tmpFile = `/tmp/adr-gate-${Date.now()}-${Math.random().toString(36).slice(2, 8)}.md`;
  // #178 指摘 2: await を追加 (= gh issue create が完成した body を読めるよう保証)
  await Bun.write(tmpFile, body);
  const r = await runGh([
    "issue", "create",
    "--title", title,
    "--body-file", tmpFile,
    "--label", "gate:adr-review,docs",
  ]);
  if (r.exit !== 0) {
    process.stderr.write(`ERROR: gh issue create failed for ${path}\n`);
    return null;
  }
  // gh issue create returns URL; extract number
  const m = r.stdout.match(/\/issues\/(\d+)$/);
  return m ? parseInt(m[1]) : null;
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;
  function arg(name: string): string | undefined {
    const i = rest.indexOf(name);
    return i >= 0 ? rest[i + 1] : undefined;
  }
  function flag(name: string): boolean { return rest.includes(name); }

  if (cmd !== "scan") {
    console.error("Usage: loop-adr-pause-detector.ts scan [--depth N] [--since-sha SHA] [--cycle-marker PATH] [--auto-accept] [--no-rescan-stale] [--dry-run]");
    process.exit(2);
  }
  const depth = parseInt(arg("--depth") ?? "5");
  const sinceShaArg = arg("--since-sha");
  const markerPath = arg("--cycle-marker") ?? DEFAULT_MARKER_PATH;
  const dryRun = flag("--dry-run");
  const autoAcceptMode = flag("--auto-accept");
  // #252: --auto-accept 時はデフォルトで滞留 gate Issue も rescan する。
  //       明示的に無効化したい場合のみ --no-rescan-stale を渡す。
  const rescanStale = autoAcceptMode && !flag("--no-rescan-stale");
  // sinceSha は引数 → marker → depth fallback の順 (#177 指摘 7)
  const sinceSha = sinceShaArg ?? readMarker(markerPath) ?? undefined;
  const adrs = await findNewAdrs(depth, sinceSha);

  // #252: 先に滞留 gate Issue を rescan (新規 scan の前に走らせ、両 LLM 復帰直後の救済優先)
  let rescanRecords: RescanRecord[] = [];
  if (rescanStale) {
    rescanRecords = await rescanStaleGateIssues({ dryRun });
  }

  if (adrs.length === 0) {
    // 検出ゼロでも marker は進める (次回スコープを狭めるため、これは安全)
    if (!dryRun) {
      const head = (await runGit(["rev-parse", "HEAD"])).trim();
      if (head) writeMarker(markerPath, head);
    }
    if (rescanStale && rescanRecords.length > 0) {
      console.log(JSON.stringify({ since: sinceSha ?? `depth=${depth}`, new_adrs: 0, rescan: rescanRecords }, null, 2));
    } else {
      console.log(`no new ADRs (since=${sinceSha ?? `depth=${depth}`})`);
    }
    process.exit(0);
  }

  // #178 指摘 2: 全 ADR の起票成功を確認してから marker 更新、1 件でも失敗なら exit 1 + marker 未更新
  const created: { adr: string; issue: number | null; auto_accept?: string }[] = [];
  let allOk = true;
  for (const a of adrs) {
    const num = await raiseGateIssue(a, dryRun);
    if (num === null && !dryRun) allOk = false;
    const rec: { adr: string; issue: number | null; auto_accept?: string } = { adr: a, issue: num };
    // ADR-013: --auto-accept モードなら起票直後に review chain を起動
    if (autoAcceptMode && num !== null && !dryRun) {
      try {
        const outcome = await autoAccept({ adrPath: a, issueNum: num });
        rec.auto_accept = outcome.kind;
      } catch (e) {
        rec.auto_accept = `error:${(e as Error).message}`;
      }
    }
    created.push(rec);
  }

  if (!dryRun && allOk) {
    const head = (await runGit(["rev-parse", "HEAD"])).trim();
    if (head) writeMarker(markerPath, head);
  }

  console.log(JSON.stringify({
    since: sinceSha ?? `depth=${depth}`,
    new_adrs: adrs.length,
    ok: allOk,
    auto_accept: autoAcceptMode,
    created,
    rescan: rescanRecords,
  }, null, 2));
  if (!allOk) {
    process.stderr.write(`ERROR: some ADR gate issues failed to create; marker NOT updated for retry\n`);
    process.exit(1);
  }
}
