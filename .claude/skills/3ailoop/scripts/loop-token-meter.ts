#!/usr/bin/env bun
// loop-token-meter.ts — dispatch log から token 使用量を集計し閾値判定
//
// 集計ソース:
//   features/*/codex-*.yaml.log  → codex token
//   features/*/glm-*.log         → glm token
//   features/*/claude-*.log      → claude session token (もしあれば)
//
// API:
//   bun loop-token-meter.ts collect          # state.json の cumulative.token_* を更新、stdout に集計
//   bun loop-token-meter.ts check            # 累積/24h 閾値判定、超過で exit 1 + stdout に PAUSE/WARN
//
// 閾値:
//   CUM_PAUSE  = 100_000_000 (合計 100M、exit 1)
//   WIN24_WARN =   5_000_000 (24h 合計 5M、warning のみ)

import { existsSync, readdirSync, readFileSync, renameSync, statSync, writeFileSync } from "fs";
import { dirname, join } from "path";

const STATE_PATH = "features/.loop/state.json";
const FEATURES_DIR = "features";
const CUM_PAUSE = 100_000_000;
const WIN24_WARN = 5_000_000;

const TOKEN_PATTERNS = [
  /total[_ ]?tokens?\s*[:=]\s*(\d+)/gi,
  /tokens?\s*[:=]\s*(\d+)/gi,
  /usage[_ ]?total\s*[:=]\s*(\d+)/gi,
];

interface LoopState {
  cumulative: {
    cycles_total: number;
    issues_closed: number;
    raised_resolved: number;
    adr_total: number;
    token_claude: number;
    token_glm: number;
    token_codex: number;
  };
  [k: string]: unknown;
}

function safeRead(p: string): string | null {
  if (!existsSync(p)) return null;
  try { return readFileSync(p, "utf-8"); } catch { return null; }
}

function readState(): LoopState | null {
  const raw = safeRead(STATE_PATH);
  if (!raw) return null;
  try { return JSON.parse(raw) as LoopState; } catch { return null; }
}

function atomicWriteState(state: LoopState): void {
  const tmp = `${STATE_PATH}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, JSON.stringify(state, null, 2), "utf-8");
  renameSync(tmp, STATE_PATH);
}

function extractTokens(text: string): number {
  let max = 0;
  for (const pat of TOKEN_PATTERNS) {
    pat.lastIndex = 0;
    for (const m of text.matchAll(pat)) {
      const n = parseInt(m[1]);
      if (Number.isFinite(n) && n > max) max = n;
    }
  }
  return max;
}

function walkFeatureLogs(): { path: string; mtime: number }[] {
  if (!existsSync(FEATURES_DIR)) return [];
  const out: { path: string; mtime: number }[] = [];
  const entries = readdirSync(FEATURES_DIR);
  for (const e of entries) {
    if (e.startsWith(".")) continue; // .batch / .loop / .dashboard.md skip
    const dir = join(FEATURES_DIR, e);
    let isDir = false;
    try { isDir = statSync(dir).isDirectory(); } catch { /* skip */ }
    if (!isDir) continue;
    let logs: string[] = [];
    try { logs = readdirSync(dir); } catch { /* skip */ }
    for (const f of logs) {
      if (/(codex|glm|claude).*\.(yaml|log)$/.test(f)) {
        const p = join(dir, f);
        try {
          const st = statSync(p);
          out.push({ path: p, mtime: st.mtimeMs });
        } catch { /* skip */ }
      }
    }
  }
  return out;
}

function classifyByName(p: string): "claude" | "glm" | "codex" | null {
  if (/codex/i.test(p)) return "codex";
  if (/glm/i.test(p)) return "glm";
  if (/claude/i.test(p)) return "claude";
  return null;
}

function collect(): { claude: number; glm: number; codex: number; tokens_24h: number } {
  const logs = walkFeatureLogs();
  let claude = 0, glm = 0, codex = 0, tokens24h = 0;
  const cutoff24h = Date.now() - 24 * 3600 * 1000;
  for (const { path: p, mtime } of logs) {
    const text = safeRead(p);
    if (!text) continue;
    const n = extractTokens(text);
    if (n === 0) continue;
    const kind = classifyByName(p);
    if (kind === "claude") claude += n;
    else if (kind === "glm") glm += n;
    else if (kind === "codex") codex += n;
    if (mtime >= cutoff24h) tokens24h += n;
  }
  return { claude, glm, codex, tokens_24h: tokens24h };
}

if (import.meta.main) {
  const cmd = process.argv[2];
  if (cmd === "collect") {
    const result = collect();
    const state = readState();
    if (state) {
      state.cumulative.token_claude = result.claude;
      state.cumulative.token_glm = result.glm;
      state.cumulative.token_codex = result.codex;
      atomicWriteState(state);
    }
    console.log(JSON.stringify(result, null, 2));
    process.exit(0);
  } else if (cmd === "check") {
    const result = collect();
    // check 内でも state.cumulative.token_* を更新して dashboard が常に最新値を見られるように (#177 指摘 9)
    const state = readState();
    if (state) {
      state.cumulative.token_claude = result.claude;
      state.cumulative.token_glm = result.glm;
      state.cumulative.token_codex = result.codex;
      atomicWriteState(state);
    }
    const total = result.claude + result.glm + result.codex;
    if (total >= CUM_PAUSE) {
      console.log(`PAUSE: cumulative token ${total} >= ${CUM_PAUSE} threshold`);
      process.exit(1);
    }
    if (result.tokens_24h >= WIN24_WARN) {
      console.log(`WARN: 24h token ${result.tokens_24h} >= ${WIN24_WARN} threshold (continue)`);
    }
    console.log(`OK: cumulative=${total} 24h=${result.tokens_24h}`);
    process.exit(0);
  } else {
    console.error("Usage: loop-token-meter.ts (collect | check)");
    process.exit(2);
  }
}
