#!/usr/bin/env bun
// intake-phase-estimate.ts — 要望テキストに最も整合する Phase を ROADMAP 突合で推定
//
// ROADMAP.md の各 Phase セクションのキーワード overlap で候補算出。
// 全 Phase score < 0.05 なら needs-phase 推奨。
//
// 使い方:
//   bun intake-phase-estimate.ts --intent <file>

import { existsSync, readFileSync } from "fs";

function tokenize(text: string): Set<string> {
  const ja = text.match(/[぀-ヿ㐀-鿿]{2,}/g) ?? [];
  const en = text.toLowerCase().match(/[a-z][a-z0-9]{2,}/g) ?? [];
  const tokens = new Set<string>([...ja, ...en]);
  for (const seg of ja) {
    for (let i = 0; i + 2 <= seg.length; i++) tokens.add(seg.slice(i, i + 2));
  }
  return tokens;
}

function jaccard(a: Set<string>, b: Set<string>): number {
  if (a.size === 0 || b.size === 0) return 0;
  let inter = 0;
  for (const x of a) if (b.has(x)) inter++;
  const union = a.size + b.size - inter;
  return union > 0 ? inter / union : 0;
}

interface PhaseSection { number: number; title: string; text: string; completed: boolean }

function parseRoadmap(path: string): PhaseSection[] {
  if (!existsSync(path)) return [];
  const text = readFileSync(path, "utf-8");
  const lines = text.split("\n");
  const phases: PhaseSection[] = [];
  let current: PhaseSection | null = null;
  for (const line of lines) {
    const m = line.match(/^##\s+(✅\s+)?Phase\s+(\d+)\s*:?\s*(.*)$/);
    if (m) {
      if (current) phases.push(current);
      current = {
        number: parseInt(m[2]),
        title: m[3].trim(),
        text: line + "\n",
        completed: !!m[1],
      };
    } else if (current) {
      if (/^##\s/.test(line)) {
        phases.push(current);
        current = null;
      } else {
        current.text += line + "\n";
      }
    }
  }
  if (current) phases.push(current);
  return phases;
}

async function main() {
  const args = process.argv.slice(2);
  let intentFile = "";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--intent") intentFile = args[++i] ?? "";
  }
  if (!intentFile || !existsSync(intentFile)) {
    console.error("Usage: intake-phase-estimate.ts --intent <file>");
    process.exit(2);
  }
  const intent = readFileSync(intentFile, "utf-8");
  const intentTokens = tokenize(intent);

  const phases = parseRoadmap("ROADMAP.md").filter(p => !p.completed);
  if (phases.length === 0) {
    console.log(JSON.stringify({ phase: "needs-phase", confidence: 0, hint: "no active Phase found in ROADMAP.md" }));
    process.exit(0);
  }

  const scored = phases.map(p => ({
    phase: p.number,
    title: p.title,
    score: jaccard(intentTokens, tokenize(p.title + " " + p.text)),
  }));
  scored.sort((a, b) => b.score - a.score);

  const top = scored[0];
  if (top.score < 0.05) {
    console.log(JSON.stringify({
      phase: "needs-phase",
      confidence: top.score,
      hint: `top match Phase ${top.phase} score ${top.score.toFixed(3)} below threshold 0.05`,
    }));
    process.exit(0);
  }
  console.log(JSON.stringify({
    phase: top.phase,
    confidence: top.score,
    title: top.title,
    runners_up: scored.slice(1, 3).map(s => ({ phase: s.phase, score: s.score })),
  }));
}

main().catch(e => {
  console.error(`ERROR: ${(e as Error).message}`);
  process.exit(1);
});
