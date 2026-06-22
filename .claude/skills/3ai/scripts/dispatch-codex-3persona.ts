#!/usr/bin/env bun
// dispatch-codex-3persona.ts — STEP 7.5-B 並列 review (#231) + GLM fallback (#281)
//
// Codex 3 ペルソナ (architect / contrarian / migration) を Promise.all で並列 spawn し、
// 各 persona の verdict.json を読んでマージ verdict を生成する。
//
// #281: Codex 3 persona 全員 exitCode≠0 (= usage limit / crash) のときは GLM (Z.AI 経由
// claude -p) に fallback して同じ prompt で 3 persona を再 dispatch する。merged yaml と
// .verdict.json の `fallback_used: true` で fallback 経路を識別できる。
//
// 入出力:
//   入力: --instruction <agent.md> --result <base.yaml> [--extra-input <md>] [--scope-hint <text>] [--base <branch>]
//   出力 per persona: <base-without-ext>-<persona>.yaml + .verdict.json + .log
//   出力 merged: <base>.yaml (各 persona の issues を id プレフィックス付きで concat) + <base>.yaml.verdict.json
//   fallback 時: 元 Codex の PersonaResult を <base>.yaml.codex.json に保全
//
// テスト用 mockMode (--mock-mode):
//   pass    全 persona verdict=pass, blocking=0
//   fail    全 persona verdict=fail, critical=1 each
//   mixed   architect=pass, contrarian=fail (critical=1), migration=pass
//
// テスト用 環境変数 (#281 GLM fallback E2E mock):
//   DISPATCH_CODEX_3P_MOCK_CODEX=usage-limit  Codex 3 persona 全員を exit=1 (usage limit) で強制
//   DISPATCH_CODEX_3P_MOCK_GLM=pass|fail|mixed  fallback 発火時の GLM 結果を強制

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname } from "path";
import { buildPrefix, dispatchCodex, parseVerdict, type CodexPersona } from "./dispatch-codex.ts";
import { runGlmViaZAI } from "./glm-via-zai.ts";

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
  /** #281: GLM 3 persona fallback ルートに乗ったか。Codex 全員 fail → GLM 3 persona 再 dispatch のとき true。 */
  fallback_used: boolean;
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

export function mergePersonaResults(
  results: PersonaResult[],
  fallbackUsed = false,
): MergedVerdict {
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
  const isAllUnknownFail = allUnknown && results.length > 0;
  const verdict: "pass" | "fail" =
    isAllUnknownFail ? "fail" : blockingSum > 0 ? "fail" : "pass";
  // codex review r2 F-arch-01 / F-cont-01: 全 unknown 時は blocking==0 のまま fail を返すと
  // 下流 (`blocking == 0` だけを見る) が silent pass する事故が起きるため、sentinel として
  // blocking=1 を立てる (severity_counts は 0 のままで、人間が「all-unknown failure」と判別可能)。
  const finalBlocking = isAllUnknownFail ? Math.max(blockingSum, 1) : blockingSum;
  return {
    verdict,
    severity_counts: acc,
    blocking: finalBlocking,
    per_persona: results,
    fallback_used: fallbackUsed,
  };
}

/** #281: Codex 3 persona の run すべてが exitCode≠0 (= crash / usage limit / 内部エラー) なら true。
 *  GLM 3 persona fallback の発動条件。空配列は false (= 何も実行されていない)。 */
export function isAllCodexFailedFor3p(runs: PersonaResult[]): boolean {
  if (runs.length === 0) return false;
  return runs.every(r => r.exitCode !== 0);
}

function buildMergedYaml(results: PersonaResult[], merged: MergedVerdict): string {
  const lines: string[] = [];
  const header = merged.fallback_used
    ? "# Merged GLM review (3 persona, #281 — Codex usage limit fallback)"
    : "# Merged Codex review (3 persona parallel, #231)";
  lines.push(header);
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
    lines.push(`fallback_used: ${merged.fallback_used}`);
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
      // codex review r2 F-arch-02 / F-mig-01: per-persona の元インデントを保持し reformat しない。
      // 元 yaml は `  - id:` で始まる 2 space indent + `    field:` の構造なので、
      // 行 1 の id だけ書き換え、それ以降は trim せずそのまま転記する。
      // これにより `finding: |` 形式の block scalar や複数行 string も保全される。
      const lines2 = block.split("\n");
      const idLine = lines2[0];
      const idMatch = idLine.match(/id:\s*(\S+)/);
      const newId = idMatch ? `${prefix}-${idMatch[1]}` : `${prefix}-?`;
      lines2[0] = idLine.replace(/id:\s*\S+/, `id: ${newId}`);
      // 先頭行が `  - id:` でない場合のみ 2 space indent を補完 (常識的フォーマット維持)
      if (!/^\s*-\s*/.test(lines2[0])) lines2[0] = "  - " + lines2[0].trimStart();
      lines.push(lines2.join("\n").replace(/\n+$/, ""));
    }
  }
  lines.push("");
  lines.push(`verdict: ${merged.verdict}`);
  lines.push(`fallback_used: ${merged.fallback_used}`);
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

/** #281 mock: Codex 3 persona を強制的に usage-limit fail にする (env: DISPATCH_CODEX_3P_MOCK_CODEX=usage-limit)。
 *  log に "hit your usage limit (mock)" を書き、yaml と .verdict.json は書かず exit=1 を返す。 */
function mockCodexUsageLimitResult(
  persona: Exclude<CodexPersona, "single">,
  yamlPath: string,
): PersonaResult {
  mkdirSync(dirname(yamlPath), { recursive: true });
  writeFileSync(`${yamlPath}.log`, `[mock] codex exit=1 hit your usage limit (mock) for ${persona}\n`, "utf-8");
  return {
    persona,
    exitCode: 1,
    yamlPath,
    verdict: "unknown" as const,
    severity_counts: { critical: 0, high: 0, medium: 0, low: 0 },
    blocking: 0,
  };
}

/** #281 mock: GLM fallback を強制結果にする (env: DISPATCH_CODEX_3P_MOCK_GLM=pass|fail|mixed)。
 *  mixed は architect=pass, contrarian=fail, migration=pass。 */
function mockGlmFallbackResult(
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
    ? `issues:\n  - id: G01\n    severity: critical\n    file: "mock"\n    finding: "mock glm fail for ${persona}"\n\nverdict: ${verdict}\n`
    : `issues: []\nverdict: ${verdict}\n`;
  mkdirSync(dirname(yamlPath), { recursive: true });
  writeFileSync(yamlPath, yamlBody, "utf-8");
  writeFileSync(`${yamlPath}.verdict.json`, JSON.stringify({ verdict, severity_counts: counts, blocking: counts.critical + counts.high }), "utf-8");
  writeFileSync(`${yamlPath}.glm.log`, `[mock] glm ${mode} for ${persona}\n`, "utf-8");
  return {
    persona,
    exitCode: 0,
    yamlPath,
    verdict,
    severity_counts: counts,
    blocking: counts.critical + counts.high,
  };
}

/** #281: GLM (Z.AI 経由 claude -p) で 1 persona の review を実行し PersonaResult を返す。
 *  Codex 3 persona 全員 fail のときに dispatchCodex3Persona から呼ばれる fallback runner。
 *  prompt = instructionFile 本体 + buildPrefix (persona hint + scope + extraInputFile) + git diff。 */
async function runGlmPersonaFor3p(
  persona: Exclude<CodexPersona, "single">,
  opts: Persona3Opts,
  diff: string,
  yamlPath: string,
): Promise<PersonaResult> {
  // mock: env で強制
  const glmMock = process.env.DISPATCH_CODEX_3P_MOCK_GLM;
  if (glmMock === "pass" || glmMock === "fail" || glmMock === "mixed") {
    return mockGlmFallbackResult(persona, yamlPath, glmMock);
  }

  const instr = readFileSync(opts.instructionFile, "utf-8");
  const prefix = buildPrefix(opts.scopeHint ?? "", opts.extraInputFile ?? "", persona);
  const prompt = `${instr}\n\n${prefix}${diff}`;

  mkdirSync(dirname(yamlPath), { recursive: true });
  const r = await runGlmViaZAI({ prompt });
  writeFileSync(`${yamlPath}.glm.log`, r.stdout + r.stderr, "utf-8");

  if (!r.ok) {
    const msg = r.error ?? `[glm exit=${r.exitCode}] ${r.stderr.trim().slice(-300) || "(no stderr)"}`;
    process.stderr.write(`WARN: glm fallback failed for persona=${persona}: ${msg}\n`);
    return {
      persona,
      exitCode: r.exitCode === 0 ? 1 : r.exitCode,
      yamlPath,
      verdict: "unknown" as const,
      severity_counts: { critical: 0, high: 0, medium: 0, low: 0 },
      blocking: 0,
    };
  }

  const text = r.result;
  writeFileSync(yamlPath, text, "utf-8");
  const parsed = parseVerdict(text);
  const counts = {
    critical: parsed.severity_counts.critical ?? 0,
    high: parsed.severity_counts.high ?? 0,
    medium: parsed.severity_counts.medium ?? 0,
    low: parsed.severity_counts.low ?? 0,
  };
  const verdict: "pass" | "fail" | "unknown" =
    parsed.verdict === "pass" || parsed.verdict === "fail" ? parsed.verdict : "unknown";
  writeFileSync(`${yamlPath}.verdict.json`, JSON.stringify({ verdict, severity_counts: counts, blocking: parsed.blocking }), "utf-8");
  return {
    persona,
    exitCode: 0,
    yamlPath,
    verdict,
    severity_counts: counts,
    blocking: parsed.blocking,
  };
}

/** GLM fallback 時に必要な git diff を 1 度だけ取る。baseBranch 未指定なら origin/HEAD から検出。 */
async function getDiffForFallback(baseBranch: string): Promise<string> {
  const proc = Bun.spawn(["git", "diff", `${baseBranch}...HEAD`], { stdout: "pipe", stderr: "pipe" });
  const text = await new Response(proc.stdout).text();
  await proc.exited;
  return text;
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
  const codexMockEnv = process.env.DISPATCH_CODEX_3P_MOCK_CODEX;

  let runs = await Promise.all(
    PERSONAS.map(async (persona): Promise<PersonaResult> => {
      const yamlPath = personaResultPath(resultFile, persona);
      if (mock) {
        return mockPersonaResult(persona, yamlPath, mock);
      }
      // #281 mock: Codex 3 persona を強制 usage-limit fail で fallback ルートに乗せる
      if (codexMockEnv === "usage-limit") {
        return mockCodexUsageLimitResult(persona, yamlPath);
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

  // #281: Codex 3 persona すべて exitCode≠0 (= usage limit / crash) なら GLM 3 persona に fallback。
  // mockMode (--mock-mode) 指定時は fallback しない (既存テスト互換)。
  let fallbackUsed = false;
  if (!mock && isAllCodexFailedFor3p(runs)) {
    process.stderr.write(`WARN: Codex 3 personas all failed (usage limit likely) — falling back to GLM (#281)\n`);
    // GLM fallback 用 diff: mock 時は空文字 (実 git は呼ばない)、実走時は base...HEAD
    let diff = "";
    if (process.env.DISPATCH_CODEX_3P_MOCK_GLM === undefined) {
      let baseBranch = opts.baseBranch;
      if (!baseBranch) {
        const proc = Bun.spawn(["git", "symbolic-ref", "refs/remotes/origin/HEAD"], { stdout: "pipe", stderr: "pipe" });
        const out = (await new Response(proc.stdout).text()).trim().replace("refs/remotes/origin/", "");
        await proc.exited;
        baseBranch = out || "main";
      }
      diff = await getDiffForFallback(baseBranch);
    }
    // 元 Codex 結果を traceability のため保全
    writeFileSync(`${resultFile}.codex.json`, JSON.stringify(runs, null, 2), "utf-8");
    const glmRuns = await Promise.all(
      PERSONAS.map(p => runGlmPersonaFor3p(p, opts, diff, personaResultPath(resultFile, p))),
    );
    runs = glmRuns;
    fallbackUsed = true;
  }

  const merged = mergePersonaResults(runs, fallbackUsed);
  const mergedYaml = buildMergedYaml(runs, merged);
  writeFileSync(resultFile, mergedYaml, "utf-8");
  writeFileSync(
    `${resultFile}.verdict.json`,
    JSON.stringify({
      verdict: merged.verdict,
      severity_counts: merged.severity_counts,
      blocking: merged.blocking,
      fallback_used: merged.fallback_used,
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
