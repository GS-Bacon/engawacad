#!/usr/bin/env bun
// Deterministic draft/verdict compatibility helpers. Never writes GitHub.

import { readFileSync } from "fs";
import { arg, emitJson, emitStatus } from "./cli/helpers.ts";

export interface SplitDraft { title: string; body: string; labels: string[] }

export function parseSplitDrafts(text: string, parent: number, milestone: string | null): Array<SplitDraft & { milestone: string | null }> {
  const lines = text.split("\n");
  const start = lines.findIndex(line => /^split_proposal\s*:/.test(line));
  if (start < 0) return [];
  const drafts: SplitDraft[] = [];
  let current: SplitDraft | null = null;
  for (const raw of lines.slice(start + 1)) {
    if (/^[^\s-]/.test(raw)) break;
    const title = raw.match(/^\s*-\s*title\s*:\s*["']?(.*?)["']?\s*$/);
    if (title) {
      if (current) drafts.push(current);
      current = { title: title[1], body: `Split from #${parent}`, labels: [] };
      continue;
    }
    if (!current) continue;
    const body = raw.match(/^\s+body\s*:\s*["']?(.*?)["']?\s*$/);
    if (body) current.body = body[1];
    const labels = raw.match(/^\s+labels\s*:\s*\[(.*)\]\s*$/);
    if (labels) current.labels = labels[1].split(",").map(x => x.trim().replace(/^['"]|['"]$/g, "")).filter(Boolean);
  }
  if (current) drafts.push(current);
  return drafts.map(d => ({ ...d, labels: [...new Set([...d.labels, `parent-blocked-by-split:${parent}`])], milestone }));
}

export function decideAdr(verdicts: Array<{ persona: string; verdict: "approved" | "refuted"; reason?: string }>) {
  const required = ["architect", "contrarian", "migration"];
  const missing = required.filter(persona => !verdicts.some(v => v.persona === persona));
  if (missing.length) return { outcome: "needs-human", reason: `missing independent verdicts: ${missing.join(",")}` };
  const refuted = verdicts.filter(v => v.verdict === "refuted");
  return refuted.length ? { outcome: "regen-required", refuted } : { outcome: "accepted", approved: required };
}

async function main(): Promise<number> {
  const [command, ...args] = process.argv.slice(2);
  if (command === "split-drafts") {
    const review = arg(args, "--review"), parent = Number(arg(args, "--parent")), milestone = arg(args, "--milestone") ?? null;
    if (!review || !Number.isInteger(parent)) return 2;
    emitJson({ parent, drafts: parseSplitDrafts(readFileSync(review, "utf-8"), parent, milestone), github_writes: false });
    return 0;
  }
  if (command === "adr-verdict") {
    const input = arg(args, "--input");
    if (!input) return 2;
    const result = decideAdr(JSON.parse(readFileSync(input, "utf-8")));
    emitJson({ ...result, github_writes: false });
    return result.outcome === "accepted" ? 0 : result.outcome === "regen-required" ? 2 : 3;
  }
  emitStatus("Usage: compatibility.ts split-drafts --review <yaml> --parent N [--milestone title] | adr-verdict --input <json>");
  return 2;
}

if (import.meta.main) main().then(code => process.exit(code)).catch(error => { console.error(error); process.exit(1); });
