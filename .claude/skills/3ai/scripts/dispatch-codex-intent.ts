#!/usr/bin/env bun
// dispatch-codex-intent.ts — Issue 起票時の Codex intent-check (入口 gate)
// ADR-006 §4: 新規 Issue を起票する前に「意図・スコープが明確か」を Codex に審査させる。
//
// 使い方:
//   bun dispatch-codex-intent.ts \
//     --issue-draft <issue-draft.md> \
//     --result <intent-check.yaml> \
//     [--roadmap ROADMAP.md]

import { readFileSync, writeFileSync, existsSync } from "fs";
import { dispatchCodex } from "./dispatch-codex.ts";

async function main() {
  const args = process.argv.slice(2);
  let issueDraftFile = "", resultFile = "", roadmapFile = "ROADMAP.md";

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--issue-draft": issueDraftFile = args[++i]; break;
      case "--result":      resultFile = args[++i]; break;
      case "--roadmap":     roadmapFile = args[++i]; break;
      default: console.error(`Unknown arg: ${args[i]}`); process.exit(1);
    }
  }

  if (!issueDraftFile || !resultFile) {
    console.error("Usage: dispatch-codex-intent.ts --issue-draft <file> --result <file>");
    process.exit(1);
  }

  if (!existsSync(issueDraftFile)) {
    console.error(`Issue draft not found: ${issueDraftFile}`);
    process.exit(1);
  }

  const draftText = readFileSync(issueDraftFile, "utf-8");

  // ROADMAP コンテキストを追加 (Phase 完了条件と整合しているか判定に使う)
  let roadmapCtx = "";
  if (existsSync(roadmapFile)) {
    const roadmap = readFileSync(roadmapFile, "utf-8");
    // 現 Phase のセクションのみ抽出（最初の ## Phase X: ... ブロック）
    const phaseSection = roadmap.match(/## Phase \d[^#][\s\S]*?(?=\n## Phase |\n---\n|$)/)?.[0] ?? "";
    if (phaseSection) {
      roadmapCtx = `===== ROADMAP CONTEXT =====\n現 Phase の完了条件（ROADMAP.md 抜粋）。Issue がこの Phase に寄与するか判定に使うこと。\n${phaseSection.slice(0, 2000)}\n===== END ROADMAP CONTEXT =====\n\n`;
    }
  }

  // numeric check: issue draft に数値的判断を含むキーワードがあれば警告を付与
  const hasNumericKeywords = /epsilon|tolerance|thresh|閾値|退化|ゼロ長|面積ゼロ|boolean|Boolean|交線|pcurve/.test(draftText);
  const numericNote = hasNumericKeywords
    ? `\n[NUMERIC NOTE] この Issue 案には数値判断を含む可能性があります。plan.md に '### 数値モデル' セクションが必要かどうかも判定してください。\n`
    : "";

  const fullInput = `${roadmapCtx}${numericNote}\n以下は新規 Issue の草案です。意図・スコープの明確さを審査してください。\n\n${draftText}`;

  const tmpInput = `/tmp/intent-check-input-${Date.now()}.txt`;
  writeFileSync(tmpInput, fullInput, "utf-8");

  process.stderr.write(`=== dispatch-codex-intent: ${issueDraftFile} ===\n`);

  try {
    await dispatchCodex({
      mode: "design",
      instructionFile: ".claude/skills/3ai/agents/codex-intent-checker.md",
      resultFile,
      inputFile: tmpInput,
    });
  } finally {
    try { Bun.spawnSync(["rm", "-f", tmpInput]); } catch {}
  }

  // 結果を表示してアドバイス
  try {
    const result = readFileSync(resultFile, "utf-8");
    const aligned = result.match(/^aligned:\s*(yes|no)/m)?.[1] ?? "unknown";
    if (aligned === "yes") {
      process.stderr.write(`\n✅ Codex intent-check PASSED: aligned=yes → gh issue create で起票可能\n`);
    } else if (aligned === "no") {
      process.stderr.write(`\n❌ Codex intent-check FAILED: aligned=no → Issue 案を修正して再実行してください\n`);
      process.exit(1);
    }
  } catch {}
}

main().catch((e) => { console.error(e); process.exit(1); });
