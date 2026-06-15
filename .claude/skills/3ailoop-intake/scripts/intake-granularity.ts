#!/usr/bin/env bun
// intake-granularity.ts — ADR-006 §1 粒度ガード相当の heuristic 評価
//
// ok 条件:
// - 要望文 ≤ 600 字
// - 「と」「および」「+」等の連結語が 3 個以上ない (複数機能の同時要求でない)
// - 「全部」「すべて」「総」等の包括語がない
//
// ng なら粒度分割案を 2-3 件提案 (連結語で分割)。
//
// 使い方:
//   bun intake-granularity.ts --intent <file>

import { existsSync, readFileSync } from "fs";

const CONNECTOR_PATTERNS = [
  /、\s*/g, /、と/g, /および/g, /\+/g, /\s+and\s+/gi, /,\s*/g, /そして/g,
];
const SCOPE_BROAD_PATTERNS = [
  /全部/, /すべて/, /総/, /全 \w+ について/, /\ball\b/i, /every/i,
];

function countConnectors(text: string): number {
  let n = 0;
  for (const p of CONNECTOR_PATTERNS) {
    p.lastIndex = 0;
    n += (text.match(p) ?? []).length;
  }
  return n;
}

function hasBroadScope(text: string): boolean {
  return SCOPE_BROAD_PATTERNS.some(p => p.test(text));
}

function suggestSplits(text: string): string[] {
  // 連結語で分割して 2-3 件返す
  const parts = text
    .split(/(?:、|および|\+|\s+and\s+|そして|,)/i)
    .map(s => s.trim())
    .filter(s => s.length >= 8);
  return parts.slice(0, 3);
}

async function main() {
  const args = process.argv.slice(2);
  let intentFile = "";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--intent") intentFile = args[++i] ?? "";
  }
  if (!intentFile || !existsSync(intentFile)) {
    console.error("Usage: intake-granularity.ts --intent <file>");
    process.exit(2);
  }
  const text = readFileSync(intentFile, "utf-8").trim();

  const reasons: string[] = [];
  if (text.length > 600) reasons.push(`text length ${text.length} > 600`);
  const conn = countConnectors(text);
  if (conn >= 3) reasons.push(`${conn} connectors (multiple concerns)`);
  if (hasBroadScope(text)) reasons.push("broad scope keyword detected");

  if (reasons.length === 0) {
    console.log(JSON.stringify({ ok: true, length: text.length, connectors: conn }));
    process.exit(0);
  }

  const splits = suggestSplits(text);
  console.log(JSON.stringify({
    ok: false,
    reasons,
    length: text.length,
    connectors: conn,
    split_suggestion: splits,
  }, null, 2));
  process.exit(0);
}

main().catch(e => {
  console.error(`ERROR: ${(e as Error).message}`);
  process.exit(1);
});
