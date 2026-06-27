#!/usr/bin/env bun
// loop-adr-refute-overrider.ts — Codex 3 ペルソナ refute の judge (歪み #2 修正の第 2 弾)
//
// ADR auto-accept フロー (loop-adr-auto-accept.ts) で Codex 3 ペルソナのうち
// 1 つ以上が refute を返した場合に起動。各 refute 理由を「過去 accepted ADR +
// ROADMAP 将来構想」と突き合わせ、(a) 真の矛盾 refute と (b) 字義解釈 refute を
// 区別する。全 refute が (b) ならば accept ルートへ昇格。一部でも (a) なら維持。
//
// 設計思想: refute は「維持」がデフォルト。override するためには judge が
// 「先例と整合的でない refute である」と明示的に判定する必要がある。
// 不確実な refute はすべて維持して regen ルートに回す。
//
// 使い方:
//   bun loop-adr-refute-overrider.ts --adr <path> --verdicts <path-to-verdicts.json> [--out-dir <dir>] [--dry-run]
//
// exit code:
//   0 = 全 refute override 可 (accept ルート昇格推奨)
//   2 = 1 つ以上の refute 維持 (regen ルート維持)
//   1 = 内部エラー

import { existsSync, readFileSync, writeFileSync, mkdirSync } from "fs";
import { basename } from "path";
import { runChecked } from "./loop-spawn-checked.ts";
import { detectCodexUsageLimit } from "../../3ai/scripts/dispatch-codex.ts";
import { runGlmViaZAI } from "../../3ai/scripts/glm-via-zai.ts";
import {
  collectAcceptedAdrSummaries,
  collectRoadmapPhaseSummaries,
} from "./loop-adr-cross-ref-check.ts";
import type { PersonaVerdict } from "./loop-adr-auto-accept.ts";

export type RefuteOverrideJudgement = {
  persona: string;
  keep_refute: boolean;   // true = refute 維持 / false = override
  reasoning: string;
  raw: string;
};

export type RefuteOverrideResult = {
  judgements: RefuteOverrideJudgement[];
  all_overridable: boolean;
  reviewer: "codex" | "glm" | "mock" | "dry-run";
  fallback_used: boolean;
};

function adrSlug(path: string): string {
  return basename(path).replace(/\.md$/, "");
}

function buildJudgePrompt(
  refute: PersonaVerdict,
  adrText: string,
  acceptedAdrs: ReturnType<typeof collectAcceptedAdrSummaries>,
  roadmapPhases: string[],
): string {
  const acceptedList = acceptedAdrs.map(a =>
    `- ADR-${String(a.number).padStart(3, "0")}: ${a.title}`,
  ).join("\n");
  const roadmapList = roadmapPhases.map(p => `- ${p}`).join("\n");
  return [
    "# Role: ADR refute judge (字義解釈 refute の override 判定)",
    "",
    "## Task",
    "Codex の 1 ペルソナが ADR draft を refute しました。この refute が",
    "(a) 真の矛盾 (先例・将来構想と整合的な指摘) か",
    "(b) 字義解釈 refute (条文を厳密に読みすぎて先例の意図を見落とした指摘) か",
    "を判定してください。",
    "",
    "判定基準:",
    "- **(a) 真の矛盾**: refute が指摘する内容が、過去 accepted ADR の先例と整合し、",
    "  かつ ROADMAP 将来構想と矛盾しないなら、refute は真の矛盾を指摘している。維持。",
    "- **(b) 字義解釈 refute**: refute が ADR draft の条文や用語を字義的に解釈した結果、",
    "  既存 ADR の **意図** や ROADMAP の構想と矛盾する判定をしているなら、refute は",
    "  字義解釈 refute。override 可能。",
    "",
    "**デフォルトは維持 (keep_refute: true)**。override するには judge が明確に",
    "「refute は先例・将来構想と矛盾する」と論証する必要がある。不確実なら維持。",
    "",
    "## 出力フォーマット (必須、最終行に verdict)",
    "```",
    "reasoning: <judge の論証 100-300 字。先例 ADR と将来 Phase を具体的に引用すること>",
    "verdict: keep_refute  ← または verdict: override",
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
    "",
    "---",
    "",
    `## Refute (persona: ${refute.persona})`,
    "",
    refute.raw_excerpt,
  ].join("\n");
}

function parseJudgement(text: string, persona: string): RefuteOverrideJudgement {
  const verdictMatch = text.match(/verdict:\s*(keep_refute|override)/i);
  const verdict = verdictMatch?.[1].toLowerCase() ?? "keep_refute";
  const reasoningMatch = text.match(/reasoning:\s*(.+?)(?:\n\s*verdict:|\Z)/is);
  const reasoning = reasoningMatch?.[1].trim() ?? "(no reasoning extracted)";
  return {
    persona,
    keep_refute: verdict === "keep_refute",
    reasoning: reasoning.slice(0, 500),
    raw: text.slice(0, 800),
  };
}

async function judgeViaCodex(
  refute: PersonaVerdict,
  prompt: string,
  outDir: string,
): Promise<{ ok: boolean; text: string; codexFailed: boolean }> {
  const resultFile = `${outDir}/refute-override-${refute.persona}.codex.result.md`;
  const r = await runChecked(
    ["codex", "exec", "-c", "sandbox_mode=read-only", "--output-last-message", resultFile, prompt],
    { allowFailure: true },
  );
  writeFileSync(`${resultFile}.log`, r.stdout + r.stderr, "utf-8");
  if (r.exitCode !== 0) {
    const stderr = r.stderr ?? "";
    const codexFailed = detectCodexUsageLimit(stderr) || stderr.includes("usage limit");
    return { ok: false, text: `[codex exit=${r.exitCode}] ${stderr.slice(-300)}`, codexFailed };
  }
  let text = "";
  try { text = readFileSync(resultFile, "utf-8"); } catch { text = r.stdout; }
  return { ok: true, text, codexFailed: false };
}

async function judgeViaGlm(
  refute: PersonaVerdict,
  prompt: string,
  outDir: string,
): Promise<{ ok: boolean; text: string }> {
  const resultFile = `${outDir}/refute-override-${refute.persona}.glm.result.md`;
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

export type OverrideRefutesOpts = {
  adrPath: string;
  refutes: PersonaVerdict[];
  outDir?: string;
  dryRun?: boolean;
};

export async function overrideRefutes(opts: OverrideRefutesOpts): Promise<RefuteOverrideResult> {
  if (opts.refutes.length === 0) {
    return { judgements: [], all_overridable: true, reviewer: "dry-run", fallback_used: false };
  }

  const slug = adrSlug(opts.adrPath);
  const outDir = opts.outDir ?? `features/.loop/adr-review/${slug}`;
  mkdirSync(outDir, { recursive: true });

  const mock = process.env.ADR_OVERRIDE_MOCK;
  if (mock === "override-all") {
    const judgements = opts.refutes.map(r => ({
      persona: r.persona,
      keep_refute: false,
      reasoning: "[mock:override]",
      raw: "[mock:override]",
    }));
    return { judgements, all_overridable: true, reviewer: "mock", fallback_used: false };
  }
  if (mock === "keep-all") {
    const judgements = opts.refutes.map(r => ({
      persona: r.persona,
      keep_refute: true,
      reasoning: "[mock:keep]",
      raw: "[mock:keep]",
    }));
    return { judgements, all_overridable: false, reviewer: "mock", fallback_used: false };
  }
  if (opts.dryRun || process.env.CODEX_DRY_RUN === "1") {
    // dry-run: 全 keep (= 安全側)
    const judgements = opts.refutes.map(r => ({
      persona: r.persona,
      keep_refute: true,
      reasoning: "[dry-run]",
      raw: "[dry-run]",
    }));
    return { judgements, all_overridable: false, reviewer: "dry-run", fallback_used: false };
  }

  const adrText = readFileSync(opts.adrPath, "utf-8");
  const acceptedAdrs = collectAcceptedAdrSummaries(opts.adrPath);
  const roadmapPhases = collectRoadmapPhaseSummaries();

  const judgements: RefuteOverrideJudgement[] = [];
  let fallbackUsed = false;
  let reviewer: "codex" | "glm" = "codex";

  for (const refute of opts.refutes) {
    const prompt = buildJudgePrompt(refute, adrText, acceptedAdrs, roadmapPhases);
    writeFileSync(`${outDir}/refute-override-${refute.persona}.instruction.md`, prompt, "utf-8");

    const codex = await judgeViaCodex(refute, prompt, outDir);
    let text = codex.text;
    let usedGlm = false;
    if (!codex.ok && codex.codexFailed) {
      process.stderr.write(`WARN: refute-override Codex failed for ${refute.persona} — falling back to GLM\n`);
      const glm = await judgeViaGlm(refute, prompt, outDir);
      if (glm.ok) {
        text = glm.text;
        usedGlm = true;
        fallbackUsed = true;
        reviewer = "glm";
      } else {
        // 両者 fail → 安全側で keep_refute
        judgements.push({
          persona: refute.persona,
          keep_refute: true,
          reasoning: `judge 失敗 (Codex+GLM 両者 fail): ${codex.text.slice(0, 200)}`,
          raw: `${codex.text}\n---\n${glm.text}`,
        });
        continue;
      }
    } else if (!codex.ok) {
      // Codex 非 usage-limit crash → 安全側で keep
      judgements.push({
        persona: refute.persona,
        keep_refute: true,
        reasoning: `judge 失敗 (Codex crash): ${codex.text.slice(0, 200)}`,
        raw: codex.text,
      });
      continue;
    }

    judgements.push(parseJudgement(text, refute.persona));
  }

  const all_overridable = judgements.every(j => !j.keep_refute);
  const result: RefuteOverrideResult = { judgements, all_overridable, reviewer, fallback_used };
  writeFileSync(`${outDir}/refute-override.result.json`, JSON.stringify(result, null, 2), "utf-8");
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
  const verdictsPath = arg("--verdicts");
  const outDir = arg("--out-dir");
  const dryRun = flag("--dry-run");
  if (!adrPath || !verdictsPath) {
    console.error("Usage: loop-adr-refute-overrider.ts --adr <path> --verdicts <verdicts.json> [--out-dir <dir>] [--dry-run]");
    process.exit(1);
  }
  if (!existsSync(adrPath) || !existsSync(verdictsPath)) {
    console.error(`ERROR: --adr or --verdicts file not found`);
    process.exit(1);
  }
  const raw = readFileSync(verdictsPath, "utf-8");
  const wrapper = JSON.parse(raw);
  const allVerdicts: PersonaVerdict[] = Array.isArray(wrapper) ? wrapper : wrapper.verdicts;
  const refutes = allVerdicts.filter(v => !v.approved);
  if (refutes.length === 0) {
    console.error("No refutes to judge");
    process.exit(0);
  }
  const result = await overrideRefutes({ adrPath, refutes, outDir, dryRun });
  console.log(JSON.stringify(result, null, 2));
  if (result.all_overridable) {
    console.error(`OVERRIDABLE: 全 refute override 可能 (accept ルート昇格推奨, reviewer=${result.reviewer})`);
    process.exit(0);
  }
  console.error(`KEEP: refute 維持 (regen ルート、reviewer=${result.reviewer})`);
  for (const j of result.judgements) {
    console.error(`  - ${j.persona}: ${j.keep_refute ? "keep" : "override"} — ${j.reasoning.slice(0, 100)}`);
  }
  process.exit(2);
}
