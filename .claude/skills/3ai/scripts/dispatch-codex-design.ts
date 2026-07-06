#!/usr/bin/env bun
// dispatch-codex-design.ts — STEP 3.5 Codex 1 persona 設計 gate
//
// 目的: GLM 多ペルソナ (STEP 3) が Z.AI 同系モデルの分身であることによる相関盲点を、
//   別モデル系 (Codex) の 1 発 gate で破る。設計段階で早期に発見して実装コストを削減する。
//
// 入力: plan.md + feature-dir/adr-context.md + feature-dir/judgment-summary.md
//       + feature-dir/rejection.md (存在するもののみ)
// 出力: <resultFile> + <resultFile>.verdict.json (dispatch-codex.ts が生成)
//
// ループ制御は呼び元 (SKILL.md STEP 3.5) に委ねる。このスクリプトは 1 発 gate。

import { existsSync, readFileSync, unlinkSync, writeFileSync } from "fs";
import { join } from "path";
import { dispatchCodex } from "./dispatch-codex.ts";

export interface DispatchCodexDesignOpts {
  planFile: string;
  featureDir: string;
  resultFile: string;
  instructionFile?: string;
}

/**
 * feature-dir から judgment/rejection/adr-context を検出して plan と連結する。
 * dispatch-codex.ts の buildPrefix が持つ scope hint ブロックと同じマーカー
 * (===== ADR EXCERPT ===== / ===== PRIOR JUDGMENTS ===== / ===== PRIOR REJECTIONS =====)
 * を使う。既存の codex-design-reviewer.md の SCOPE 規律セクションが認識するため。
 */
export function buildDesignInput(planFile: string, featureDir: string): string {
  const parts: string[] = [];
  const push = (path: string, header: string) => {
    if (existsSync(path)) {
      parts.push(
        `===== ${header} =====\n${readFileSync(path, "utf-8")}\n===== END ${header} =====\n`,
      );
    }
  };
  push(join(featureDir, "adr-context.md"), "ADR EXCERPT");
  push(join(featureDir, "judgment-summary.md"), "PRIOR JUDGMENTS");
  push(join(featureDir, "rejection.md"), "PRIOR REJECTIONS");
  parts.push(
    `===== PLAN =====\n${readFileSync(planFile, "utf-8")}\n===== END PLAN =====\n`,
  );
  return parts.join("\n");
}

export async function dispatchCodexDesign(
  opts: DispatchCodexDesignOpts,
): Promise<number> {
  const { planFile, featureDir, resultFile } = opts;
  const instructionFile =
    opts.instructionFile ??
    ".claude/skills/3ai/agents/codex-design-reviewer.md";

  const combined = buildDesignInput(planFile, featureDir);
  const tmp = join(featureDir, ".codex-design-input.tmp.md");
  writeFileSync(tmp, combined, "utf-8");

  try {
    const code = await dispatchCodex({
      mode: "design",
      instructionFile,
      inputFile: tmp,
      resultFile,
    });
    return code;
  } finally {
    try {
      if (existsSync(tmp)) unlinkSync(tmp);
    } catch {}
  }
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let planFile = "";
  let featureDir = "";
  let resultFile = "";
  let instructionFile = "";

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--plan-file":
        planFile = args[++i];
        break;
      case "--feature-dir":
        featureDir = args[++i];
        break;
      case "--result":
        resultFile = args[++i];
        break;
      case "--instruction":
        instructionFile = args[++i];
        break;
      default:
        console.error(`Unknown arg: ${args[i]}`);
        process.exit(1);
    }
  }

  if (!planFile || !featureDir || !resultFile) {
    console.error(
      "Usage: dispatch-codex-design.ts --plan-file <path> --feature-dir <dir> --result <path> [--instruction <path>]",
    );
    process.exit(1);
  }

  const code = await dispatchCodexDesign({
    planFile,
    featureDir,
    resultFile,
    instructionFile: instructionFile || undefined,
  });
  process.exit(code);
}
