#!/usr/bin/env bun
// external-fallback.ts — OpenCode 候補が尽きたときのみ入る external free provider の gate
//
// contract §"Free-model policy" #4/#5/#6:
//   - OpenCode candidate set が空 / 全失敗 / 全期限切れ rate-limit / OpenCode 持続失敗 のみ external を評価。
//   - external は cloud-hosted, non-Gemini, officially free, resumable CLI/API, pinned provider+model,
//     paid/automatic fallback 不可。automatic free-model router 禁止。
//   - account 作成 / terms 承諾 / billing 情報 / credentials 欠落 なら stop for the user (bypass しない)。
//   - qualified free worker が無ければ stop & notify。Codex を worker にしない。paid fallback しない。
//
// 本モジュールは「fallback に入ってよいか」「external 候補が要件を満たすか」を純粋関数で判定する。
// 実際の CLI/API 呼び出しは run-worker 側。DI で test 可能。
//
// 関連: contract §"Free-model policy"

import { detectForbidden, isFreeCost } from "./discover.ts";

export interface ExternalCandidate {
  /** canonical identity, e.g. "openrouter/deepseek/deepseek-chat:free". */
  identity: string;
  provider: string;
  model: string;
  /** cloud-hosted base url or harness descriptor. */
  harness: string;
  /** whether the provider's free tier is officially documented (not a promo/trial). */
  officially_free: boolean;
  /** whether it is usable noninteractively through a resumable CLI/API. */
  resumable_cli: boolean;
  /** whether paid/automatic fallback is possible on this harness (must be false). */
  paid_automatic_fallback_possible: boolean;
  /** supports tool calling. */
  toolcall: boolean;
  cost?: { input: number; output: number };
  /** account/terms/billing/credentials preconditions met. */
  credentials_present: boolean;
  terms_accepted: boolean;
}

export interface FallbackGateInput {
  /** result of the OpenCode discovery. */
  opencodeExhausted: boolean;
  /** external candidates to evaluate. */
  candidates: ExternalCandidate[];
}

export interface FallbackGateResult {
  /** whether the fallback path is even allowed to run. */
  mayEnterFallback: boolean;
  /** the chosen candidate, if any. */
  chosen: ExternalCandidate | null;
  /** why each candidate was rejected (audit). */
  rejected: { identity: string; reason: string }[];
  /** if a precondition (account/terms/billing/credentials) is missing, the user-facing
   *  stop reason. The harness MUST stop for the user rather than bypass (policy #5). */
  stopForUser: string | null;
}

/** Evaluate whether entering the external fallback is permitted (policy #4). */
export function mayEnterFallback(opencodeExhausted: boolean): boolean {
  return opencodeExhausted;
}

/** Validate one external candidate against policy #5. Returns a reason or null if ok. */
export function validateExternal(c: ExternalCandidate): string | null {
  if (!c.credentials_present) {
    return "credentials missing — stop for the user (do not bypass)";
  }
  if (!c.terms_accepted) {
    return "terms not accepted — stop for the user (do not bypass)";
  }
  if (c.paid_automatic_fallback_possible) {
    return "paid/automatic fallback possible — forbidden";
  }
  if (!c.officially_free) {
    return "not officially free (promo/trial) — forbidden";
  }
  if (!c.resumable_cli) {
    return "not usable through a resumable CLI/API — forbidden";
  }
  if (!c.toolcall) {
    return "no tool calling — forbidden";
  }
  const forbidden = detectForbidden({
    id: c.model,
    providerID: c.provider,
    name: c.identity,
    family: c.provider,
    api: { url: c.harness },
  });
  if (forbidden === "forbidden-gemini") {
    return "Gemini — forbidden";
  }
  if (forbidden === "forbidden-local") {
    return "local inference — forbidden";
  }
  if (forbidden === "forbidden-provider-name") {
    return `forbidden provider (${c.provider}) — forbidden`;
  }
  const cost = isFreeCost(c.cost);
  if (!cost.free) {
    return cost.reason === "nonzero-cost" ? "nonzero cost — forbidden" : "unknown cost — forbidden";
  }
  return null;
}

/** Run the full external fallback gate. */
export function runFallbackGate(input: FallbackGateInput): FallbackGateResult {
  if (!mayEnterFallback(input.opencodeExhausted)) {
    return {
      mayEnterFallback: false,
      chosen: null,
      rejected: [],
      stopForUser: null,
    };
  }

  const rejected: { identity: string; reason: string }[] = [];
  let stopForUser: string | null = null;

  for (const c of input.candidates) {
    const reason = validateExternal(c);
    if (reason === null) {
      return { mayEnterFallback: true, chosen: c, rejected, stopForUser: null };
    }
    rejected.push({ identity: c.identity, reason });
    if (reason.includes("stop for the user") && stopForUser === null) {
      stopForUser = `${c.identity}: ${reason}`;
    }
  }

  return {
    mayEnterFallback: true,
    chosen: null,
    rejected,
    // no usable candidate; surface a user-facing stop reason if a precondition was the blocker,
    // otherwise the harness stops & notifies "no qualified free worker" (policy #6).
    stopForUser,
  };
}
