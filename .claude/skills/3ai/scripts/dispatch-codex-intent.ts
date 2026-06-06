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
  let issueDraftFile = "", issueNum = "", resultFile = "", roadmapFile = "ROADMAP.md";

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--issue-draft": issueDraftFile = args[++i]; break;
      case "--issue":       issueNum = args[++i]; break;
      case "--result":      resultFile = args[++i]; break;
      case "--roadmap":     roadmapFile = args[++i]; break;
      default: console.error(`Unknown arg: ${args[i]}`); process.exit(1);
    }
  }

  if ((!issueDraftFile && !issueNum) || !resultFile) {
    console.error("Usage: dispatch-codex-intent.ts (--issue-draft <file> | --issue N) --result <file>");
    process.exit(1);
  }
  if (issueDraftFile && issueNum) {
    console.error("--issue-draft と --issue は排他的です");
    process.exit(1);
  }

  // draft テキストの取得
  let draftText: string;
  if (issueNum) {
    const proc = Bun.spawn(
      ["gh", "issue", "view", issueNum, "--json", "body", "-q", ".body"],
      { stdout: "pipe", stderr: "pipe" },
    );
    draftText = (await new Response(proc.stdout).text()).trim();
    await proc.exited;
    if (!draftText) {
      console.error(`Issue #${issueNum} の本文を取得できませんでした`);
      process.exit(1);
    }
  } else {
    if (!existsSync(issueDraftFile)) {
      console.error(`Issue draft not found: ${issueDraftFile}`);
      process.exit(1);
    }
    draftText = readFileSync(issueDraftFile, "utf-8");
  }

  // ROADMAP コンテキストを追加 (Phase 完了条件と整合しているか判定に使う)
  let roadmapCtx = "";
  if (existsSync(roadmapFile)) {
    const roadmap = readFileSync(roadmapFile, "utf-8");
    // 現 Phase のセクションを抽出: ✅ を含まない最初の ## Phase X: ... ブロック
    // ✅ の位置はヘッダ先頭 ("## ✅ Phase N") またはタイトル末尾 ("## Phase N: ... ✅") の両方を考慮
    const allPhaseBlocks = [...roadmap.matchAll(/## (?:✅ )?Phase \d[^#\n]*\n[\s\S]*?(?=\n## (?:✅ )?Phase |\n---\n|$)/g)];
    const currentBlock = allPhaseBlocks.find(m => !m[0].split('\n')[0].includes('✅'));
    const phaseSection = currentBlock?.[0] ?? "";
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

  const tmpInput = `/tmp/intent-check-input-${Date.now()}-${Math.random().toString(36).slice(2, 8)}.txt`;
  writeFileSync(tmpInput, fullInput, "utf-8");

  const src = issueNum ? `#${issueNum}` : issueDraftFile;
  process.stderr.write(`=== dispatch-codex-intent: ${src} ===\n`);

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
