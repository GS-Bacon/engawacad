#!/usr/bin/env bun
// loop-adr-auto-accept.ts — ADR 自動 accept フロー (ADR-013)
//
// 動作:
//   1. Decision Matrix lint (loop-adr-decision-matrix-lint)
//      → fail なら incRegen → cap 越えで needs-human 退避 / 未 cap で exit 2 (再生成要求)
//   2. pass なら Codex 3 ペルソナ (architect / contrarian / migration) で adversarial review
//      → 全員 approved なら gate:adr-review 削除 + Issue close (= auto-accept)
//      → 1 つでも refute なら incRegen → cap 越えで needs-human 退避 / 未 cap で exit 2
//
// 使い方:
//   bun loop-adr-auto-accept.ts --adr <path-to-adr.md> --issue <gh-issue-num> [--dry-run] [--mock-review pass|refute]
//
// 環境変数:
//   CODEX_DRY_RUN=1   Codex を呼ばず stub (常に approved を返す)
//   ADR_AUTOACCEPT_MOCK=pass|refute  テスト用に review 結果を強制
//
// exit code:
//   0 = auto-accept 成功 (gate 削除 + Issue close)
//   2 = 再生成要求 (cap 未越え、呼び元が draft 再生成すべき)
//   3 = needs-human 退避 (cap 越え)
//   1 = 内部エラー

import { existsSync, readFileSync, writeFileSync, mkdirSync } from "fs";
import { basename } from "path";
import { lintAdr } from "./loop-adr-decision-matrix-lint";
import { addTokens, incRegen, retire } from "./loop-adr-regen-tracker";
import { runChecked } from "./loop-spawn-checked.ts";
import { detectCodexUsageLimit } from "../../3ai/scripts/dispatch-codex.ts";
import { runGlmViaZAI } from "../../3ai/scripts/glm-via-zai.ts";

const PERSONAS = [
  {
    key: "architect",
    role: "ADR architect (過去 ADR との整合 / 設計階層の安定性で refute せよ)",
  },
  {
    key: "contrarian",
    role: "ADR contrarian (採用 option を refute し、却下 option の利点を強調せよ)",
  },
  {
    key: "migration",
    role: "ADR migration reviewer (既存実装からの移行容易性 / 後方互換性で refute せよ)",
  },
] as const;

export type PersonaVerdict = {
  persona: string;
  approved: boolean;
  raw_excerpt: string;
};

/** #251: 3 verdict すべてが Codex 起因の fail (crash or usage limit) なら true。
 *  GLM fallback の発動条件。raw_excerpt の prefix と detectCodexUsageLimit で判定する。 */
export function isAllCodexFailed(verdicts: PersonaVerdict[]): boolean {
  if (verdicts.length === 0) return false;
  return verdicts.every(v =>
    !v.approved && (v.raw_excerpt.startsWith("[codex exit=") || detectCodexUsageLimit(v.raw_excerpt))
  );
}

async function runGh(args: string[]): Promise<{ stdout: string; exit: number }> {
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return { stdout: out.trim(), exit: proc.exitCode ?? 0 };
}

function adrSlug(path: string): string {
  return basename(path).replace(/\.md$/, "");
}

function buildPersonaPrompt(persona: typeof PERSONAS[number], adrText: string): string {
  return [
    `# Role: ${persona.role}`,
    "",
    "## Task",
    "以下の ADR draft を adversarial に review してください。",
    "REFUTE を default とし、明確に refute できなければ approved を返してください。",
    "理由が浅い (1 文以下、根拠なし) refute は失敗判定とし approved を返してください。",
    "",
    "## 出力フォーマット (必須)",
    "最終行に必ず以下のいずれかを記載:",
    "  verdict: approved",
    "  verdict: refuted",
    "",
    "refuted の場合、その直前に 200 字以上の refute 理由を記載してください。",
    "",
    "## ADR draft",
    "",
    adrText,
  ].join("\n");
}

async function runPersonaReview(
  persona: typeof PERSONAS[number],
  adrText: string,
  outDir: string,
): Promise<PersonaVerdict> {
  // mock 経路 (テスト・dry-run 用)
  const mock = process.env.ADR_AUTOACCEPT_MOCK;
  if (mock === "pass") {
    return { persona: persona.key, approved: true, raw_excerpt: "[mock:pass]" };
  }
  if (mock === "refute") {
    return { persona: persona.key, approved: false, raw_excerpt: "[mock:refute]" };
  }
  // #251: Codex usage limit fallback テスト用。GLM fallback を発火させる。
  if (process.env.ADR_AUTOACCEPT_MOCK_CODEX === "fail") {
    return { persona: persona.key, approved: false, raw_excerpt: "[codex exit=1] hit your usage limit (mock)" };
  }
  if (process.env.CODEX_DRY_RUN === "1") {
    return { persona: persona.key, approved: true, raw_excerpt: "[CODEX_DRY_RUN]" };
  }

  const instructionFile = `${outDir}/persona-${persona.key}.instruction.md`;
  const resultFile = `${outDir}/persona-${persona.key}.result.md`;
  const prompt = buildPersonaPrompt(persona, adrText);
  mkdirSync(outDir, { recursive: true });
  writeFileSync(instructionFile, prompt, "utf-8");

  // Codex CLI を直接呼ぶ (dispatch-codex.ts は git diff を要求するためここでは独自呼び出し)
  // #227: runChecked allowFailure=true で exit code を捕捉、crash 時は exit/stderr を log + raw_excerpt に明示。
  const r = await runChecked(
    ["codex", "exec", "-c", "sandbox_mode=read-only", "--output-last-message", resultFile, prompt],
    { allowFailure: true },
  );
  writeFileSync(`${resultFile}.log`, r.stdout + r.stderr, "utf-8");

  // codex が crash した (= resultFile が書かれなかった or 空) 場合、persona を "failed" として扱い
  // 後段の judgement に「unknown verdict」を伝える。silent fail で stale resultFile を読まない。
  if (r.exitCode !== 0) {
    const errSnippet = r.stderr.trim().slice(-300) || "(no stderr)";
    process.stderr.write(`WARN: codex exec exit=${r.exitCode} for persona=${persona.key}: ${errSnippet}\n`);
    return {
      persona: persona.key,
      approved: false,
      raw_excerpt: `[codex exit=${r.exitCode}] ${errSnippet}`,
    };
  }

  let text = "";
  try { text = readFileSync(resultFile, "utf-8"); } catch { text = r.stdout; }
  const verdictMatch = text.match(/verdict:\s*(approved|refuted)/i);
  const verdict = verdictMatch?.[1].toLowerCase() ?? "unknown";
  return {
    persona: persona.key,
    approved: verdict === "approved",
    raw_excerpt: text.slice(0, 200),
  };
}

/** #251: GLM (Z.AI 経由 claude -p) で persona review を実行する fallback ルート。
 *  Codex usage limit 時に runPersonaReview の代わりに呼ばれる。
 *  Z.AI 呼び出しの低レベル部分は glm-via-zai.ts に共通化 (#281)。 */
async function runPersonaReviewViaGlm(
  persona: typeof PERSONAS[number],
  adrText: string,
  outDir: string,
): Promise<PersonaVerdict> {
  if (process.env.ADR_AUTOACCEPT_MOCK === "glm-pass") {
    return { persona: persona.key, approved: true, raw_excerpt: "[mock:glm-pass]" };
  }
  if (process.env.ADR_AUTOACCEPT_MOCK === "glm-refute") {
    return { persona: persona.key, approved: false, raw_excerpt: "[mock:glm-refute]" };
  }

  const instructionFile = `${outDir}/persona-${persona.key}.glm-instruction.md`;
  const resultFile = `${outDir}/persona-${persona.key}.glm-result.md`;
  const prompt = buildPersonaPrompt(persona, adrText);
  mkdirSync(outDir, { recursive: true });
  writeFileSync(instructionFile, prompt, "utf-8");

  const r = await runGlmViaZAI({ prompt });
  writeFileSync(`${resultFile}.raw`, r.stdout + r.stderr, "utf-8");

  if (!r.ok) {
    if (r.error) {
      return {
        persona: persona.key,
        approved: false,
        raw_excerpt: `[glm: ${r.error}]`,
      };
    }
    const errSnippet = r.stderr.trim().slice(-300) || "(no stderr)";
    process.stderr.write(`WARN: glm exec exit=${r.exitCode} for persona=${persona.key}: ${errSnippet}\n`);
    return {
      persona: persona.key,
      approved: false,
      raw_excerpt: `[glm exit=${r.exitCode}] ${errSnippet}`,
    };
  }

  const text = r.result || r.stdout;
  writeFileSync(resultFile, text, "utf-8");

  const verdictMatch = text.match(/verdict:\s*(approved|refuted)/i);
  const verdict = verdictMatch?.[1].toLowerCase() ?? "unknown";
  return {
    persona: persona.key,
    approved: verdict === "approved",
    raw_excerpt: text.slice(0, 200),
  };
}

export type AutoAcceptOutcome =
  | { kind: "accepted"; verdicts: PersonaVerdict[] }
  | { kind: "regen_required"; reason: "lint" | "review"; details: string[]; regen_count: number }
  | { kind: "retired"; reason: "regen_cap" | "token_cap"; regen_count: number; token_used: number };

export type AutoAcceptOpts = {
  adrPath: string;
  issueNum: number;
  outDir?: string;
  dryRun?: boolean;
  estimatedReviewTokens?: number;
};

export async function autoAccept(opts: AutoAcceptOpts): Promise<AutoAcceptOutcome> {
  const slug = adrSlug(opts.adrPath);
  const outDir = opts.outDir ?? `features/.loop/adr-review/${slug}`;
  const adrText = readFileSync(opts.adrPath, "utf-8");

  // Step 1: Decision Matrix lint
  const lintResult = lintAdr(adrText);
  if (!lintResult.ok) {
    const inc = incRegen(slug);
    if (inc.capped) {
      if (!opts.dryRun) await retire(slug, opts.issueNum, "regen_cap");
      return { kind: "retired", reason: "regen_cap", regen_count: inc.state.regen_count, token_used: inc.state.token_used };
    }
    return {
      kind: "regen_required",
      reason: "lint",
      details: lintResult.issues.map(i => `[${i.rule}] ${i.message}`),
      regen_count: inc.state.regen_count,
    };
  }

  // Step 2: token 予算チェック (review 起動前の予算予約)
  const estimatedTokens = opts.estimatedReviewTokens ?? 30_000 * PERSONAS.length;
  const tok = addTokens(slug, estimatedTokens);
  if (tok.capped) {
    if (!opts.dryRun) await retire(slug, opts.issueNum, "token_cap");
    return { kind: "retired", reason: "token_cap", regen_count: tok.state.regen_count, token_used: tok.state.token_used };
  }

  // Step 3: Multi-LLM Adversarial Review (3 ペルソナ並列)
  if (opts.dryRun) {
    return { kind: "accepted", verdicts: PERSONAS.map(p => ({ persona: p.key, approved: true, raw_excerpt: "[dry-run]" })) };
  }
  const codexVerdicts = await Promise.all(PERSONAS.map(p => runPersonaReview(p, adrText, outDir)));
  mkdirSync(outDir, { recursive: true });
  writeFileSync(`${outDir}/verdicts.codex.json`, JSON.stringify(codexVerdicts, null, 2), "utf-8");

  // #251: Codex 3 persona すべて usage limit / crash で fail なら GLM 3 persona に fallback
  let verdicts = codexVerdicts;
  let fallbackUsed = false;
  if (isAllCodexFailed(codexVerdicts)) {
    process.stderr.write(`WARN: Codex 3 personas all failed (usage limit likely) — falling back to GLM (#251)\n`);
    const glmVerdicts = await Promise.all(PERSONAS.map(p => runPersonaReviewViaGlm(p, adrText, outDir)));
    writeFileSync(`${outDir}/verdicts.glm.json`, JSON.stringify(glmVerdicts, null, 2), "utf-8");
    verdicts = glmVerdicts;
    fallbackUsed = true;
  }
  writeFileSync(`${outDir}/verdicts.json`, JSON.stringify({
    fallback_used: fallbackUsed,
    verdicts,
  }, null, 2), "utf-8");

  const refuted = verdicts.filter(v => !v.approved);
  if (refuted.length > 0) {
    const inc = incRegen(slug);
    if (inc.capped) {
      await retire(slug, opts.issueNum, "regen_cap");
      return { kind: "retired", reason: "regen_cap", regen_count: inc.state.regen_count, token_used: inc.state.token_used };
    }
    return {
      kind: "regen_required",
      reason: "review",
      details: refuted.map(v => `${v.persona}: refuted (${v.raw_excerpt.slice(0, 80)}...)`),
      regen_count: inc.state.regen_count,
    };
  }

  // Step 4: auto-accept (gate ラベル削除 + Issue close)
  await runGh(["issue", "edit", String(opts.issueNum), "--remove-label", "gate:adr-review"]);
  const reviewer = fallbackUsed ? "GLM (Codex usage limit fallback, #251)" : "Codex";
  await runGh(["issue", "close", String(opts.issueNum), "--comment", `auto-accepted by ADR-013 flow (3 ${reviewer} personas approved)`]);
  return { kind: "accepted", verdicts };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  function arg(name: string): string | undefined {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
  }
  function flag(name: string): boolean { return argv.includes(name); }

  const adrPath = arg("--adr");
  const issueNumStr = arg("--issue");
  const dryRun = flag("--dry-run");

  if (!adrPath || !issueNumStr) {
    console.error("Usage: loop-adr-auto-accept.ts --adr <path> --issue <n> [--dry-run]");
    process.exit(1);
  }
  if (!existsSync(adrPath)) {
    console.error(`ERROR: ADR file not found: ${adrPath}`);
    process.exit(1);
  }
  const issueNum = parseInt(issueNumStr);
  const result = await autoAccept({ adrPath, issueNum: issueNum, dryRun });
  console.log(JSON.stringify(result, null, 2));
  switch (result.kind) {
    case "accepted": process.exit(0);
    case "regen_required": process.exit(2);
    case "retired": process.exit(3);
  }
}
