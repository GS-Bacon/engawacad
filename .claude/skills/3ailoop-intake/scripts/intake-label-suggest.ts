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

// #178 指摘 4: 慎重化
// - 弱単独マッチでは判定しない、複合条件を強める
// - workflow / skill 等の generic 語だけで type:foundation 化しない
// - batch 未判定なら batch ラベルを付けず人間判断に委ねる (blanket fallback 廃止)
const TYPE_RULES: Array<{ pattern: RegExp; label: string }> = [
  // 強マッチのみ採用
  { pattern: /(?:^|\b)(?:ドキュメント|手順書|README|説明書)\b|docs:|documentation/i, label: "docs" },
  { pattern: /panic|crash|\bbug\b|エラー(?:出|発生)|落ちる|破損|壊れる/i, label: "bug" },
  { pattern: /\brefactor\b|リファクタ|内部設計の見直し|構造の整理/i, label: "type: refactor" },
  // foundation: skill 系の明示語 + 2 語以上の複合条件
  {
    pattern: /(?:loop|3ai|3ailoop|CI 全体|ビルドシステム|tooling)(?:.*(?:整備|改善|刷新|再構築|追加))/i,
    label: "type: foundation",
  },
];

const BATCH_RULES: Array<{ pattern: RegExp; label: string }> = [
  { pattern: /kernel|B-rep|幾何|トポロジー|Boolean|extrude|sketch.*to.*solid|edge|face|vertex|Cuboid|Sphere|Prism|primitive|プリミティブ|mesh|メッシュ|tessellat|テッセレーション|精度|tolerance|公差|曲面/i, label: "batch:kernel" },
  { pattern: /\b(?:format|engawa-format|YAML.*spec|document)\b|persistence|storage|シリアライズ/i, label: "batch:data" },
  { pattern: /viewer|3D 表示|ブラウザ.*画面|render|ビューア|GUI|ウィンドウ/i, label: "batch:viewer" },
  { pattern: /(?:^|\b)(?:3ailoop|cron job|lint .*ラベル|スクリプト.*整備|skill .*追加)/i, label: "batch:skill" },
];

function pickFirst<T extends { pattern: RegExp; label: string }>(rules: T[], text: string): string | null {
  for (const r of rules) if (r.pattern.test(text)) return r.label;
  return null;
}

async function main() {
  const args = process.argv.slice(2);
  let intentFile = "";
  let gateJson = "";
  let phaseArg = "";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--intent") intentFile = args[++i] ?? "";
    else if (args[i] === "--gate") gateJson = args[++i] ?? "";
    else if (args[i] === "--phase") phaseArg = args[++i] ?? "";
  }
  if (!intentFile || !existsSync(intentFile)) {
    console.error("Usage: intake-label-suggest.ts --intent <file> [--gate <json>] [--phase <N|needs-phase>]");
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
  // #178 指摘 4: batch:skill blanket fallback を廃止
  // batch 軸が推定できなければラベル無しで出力 → ユーザーが I-7 で手動追加
  if (batchLabel) labels.push(batchLabel);

  if (gateHumanFeel) labels.push("gate:human-feel");

  // ROADMAP 外要望は needs-phase ラベルを付与 (#177 指摘 5)
  // 通常 phase 数字の場合は --milestone で別途付与されるためラベルは追加しない
  if (phaseArg === "needs-phase") labels.push("needs-phase");

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
