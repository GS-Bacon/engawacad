---
name: 3ailoop-work-select
description: Select the next actionable EngawaCAD Issue for the loop. Reads CAS assign-state and excludes closed and already-assigned Issues (prevents #333 stale-plan reassignment), then orders candidates by tier (split-batch > phase-feature > foundation > general) and drops gate/needs/blocked-by-split labels.
allowed-tools: Read, Bash, Glob, Grep
---

# $3ailoop-work-select — work selection

次に着手する actionable Issue を **決定的に** 選ぶ。旧 `batch-select.ts --loop` の tier 付き選定を
引き継ぎつつ、CAS state と連携して **closed / assigned の Issue を構造的に除外** する (#333)。

## 手順

1. 候補 Issue を `gh issue list` から生成し `candidates.json` に書く (Codex が JSON を組む):
   ```
   [{ "number": 295, "tier": "phase-feature", "title": "...", "labels": ["type:feature"] }, ...]
   ```
   tier は split-batch (親 `blocked-by-split` の子) → phase-feature (現 Phase milestone の type:feature)
   → foundation (type:foundation / batch:*) → general の順。

2. 選定 script を呼ぶ:
   ```bash
   bun .agents/skills/3ailoop/scripts/work-select.ts --candidates candidates.json
   ```
   - exit 0: 次候補あり (stdout JSON の `next` が次 Issue)
   - exit 1: actionable なし (= stop 要因)。`dropped` で除外理由を確認。

3. Codex は `next` を受け取り L-2 (plan) へ。`next` が無ければ stop & notify。

## 不変量 (lib/work-select.ts)

- closed issue → 除外 (reason: closed)
- CAS assigned (assigned/verifying/landing) → 除外 (reason: already-assigned, 二重割当防止)
- `gate:*` / `needs-*` / `blocked-by-split` label → 除外
- 順序は tier asc, さらに number asc で決定的

詳細は [../3ailoop/references/runtime.md](../3ailoop/references/runtime.md) の CAS state 節。
