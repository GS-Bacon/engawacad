#!/usr/bin/env bun
// dispatch-codex-3persona.ts — STEP 7.5-B 並列 review (#231)
//
// Codex 3 ペルソナ (architect / contrarian / migration) を Promise.all で並列 spawn し、
// 各 persona の verdict.json を読んでマージ verdict を生成する。
//
// 入出力:
//   入力: --instruction <agent.md> --result <base.yaml> [--extra-input <md>] [--scope-hint <text>] [--base <branch>]
//   出力 per persona: <base-without-ext>-<persona>.yaml + .verdict.json + .log
//   出力 merged: <base>.yaml (各 persona の issues を id プレフィックス付きで concat) + <base>.yaml.verdict.json
//
// テスト用 mockMode:
//   pass    全 persona verdict=pass, blocking=0
//   fail    全 persona verdict=fail, critical=1 each
//   mixed   architect=pass, contrarian=fail (critical=1), migration=pass

import { existsSync, readFileSync, writeFileSync } from "fs";
import { dirname } from "path";
import { dispatchCodex, type CodexPersona } from "./dispatch-codex.ts";

export const PERSONAS: readonly Exclude<CodexPersona, "single">[] = [
  "architect",
  "contrarian",
  "migration",
];

export type Persona3MockMode = "pass" | "fail" | "mixed";

export interface Persona3Opts {
  instructionFile: string;
  resultFile: string;
  extraInputFile?: string;
  scopeHint?: string;
  baseBranch?: string;
  mockMode?: Persona3MockMode;
}

export type PersonaResult = {
  persona: Exclude<CodexPersona, "single">;
  exitCode: number;
  yamlPath: string;
  verdict: "pass" | "fail" | "unknown";
  severity_counts: { critical: number; high: number; medium: number; low: number };
  blocking: number;
};

export type MergedVerdict = {
  verdict: "pass" | "fail";
  severity_counts: { critical: number; high: number; medium: number; low: number };
  blocking: number;
  per_persona: PersonaResult[];
};

function personaResultPath(resultFile: string, persona: string): string {
  const idx = resultFile.lastIndexOf(".");
  if (idx <= dirname(resultFile).length) {
    return `${resultFile}-${persona}`;
  }
  const base = resultFile.slice(0, idx);
  const ext = resultFile.slice(idx);
  return `${base}-${persona}${ext}`;
}

function readVerdictJson(yamlPath: string): {
  verdict: "pass" | "fail" | "unknown";
  severity_counts: { critical: number; high: number; medium: number; low: number };
  blocking: number;
} {
  const path = `${yamlPath}.verdict.json`;
  if (!existsSync(path)) {
    return { verdict: "unknown", severity_counts: { critical: 0, high: 0, medium: 0, low: 0 }, blocking: 0 };
  }
  try {
    const raw = JSON.parse(readFileSync(path, "utf-8"));
    const sc = raw.severity_counts ?? { critical: 0, high: 0, medium: 0, low: 0 };
    return {
      verdict: raw.verdict === "pass" || raw.verdict === "fail" ? raw.verdict : "unknown",
      severity_counts: {
        critical: sc.critical ?? 0,
        high: sc.high ?? 0,
        medium: sc.medium ?? 0,
        low: sc.low ?? 0,
      },
      blocking: typeof raw.blocking === "number" && raw.blocking >= 0 ? raw.blocking : 0,
    };
  } catch {
    return { verdict: "unknown", severity_counts: { critical: 0, high: 0, medium: 0, low: 0 }, blocking: 0 };
  }
}

export function mergePersonaResults(results: PersonaResult[]): MergedVerdict {
  const acc = { critical: 0, high: 0, medium: 0, low: 0 };
  let blockingSum = 0;
  let allUnknown = true;
  for (const r of results) {
    acc.critical += r.severity_counts.critical;
    acc.high += r.severity_counts.high;
    acc.medium += r.severity_counts.medium;
    acc.low += r.severity_counts.low;
    blockingSum += r.blocking;
    if (r.verdict !== "unknown") allUnknown = false;
  }
  const verdict: "pass" | "fail" =
    allUnknown && results.length > 0 ? "fail" : blockingSum > 0 ? "fail" : "pass";
  return {
    verdict,
    severity_counts: acc,
    blocking: blockingSum,
    per_persona: results,
  };
}

function buildMergedYaml(results: PersonaResult[], merged: MergedVerdict): string {
  const lines: string[] = [];
  lines.push("# Merged Codex review (3 persona parallel, #231)");
  // 全 persona が issues=[] のときは厳密に YAML 配列 [] を出力 (codex review F-mig-02:
  // 後方互換のため `issues:` のキー値型が null ではなく [] であること)
  const totalIssueBlocks = results.reduce((sum, r) => {
    const t = existsSync(r.yamlPath) ? readFileSync(r.yamlPath, "utf-8") : "";
    return sum + extractIssueBlocks(t).length;
  }, 0);
  if (totalIssueBlocks === 0) {
    lines.push("issues: []");
    for (const r of results) {
      lines.push(`# [${r.persona}] issues: [] (verdict=${r.verdict})`);
    }
    lines.push("");
    lines.push(`verdict: ${merged.verdict}`);
    lines.push(`# severity_counts: critical=${merged.severity_counts.critical}, high=${merged.severity_counts.high}, medium=${merged.severity_counts.medium}, low=${merged.severity_counts.low}`);
    lines.push(`# blocking: ${merged.blocking}`);
    return lines.join("\n") + "\n";
  }
  lines.push("issues:");
  for (const r of results) {
    const prefix = r.persona[0].toUpperCase();
    const yamlText = existsSync(r.yamlPath) ? readFileSync(r.yamlPath, "utf-8") : "";
    const issueBlocks = extractIssueBlocks(yamlText);
    if (issueBlocks.length === 0) {
      lines.push(`  # [${r.persona}] issues: [] (verdict=${r.verdict})`);
      continue;
    }
    for (const block of issueBlocks) {
      const lines2 = block.split("\n");
      const idLine = lines2[0];
      const idMatch = idLine.match(/id:\s*(\S+)/);
      const newId = idMatch ? `${prefix}-${idMatch[1]}` : `${prefix}-?`;
      lines2[0] = idLine.replace(/id:\s*\S+/, `id: ${newId}`);
      lines2[0] = lines2[0].replace(/^\s*-\s*/, "  - ");
      for (let i = 1; i < lines2.length; i++) {
        if (lines2[i].trim()) lines2[i] = "    " + lines2[i].trim();
      }
      lines.push(lines2.filter(l => l.length > 0).join("\n"));
    }
  }
  lines.push("");
  lines.push(`verdict: ${merged.verdict}`);
  lines.push(`# severity_counts: critical=${merged.severity_counts.critical}, high=${merged.severity_counts.high}, medium=${merged.severity_counts.medium}, low=${merged.severity_counts.low}`);
  lines.push(`# blocking: ${merged.blocking}`);
  return lines.join("\n") + "\n";
}

function extractIssueBlocks(yamlText: string): string[] {
  const blocks: string[] = [];
  const lines = yamlText.split("\n");
  let inIssues = false;
  let current: string[] = [];
  for (const line of lines) {
    if (/^issues:\s*$/.test(line)) {
      inIssues = true;
      continue;
    }
    if (inIssues) {
      if (/^[a-zA-Z]/.test(line)) {
        if (current.length > 0) blocks.push(current.join("\n"));
        current = [];
        inIssues = false;
        continue;
      }
      const isItemStart = /^\s*-\s*id:/.test(line);
      if (isItemStart) {
        if (current.length > 0) blocks.push(current.join("\n"));
        current = [line];
      } else if (current.length > 0) {
        current.push(line);
      }
    }
  }
  if (current.length > 0) blocks.push(current.join("\n"));
  return blocks;
}

function mockPersonaResult(
  persona: Exclude<CodexPersona, "single">,
  yamlPath: string,
  mode: Persona3MockMode,
): PersonaResult {
  let isFail = false;
  if (mode === "fail") isFail = true;
  else if (mode === "mixed" && persona === "contrarian") isFail = true;
  const counts = { critical: 0, high: 0, medium: 0, low: 0 };
  if (isFail) counts.critical = 1;
  const verdict: "pass" | "fail" = isFail ? "fail" : "pass";
  const yamlBody = isFail
    ? `issues:\n  - id: F01\n    severity: critical\n    file: "mock"\n    finding: "mock fail for ${persona}"\n\nverdict: ${verdict}\n`
    : `issues: []\nverdict: ${verdict}\n`;
  writeFileSync(yamlPath, yamlBody, "utf-8");
  writeFileSync(`${yamlPath}.verdict.json`, JSON.stringify({ verdict, severity_counts: counts, blocking: counts.critical + counts.high }), "utf-8");
  return {
    persona,
    exitCode: 0,
    yamlPath,
    verdict,
    severity_counts: counts,
    blocking: counts.critical + counts.high,
  };
}

export async function dispatchCodex3Persona(opts: Persona3Opts): Promise<MergedVerdict> {
  const { resultFile, instructionFile } = opts;
  const mock = opts.mockMode;

  const runs = await Promise.all(
    PERSONAS.map(async (persona): Promise<PersonaResult> => {
      const yamlPath = personaResultPath(resultFile, persona);
      if (mock) {
        return mockPersonaResult(persona, yamlPath, mock);
      }
      let exitCode = -1;
      try {
        exitCode = await dispatchCodex({
          mode: "review",
          instructionFile,
          resultFile: yamlPath,
          extraInputFile: opts.extraInputFile,
          scopeHint: opts.scopeHint,
          baseBranch: opts.baseBranch,
          persona,
        });
      } catch (err) {
        process.stderr.write(`[${persona}] dispatchCodex threw: ${err instanceof Error ? err.message : String(err)}\n`);
      }
      // codex review F-arch-02 / F-cont-02 / F-mig-01: Codex CLI が非 0 で落ちた場合は
      // verdict.json (前回 run の残骸 or 自動生成された unknown) を信用せず unknown 扱いにする。
      // mergePersonaResults は全 unknown を fail に倒すため silent fail が発生しない。
      if (exitCode !== 0) {
        return {
          persona,
          exitCode,
          yamlPath,
          verdict: "unknown" as const,
          severity_counts: { critical: 0, high: 0, medium: 0, low: 0 },
          blocking: 0,
        };
      }
      const v = readVerdictJson(yamlPath);
      return {
        persona,
        exitCode,
        yamlPath,
        verdict: v.verdict,
        severity_counts: v.severity_counts,
        blocking: v.blocking,
      };
    }),
  );

  const merged = mergePersonaResults(runs);
  const mergedYaml = buildMergedYaml(runs, merged);
  writeFileSync(resultFile, mergedYaml, "utf-8");
  writeFileSync(
    `${resultFile}.verdict.json`,
    JSON.stringify({
      verdict: merged.verdict,
      severity_counts: merged.severity_counts,
      blocking: merged.blocking,
      per_persona: merged.per_persona.map(p => ({
        persona: p.persona,
        verdict: p.verdict,
        severity_counts: p.severity_counts,
        blocking: p.blocking,
        yamlPath: p.yamlPath,
      })),
    }),
    "utf-8",
  );
  return merged;
}

if (import.meta.main) {
  let instructionFile = "";
  let resultFile = "";
  let extraInputFile: string | undefined;
  let scopeHint: string | undefined;
  let baseBranch: string | undefined;
  let mockMode: Persona3MockMode | undefined;

  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--instruction") instructionFile = args[++i];
    else if (args[i] === "--result") resultFile = args[++i];
    else if (args[i] === "--extra-input") extraInputFile = args[++i];
    else if (args[i] === "--scope-hint") scopeHint = args[++i];
    else if (args[i] === "--base") baseBranch = args[++i];
    else if (args[i] === "--mock-mode") {
      const m = args[++i];
      if (m !== "pass" && m !== "fail" && m !== "mixed") {
        console.error(`--mock-mode must be pass|fail|mixed, got: ${m}`);
        process.exit(1);
      }
      mockMode = m;
    }
    else { console.error(`Unknown arg: ${args[i]}`); process.exit(1); }
  }

  if (!instructionFile || !resultFile) {
    console.error("ERROR: --instruction and --result are required");
    process.exit(1);
  }

  const merged = await dispatchCodex3Persona({
    instructionFile,
    resultFile,
    extraInputFile,
    scopeHint,
    baseBranch,
    mockMode,
  });
  process.stderr.write(`=== dispatch-codex-3persona: merged verdict=${merged.verdict}, blocking=${merged.blocking} ===\n`);
  // codex review F-arch-01 / F-cont-01: silent fail 防止のため verdict=fail なら必ず非 0 exit。
  // blocking==0 でも 全 persona unknown (= fail) のときは fail 扱いになる。
  process.exit(merged.verdict === "fail" ? 1 : 0);
}
