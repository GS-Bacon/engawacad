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
import { withFileLockSync } from "./loop-file-lock.ts";

const STATE_PATH = "features/.loop/state.json";
const FEATURES_DIR = "features";
// #228: claude/glm wiring 修復で実集計が ~10x になったため閾値を調整。
// 旧 100M (CUM_PAUSE) は codex のみ計上時の値。新運用ベースで再評価が必要 (別 Issue)。
const CUM_PAUSE = 1_000_000_000;
const WIN24_WARN = 100_000_000;

// #178 指摘 3: 実 log の input_tokens / output_tokens / cache 系も拾い、合算する
// JSON のキー ("input_tokens": 1000) と YAML のキー (input_tokens: 1000) の両方対応のため
// フィールド名の後ろに任意の quote を許容する
const TOKEN_FIELDS = [
  /(?:^|\W)total[_ ]?tokens?["']?\s*[:=]\s*(\d+)/gi,
  /(?:^|\W)input[_ ]?tokens?["']?\s*[:=]\s*(\d+)/gi,
  /(?:^|\W)output[_ ]?tokens?["']?\s*[:=]\s*(\d+)/gi,
  /(?:^|\W)cache[_ ]?read[_ ]?input[_ ]?tokens?["']?\s*[:=]\s*(\d+)/gi,
  /(?:^|\W)cache[_ ]?creation[_ ]?input[_ ]?tokens?["']?\s*[:=]\s*(\d+)/gi,
  /(?:^|\W)usage[_ ]?total["']?\s*[:=]\s*(\d+)/gi,
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
  // 各フィールドごとに最大値 (重複避け) を取り、フィールド間で合算
  // (実 log は同じ fields が複数回出る場合があり、最大値を取る方が安全)
  let total = 0;
  for (const pat of TOKEN_FIELDS) {
    let maxForField = 0;
    pat.lastIndex = 0;
    for (const m of text.matchAll(pat)) {
      const n = parseInt(m[1]);
      if (Number.isFinite(n) && n > maxForField) maxForField = n;
    }
    total += maxForField;
  }
  return total;
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
      // #228: GLM の usage は glm-*.json.raw (Z.AI raw response) に入っているため `.json` と
      // `.json.raw` も走査対象にする。codex-*.yaml.log の挙動は維持。
      if (/(codex|glm|claude).*\.(yaml|log|json)(\.raw)?$/.test(f)) {
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

/** #228: Z.AI (GLM) raw response の最終 result 行を取り出す。
 *  - 形式: 1 行ごとに JSON が並ぶ stream-of-events。最後の `"type": "result"` が summary。
 *  - 単一 JSON object のときは丸ごと parse する。 */
function tryParseGlmResult(text: string): Record<string, unknown> | null {
  const trimmed = text.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) return null;
  // 単一 JSON を優先
  try {
    const obj = JSON.parse(trimmed);
    if (typeof obj === "object" && obj !== null) return obj as Record<string, unknown>;
  } catch { /* fall through to NDJSON 走査 */ }
  // NDJSON: 末尾から `type: "result"` を探す
  const lines = trimmed.split("\n");
  for (let i = lines.length - 1; i >= 0; i--) {
    const l = lines[i].trim();
    if (!l.startsWith("{")) continue;
    try {
      const obj = JSON.parse(l);
      if (obj?.type === "result" && (obj.usage || obj.modelUsage)) {
        return obj as Record<string, unknown>;
      }
    } catch { /* try previous line */ }
  }
  return null;
}

/** #228: GLM の modelUsage から GLM 専有と claude 専有を分離して返す。
 *  Z.AI claude proxy 経由で claude-opus-4-7 が sub-call されるため、modelUsage
 *  を読まないと claude が GLM に丸ごと埋まる。 */
function extractFromGlmJson(
  obj: Record<string, unknown>,
  fallbackKind: "claude" | "glm" | "codex",
): { claude: number; glm: number; codex: number } | null {
  const modelUsage = obj.modelUsage as Record<string, unknown> | undefined;
  if (modelUsage && typeof modelUsage === "object") {
    let glm = 0, claude = 0;
    for (const [model, raw] of Object.entries(modelUsage)) {
      if (!raw || typeof raw !== "object") continue;
      const u = raw as Record<string, unknown>;
      const sum =
        ((u.inputTokens as number) ?? 0) +
        ((u.outputTokens as number) ?? 0) +
        ((u.cacheReadInputTokens as number) ?? 0) +
        ((u.cacheCreationInputTokens as number) ?? 0);
      const key = model.toLowerCase();
      if (key.startsWith("glm")) glm += sum;
      else if (key.includes("claude")) claude += sum;
    }
    return { claude, glm, codex: 0 };
  }
  const usage = obj.usage as Record<string, unknown> | undefined;
  if (usage && typeof usage === "object") {
    const u = usage;
    const sum =
      ((u.input_tokens as number) ?? 0) +
      ((u.output_tokens as number) ?? 0) +
      ((u.cache_read_input_tokens as number) ?? 0) +
      ((u.cache_creation_input_tokens as number) ?? 0);
    if (fallbackKind === "claude") return { claude: sum, glm: 0, codex: 0 };
    if (fallbackKind === "glm") return { claude: 0, glm: sum, codex: 0 };
    return { claude: 0, glm: 0, codex: sum };
  }
  return null;
}

/** Per-file の token 抽出。JSON 系は構造化 parse を試し、失敗時は regex fallback。 */
export function classifyAndExtractFile(
  p: string,
  text: string,
): { claude: number; glm: number; codex: number } {
  const kind = classifyByName(p);
  if (kind === null) return { claude: 0, glm: 0, codex: 0 };

  // #228: JSON 形式 (.json / .json.raw) は structured parse を優先
  if (/\.(json)(\.raw)?$/.test(p)) {
    const obj = tryParseGlmResult(text);
    if (obj) {
      const r = extractFromGlmJson(obj, kind);
      if (r) return r;
    }
  }
  // regex fallback (主に codex-*.yaml.log 等)
  const n = extractTokens(text);
  if (kind === "claude") return { claude: n, glm: 0, codex: 0 };
  if (kind === "glm") return { claude: 0, glm: n, codex: 0 };
  return { claude: 0, glm: 0, codex: n };
}

export function collect(): { claude: number; glm: number; codex: number; tokens_24h: number } {
  const logs = walkFeatureLogs();
  let claude = 0, glm = 0, codex = 0, tokens24h = 0;
  const cutoff24h = Date.now() - 24 * 3600 * 1000;
  for (const { path: p, mtime } of logs) {
    const text = safeRead(p);
    if (!text) continue;
    const r = classifyAndExtractFile(p, text);
    const fileTotal = r.claude + r.glm + r.codex;
    if (fileTotal === 0) continue;
    claude += r.claude;
    glm += r.glm;
    codex += r.codex;
    if (mtime >= cutoff24h) tokens24h += fileTotal;
  }
  return { claude, glm, codex, tokens_24h: tokens24h };
}

if (import.meta.main) {
  const cmd = process.argv[2];
  if (cmd === "collect") {
    const result = collect();
    // #226: state.json の token_* 更新は cycle-record の cumulative 更新と競合し得るため、
    // withFileLockSync で RMW を直列化する。
    withFileLockSync(STATE_PATH, () => {
      const state = readState();
      if (state) {
        state.cumulative.token_claude = result.claude;
        state.cumulative.token_glm = result.glm;
        state.cumulative.token_codex = result.codex;
        atomicWriteState(state);
      }
    });
    console.log(JSON.stringify(result, null, 2));
    process.exit(0);
  } else if (cmd === "check") {
    const result = collect();
    // check 内でも state.cumulative.token_* を更新して dashboard が常に最新値を見られるように (#177 指摘 9)
    // #226: 同上、collect/check 両経路で RMW serialize。
    withFileLockSync(STATE_PATH, () => {
      const state = readState();
      if (state) {
        state.cumulative.token_claude = result.claude;
        state.cumulative.token_glm = result.glm;
        state.cumulative.token_codex = result.codex;
        atomicWriteState(state);
      }
    });
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
