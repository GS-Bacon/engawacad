#!/usr/bin/env bun
// loop-context-bootstrap.ts — 各 /3ailoop サイクル冒頭の context bootstrap
//
// 新セッション or 長時間自走後の文脈劣化対策として、以下を要点 stdout する:
// - 現 Phase (ROADMAP.md から抽出)
// - CLAUDE.md (project rules 抜粋)
// - MEMORY index (~/.claude/projects/-home-bacon-engawacad/memory/MEMORY.md)
// - 直近 dashboard (features/.dashboard.md、存在すれば)
// - 直近 cycle サマリ (features/.loop/state.json、存在すれば)
// - open Issue 件数 + gate/needs-* 内訳 (gh issue list)
//
// #230 差分モード:
// - ROADMAP / CLAUDE.md / Memory は mtime+contentSha で変更検出。未変更なら
//   "cache: <sha>" マーカーのみ。変更時のみ全文 + cache 更新。
// - gh issue list 結果を features/.loop/issue-cache.json にキャッシュ。新 cycle では
//   diff (added / closed / labels changed) を出して classification は要約のみ。
// - state.json は直近 1 cycle (recent_cycles[-1]) のみ展開。
// - `--full` で cache を破棄して従来挙動 (全文 + classification 完全表示)。
//
// 設計トレードオフ (#230 Codex review accepted):
// - 「/clear 後の新セッションでも ROADMAP/CLAUDE.md/Memory を毎回再注入すべき」という
//   見解はあるが、ハーネスが CLAUDE.md と MEMORY.md index を system-reminder で
//   自動再注入するため、本 script の再 emit は冗長。ROADMAP は自動注入されないが、
//   現 Phase ヘッダは loop-should-stop / batch-select の出力で十分カバーされるため
//   diff モードでの抑制を許容する。再 hydration が必要な救済として `--full` がある。
//
// 使い方:
//   bun loop-context-bootstrap.ts [--memory-index <path>] [--cache-dir <dir>] [--full]

import { createHash } from "crypto";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "fs";
import { homedir } from "os";
import { dirname, join } from "path";

const DEFAULT_MEMORY_INDEX = join(
  homedir(),
  ".claude/projects/-home-bacon-engawacad/memory/MEMORY.md",
);
const ROADMAP_PATH = "ROADMAP.md";
const CLAUDE_MD_PATH = "CLAUDE.md";
const DASHBOARD_PATH = "features/.dashboard.md";
const LOOP_STATE_PATH = "features/.loop/state.json";
const DEFAULT_CACHE_DIR = "features/.loop";

const LOOP_EXCLUDE_LABELS = new Set([
  "gate:human-feel",
  "gate:adr-review",
  "needs-triage",
  "needs-phase",
  "needs-human",
  "needs-intent-review",
  "needs-review",
  "blocked-by-split",
]);

function parseArg(name: string): string | undefined {
  const args = process.argv.slice(2);
  const idx = args.indexOf(name);
  return idx >= 0 ? args[idx + 1] : undefined;
}

function hasFlag(name: string): boolean {
  return process.argv.slice(2).includes(name);
}

function safeRead(path: string): string | null {
  if (!existsSync(path)) return null;
  try {
    return readFileSync(path, "utf-8");
  } catch {
    return null;
  }
}

function shortSha(content: string): string {
  return createHash("sha256").update(content).digest("hex").slice(0, 12);
}

function safeMtime(path: string): number | null {
  if (!existsSync(path)) return null;
  try {
    return statSync(path).mtimeMs;
  } catch {
    return null;
  }
}

/** ROADMAP.md から現 Phase セクションを抽出 (suffix/prefix どちらの ✅ もスキップ)
 *  #178 指摘 7 対応: 行全体に ✅ が含まれる Phase ヘッダは完了扱い
 */
export function extractCurrentPhase(roadmap: string): string {
  const lines = roadmap.split("\n");
  let startIdx = -1;
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(/^##\s+(?:✅\s+)?Phase\s+\d/);
    if (m && !lines[i].includes("✅")) {
      startIdx = i;
      break;
    }
  }
  if (startIdx < 0) return "(現 Phase セクション未検出: 全 Phase が完了済みかも)";
  let endIdx = lines.length;
  for (let i = startIdx + 1; i < lines.length; i++) {
    if (/^##\s+(?:✅\s+)?Phase\s+\d/.test(lines[i]) || /^---\s*$/.test(lines[i])) {
      endIdx = i;
      break;
    }
  }
  return lines.slice(startIdx, endIdx).join("\n").slice(0, 2000);
}

interface IssueSummary {
  number: number;
  title: string;
  labels: string[];
}

async function fetchOpenIssues(): Promise<IssueSummary[] | null> {
  const proc = Bun.spawn(
    ["gh", "issue", "list", "--state", "open", "--limit", "200",
     "--json", "number,title,labels"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  if (proc.exitCode !== 0) return null;
  try {
    const raw = JSON.parse(out) as Array<{ number: number; title: string; labels: { name: string }[] }>;
    return raw.map(i => ({
      number: i.number,
      title: i.title,
      labels: i.labels.map(l => l.name),
    }));
  } catch {
    return null;
  }
}

export function classifyIssues(issues: IssueSummary[]): {
  total: number;
  gateBreakdown: Record<string, number>;
  needsBreakdown: Record<string, number>;
  loopActionable: number;
} {
  const gateBreakdown: Record<string, number> = {};
  const needsBreakdown: Record<string, number> = {};
  let loopActionable = 0;
  for (const i of issues) {
    let excluded = false;
    for (const l of i.labels) {
      if (l.startsWith("gate:")) {
        gateBreakdown[l] = (gateBreakdown[l] ?? 0) + 1;
        excluded = true;
      } else if (LOOP_EXCLUDE_LABELS.has(l)) {
        needsBreakdown[l] = (needsBreakdown[l] ?? 0) + 1;
        excluded = true;
      }
    }
    if (!excluded) loopActionable += 1;
  }
  return { total: issues.length, gateBreakdown, needsBreakdown, loopActionable };
}

export interface IssueDiff {
  added: IssueSummary[];          // 新規 (prev に無い number)
  closed: IssueSummary[];         // 消えた (prev にあって cur に無い)
  labelsChanged: Array<{ number: number; title: string; before: string[]; after: string[] }>;
  unchanged: number;
}

export function diffIssues(prev: IssueSummary[], cur: IssueSummary[]): IssueDiff {
  const prevByNum = new Map(prev.map(i => [i.number, i]));
  const curByNum = new Map(cur.map(i => [i.number, i]));
  const added: IssueSummary[] = [];
  const closed: IssueSummary[] = [];
  const labelsChanged: IssueDiff["labelsChanged"] = [];
  let unchanged = 0;

  for (const c of cur) {
    const p = prevByNum.get(c.number);
    if (!p) {
      added.push(c);
    } else {
      const before = [...p.labels].sort();
      const after = [...c.labels].sort();
      // delimiter-safe compare: ラベルに `|` が含まれても誤検出しない
      if (JSON.stringify(before) !== JSON.stringify(after)) {
        labelsChanged.push({ number: c.number, title: c.title, before, after });
      } else {
        unchanged += 1;
      }
    }
  }
  for (const p of prev) {
    if (!curByNum.has(p.number)) closed.push(p);
  }
  return { added, closed, labelsChanged, unchanged };
}

interface IssueCache {
  fetched_at: string;
  issues: IssueSummary[];
}

interface FileCacheEntry {
  mtime: number;
  sha: string;
}

interface ContextCache {
  files: Record<string, FileCacheEntry>;
}

function isValidIssue(x: unknown): x is IssueSummary {
  if (!x || typeof x !== "object") return false;
  const o = x as Record<string, unknown>;
  if (typeof o.number !== "number") return false;
  if (typeof o.title !== "string") return false;
  if (!Array.isArray(o.labels)) return false;
  return o.labels.every(l => typeof l === "string");
}

function loadIssueCache(path: string): IssueCache | null {
  const raw = safeRead(path);
  if (!raw) return null;
  try {
    const obj = JSON.parse(raw) as IssueCache;
    if (!Array.isArray(obj.issues)) return null;
    if (!obj.issues.every(isValidIssue)) return null;
    return obj;
  } catch {
    return null;
  }
}

function writeIssueCache(path: string, cache: IssueCache): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify(cache, null, 2));
}

function loadContextCache(path: string): ContextCache {
  const raw = safeRead(path);
  if (!raw) return { files: {} };
  try {
    const obj = JSON.parse(raw) as ContextCache;
    if (!obj.files || typeof obj.files !== "object") return { files: {} };
    return obj;
  } catch {
    return { files: {} };
  }
}

function writeContextCache(path: string, cache: ContextCache): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify(cache, null, 2));
}

/** mtime+sha で変更検出。未変更なら null、変更ありなら現在 entry を返す */
export function checkFileChange(
  path: string,
  content: string,
  prev: FileCacheEntry | undefined,
): { changed: boolean; entry: FileCacheEntry } {
  const mtime = safeMtime(path) ?? 0;
  const sha = shortSha(content);
  const entry: FileCacheEntry = { mtime, sha };
  if (!prev) return { changed: true, entry };
  // sha が一致すれば未変更 (mtime touch だけでは無視)
  if (prev.sha === sha) return { changed: false, entry };
  return { changed: true, entry };
}

interface LoopState {
  cycle?: number;
  last_cycle_at?: string;
  recent_cycles?: Array<{ cycle: number; closed: number[]; raised: number[]; merged_commits: number; pause_reason?: string }>;
}

async function main() {
  const memoryPath = parseArg("--memory-index") ?? DEFAULT_MEMORY_INDEX;
  const cacheDir = parseArg("--cache-dir") ?? DEFAULT_CACHE_DIR;
  const fullMode = hasFlag("--full");

  const issueCachePath = join(cacheDir, "issue-cache.json");
  const contextCachePath = join(cacheDir, "context-cache.json");

  const lines: string[] = [];

  lines.push(`# /3ailoop Context Bootstrap`);
  lines.push(`Generated: ${new Date().toISOString()}`);
  lines.push(`Mode: ${fullMode ? "full (cache rebuild)" : "diff"}`);
  lines.push("");

  const ctxCache = fullMode ? { files: {} } as ContextCache : loadContextCache(contextCachePath);
  const nextCtxCache: ContextCache = { files: { ...ctxCache.files } };

  // 1. Current Phase (ROADMAP) — diff モードで未変更なら全文を抑制
  lines.push(`## Current Phase (ROADMAP.md)`);
  const roadmap = safeRead(ROADMAP_PATH);
  if (!roadmap) {
    lines.push(`*missing: ${ROADMAP_PATH}*`);
  } else {
    const chk = checkFileChange(ROADMAP_PATH, roadmap, ctxCache.files[ROADMAP_PATH]);
    nextCtxCache.files[ROADMAP_PATH] = chk.entry;
    if (chk.changed) {
      lines.push("```");
      lines.push(extractCurrentPhase(roadmap));
      lines.push("```");
    } else {
      lines.push(`*unchanged (cache: ${chk.entry.sha}) — 前 cycle と同一、再表示を抑制*`);
    }
  }
  lines.push("");

  // 2. CLAUDE.md (project rules)
  lines.push(`## CLAUDE.md (project rules, first 40 lines)`);
  const claudeMd = safeRead(CLAUDE_MD_PATH);
  if (!claudeMd) {
    lines.push(`*missing: ${CLAUDE_MD_PATH}*`);
  } else {
    const chk = checkFileChange(CLAUDE_MD_PATH, claudeMd, ctxCache.files[CLAUDE_MD_PATH]);
    nextCtxCache.files[CLAUDE_MD_PATH] = chk.entry;
    if (chk.changed) {
      lines.push("```");
      lines.push(claudeMd.split("\n").slice(0, 40).join("\n"));
      lines.push("```");
    } else {
      lines.push(`*unchanged (cache: ${chk.entry.sha})*`);
    }
  }
  lines.push("");

  // 3. MEMORY index
  lines.push(`## Memory Index (${memoryPath})`);
  const memory = safeRead(memoryPath);
  if (!memory) {
    lines.push(`*missing: ${memoryPath}*`);
  } else {
    const chk = checkFileChange(memoryPath, memory, ctxCache.files[memoryPath]);
    nextCtxCache.files[memoryPath] = chk.entry;
    if (chk.changed) {
      lines.push("```");
      lines.push(memory.split("\n").slice(0, 100).join("\n"));
      lines.push("```");
    } else {
      lines.push(`*unchanged (cache: ${chk.entry.sha})*`);
    }
  }
  lines.push("");

  // 4. Dashboard (直近サマリ、変更検出対象だが内容は cycle ごとに変わるため通常は全文)
  lines.push(`## Recent Dashboard (${DASHBOARD_PATH})`);
  const dashboard = safeRead(DASHBOARD_PATH);
  if (!dashboard) {
    lines.push(`*no dashboard yet (initial cycle or post-purge)*`);
  } else {
    const chk = checkFileChange(DASHBOARD_PATH, dashboard, ctxCache.files[DASHBOARD_PATH]);
    nextCtxCache.files[DASHBOARD_PATH] = chk.entry;
    if (chk.changed) {
      lines.push("```");
      lines.push(dashboard.split("\n").slice(0, 60).join("\n"));
      lines.push("```");
    } else {
      lines.push(`*unchanged (cache: ${chk.entry.sha})*`);
    }
  }
  lines.push("");

  // 5. Loop state (直近 1 cycle のみ、diff モード時)
  lines.push(`## Recent Loop State (${LOOP_STATE_PATH})`);
  const stateRaw = safeRead(LOOP_STATE_PATH);
  if (!stateRaw) {
    lines.push(`*no loop state yet*`);
  } else {
    try {
      const state = JSON.parse(stateRaw) as LoopState;
      lines.push(`- Current cycle: ${state.cycle ?? "?"}`);
      lines.push(`- Last cycle at: ${state.last_cycle_at ?? "?"}`);
      const sliceN = fullMode ? 3 : 1;
      const recent = (state.recent_cycles ?? []).slice(-sliceN);
      if (recent.length > 0) {
        lines.push(`- Recent ${recent.length} cycle${recent.length > 1 ? "s" : ""}:`);
        for (const c of recent) {
          const closedStr = c.closed?.length ? c.closed.join(", ") : "none";
          const raisedStr = c.raised?.length ? c.raised.join(", ") : "none";
          const pause = c.pause_reason ? ` (paused: ${c.pause_reason})` : "";
          lines.push(`  - Cycle #${c.cycle}: closed=[${closedStr}] raised=[${raisedStr}] merged=${c.merged_commits ?? 0}${pause}`);
        }
      } else {
        lines.push(`- (no recent_cycles recorded)`);
      }
    } catch (e) {
      lines.push(`*corrupt state.json: ${(e as Error).message}*`);
    }
  }
  lines.push("");

  // 6. Open Issues (diff mode 時は差分のみ、full mode は全件 classification)
  lines.push(`## Open Issues (gh issue list)`);
  const issues = await fetchOpenIssues();
  if (issues === null) {
    lines.push(`*gh issue list failed (offline?)*`);
  } else {
    const c = classifyIssues(issues);
    const prevCache = fullMode ? null : loadIssueCache(issueCachePath);
    if (prevCache && !fullMode) {
      const d = diffIssues(prevCache.issues, issues);
      lines.push(`- Total open: ${c.total}`);
      lines.push(`- Loop-actionable (no gate / needs-*): **${c.loopActionable}**`);
      if (Object.keys(c.gateBreakdown).length > 0) {
        lines.push(`- Gate breakdown:`);
        for (const [k, v] of Object.entries(c.gateBreakdown)) {
          lines.push(`  - ${k}: ${v}`);
        }
      }
      if (Object.keys(c.needsBreakdown).length > 0) {
        lines.push(`- Needs/blocked breakdown:`);
        for (const [k, v] of Object.entries(c.needsBreakdown)) {
          lines.push(`  - ${k}: ${v}`);
        }
      }
      lines.push(`- Diff since cache (${prevCache.fetched_at}):`);
      if (d.added.length === 0 && d.closed.length === 0 && d.labelsChanged.length === 0) {
        lines.push(`  - (no changes; unchanged=${d.unchanged})`);
      } else {
        if (d.added.length > 0) {
          lines.push(`  - Added (${d.added.length}):`);
          for (const i of d.added.slice(0, 20)) {
            lines.push(`    - #${i.number} ${truncate(i.title, 80)} [${i.labels.join(", ") || "no-label"}]`);
          }
          if (d.added.length > 20) lines.push(`    - ... and ${d.added.length - 20} more`);
        }
        if (d.closed.length > 0) {
          lines.push(`  - Closed (${d.closed.length}): ${d.closed.slice(0, 30).map(i => `#${i.number}`).join(", ")}${d.closed.length > 30 ? ", ..." : ""}`);
        }
        if (d.labelsChanged.length > 0) {
          lines.push(`  - Labels changed (${d.labelsChanged.length}):`);
          for (const e of d.labelsChanged.slice(0, 10)) {
            lines.push(`    - #${e.number}: [${e.before.join(",")}] → [${e.after.join(",")}]`);
          }
          if (d.labelsChanged.length > 10) lines.push(`    - ... and ${d.labelsChanged.length - 10} more`);
        }
      }
    } else {
      lines.push(`- Total open: ${c.total}`);
      lines.push(`- Loop-actionable (no gate / needs-*): **${c.loopActionable}**`);
      if (Object.keys(c.gateBreakdown).length > 0) {
        lines.push(`- Gate breakdown:`);
        for (const [k, v] of Object.entries(c.gateBreakdown)) {
          lines.push(`  - ${k}: ${v}`);
        }
      }
      if (Object.keys(c.needsBreakdown).length > 0) {
        lines.push(`- Needs/blocked breakdown:`);
        for (const [k, v] of Object.entries(c.needsBreakdown)) {
          lines.push(`  - ${k}: ${v}`);
        }
      }
      if (c.loopActionable === 0) {
        lines.push("");
        lines.push(`> **NOTE**: loop-actionable Issue が 0 件です。loop-should-stop が pause を返すはず`);
      }
    }
    // Issue cache 更新
    writeIssueCache(issueCachePath, { fetched_at: new Date().toISOString(), issues });
  }

  // Context cache 更新
  writeContextCache(contextCachePath, nextCtxCache);

  lines.push("");
  lines.push(`---`);
  lines.push(`(end of context bootstrap)`);

  console.log(lines.join("\n"));
}

function sumValues(o: Record<string, number>): number {
  return Object.values(o).reduce((a, b) => a + b, 0);
}

function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n - 1) + "…" : s;
}

if (import.meta.main) {
  main().catch(e => {
    console.error(`ERROR: ${(e as Error).message}`);
    process.exit(1);
  });
}
