#!/usr/bin/env bun
// work-select.ts — 次に着手する actionable Issue の選定 (pure, DI testable)
//
// contract §"Orchestration and authority" #1/#2 と既存 batch-select の tier 付き選定を
// 踏襲しつつ、CAS state と連携する: closed issue / already-assigned issue は除外する。
// (#333: stale plan が closed Issue を再割当するのを構造的に防ぐ)
//
// 関連: contract §"Orchestration and authority", §"State, receipts, Git, and landing"

export type IssueTier = "split-batch" | "phase-feature" | "foundation" | "general";

export interface CandidateIssue {
  number: number;
  tier: IssueTier;
  title: string;
  /** labels on the issue. */
  labels: string[];
}

export interface WorkSelectInput {
  candidates: CandidateIssue[];
  /** CAS-closed issue numbers — always excluded. */
  closedIssues: Set<number>;
  /** CAS-assigned (in-flight) issue numbers — excluded to avoid double-assign. */
  assignedIssues: Set<number>;
  /** label substrings that disqualify (gate:* / needs-* / blocked-by-split). */
  excludeLabels: string[];
}

export interface WorkSelectResult {
  /** chosen issue numbers in priority order (most actionable first). */
  ordered: CandidateIssue[];
  /** the single next pick, or null when nothing actionable. */
  next: CandidateIssue | null;
  /** reasons issues were dropped (audit). */
  dropped: { number: number; reason: string }[];
}

const TIER_RANK: Record<IssueTier, number> = {
  "split-batch": 0,
  "phase-feature": 1,
  foundation: 2,
  general: 3,
};

/** Pure selection. Deterministic ordering: tier asc, then number asc. */
export function selectWork(input: WorkSelectInput): WorkSelectResult {
  const dropped: { number: number; reason: string }[] = [];
  const kept: CandidateIssue[] = [];

  for (const c of input.candidates) {
    if (input.closedIssues.has(c.number)) {
      dropped.push({ number: c.number, reason: "closed" });
      continue;
    }
    if (input.assignedIssues.has(c.number)) {
      dropped.push({ number: c.number, reason: "already-assigned" });
      continue;
    }
    const labelHit = c.labels.find(l => input.excludeLabels.some(ex => l.includes(ex)));
    if (labelHit) {
      dropped.push({ number: c.number, reason: `label:${labelHit}` });
      continue;
    }
    kept.push(c);
  }

  kept.sort((a, b) => {
    const ta = TIER_RANK[a.tier];
    const tb = TIER_RANK[b.tier];
    if (ta !== tb) return ta - tb;
    return a.number - b.number;
  });

  return { ordered: kept, next: kept[0] ?? null, dropped };
}

/** Default exclude label substrings (matches old loop behavior). */
export const DEFAULT_EXCLUDE_LABELS = [
  "gate:",
  "needs-",
  "blocked-by-split",
];
