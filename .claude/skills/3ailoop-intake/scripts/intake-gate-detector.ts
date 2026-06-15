#!/usr/bin/env bun
// intake-gate-detector.ts — UI/UX 視覚判断要素を検出して gate:human-feel 推奨
//
// キーワード (色 / レイアウト / 操作感 / 視覚 / 見栄え / アニメ / アイコン / カラー /
// font / hover / feedback / 描画 ...) が含まれていれば推奨。
//
// 使い方:
//   bun intake-gate-detector.ts --intent <file>

import { existsSync, readFileSync } from "fs";

const KEYWORDS = [
  // 日本語
  "色", "カラー", "レイアウト", "見栄え", "見た目", "視覚", "操作感", "感触", "触り心地",
  "アニメ", "アニメーション", "アイコン", "フォント", "ホバー", "ハイライト", "ツールチップ",
  "ドラッグ", "クリック感", "スクロール", "余白", "間隔", "配色",
  // 英語
  "color", "colour", "layout", "look", "feel", "ux", "animation", "icon", "font",
  "hover", "tooltip", "drag", "scroll", "spacing", "padding", "margin", "highlight",
  "responsive", "transition", "easing", "shadow", "border-radius",
];

async function main() {
  const args = process.argv.slice(2);
  let intentFile = "";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--intent") intentFile = args[++i] ?? "";
  }
  if (!intentFile || !existsSync(intentFile)) {
    console.error("Usage: intake-gate-detector.ts --intent <file>");
    process.exit(2);
  }
  const text = readFileSync(intentFile, "utf-8").toLowerCase();

  const hits: string[] = [];
  for (const k of KEYWORDS) {
    if (text.includes(k.toLowerCase())) hits.push(k);
  }

  const result = {
    gate_human_feel: hits.length > 0,
    hits,
  };
  console.log(JSON.stringify(result));
}

main().catch(e => {
  console.error(`ERROR: ${(e as Error).message}`);
  process.exit(1);
});
