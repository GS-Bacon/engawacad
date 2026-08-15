#!/usr/bin/env bun
// consult.ts — needs_supervisor 相談と resume の純粋な型付け (new runtime)
//
// contract §"Orchestration and authority" #3:
//   worker が material choice で block したとき needs_supervisor receipt (question/options/recommendation)
//   を出す。Codex が決定し、同じ worker session で resume できる。
//
// consult の状態機械は CAS state とは別に、features/.loop/consultations.jsonl に追記される。
// resume は receipt の consultation id を codex decision と紐付けて worker に返す。
//
// 関連: contract §"Orchestration and authority" #3

import { appendFileSync, mkdirSync } from "fs";
import { dirname, resolve } from "path";

const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
const DEFAULT_CONSULT_PATH = resolve(REPO_ROOT, "features/.loop/consultations.jsonl");

export type ConsultStatus = "open" | "answered" | "resolved" | "abandoned";

export interface ConsultRecord {
  id: string;
  issue: number;
  worker_id: string;
  question: string;
  options: string[];
  recommendation: string;
  status: ConsultStatus;
  asked_at: string;
  decision?: string;
  decided_at?: string;
  resolved_at?: string;
}

/** Append a new open consultation. Returns the record. */
export function openConsultation(
  rec: Omit<ConsultRecord, "status" | "asked_at">,
  now: () => Date,
): ConsultRecord {
  return { ...rec, status: "open", asked_at: now().toISOString() };
}

/** Codex records a decision on an open consultation (resume seed). */
export function answerConsultation(rec: ConsultRecord, decision: string, now: () => Date): ConsultRecord {
  if (rec.status !== "open") return rec;
  return { ...rec, status: "answered", decision, decided_at: now().toISOString() };
}

/** The same worker session resumes after the decision and closes the loop. */
export function resolveConsultation(rec: ConsultRecord, now: () => Date): ConsultRecord {
  if (rec.status !== "answered") return rec;
  return { ...rec, status: "resolved", resolved_at: now().toISOString() };
}

/** Append a consult record to the JSONL log (best-effort). */
export function appendConsult(rec: ConsultRecord, path: string = DEFAULT_CONSULT_PATH): void {
  mkdirSync(dirname(path), { recursive: true });
  appendFileSync(path, JSON.stringify(rec) + "\n", "utf-8");
}

export const CONSULT_PATHS = {
  DEFAULT_CONSULT_PATH,
};
