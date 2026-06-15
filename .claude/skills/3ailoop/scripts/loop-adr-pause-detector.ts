#!/usr/bin/env bun
// loop-adr-pause-detector.ts — 新規 ADR draft を検出して gate:adr-review Issue を起票
//
// 直近 N commit (デフォルト 5) の git diff で新規追加された docs/decisions/*.md を検出。
// 該当ファイルがあれば、各 ADR について以下を実施:
// - ADR 内容の冒頭 80 行を抜粋
// - gh issue create --label "gate:adr-review,docs" で gate Issue を起票
//
// 使い方:
//   bun loop-adr-pause-detector.ts scan [--depth N] [--dry-run]
//
// 関連 plan: dashboard の Decision Log に「ADR-NNN draft 起票」として追加される想定

import { existsSync, readFileSync } from "fs";

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

async function findNewAdrs(depth: number): Promise<string[]> {
  // pathspec を渡すと「ADR を含む最新 N commit」になり古い ADR まで拾われるため、
  // pathspec なしで「最新 N commit」を取り、後で ADR pattern で filter する。
  const out = await runGit([
    "log", `-${depth}`, "--diff-filter=A", "--name-only",
    "--pretty=format:",
  ]);
  const lines = out.split("\n").map(l => l.trim()).filter(l => l.length > 0);
  const adrs = lines.filter(l => /^docs\/decisions\/.+\.md$/.test(l));
  return [...new Set(adrs)];
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
  Bun.write(tmpFile, body);
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
    console.error("Usage: loop-adr-pause-detector.ts scan [--depth N] [--dry-run]");
    process.exit(2);
  }
  const depth = parseInt(arg("--depth") ?? "5");
  const dryRun = flag("--dry-run");
  const adrs = await findNewAdrs(depth);
  if (adrs.length === 0) {
    console.log("no new ADRs in recent commits");
    process.exit(0);
  }
  const created: { adr: string; issue: number | null }[] = [];
  for (const a of adrs) {
    const num = await raiseGateIssue(a, dryRun);
    created.push({ adr: a, issue: num });
  }
  console.log(JSON.stringify({ depth, new_adrs: adrs.length, created }, null, 2));
}
