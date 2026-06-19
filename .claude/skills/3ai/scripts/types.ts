// types.ts — /3ai スクリプト共有型定義

export interface PhaseStats {
  glm_runs: number;
}

export interface Judgment {
  round: number;
  adopted: number;
  rejected: number;
}

/** raise-issue-on-failure.ts が起票した Issue の記録 */
export interface RaisedIssue {
  /** GitHub Issue 番号 */
  number: number;
  /** 起票したステップ名 (STEP X-Y ...) */
  step: string;
}

export interface StateData {
  issue: number;
  slug: string;
  steps: Record<string, string>;
  loops: Record<string, number>;
  judgments: Judgment[];
  phases: {
    core_impl: PhaseStats;
    test_impl: PhaseStats;
  };
  /** このフィーチャーで自動起票された GitHub Issue のリスト */
  raised_issues?: RaisedIssue[];
}

export interface VerdictData {
  verdict: string;
  blocking: number;
  severity_counts?: {
    critical: number;
    high: number;
    medium: number;
    low: number;
  };
}

export type Deliverable = "code" | "docs";
export type ReviewMode = "design" | "final";

export interface ReviewConfig {
  deliverable: Deliverable;
  reviewInstructionPath: string;
  maxLoops: number;
  scopeHint: string;
}

// --- バッチモード型 ---

export type BatchFlowType = "light" | "full";
export type BatchGateType = "auto" | "pause";
export type BatchTierType = "split-batch" | "bug-batch" | "enh-batch" | "foundation-batch" | "refactor-batch" | "phase-feature";

export interface BatchIssue {
  number: number;
  slug: string;
  title: string;
  labels: string[];
  deliverable: Deliverable;
  flow: BatchFlowType;
  gate: BatchGateType;
  pause_reasons: string[];
  ambiguous: boolean;
  intent_check_required: boolean;
  keep_codex_gate: boolean;
  deps: number[];
  raw_refs: number[];
}

export interface BatchGroup {
  group: string;
  order: number;
  issues: BatchIssue[];
}

export interface BatchPlan {
  generated_at: string;
  batch_start_sha: string;
  tier: BatchTierType;
  batch_arg: "fixes" | "phase" | null;
  current_phase: number | null;
  groups: BatchGroup[];
  warnings: string[];
}
