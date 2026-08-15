#!/usr/bin/env bun
// model-select.ts — qualified free worker 選定 CLI
//
// policy #6: qualified free worker が無ければ stop & notify。exit 1 + reason を出す。
//
// 使い方:
//   bun model-select.ts [--discover-if-empty] [--fallback-candidates <file>]
//
// 流れ:
//   1. model-registry から qualified model を探す。
//   2. 無い + --discover-if-empty: model-discover を走らせ、eligible があれば probation で仮登録。
//      ただし probation は qualified ではないため選定結果としては扱わない (policy #6)。
//      仮登録したうえで、 qualified model が無い場合と同じく fallback / stop へ進む。
//   3. qualified が無い: external fallback gate を評価 (--fallback-candidates 指定時のみ)。
//   4. それでも無い: exit 1 + "NO_QUALIFIED_FREE_WORKER"。
//
// 出力 (stdout JSON): { identity, status } or { stop: "NO_QUALIFIED_FREE_WORKER" | "STOP_FOR_USER", reason }
// exit: 0=選定成功 (qualified or external fallback gate 承認) / 1=stop & notify / 2=usage error

import { conservativeTaskProfile, parseSliceRequirements, readModelRegistry, selectQualified, selectQualifiedForSlice, mutateModelRegistry, upsertModel, MODEL_REGISTRY_PATHS, type ModelEntry } from "./lib/model-registry.ts";
import { discover } from "./lib/discover.ts";
import { runFallbackGate, type ExternalCandidate } from "./lib/external-fallback.ts";
import { readFileSync } from "fs";
import { arg, emitJson, emitStatus, runCmd } from "./cli/helpers.ts";

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const discoverIfEmpty = args.includes("--discover-if-empty");
  const fallbackFile = arg(args, "--fallback-candidates");
  const registryPath = arg(args, "--registry-path") ?? MODEL_REGISTRY_PATHS.DEFAULT_REGISTRY_PATH;
  const requirementsFile = arg(args, "--requirements");

  const reg = readModelRegistry(registryPath);
  const now = () => new Date();
  let requirements;
  try {
    requirements = requirementsFile
      ? parseSliceRequirements(JSON.parse(readFileSync(requirementsFile, "utf-8")))
      : undefined;
  } catch (e) {
    emitJson({ stop: "INVALID_SLICE_REQUIREMENTS", reason: (e as Error).message });
    return 2;
  }
  let chosen = requirements ? selectQualifiedForSlice(reg, requirements, now) : selectQualified(reg, now);

  if (chosen) {
    emitStatus(`selected qualified model: ${chosen.identity}`);
    emitJson({ identity: chosen.identity, status: chosen.status, task_profile: chosen.task_profile, slice_requirements: requirements ?? null });
    return 0;
  }

  // try discovery (registers eligible candidates as PROBATION — qualification still pending).
  // A probation model is NOT a qualified free worker: it must never be returned as a
  // selection (policy #6: only `qualified` free workers are selectable; otherwise stop &
  // notify). We register it for a later evaluation run, then fall through to the external
  // fallback gate / stop-for-user path exactly as if no qualified model existed.
  let opencodeExhausted = true; // absent eligible qualified models ⇒ may consult fallback
  if (discoverIfEmpty) {
    const res = await discover({
      runOpenCode: async (cliArgs) => runCmd(["opencode", ...cliArgs]),
      now,
    });
    opencodeExhausted = res.opencodeSetExhausted;
    if (res.eligible.length > 0) {
      const top = res.eligible[0];
      const entry: ModelEntry = {
        identity: top.identity,
        harness: "opencode",
        provider: top.model.providerID,
        model: top.model.id,
        status: "probation",
        evaluation: {
          suite_version: reg.suite_version,
          passed: 0,
          total: 0,
          passed_classes: [],
          tested_at: now().toISOString(),
        },
        supported_task_class: "general",
        task_profile: conservativeTaskProfile(),
        free_tier: {
          ...top.evidence,
          expiry: null,
          expiry_known: false,
        },
        observations: { successes: 0, failures: 0, rate_limits: 0, last_success_at: null, last_failure_at: null },
        last_evaluated_at: now().toISOString(),
      };
      mutateModelRegistry(registryPath, r => upsertModel(r, entry));
      emitStatus(`discovered + provisionally registered (probation, NOT selected yet): ${top.identity}`);
      // explicitly do NOT return here — a probation model is not qualified, so selection
      // must continue to the fallback gate or stop & notify (policy #6).
    }
  }

  // external fallback gate — only when OpenCode is actually exhausted (policy #4). The gate
  // validates an external candidate's structural eligibility, but a passing candidate is
  // NOT yet qualified: it has not run the fixed evaluation suite (policy #3). It is
  // registered as probation and selection stops & notifies (policy #6) — the operator must
  // run qualification before this model can be selected. The gate is never allowed to
  // return exit 0 as a directly-usable worker (F5).
  if (fallbackFile) {
    let candidates: ExternalCandidate[] = [];
    try {
      candidates = JSON.parse(readFileSync(fallbackFile, "utf-8")) as ExternalCandidate[];
    } catch (e) {
      emitStatus(`bad fallback-candidates file: ${(e as Error).message}`);
    }
    const gate = runFallbackGate({ opencodeExhausted, candidates });
    if (gate.stopForUser) {
      emitJson({ stop: "STOP_FOR_USER", reason: gate.stopForUser });
      return 1;
    }
    if (gate.chosen) {
      // Provisionally register the external candidate as probation (harness recorded so a
      // later qualification run knows which resumable CLI/API to use). Still NOT selected.
      const c = gate.chosen;
      const entry: ModelEntry = {
        identity: c.identity,
        harness: c.harness,
        provider: c.provider,
        model: c.model,
        status: "probation",
        evaluation: {
          suite_version: reg.suite_version,
          passed: 0,
          total: 0,
          passed_classes: [],
          tested_at: now().toISOString(),
        },
        supported_task_class: "general",
        task_profile: conservativeTaskProfile(),
        free_tier: {
          status: "active",
          input_cost: c.cost?.input ?? 0,
          output_cost: c.cost?.output ?? 0,
          toolcall: c.toolcall,
          checked_at: now().toISOString(),
          expiry: null,
          expiry_known: false,
        },
        observations: { successes: 0, failures: 0, rate_limits: 0, last_success_at: null, last_failure_at: null },
        last_evaluated_at: now().toISOString(),
      };
      mutateModelRegistry(registryPath, r => upsertModel(r, entry));
      emitStatus(`external candidate passed gate, registered as probation (NOT qualified yet): ${c.identity}`);
      // fall through to policy #6 — no qualified free worker exists yet.
    }
  }

  // policy #6: stop & notify
  emitJson({ stop: "NO_QUALIFIED_FREE_WORKER", reason: requirements ? "no qualified free cloud model fits this slice; split it smaller or qualify another worker" : "no qualified free cloud model available; Codex/paid fallback forbidden (new probation candidates await evaluation)" });
  return 1;
}

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}
