---
name: 3ailoop-worker-select
description: Select the best currently available worker from the already-qualified free-cloud pool. Use for each 3ailoop assignment. Stops and invokes the separate qualification workflow when the pool is empty or needs replenishment; never selects probation, local, Gemini, paid, unknown-cost, expired, or rate-limited models.
allowed-tools: Read, Bash, Glob, Grep
---

# $3ailoop-worker-select — select a free worker

認定済みpoolから現在利用可能なfree cloud modelを選ぶ。発見・評価・補充は `$3ailoop-worker-qualify` が担当する。

## 手順

1. model-registryからqualified modelを選ぶ。
   ```bash
   bun .agents/skills/3ailoop/scripts/model-select.ts --requirements <slice>.requirements.json
   ```
   - exit 0: 選定成功 (stdout に `identity`)
   - exit 1: **NO_QUALIFIED_FREE_WORKER** または STOP_FOR_USER → **stop & notify**
   - `task_profile`不適合: 別qualifiedを選ぶ。全員不適合ならsliceを小さく分割し、基準を緩めない。

2. `NO_QUALIFIED_FREE_WORKER` またはpoolが3件未満なら `$3ailoop-worker-qualify` を使う。OpenCode候補が完全に尽きた場合だけexternal fallback gateを検討する。

3. (必要時のみ) external fallback gate:
   ```bash
   bun .agents/skills/3ailoop/scripts/model-select.ts --discover-if-empty --fallback-candidates external.json
   ```
   external 候補は cloud / non-Gemini / officially free / resumable CLI / pinned provider+model /
   paid-fallback 不可能。account/terms/billing/credentials 欠落なら **stop for the user**。

## 拒否ルール

- `status != active` → inactive
- input/output cost のいずれかが **未定義/非数** → unknown-cost (**unknown は free ではない**)
- input/output cost のいずれかが **非ゼロ** → nonzero-cost
- `toolcall != true` → no-toolcall
- id/provider/name/family/url に `ollama|lm studio|local|llama.cpp|vllm|...` → forbidden-local
- 同上に `gemini|google-ai` → forbidden-gemini

## 再評価 (lib/model-registry.ts `needsReevaluation`)

初期 + その後は **expiry / auth 失敗 / rate limit / harness 持続失敗 / 品質劣化** 時のみ。
**定期週次 discovery はしない** (AGENTS.md)。

選考と補充の詳細は `$3ailoop-worker-qualify`。runtime不変量は [../3ailoop/references/runtime.md](../3ailoop/references/runtime.md) を参照。
