#!/usr/bin/env bun
// loop-split-detector.ts — review yaml の split_proposal を読んで親 blocked-by-split + 子起票
//
// 想定 yaml format:
//   split_proposal:
//     - title: "子 Issue タイトル"
//       body: "子 Issue body"
//       labels: ["type: feature", "batch:kernel"]
//
// 使い方:
//   bun loop-split-detector.ts process --review-yaml <path> --parent-issue N [--dry-run]
//
// 効果:
// - 親 Issue に blocked-by-split ラベル付与
// - 各子 entry を gh issue create --label "<元 labels>,parent-blocked-by-split:<親#>"
// - stdout に created 子 Issue 番号一覧

import { existsSync, readFileSync } from "fs";

interface SplitEntry {
  title: string;
  body?: string;
  labels?: string[];
}

async function runGh(args: string[]): Promise<{ stdout: string; exit: number }> {
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return { stdout: out.trim(), exit: proc.exitCode ?? 0 };
}

/** 極めて単純な split_proposal YAML パーサー (依存ゼロ、想定形式のみサポート) */
function parseSplitProposalYaml(text: string): SplitEntry[] {
  const lines = text.split("\n");
  const startIdx = lines.findIndex(l => /^split_proposal\s*:/.test(l));
  if (startIdx < 0) return [];
  const entries: SplitEntry[] = [];
  let current: SplitEntry | null = null;
  let inBody = false;
  let bodyLines: string[] = [];
  let labelMode = false;

  for (let i = startIdx + 1; i < lines.length; i++) {
    const raw = lines[i];
    if (/^[^\s-]/.test(raw)) break; // 次のトップキー
    const trimmed = raw.trim();
    if (trimmed === "") continue;

    // 子エントリの開始: 「  - title: ...」
    const newEntry = raw.match(/^\s*-\s*title\s*:\s*(.*)$/);
    if (newEntry) {
      if (current) {
        if (bodyLines.length > 0) current.body = bodyLines.join("\n").trim();
        entries.push(current);
      }
      current = { title: newEntry[1].replace(/^["']|["']$/g, "").trim() };
      bodyLines = [];
      inBody = false;
      labelMode = false;
      continue;
    }
    if (!current) continue;

    const titleKv = raw.match(/^\s+title\s*:\s*(.*)$/);
    if (titleKv) {
      current.title = titleKv[1].replace(/^["']|["']$/g, "").trim();
      inBody = false; labelMode = false;
      continue;
    }
    const bodyKv = raw.match(/^\s+body\s*:\s*(.*)$/);
    if (bodyKv) {
      const inline = bodyKv[1].trim();
      if (inline.startsWith("|") || inline.startsWith(">") || inline === "") {
        inBody = true; bodyLines = [];
      } else {
        current.body = inline.replace(/^["']|["']$/g, "");
        inBody = false;
      }
      labelMode = false;
      continue;
    }
    const labelsKv = raw.match(/^\s+labels\s*:\s*(.*)$/);
    if (labelsKv) {
      const inline = labelsKv[1].trim();
      if (inline.startsWith("[") && inline.endsWith("]")) {
        current.labels = inline
          .slice(1, -1)
          .split(",")
          .map(s => s.trim().replace(/^["']|["']$/g, ""))
          .filter(Boolean);
        labelMode = false;
      } else {
        labelMode = true;
        current.labels = [];
      }
      inBody = false;
      continue;
    }
    if (labelMode) {
      const m = raw.match(/^\s+-\s*(.*)$/);
      if (m) {
        current.labels!.push(m[1].replace(/^["']|["']$/g, "").trim());
        continue;
      }
    }
    if (inBody) bodyLines.push(raw);
  }
  if (current) {
    if (bodyLines.length > 0 && !current.body) current.body = bodyLines.join("\n").trim();
    entries.push(current);
  }
  return entries;
}

async function addParentLabel(parent: number): Promise<void> {
  await runGh(["issue", "edit", String(parent), "--add-label", "blocked-by-split"]);
}

async function createChild(entry: SplitEntry, parent: number, dryRun: boolean): Promise<number | null> {
  const labels = [...(entry.labels ?? []), `parent-blocked-by-split:${parent}`];
  if (dryRun) {
    console.log(`[dry-run] would create child: ${entry.title} labels=${labels.join(",")}`);
    return null;
  }
  const body = entry.body ?? `分割元: #${parent}`;
  const tmp = `/tmp/split-child-${Date.now()}-${Math.random().toString(36).slice(2, 8)}.md`;
  Bun.write(tmp, body);
  const r = await runGh([
    "issue", "create",
    "--title", entry.title,
    "--body-file", tmp,
    "--label", labels.join(","),
  ]);
  if (r.exit !== 0) {
    process.stderr.write(`ERROR: gh issue create failed for ${entry.title}\n`);
    return null;
  }
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

  if (cmd !== "process") {
    console.error("Usage: loop-split-detector.ts process --review-yaml <path> --parent-issue N [--dry-run]");
    process.exit(2);
  }
  const yamlPath = arg("--review-yaml");
  const parentStr = arg("--parent-issue");
  const parent = parseInt(parentStr ?? "");
  const dryRun = flag("--dry-run");
  if (!yamlPath || !parent || isNaN(parent)) {
    console.error("Usage: loop-split-detector.ts process --review-yaml <path> --parent-issue N");
    process.exit(2);
  }
  if (!existsSync(yamlPath)) {
    console.error(`review yaml not found: ${yamlPath}`);
    process.exit(1);
  }
  const text = readFileSync(yamlPath, "utf-8");
  const entries = parseSplitProposalYaml(text);
  if (entries.length === 0) {
    console.log("no split_proposal in yaml");
    process.exit(0);
  }

  if (!dryRun) await addParentLabel(parent);

  const created: { title: string; child: number | null }[] = [];
  for (const e of entries) {
    const child = await createChild(e, parent, dryRun);
    created.push({ title: e.title, child });
  }
  console.log(JSON.stringify({ parent, dry_run: dryRun, children: created }, null, 2));
}
