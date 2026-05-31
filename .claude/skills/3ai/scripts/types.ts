// types.ts — /3ai スクリプト共有型定義

export interface PhaseStats {
  glm_runs: number;
}

export interface Judgment {
  round: number;
  adopted: number;
  rejected: number;
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
