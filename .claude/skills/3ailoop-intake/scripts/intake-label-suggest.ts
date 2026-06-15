#!/usr/bin/env bun
// intake-label-suggest.ts — 要望テキストから type 軸 + batch 軸 + gate ラベルを heuristic 提案
//
// type 推定 (ヒット順):
//   docs    : ドキュメント/ガイド/手順
//   bug     : 直し/治し/破損/失敗/エラー/壊れ
//   refactor: refactor/再構築/整理/見通し/読みやす
//   foundation: 基盤/インフラ/skill/ツール
//   feature : 上記いずれにも該当しない (デフォルト)
//
// batch 推定 (ヒット順):
//   kernel  : kernel/B-rep/幾何/トポロジー/Boolean
//   data    : format/document/YAML/storage/persistence
//   viewer  : viewer/GUI/画面/ビューア/3D
//   skill   : skill/ツール/loop/3ai/CI/lint
//
// gate: intake-gate-detector の結果 (JSON 入力) を取り込む
//
// 使い方:
//   bun intake-label-suggest.ts --intent <file> [--gate <gate-detector-json>]
//
// 内部で lint-issue-labels.ts に投げて exit 0 確認。NG なら exit 1。

import { existsSync, readFileSync } from "fs";

const TYPE_RULES: Array<{ pattern: RegExp; label: string }> = [
  { pattern: /ドキュメント|手順書|ガイド|\bdocs?\b|README|説明書/i, label: "docs" },
  { pattern: /直し|治し|破損|壊れ|失敗|エラー|落ちる|crash|\bbug\b|panic/i, label: "bug" },
  { pattern: /refactor|リファクタ|再構築|読みやす|整理|見通し/i, label: "type: refactor" },
  { pattern: /基盤|インフラ|skill|ツール|tooling|foundation|workflow|CI/i, label: "type: foundation" },
];

const BATCH_RULES: Array<{ pattern: RegExp; label: string }> = [
  { pattern: /kernel|B-rep|幾何|トポロジー|Boolean|extrude|sketch.*to.*solid|edge|face|vertex|Cuboid|Sphere|Prism|primitive|プリミティブ|mesh|メッシュ|tessellat|テッセレーション|精度|tolerance|公差|曲面/i, label: "batch:kernel" },
  { pattern: /format|document|yaml|persistence|storage|シリアライズ|engawa\b|export/i, label: "batch:data" },
  { pattern: /viewer|GUI|画面|3D|render|ブラウザ|web|UI|drag|hover|表示/i, label: "batch:viewer" },
  { pattern: /\bskill\b|ツール|loop|3ai|CI|lint|3ailoop|cron|workflow/i, label: "batch:skill" },
];

function pickFirst<T extends { pattern: RegExp; label: string }>(rules: T[], text: string): string | null {
  for (const r of rules) if (r.pattern.test(text)) return r.label;
  return null;
}

async function main() {
  const args = process.argv.slice(2);
  let intentFile = "";
  let gateJson = "";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--intent") intentFile = args[++i] ?? "";
    else if (args[i] === "--gate") gateJson = args[++i] ?? "";
  }
  if (!intentFile || !existsSync(intentFile)) {
    console.error("Usage: intake-label-suggest.ts --intent <file> [--gate <json>]");
    process.exit(2);
  }
  const text = readFileSync(intentFile, "utf-8");

  const typeLabel = pickFirst(TYPE_RULES, text) ?? "type: feature";
  let batchLabel = pickFirst(BATCH_RULES, text);

  // gate 取得 (label 推定にも使う)
  let gateHumanFeel = false;
  if (gateJson) {
    try {
      const g = JSON.parse(gateJson);
      gateHumanFeel = !!g.gate_human_feel;
    } catch { /* ignore */ }
  }

  // gate:human-feel hit のとき batch 軸が未推定なら viewer を補完
  // (UI/UX 視覚判断は viewer 領域に集約される、plan 方針)
  if (gateHumanFeel && !batchLabel) batchLabel = "batch:viewer";

  const labels: string[] = [typeLabel];
  if (batchLabel) labels.push(batchLabel);
  else if (typeLabel !== "type: feature") labels.push("batch:skill"); // fallback

  if (gateHumanFeel) labels.push("gate:human-feel");

  const csv = labels.join(",");

  // lint pass
  const proc = Bun.spawn(
    ["bun", ".claude/skills/3ai/scripts/lint-issue-labels.ts", "--labels", csv],
    { stdout: "pipe", stderr: "pipe" },
  );
  await proc.exited;
  if (proc.exitCode !== 0) {
    const err = await new Response(proc.stderr).text();
    console.error(`lint failed for [${csv}]:\n${err}`);
    process.exit(1);
  }
  console.log(csv);
}

main().catch(e => {
  console.error(`ERROR: ${(e as Error).message}`);
  process.exit(1);
});
