#!/usr/bin/env bun
// intake-gate-detector.ts — UI/UX 視覚判断要素を検出して gate:human-feel 推奨
//
// #178 指摘 5 対応: 強/弱 2 グループに分離。
//   STRONG: UI 専用語。1 個でも hit すれば gate=true
//   WEAK:   弱単語 (feel/look 等)。≥2 hit で gate=true
// 単独 weak hit (例: "I feel export is slow") では gate を立てない。
//
// 使い方:
//   bun intake-gate-detector.ts --intent <file>

import { existsSync, readFileSync } from "fs";

const STRONG_KEYWORDS = [
  // 日本語 — UI 専用語
  "色", "カラー", "レイアウト", "見た目", "視覚",
  "アニメーション", "アイコン", "フォント", "ホバー", "ハイライト", "ツールチップ",
  "余白", "間隔", "配色",
  // 英語
  "color", "colour", "layout", "animation", "icon", "font",
  "hover", "tooltip", "highlight", "spacing", "padding", "margin",
];

const WEAK_KEYWORDS = [
  // 日本語 — 単独だと別意味の可能性
  "見栄え", "感触", "触り心地", "操作感",
  "ドラッグ", "クリック感", "スクロール",
  // 英語
  "feel", "look", "ux", "shadow", "transition", "easing",
  "responsive", "border-radius", "drag", "scroll", "focus",
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

  const strongHits: string[] = [];
  for (const k of STRONG_KEYWORDS) if (text.includes(k.toLowerCase())) strongHits.push(k);

  const weakHits: string[] = [];
  for (const k of WEAK_KEYWORDS) if (text.includes(k.toLowerCase())) weakHits.push(k);

  // 強 1 個以上 → 確定 gate
  // 弱 2 個以上 → gate
  // それ以下 → non-gate
  const gate = strongHits.length >= 1 || weakHits.length >= 2;

  console.log(JSON.stringify({
    gate_human_feel: gate,
    strong_hits: strongHits,
    weak_hits: weakHits,
    reason: gate
      ? (strongHits.length >= 1 ? `${strongHits.length} strong hit(s)` : `${weakHits.length} weak hits >= 2`)
      : "no strong hit and weak hits < 2",
  }));
}

main().catch(e => {
  console.error(`ERROR: ${(e as Error).message}`);
  process.exit(1);
});
