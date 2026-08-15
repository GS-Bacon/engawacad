#!/usr/bin/env bun
// worker-pane.ts — operator-facing per-worker tmux pane lifecycle (new runtime)
//
// contract §"Orchestration and authority" #5/#6:
//   #5  各 active external worker に右側 専用 pane を割り当て、通常の CLI 出力を流す。
//       pane の header/status line は Issue / provider / pinned model / session / lifecycle state
//       を識別する。orchestrator pane は使えるまま。N=3 layout を維持。credential や hidden
//       reasoning trace は表示しない。
//   #6  pane は operator interface のみ。scheduling / completion / failure detection /
//       consultation / resume は durable state + receipts + event-driven termination で行い、
//       pane scrape / process polling には依存しない。terminal 出力は確認可能な期間残し、
//       明示的・決定的な cleanup/reuse 遷移で後始末する。
//
// 設計 (他の lib と同じ DI 原則):
// - 実際の tmux 呼び出しは WorkerPaneDeps (tmux adapter) の裏に隠す。test は fake adapter を
//   注入し、real tmux / 外部書き込みを一切起こさない (contract acceptance #99/#100)。
// - allocatePane は「右側 split (-h)」で pane を 1 つ作り、pane_id を返す。失敗は明示的な
//   bounded state (outcome: "allocate-failed") に落ち、CAS/registry は触らない。
// - setStatusLine は 5 つの識別フィールド (issue / provider / model / session / state) を
//   pane の status-line に設定する。credential / reasoning trace は含めない (redact 済み)。
// - streamLaunch は launch.command を pane へ send する (worker の通常 CLI 出力を pane に流す)。
//   これは operator 可視化のみで、launch 自体の実行成否・終了検知は event-driven delegation
//   (AGENTS.md) が担う。ここでは pane を poll しない。
// - releasePane は terminal outcome 後の決定的 cleanup。pane は「reuse 可能」状態に戻るか kill する。
//   cleanup は lifecycle (CAS/registry) に一切影響しない — pane が壊れていても loop は正しい。
//
// このモジュール自体は pure-ish (副作用は deps 経由のみ)。real tmux adapter は
// scripts/worker-pane.ts (CLI) が注入する。

export type PaneLifecycleState =
  | "allocated"   // pane 確保済み・worker 起動待ち or 実行中
  | "streaming"   // launch を pane へ投入済み
  | "released";   // terminal 後 cleanup 済み (reuse 可能 or killed)

/** pane を識別する 5 フィールド (contract §Orchestration #5)。
 *  credential / hidden reasoning trace は絶対に含めない。 */
export interface PaneIdentity {
  issue: number;
  workerId: string;
  /** pinned free-model identity (provider/model). */
  provider: string;
  model: string;
  /** opaque worker session/run id (委譲 marker や resume id と同じ粒度)。 */
  session: string;
}

/** tmux 操作の抽象。real 実装は scripts/worker-pane.ts、test は fake を注入する。 */
export interface WorkerPaneDeps {
  /** 右側に pane を split して新 pane_id を返す。失敗時は例外ではなく {ok:false}。 */
  splitRight: (opts: { splitPct: number }) => Promise<{ ok: true; paneId: string } | { ok: false; error: string }>;
  /** pane の status-line (title 等) を設定。 */
  setStatusLine: (paneId: string, text: string) => Promise<{ ok: boolean; error?: string }>;
  /** pane へコマンド文字列を送信 (worker の CLI 起動を流す)。 */
  sendKeys: (paneId: string, command: string) => Promise<{ ok: boolean; error?: string }>;
  /** pane を cleanup (kill または reuse-mark)。lifecycle 影響なし。 */
  release: (paneId: string) => Promise<{ ok: boolean; error?: string }>;
  /** pane の現在内容を取得 (operator 確認用・テストの観測点)。scrape による状態判定には使わない。 */
  capture: (paneId: string) => Promise<{ ok: boolean; output: string }>;
  now: () => number;
}

export interface PaneRecord {
  paneId: string;
  /** Present when allocated in-process; a stateless CLI release may omit it (cosmetic
   *  status refresh is skipped, pane is killed regardless). */
  identity?: PaneIdentity;
  state: PaneLifecycleState;
  /** 最後に設定した status-line 文字列 (redact 済み・credential なし)。 */
  statusLine: string;
  allocatedAt: number;
}

/** credential / hidden-reasoning と思しき token を status-line から弾くための denylist。
 *  model identity の一部 (provider/model 名) は許可するが、API key 構文や reasoning
 *  marker は機械的に除去する。 */
const CREDENTIAL_OR_TRACE_PATTERNS: RegExp[] = [
  /sk-[A-Za-z0-9_-]{8,}/g,            // OpenAI-style keys
  /[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{16,}/g, // JWT-ish / bearer tokens
  /\b(Bearer|Authorization|X-Api-Key|ANTHROPIC_API_KEY|OPENAI_API_KEY)\b[^\s]*/gi,
  /<thinking>[\s\S]*?<\/thinking>/g,  // hidden reasoning trace
  /<reasoning>[\s\S]*?<\/reasoning>/g,
];

/** Build the operator-facing status line from a PaneIdentity + lifecycle state.
 *  Deterministic, single line, newline-free, credentials/traces redacted. Pure. */
export function buildStatusLine(identity: PaneIdentity, state: PaneLifecycleState): string {
  const raw = [
    `issue=#${identity.issue}`,
    `worker=${identity.workerId}`,
    `provider=${identity.provider}`,
    `model=${identity.model}`,
    `session=${identity.session}`,
    `state=${state}`,
  ].join(" ");
  return redact(raw);
}

/** Mechanically redact credential / hidden-reasoning tokens from a status string. Pure. */
export function redact(text: string): string {
  let out = text;
  for (const re of CREDENTIAL_OR_TRACE_PATTERNS) {
    out = out.replace(re, "[redacted]");
  }
  // status-line must be single-line; collapse any residual newlines from redaction
  return out.replace(/[\r\n]+/g, " ").trim();
}

/** True if a candidate status-line text is safe (no credential / no reasoning trace). Pure. */
export function isSafeStatusLine(text: string): boolean {
  for (const re of CREDENTIAL_OR_TRACE_PATTERNS) {
    if (re.test(text)) return false;
    re.lastIndex = 0; // reset for global regex reuse
  }
  return !/[\r\n]/.test(text);
}

export interface AllocateResult {
  ok: boolean;
  pane?: PaneRecord;
  error?: string;
}

/** Allocate a dedicated right-side pane for an active worker and stamp its status line.
 *  Returns the pane record (with the redacted status line) or a bounded failure.
 *  Does NOT touch CAS/registry — pane is operator-only (contract #6). */
export async function allocatePane(
  identity: PaneIdentity,
  deps: WorkerPaneDeps,
  opts: { splitPct?: number } = {},
): Promise<AllocateResult> {
  const splitPct = opts.splitPct ?? DEFAULT_SPLIT_PCT;
  const split = await deps.splitRight({ splitPct });
  if (!split.ok) {
    return { ok: false, error: `split-right failed: ${split.error}` };
  }
  const statusLine = buildStatusLine(identity, "allocated");
  const sl = await deps.setStatusLine(split.paneId, statusLine);
  if (!sl.ok) {
    // best-effort release the half-allocated pane; lifecycle is unaffected either way
    await deps.release(split.paneId);
    return { ok: false, error: `set-status-line failed: ${sl.error ?? "unknown"}` };
  }
  return {
    ok: true,
    pane: {
      paneId: split.paneId,
      identity,
      state: "allocated",
      statusLine,
      allocatedAt: deps.now(),
    },
  };
}

/** Stream the worker's launch command into its pane (operator visibility of ordinary CLI
 *  output). Marks the pane streaming. Does NOT detect completion — that is event-driven.
 *  Refreshes the status line to `streaming` using the pane's identity (in-process callers
 *  hold the full PaneRecord from allocatePane). */
export async function streamLaunch(
  pane: PaneRecord,
  command: string,
  deps: WorkerPaneDeps,
): Promise<{ ok: boolean; error?: string }> {
  // refresh status line to streaming
  const streaming = buildStatusLine(pane.identity, "streaming");
  await deps.setStatusLine(pane.paneId, streaming);
  pane.statusLine = streaming;
  pane.state = "streaming";
  const sent = await deps.sendKeys(pane.paneId, command);
  if (!sent.ok) {
    return { ok: false, error: `send-keys failed: ${sent.error ?? "unknown"}` };
  }
  return { ok: true };
}

/** Deterministic terminal cleanup: release the pane back to reusable / killed and stamp
 *  the final lifecycle state. Idempotent (releasing an already-released pane is a no-op).
 *  Never throws into the caller's lifecycle path.
 *
 *  `identity` is optional: in-process callers pass the PaneRecord (identity known); a
 *  stateless CLI release without identity skips the cosmetic status refresh (the pane is
 *  being killed regardless). Either way lifecycle (CAS/registry) is untouched. */
export async function releasePane(
  pane: PaneRecord,
  deps: WorkerPaneDeps,
): Promise<{ ok: boolean; error?: string }> {
  if (pane.state === "released") {
    return { ok: true }; // idempotent
  }
  if (pane.identity) {
    const finalLine = buildStatusLine(pane.identity, "released");
    await deps.setStatusLine(pane.paneId, finalLine);
    pane.statusLine = finalLine;
  }
  const r = await deps.release(pane.paneId);
  pane.state = "released";
  return r;
}

/** Capture the pane's current output for operator inspection / test observation.
 *  This is an observation point ONLY — callers MUST NOT branch lifecycle decisions on it. */
export async function capturePane(
  pane: PaneRecord,
  deps: WorkerPaneDeps,
): Promise<{ ok: boolean; output: string }> {
  return deps.capture(pane.paneId);
}

// Right-side pane width (percent). Matches the old runtime's LOOP_TMUX_PANE_PCT default.
export const DEFAULT_SPLIT_PCT = parseInt(process.env.LOOP_TMUX_PANE_PCT ?? "50", 10);

/** Names of the 5 identifying fields the status line must carry (contract #5).
 *  Used by tests/dry-run to prove coverage. */
export const REQUIRED_STATUS_FIELDS = ["issue", "worker", "provider", "model", "session", "state"] as const;

/** Assert (for dry-run / tests) that a status line carries every required identifying field. Pure. */
export function statusLineHasAllFields(statusLine: string): boolean {
  return REQUIRED_STATUS_FIELDS.every(f => statusLine.includes(`${f}=`) || statusLine.includes(`${f}:`));
}
