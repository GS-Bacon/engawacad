#!/usr/bin/env bun
// loop-adr-cross-ref-check.ts — ADR draft の cross-ADR 意味整合 precheck (歪み #2 修正)
//
// Decision Matrix lint (形式 check) の後段、Codex 3 ペルソナ adversarial review の前段に
// 挿入し、ADR draft が触れる領域で「引用すべき過去 ADR」「矛盾する既存 ADR」「ROADMAP
// 将来 Phase の言及との整合」を LLM (Codex → GLM fallback) でチェックする。
//
// ADR-015 amend 教訓: schema/format/topology/boolean に触れる ADR が単独で書かれ、
// Codex 3 ペルソナまで進んで初めて refute されるパターンを draft 段階で潰すための層。
//
// 使い方:
//   bun loop-adr-cross-ref-check.ts --adr <path> [--out-dir <dir>] [--dry-run]
//
// exit code:
//   0 = aligned (pass)
//   2 = 不整合あり (regen 推奨、結果は stderr + result file)
//   1 = 内部エラー

import { existsSync, readFileSync, writeFileSync, mkdirSync, readdirSync } from "fs";
import { basename } from "path";
import { runChecked } from "./loop-spawn-checked.ts";
import { detectCodexUsageLimit } from "../../3ai/scripts/dispatch-codex.ts";
import { runGlmViaZAI } from "../../3ai/scripts/glm-via-zai.ts";

export type CrossRefCheckResult = {
  aligned: boolean;
  missing_refs: string[];
  conflicts: string[];
  suggestions: string[];
  reviewer: "codex" | "glm" | "mock" | "dry-run";
  raw: string;
};

function adrSlug(path: string): string {
  return basename(path).replace(/\.md$/, "");
}

/** docs/decisions/ から accepted ADR 一覧を抽出 (自分自身は除外)。 */
export function collectAcceptedAdrSummaries(
  selfAdrPath: string,
  decisionsDir = "docs/decisions",
): { number: number; title: string; relatedLine: string }[] {
  if (!existsSync(decisionsDir)) return [];
  const selfBase = basename(selfAdrPath);
  const entries: { number: number; title: string; relatedLine: string }[] = [];
  for (const fname of readdirSync(decisionsDir).sort()) {
    if (!fname.endsWith(".md") || fname === selfBase) continue;
    const m = fname.match(/^(\d{3})-/);
    if (!m) continue;
    const num = parseInt(m[1], 10);
    let content: string;
    try { content = readFileSync(`${decisionsDir}/${fname}`, "utf-8"); } catch { continue; }
    // Title 行: `# ADR-NNN: ...`
    const titleMatch = content.match(/^#\s+ADR-\d{3}:?\s*(.+)$/m);
    const title = titleMatch?.[1].trim() ?? fname;
    // Status: Accepted 以外はスキップ (Proposed はまだ accept 確定していない)
    if (!/\*\*Status\*\*:\s*Accepted/i.test(content)) continue;
    // Related 行
    const relatedMatch = content.match(/^\*\*Related\*\*:\s*(.+)$/m);
    const relatedLine = relatedMatch?.[1].trim() ?? "(none)";
    entries.push({ number: num, title, relatedLine });
  }
  return entries;
}

/** ROADMAP.md から Phase 見出し + 1 行 summary を抽出 (将来構想 cross-ref 用)。 */
export function collectRoadmapPhaseSummaries(roadmapPath = "ROADMAP.md"): string[] {
  if (!existsSync(roadmapPath)) return [];
  const lines = readFileSync(roadmapPath, "utf-8").split("\n");
  const result: string[] = [];
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(/^##\s+(?:✅\s+)?Phase\s+\d+:\s*(.+)$/);
    if (!m) continue;
    // Phase 見出し直後の最初の 非空行 (5 行以内に探す)
    let summary = "";
    for (let j = i + 1; j < Math.min(i + 6, lines.length); j++) {
      const t = lines[j].trim();
      if (!t || t.startsWith("```")) continue;
      summary = t.replace(/^\*+|\*+$/g, "").slice(0, 120);
      break;
    }
    result.push(`${lines[i].replace(/^##\s+/, "").trim()} — ${summary}`);
  }
  return result;
}

function buildPrompt(
  adrText: string,
  acceptedAdrs: ReturnType<typeof collectAcceptedAdrSummaries>,
  roadmapPhases: string[],
): string {
  const acceptedList = acceptedAdrs.map(a =>
    `- ADR-${String(a.number).padStart(3, "0")}: ${a.title}\n    Related: ${a.relatedLine}`,
  ).join("\n");
  const roadmapList = roadmapPhases.map(p => `- ${p}`).join("\n");
  return [
    "# Role: ADR cross-reference auditor",
    "",
    "## Task",
    "以下の ADR draft が、既存 accepted ADR および ROADMAP 将来 Phase 構想と意味的に",
    "整合しているかを判定してください。Decision Matrix lint (形式 check) は通過済みなので、",
    "形式ではなく **意味的整合** (引用漏れ・矛盾・先例との競合) のみ評価します。",
    "",
    "判定基準:",
    "1. **引用漏れ (missing_refs)**: draft が触れる領域 (schema, format, topology, boolean, sketch, kernel 等) で、",
    "   先例として参照すべき過去 ADR が Related 行・本文中で言及されていない場合。",
    "2. **矛盾 (conflicts)**: draft の決定が既存 accepted ADR と整合しない場合 (字義的解釈ではなく、",
    "   先例の意図・将来構想を踏まえた実質的矛盾のみ)。",
    "3. **整合 (aligned)**: 上記 1 / 2 のいずれも該当しなければ aligned。",
    "",
    "## 出力フォーマット (必須、最終行に必ず verdict 行)",
    "```",
    "missing_refs:",
    "  - ADR-NNN: 理由 (引用すべき理由を 1 行で)",
    "  (なければ '- (none)')",
    "conflicts:",
    "  - ADR-NNN §X: 矛盾の内容 (1-2 行)",
    "  (なければ '- (none)')",
    "suggestions:",
    "  - draft への具体的修正提案 (1 行 × N)",
    "  (なければ '- (none)')",
    "verdict: aligned  ← または verdict: misaligned",
    "```",
    "",
    "## 既存 accepted ADR 一覧",
    acceptedList || "(none)",
    "",
    "## ROADMAP 将来 Phase 構想",
    roadmapList || "(none)",
    "",
    "## ADR draft",
    "",
    adrText,
  ].join("\n");
}

function parseResult(text: string): Omit<CrossRefCheckResult, "reviewer" | "raw"> {
  const verdictMatch = text.match(/verdict:\s*(aligned|misaligned)/i);
  const aligned = verdictMatch?.[1].toLowerCase() === "aligned";

  const parseList = (sectionName: string): string[] => {
    const re = new RegExp(`${sectionName}\\s*:\\s*\\n((?:\\s*-\\s+.+\\n?)+)`, "i");
    const m = text.match(re);
    if (!m) return [];
    return m[1]
      .split("\n")
      .map(l => l.replace(/^\s*-\s+/, "").trim())
      .filter(l => l && l.toLowerCase() !== "(none)" && l.toLowerCase() !== "none");
  };

  return {
    aligned,
    missing_refs: parseList("missing_refs"),
    conflicts: parseList("conflicts"),
    suggestions: parseList("suggestions"),
  };
}

async function runCodexCheck(prompt: string, outDir: string): Promise<{ ok: boolean; text: string; codexFailed: boolean }> {
  const resultFile = `${outDir}/cross-ref.codex.result.md`;
  const r = await runChecked(
    ["codex", "exec", "-c", "sandbox_mode=read-only", "--output-last-message", resultFile, prompt],
    { allowFailure: true },
  );
  writeFileSync(`${resultFile}.log`, r.stdout + r.stderr, "utf-8");
  if (r.exitCode !== 0) {
    const stderrCheck = r.stderr ?? "";
    const codexFailed = detectCodexUsageLimit(stderrCheck) || stderrCheck.includes("usage limit");
    return { ok: false, text: `[codex exit=${r.exitCode}] ${stderrCheck.slice(-300)}`, codexFailed: true };
  }
  let text = "";
  try { text = readFileSync(resultFile, "utf-8"); } catch { text = r.stdout; }
  return { ok: true, text, codexFailed: false };
}

async function runGlmCheck(prompt: string, outDir: string): Promise<{ ok: boolean; text: string }> {
  const resultFile = `${outDir}/cross-ref.glm.result.md`;
  const r = await runGlmViaZAI({ prompt });
  writeFileSync(`${resultFile}.raw`, r.stdout + r.stderr, "utf-8");
  if (!r.ok) {
    const err = r.error ?? `[glm exit=${r.exitCode}] ${(r.stderr ?? "").slice(-300)}`;
    return { ok: false, text: err };
  }
  const text = r.result || r.stdout;
  writeFileSync(resultFile, text, "utf-8");
  return { ok: true, text };
}

export type CrossRefCheckOpts = {
  adrPath: string;
  outDir?: string;
  dryRun?: boolean;
};

export async function crossRefCheck(opts: CrossRefCheckOpts): Promise<CrossRefCheckResult> {
  const slug = adrSlug(opts.adrPath);
  const outDir = opts.outDir ?? `features/.loop/adr-review/${slug}`;
  mkdirSync(outDir, { recursive: true });

  const adrText = readFileSync(opts.adrPath, "utf-8");
  const acceptedAdrs = collectAcceptedAdrSummaries(opts.adrPath);
  const roadmapPhases = collectRoadmapPhaseSummaries();

  // mock 経路 (テスト用)
  const mock = process.env.ADR_CROSSREF_MOCK;
  if (mock === "aligned") {
    return { aligned: true, missing_refs: [], conflicts: [], suggestions: [], reviewer: "mock", raw: "[mock:aligned]" };
  }
  if (mock === "misaligned") {
    return {
      aligned: false,
      missing_refs: ["ADR-010: serde default + 互換維持の先例"],
      conflicts: [],
      suggestions: ["Related 行に ADR-010 を追加"],
      reviewer: "mock",
      raw: "[mock:misaligned]",
    };
  }
  if (opts.dryRun || process.env.CODEX_DRY_RUN === "1") {
    return { aligned: true, missing_refs: [], conflicts: [], suggestions: [], reviewer: "dry-run", raw: "[dry-run]" };
  }

  const prompt = buildPrompt(adrText, acceptedAdrs, roadmapPhases);
  writeFileSync(`${outDir}/cross-ref.instruction.md`, prompt, "utf-8");

  // Codex first, GLM fallback (#251 同等)
  const codex = await runCodexCheck(prompt, outDir);
  let text = codex.text;
  let reviewer: "codex" | "glm" = "codex";
  if (!codex.ok && codex.codexFailed) {
    process.stderr.write(`WARN: cross-ref Codex failed (usage limit likely) — falling back to GLM\n`);
    const glm = await runGlmCheck(prompt, outDir);
    if (glm.ok) {
      text = glm.text;
      reviewer = "glm";
    } else {
      // 両者 fail → safe-side で aligned = false (規制側に倒す)
      return {
        aligned: false,
        missing_refs: [],
        conflicts: [`cross-ref Codex+GLM 共に失敗: ${codex.text}; ${glm.text}`],
        suggestions: [],
        reviewer: "glm",
        raw: `${codex.text}\n---\n${glm.text}`,
      };
    }
  } else if (!codex.ok) {
    // Codex 内部エラー (usage limit ではない crash) → aligned=false で regen 推奨
    return {
      aligned: false,
      missing_refs: [],
      conflicts: [`cross-ref Codex 失敗: ${codex.text}`],
      suggestions: [],
      reviewer: "codex",
      raw: codex.text,
    };
  }

  const parsed = parseResult(text);
  const result: CrossRefCheckResult = { ...parsed, reviewer, raw: text };
  writeFileSync(`${outDir}/cross-ref.result.json`, JSON.stringify(result, null, 2), "utf-8");
  return result;
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  function arg(name: string): string | undefined {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
  }
  function flag(name: string): boolean { return argv.includes(name); }

  const adrPath = arg("--adr");
  const outDir = arg("--out-dir");
  const dryRun = flag("--dry-run");
  if (!adrPath) {
    console.error("Usage: loop-adr-cross-ref-check.ts --adr <path> [--out-dir <dir>] [--dry-run]");
    process.exit(1);
  }
  if (!existsSync(adrPath)) {
    console.error(`ERROR: ADR file not found: ${adrPath}`);
    process.exit(1);
  }

  const result = await crossRefCheck({ adrPath, outDir, dryRun });
  console.log(JSON.stringify(result, null, 2));
  if (result.aligned) {
    console.error(`OK: cross-ref aligned (reviewer=${result.reviewer})`);
    process.exit(0);
  }
  console.error(`MISALIGNED: cross-ref check (reviewer=${result.reviewer})`);
  for (const m of result.missing_refs) console.error(`  - missing: ${m}`);
  for (const c of result.conflicts) console.error(`  - conflict: ${c}`);
  for (const s of result.suggestions) console.error(`  - suggest: ${s}`);
  process.exit(2);
}
