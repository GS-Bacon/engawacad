#!/usr/bin/env bun
// loop-adr-regen-tracker.ts — ADR 自動 accept フローの暴走防止 cap (ADR-013)
//
// 各 ADR ごとに「再生成回数」「累積 token」を永続化し、
//   - 再生成 3 回越え
//   - 累積 token 200k 越え
// のいずれかで該当 ADR Issue を gate:adr-review 削除 + needs-human 付与に切り替える。
//
// 使い方 (CLI):
//   bun loop-adr-regen-tracker.ts inc-regen --adr <slug>
//   bun loop-adr-regen-tracker.ts add-tokens --adr <slug> --tokens <n>
//   bun loop-adr-regen-tracker.ts get --adr <slug>
//   bun loop-adr-regen-tracker.ts retire --adr <slug> --issue <n> --reason <text>
//   bun loop-adr-regen-tracker.ts clear-retired --adr <slug> --issue <n>
//
// inc-regen / add-tokens は stdout に JSON {state, capped: bool, reason?: string} を返す。
// clear-retired は stdout に JSON {cleared: bool, reason?: string} を返す。

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname } from "path";

const ROOT = "features/.loop/adr-regen-count";
export const REGEN_CAP = 3;
export const TOKEN_CAP = 200_000;

export type AdrTrackerState = {
  adr: string;
  regen_count: number;
  token_used: number;
  started_at: string;
  last_updated_at: string;
  retired?: { at: string; reason: string };
};

function stateFile(slug: string): string {
  return `${ROOT}/${slug}.json`;
}

function nowIso(): string {
  // 注: Date.now() / new Date() は禁止されている (Workflow context のみ)
  // この CLI は worker pane で実行されるので、Bun 標準の Date を使う
  // (Workflow 制約は Workflow tool 内のみ適用される)
  return new Date().toISOString();
}

function readState(slug: string): AdrTrackerState | null {
  const path = stateFile(slug);
  if (!existsSync(path)) return null;
  try {
    return JSON.parse(readFileSync(path, "utf-8")) as AdrTrackerState;
  } catch {
    return null;
  }
}

function writeState(state: AdrTrackerState): void {
  const path = stateFile(state.adr);
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp.${process.pid}.${Math.random().toString(36).slice(2, 8)}`;
  writeFileSync(tmp, JSON.stringify(state, null, 2), "utf-8");
  renameSync(tmp, path);
}

export function getState(slug: string): AdrTrackerState | null {
  return readState(slug);
}

export type CapResult = {
  state: AdrTrackerState;
  capped: boolean;
  reason?: "regen_cap" | "token_cap";
};

export function incRegen(slug: string): CapResult {
  const cur = readState(slug);
  const state: AdrTrackerState = cur ?? {
    adr: slug,
    regen_count: 0,
    token_used: 0,
    started_at: nowIso(),
    last_updated_at: nowIso(),
  };
  state.regen_count += 1;
  state.last_updated_at = nowIso();
  writeState(state);
  if (state.regen_count > REGEN_CAP) {
    return { state, capped: true, reason: "regen_cap" };
  }
  return { state, capped: false };
}

export function addTokens(slug: string, n: number): CapResult {
  if (n < 0) throw new Error("addTokens: n must be >= 0");
  const cur = readState(slug);
  const state: AdrTrackerState = cur ?? {
    adr: slug,
    regen_count: 0,
    token_used: 0,
    started_at: nowIso(),
    last_updated_at: nowIso(),
  };
  state.token_used += n;
  state.last_updated_at = nowIso();
  writeState(state);
  if (state.token_used > TOKEN_CAP) {
    return { state, capped: true, reason: "token_cap" };
  }
  return { state, capped: false };
}

async function runGh(args: string[]): Promise<{ stdout: string; exit: number }> {
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return { stdout: out.trim(), exit: proc.exitCode ?? 0 };
}

export async function retire(slug: string, issueNum: number, reason: string): Promise<{ ok: boolean }> {
  const state = readState(slug);
  if (state) {
    state.retired = { at: nowIso(), reason };
    state.last_updated_at = nowIso();
    writeState(state);
  }
  // gate:adr-review 削除 + needs-human 付与
  const r1 = await runGh(["issue", "edit", String(issueNum), "--remove-label", "gate:adr-review"]);
  const r2 = await runGh(["issue", "edit", String(issueNum), "--add-label", "needs-human"]);
  // decision-log への append は呼び元 (caller) で行う想定
  return { ok: r1.exit === 0 && r2.exit === 0 };
}

export type ClearRetiredResult = {
  cleared: boolean;
  reason?: "no_tracker_file" | "not_retired" | "needs_human_still_present" | "gh_error";
};

/** ADR-013 運用: retired (regen_cap / token_cap) 状態の ADR を再評価ルートに戻す。
 *  人間が `needs-human` ラベルを外していれば retired フィールドを削除 + regen_count を 0 リセット。
 *  needs-human が残っていれば no-op (= 人間がまだ released していない)。
 *  L-1.5 (auto-accept rescan) 入口で全 gate:adr-review Issue について呼ばれる。 */
export async function clearRetiredIfHumanReleased(
  slug: string,
  issueNum: number,
  ghFn: (args: string[]) => Promise<{ stdout: string; exit: number }> = runGh,
): Promise<ClearRetiredResult> {
  const state = readState(slug);
  if (!state) return { cleared: false, reason: "no_tracker_file" };
  if (!state.retired) return { cleared: false, reason: "not_retired" };

  const r = await ghFn(["issue", "view", String(issueNum), "--json", "labels"]);
  if (r.exit !== 0) return { cleared: false, reason: "gh_error" };
  let labels: Array<{ name: string }>;
  try {
    const parsed = JSON.parse(r.stdout) as { labels?: Array<{ name: string }> };
    labels = parsed.labels ?? [];
  } catch {
    return { cleared: false, reason: "gh_error" };
  }
  if (labels.some(l => l.name === "needs-human")) {
    return { cleared: false, reason: "needs_human_still_present" };
  }

  delete state.retired;
  state.regen_count = 0;
  state.last_updated_at = nowIso();
  writeState(state);
  return { cleared: true };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const cmd = argv[0];
  function arg(name: string): string | undefined {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
  }
  const slug = arg("--adr");
  if (!cmd || !slug) {
    console.error("Usage: loop-adr-regen-tracker.ts <inc-regen|add-tokens|get|retire|clear-retired> --adr <slug> [...]");
    process.exit(1);
  }
  switch (cmd) {
    case "inc-regen": {
      const r = incRegen(slug);
      console.log(JSON.stringify(r));
      process.exit(r.capped ? 2 : 0);
    }
    case "add-tokens": {
      const n = parseInt(arg("--tokens") ?? "0");
      const r = addTokens(slug, n);
      console.log(JSON.stringify(r));
      process.exit(r.capped ? 2 : 0);
    }
    case "get": {
      const s = readState(slug);
      console.log(JSON.stringify(s));
      process.exit(s ? 0 : 1);
    }
    case "retire": {
      const issueNum = parseInt(arg("--issue") ?? "0");
      const reason = arg("--reason") ?? "manual";
      if (!issueNum) {
        console.error("retire: --issue <n> required");
        process.exit(1);
      }
      const r = await retire(slug, issueNum, reason);
      console.log(JSON.stringify(r));
      process.exit(r.ok ? 0 : 1);
    }
    case "clear-retired": {
      const issueNum = parseInt(arg("--issue") ?? "0");
      if (!issueNum) {
        console.error("clear-retired: --issue <n> required");
        process.exit(1);
      }
      const r = await clearRetiredIfHumanReleased(slug, issueNum);
      console.log(JSON.stringify(r));
      process.exit(r.cleared ? 0 : 1);
    }
    default:
      console.error(`unknown command: ${cmd}`);
      process.exit(1);
  }
}
