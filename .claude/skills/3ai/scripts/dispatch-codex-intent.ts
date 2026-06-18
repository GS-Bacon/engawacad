#!/usr/bin/env bun
// dispatch-codex-intent.ts — Issue 起票時の Codex intent-check (入口 gate)
// ADR-006 §4: 新規 Issue を起票する前に「意図・スコープが明確か」を Codex に審査させる。
//
// #235: loop-split-detector で分割される想定の大粒度 Phase 起点 Issue は、
//       Codex を呼ばず skip 経路に乗せて aligned: skip (split-detector parent) を返す。
//       (body に固定文字列 / label `splittable` の二重マーカー)
//
// 使い方:
//   bun dispatch-codex-intent.ts \
//     --issue-draft <issue-draft.md> \
//     --result <intent-check.yaml> \
//     [--roadmap ROADMAP.md]

import { readFileSync, writeFileSync, existsSync } from "fs";
import { dispatchCodex } from "./dispatch-codex.ts";

// #235: 起点 Issue 検出マーカー。canonical な固定文字列は `loop-split-detector で分割される想定` だが、
// 既存 Phase 起点 Issue (#194-#206) は実際には「loop-split-detector で粒度に合うように分割される想定」
// や順序反転バリアントを含むため、body に「loop-split-detector」と「分割される想定」が共存していれば
// 同じ意図とみなして skip 経路に乗せる (Issue #235 本文 "推奨: 既存起点 Issue 全件が該当" を満たす意図)。
export const SPLIT_DETECTOR_MARKER = "loop-split-detector で分割される想定";
export const SPLIT_DETECTOR_TERMS = ["loop-split-detector", "分割される想定"] as const;
export const SPLIT_DETECTOR_LABEL = "splittable";
export const ALIGNED_SKIP_LINE = "aligned: skip (split-detector parent)";

export interface IssueMeta {
  body: string;
  labels: string[];
}

export interface RunIntentCheckOpts {
  issueDraftFile?: string;
  issueNum?: string;
  resultFile: string;
  roadmapFile?: string;
  /** テスト用: Issue メタ取得を差し替え (gh issue view を呼ばない) */
  fetchOverride?: (issueNum: string) => Promise<IssueMeta>;
}

async function fetchIssueMeta(issueNum: string): Promise<IssueMeta> {
  const proc = Bun.spawn(
    ["gh", "issue", "view", issueNum, "--json", "title,body,labels"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const stdoutText = (await new Response(proc.stdout).text()).trim();
  await proc.exited;
  if (!stdoutText) {
    throw new Error(`Issue #${issueNum} の本文を取得できませんでした`);
  }
  const json = JSON.parse(stdoutText) as { title?: string; body?: string; labels?: Array<{ name: string }> };
  const title = json.title ?? "";
  const body = json.body ?? "";
  const labels = (json.labels ?? []).map((l) => l.name);
  return { body: `# ${title}\n\n${body}`, labels };
}

export function shouldSkipForSplitDetector(meta: IssueMeta): boolean {
  if (meta.labels.includes(SPLIT_DETECTOR_LABEL)) return true;
  if (meta.body.includes(SPLIT_DETECTOR_MARKER)) return true;
  if (SPLIT_DETECTOR_TERMS.every((t) => meta.body.includes(t))) return true;
  return false;
}

export async function runIntentCheck(opts: RunIntentCheckOpts): Promise<number> {
  const { issueDraftFile, issueNum, resultFile, roadmapFile = "ROADMAP.md", fetchOverride } = opts;

  if ((!issueDraftFile && !issueNum) || !resultFile) {
    throw new Error("issueDraftFile/issueNum のいずれかと resultFile が必須です");
  }
  if (issueDraftFile && issueNum) {
    throw new Error("issueDraftFile と issueNum は排他的です");
  }

  // Issue meta 取得 (body + labels)
  let meta: IssueMeta;
  if (issueNum) {
    meta = await (fetchOverride ? fetchOverride(issueNum) : fetchIssueMeta(issueNum));
  } else {
    if (!existsSync(issueDraftFile!)) {
      throw new Error(`Issue draft not found: ${issueDraftFile}`);
    }
    meta = { body: readFileSync(issueDraftFile!, "utf-8"), labels: [] };
  }

  // #235 skip ガード: split-detector 想定の親 Issue は Codex を呼ばず skip 経路
  if (shouldSkipForSplitDetector(meta)) {
    writeFileSync(resultFile, `${ALIGNED_SKIP_LINE}\n`, "utf-8");
    process.stdout.write(`${ALIGNED_SKIP_LINE}\n`);
    process.stderr.write(`\n✅ Codex intent-check SKIPPED: split-detector parent → STEP 3 へ進行\n`);
    return 0;
  }

  const draftText = meta.body;

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

  let codexExit = 0;
  try {
    codexExit = await dispatchCodex({
      mode: "design",
      instructionFile: ".claude/skills/3ai/agents/codex-intent-checker.md",
      resultFile,
      inputFile: tmpInput,
    });
  } finally {
    try { Bun.spawnSync(["rm", "-f", tmpInput]); } catch {}
  }
  if (codexExit !== 0) {
    process.stderr.write(`\n❌ Codex CLI failed (exit ${codexExit})\n`);
    return codexExit;
  }

  // 結果を表示してアドバイス
  try {
    const result = readFileSync(resultFile, "utf-8");
    const aligned = result.match(/^aligned:\s*(yes|no)/m)?.[1] ?? "unknown";
    if (aligned === "yes") {
      process.stderr.write(`\n✅ Codex intent-check PASSED: aligned=yes → gh issue create で起票可能\n`);
    } else if (aligned === "no") {
      process.stderr.write(`\n❌ Codex intent-check FAILED: aligned=no → Issue 案を修正して再実行してください\n`);
      return 1;
    }
  } catch {}
  return 0;
}

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

  const code = await runIntentCheck({
    issueDraftFile: issueDraftFile || undefined,
    issueNum: issueNum || undefined,
    resultFile,
    roadmapFile,
  });
  process.exit(code);
}

if (import.meta.main) {
  main().catch((e) => { console.error(e); process.exit(1); });
}
