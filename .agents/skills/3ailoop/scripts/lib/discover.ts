#!/usr/bin/env bun
// discover.ts — OpenCode free-model discovery + filtering (new runtime)
//
// contract §"Free-model policy":
//   1. installed OpenCode CLI を machine-readably に query して provider models を得る。
//   2. candidate は active + input/output cost 0 + tool calling 対応 のときのみ eligible。
//   3. local inference / Ollama / LM Studio / Gemini / nonzero-or-unknown price を機械的に拒否。
//   4. OpenCode 候補を先に評価。空 / 全失敗 / 全期限切れ rate-limit / OpenCode 持続失敗 のみ external fallback。
//
// 入力源: `opencode models --verbose` (provider/model id 行 + JSON block を交互に出力)。
// DI 設計: runOpenCode / now を injection するので test は real CLI 不要 (contract §"Evaluation and registry" #5)。
//
// 関連: contract §"Free-model policy", §"Evaluation and registry"

export interface Cost {
  input: number;
  output: number;
  cache?: { read?: number; write?: number };
}

export interface Capabilities {
  toolcall?: boolean;
  temperature?: boolean;
  reasoning?: boolean;
  attachment?: boolean;
  input?: { text?: boolean; audio?: boolean; image?: boolean; video?: boolean; pdf?: boolean };
  output?: { text?: boolean; audio?: boolean; image?: boolean; video?: boolean; pdf?: boolean };
}

/** A model entry exactly as opencode emits it (the JSON block after the id line). */
export interface OpenCodeModel {
  id: string;
  providerID: string;
  name: string;
  family?: string;
  api?: { id?: string; url?: string; npm?: string };
  status?: string;
  cost?: Cost;
  limit?: { context?: number; input?: number; output?: number };
  capabilities?: Capabilities;
  release_date?: string;
}

export interface RunCliDeps {
  /** run opencode (or a fake) and return its full stdout. */
  runOpenCode: (args: string[]) => Promise<{ ok: boolean; stdout: string; stderr: string }>;
  now: () => Date;
}

/** reason a candidate was rejected, for evidence/audit. */
export type RejectReason =
  | "inactive"
  | "nonzero-cost"
  | "unknown-cost"
  | "no-toolcall"
  | "forbidden-local"
  | "forbidden-gemini"
  | "forbidden-provider-name";

export interface Rejected {
  model: OpenCodeModel;
  reason: RejectReason;
  detail: string;
}

export interface Eligible {
  model: OpenCodeModel;
  /** canonical "provider/model" identity used everywhere downstream. */
  identity: string;
  /** snapshot of the free-tier evidence at discovery time. */
  evidence: {
    status: string;
    input_cost: number;
    output_cost: number;
    toolcall: boolean;
    checked_at: string;
  };
}

export interface DiscoveryResult {
  /** eligible, free, tool-capable candidates in priority order. */
  eligible: Eligible[];
  /** rejected candidates with reasons (audit trail). */
  rejected: Rejected[];
  /** true when the OpenCode candidate set is empty OR all candidates were rejected.
   *  When true, the caller MAY consult the external fallback gate (contract policy #4). */
  opencodeSetExhausted: boolean;
}

// ---------- forbidden-name detection (policy #3) ----------

const FORBIDDEN_LOCAL = [
  "ollama",
  "lm studio",
  "lmstudio",
  "local",
  "llama.cpp",
  "llamacpp",
  "vllm",
  "gpt4all",
  "jan",
  "kobold",
  "llamafile",
];
const FORBIDDEN_GEMINI = ["gemini", "google-ai", "generativelanguage"];

function lower(s: unknown): string {
  return typeof s === "string" ? s.toLowerCase() : "";
}

/** Mechanically detect forbidden providers/models/runtimes (policy #3).
 *  Checks id, providerID, name, family, and api.url/npm for forbidden tokens. */
export function detectForbidden(m: OpenCodeModel): RejectReason | null {
  const hay = [
    m.id,
    m.providerID,
    m.name,
    m.family ?? "",
    m.api?.url ?? "",
    m.api?.npm ?? "",
  ]
    .map(lower)
    .join(" \n ");

  for (const tok of FORBIDDEN_LOCAL) {
    if (hay.includes(tok)) return "forbidden-local";
  }
  for (const tok of FORBIDDEN_GEMINI) {
    if (hay.includes(tok)) return "forbidden-gemini";
  }
  // explicit provider-id blocklist guard (in case a forbidden provider doesn't match tokens)
  const pid = lower(m.providerID);
  if (pid === "ollama" || pid === "lmstudio" || pid === "gemini" || pid === "google") {
    return "forbidden-provider-name";
  }
  return null;
}

// ---------- cost policy (#1, #3: unknown is not free) ----------

/** A cost is "free" only when input AND output are explicit zero.
 *  Any undefined/null/NaN field = unknown price = NOT free (policy #3). */
export function isFreeCost(cost: Cost | undefined): { free: boolean; reason?: RejectReason } {
  if (!cost) return { free: false, reason: "unknown-cost" };
  const { input, output } = cost;
  if (input === undefined || output === undefined) return { free: false, reason: "unknown-cost" };
  if (typeof input !== "number" || typeof output !== "number" || isNaN(input) || isNaN(output)) {
    return { free: false, reason: "unknown-cost" };
  }
  if (input !== 0 || output !== 0) return { free: false, reason: "nonzero-cost" };
  return { free: true };
}

// ---------- the filter ----------

/** Evaluate a single parsed model against the free-model policy. Pure. */
export function evaluate(m: OpenCodeModel, now: () => Date): Eligible | Rejected {
  const forbidden = detectForbidden(m);
  if (forbidden) {
    return { model: m, reason: forbidden, detail: `matched forbidden token in identity fields` };
  }
  if (m.status !== "active") {
    return { model: m, reason: "inactive", detail: `status=${m.status ?? "undefined"}` };
  }
  const cost = isFreeCost(m.cost);
  if (!cost.free) {
    return { model: m, reason: cost.reason!, detail: `cost=${JSON.stringify(m.cost ?? null)}` };
  }
  if (!m.capabilities?.toolcall) {
    return { model: m, reason: "no-toolcall", detail: `toolcall=${m.capabilities?.toolcall ?? "undefined"}` };
  }
  return {
    model: m,
    identity: `${m.providerID}/${m.id}`,
    evidence: {
      status: m.status!,
      input_cost: m.cost!.input,
      output_cost: m.cost!.output,
      toolcall: true,
      checked_at: now().toISOString(),
    },
  };
}

/** Run the full filter over a list of parsed models. Pure. */
export function filterModels(models: OpenCodeModel[], now: () => Date): DiscoveryResult {
  const eligible: Eligible[] = [];
  const rejected: Rejected[] = [];
  for (const m of models) {
    const r = evaluate(m, now);
    if ("identity" in r) eligible.push(r);
    else rejected.push(r);
  }
  // stable priority: larger context first, then more recent release_date, then id.
  eligible.sort((a, b) => {
    const ca = a.model.limit?.context ?? 0;
    const cb = b.model.limit?.context ?? 0;
    if (cb !== ca) return cb - ca;
    const ra = a.model.release_date ?? "";
    const rb = b.model.release_date ?? "";
    if (rb !== ra) return rb < ra ? 1 : -1;
    return a.identity < b.identity ? -1 : 1;
  });
  return {
    eligible,
    rejected,
    opencodeSetExhausted: eligible.length === 0,
  };
}

// ---------- opencode output parser ----------

/**
 * Parse the stdout of `opencode models opencode --verbose`. Format:
 *   <provider>/<model>          ← id line
 *   { ...json block... }        ← model object
 *   <provider>/<model>
 *   { ... }
 *
 * Lines that don't parse as JSON are skipped. Robust to interleaved log noise.
 */
export function parseOpenCodeModels(stdout: string): OpenCodeModel[] {
  const lines = stdout.split("\n");
  const out: OpenCodeModel[] = [];
  let buf: string[] = [];
  let depth = 0;
  let inJson = false;
  const flush = () => {
    if (!inJson || buf.length === 0) return;
    const text = buf.join("\n");
    buf = [];
    inJson = false;
    depth = 0;
    try {
      const obj = JSON.parse(text) as OpenCodeModel;
      if (typeof obj.id === "string" && typeof obj.providerID === "string") {
        out.push(obj);
      }
    } catch {
      // not a valid JSON block — ignore
    }
  };
  for (const line of lines) {
    const trimmed = line.trim();
    if (!inJson) {
      if (trimmed.startsWith("{")) {
        inJson = true;
        buf = [line];
        depth = countBraces(trimmed);
        if (depth === 0) flush();
      }
      continue;
    }
    buf.push(line);
    depth += countBraces(trimmed);
    if (depth <= 0) flush();
  }
  flush();
  return out;
}

function countBraces(s: string): number {
  let d = 0;
  let inStr = false;
  let esc = false;
  for (const ch of s) {
    if (esc) {
      esc = false;
      continue;
    }
    if (ch === "\\") {
      esc = true;
      continue;
    }
    if (ch === '"') {
      inStr = !inStr;
      continue;
    }
    if (inStr) continue;
    if (ch === "{") d++;
    else if (ch === "}") d--;
  }
  return d;
}

// ---------- the discover entrypoint (side-effectful) ----------

/** Query OpenCode, parse, filter. Returns the full discovery result. */
export async function discover(deps: RunCliDeps): Promise<DiscoveryResult> {
  // The provider selector is a policy boundary: without it, configured paid/account
  // providers and automatic routers can appear as apparent zero-cost candidates.
  const r = await deps.runOpenCode(["models", "opencode", "--verbose"]);
  if (!r.ok) {
    // OpenCode persistently failed → exhausted (caller may consult fallback gate)
    return { eligible: [], rejected: [], opencodeSetExhausted: true };
  }
  const models = parseOpenCodeModels(r.stdout);
  return filterModels(models, deps.now);
}
