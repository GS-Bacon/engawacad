---
name: 3ailoop
description: Codex-orchestrated autonomous loop that digests EngawaCAD Issues by delegating implementation work to qualified free cloud models (OpenCode first). Sequences the component Skills for work selection, planning, worker selection, worker execution, delivery verification, serial landing, and Phase maintenance. Replaces the ClaudeCode/GLM-centered loop; the active worker path never requires ClaudeCode, GLM, Gemini, or a local model.
allowed-tools: Read, Bash, Glob, Grep, Skill
---

# $3ailoop — Codex-orchestrated autonomous loop (free-cloud workers)

EngawaCAD の Issue を **Codex が指揮し、適格な無料クラウドモデルに実装を委譲** して無人で消化する
Skill。旧 `/3ailoop` (ClaudeCode/GLM 中心) を置き換える。**活性 worker 経路は ClaudeCode / GLM /
Gemini / local model に依存しない**。

**前提・権限**:
- Codex が orchestrator。全 Issue の plan / 全 worker consultation 決定 / 最終 landing 決定を Codex が行う。
- worker は適格な **free cloud model** (OpenCode 優先)。isolated worktree 内でのみ inspect/edit/test/local-commit 可。
- worker は push / GitHub mutate / default-branch land / 別 worker worktree 変更 / shared stash 禁止。
- 適格な無料 worker が無ければ **停止して通知**。Codex を worker にしない。有料 fallback しない。
- 詳細な schema / 安全規則 / 不変量は [references/runtime.md](references/runtime.md) を参照。

## 実行フロー (component Skills を順に呼ぶだけ)

Codex は薄い orchestrator として以下を順に実行する。各 STEP は **1 つの component Skill または
1 つの決定的 script** を呼ぶ。判定は script 側 (TS)。

### A. cutover 確認 (初回または明示時のみ)

```bash
bun .agents/skills/3ailoop/scripts/cutover.ts --check
```

old runtime (旧 `.claude` 系 watcher / lock / tmux) が残っていれば停止/正規化してから新 runtime を
有効化する。**絶対に両 orchestrator を同時に動かさない**。cutover の実行はユーザー明示承認時のみ。

### L-0. lock 取得

```bash
TOKEN=$(bun .agents/skills/3ailoop/scripts/loop-lock.ts acquire --owner loop)
```

### L-1. work 選定 → `$3ailoop-work-select`

次の actionable Issue を選ぶ。CAS state を読み、**closed / assigned の Issue を除外**する (#333)。
詳細は `$3ailoop-work-select` Skill。

### L-2. plan (Codex) → `$3ailoop-plan`

選んだ Issue を Codex がplan.mdへ落とし、単独検証可能なdelivery sliceへ分ける
(AGENTS.md ADR-006 の粒度ガード適用)。分割は成果単位で行い、slice内の実装手順はworkerへ委ねる。

### L-3. free worker 選定 → `$3ailoop-worker-select`

qualified poolから適格workerを選ぶ。pool不足・失効・rate limit・品質低下時だけ
`$3ailoop-worker-qualify` で補充する。無ければ **stop & notify**。
詳細は `$3ailoop-worker-select` Skill。

### L-4. worker 起動/再開 → `$3ailoop-worker-run`

worker に現在のslice contractを委譲 (async, event-driven, **polling なし**)。worktree 内で自由に
調査・実装・test → local commit + receipt.jsonを出す。
`needs_supervisor` が出たら Codex が決定し、**同じ worker session で resume**。

### L-5. 検証 → `$3ailoop-verify`

receipt + **完全 diff (untracked 含む, #330)** + safety を検証。
slice合格後、Issueに未完sliceがあればCodexが短いhandoffを作り、新しいworker sessionでL-4へ戻る。
Issue全体のsliceが揃うまでlandingしない。

### L-6. serial landing → `$3ailoop-land`

直列 landing。default-branch 変更前に receipt / generation / closed / scope / complete diff / tests を
全面再検証。失敗は bounded state に戻す (破壊的 recovery しない)。

### L-7. Phase 保守 → `$3ailoop-phase`

Phase close/seed / retrospective / dashboard のユーザー可視機能を維持。

### L-8. lock 解放

```bash
bun .agents/skills/3ailoop/scripts/loop-lock.ts release --token "$TOKEN"
```

## stop / resume

- **stop**: 適格無料 worker なし / token 上限 / 全候補 Issue が gate:*/needs-* のみ / ユーザー停止指示。
  そのサイクルの理由を記録し通知して終了 (sentinel は出さない)。
- **resume**: 次サイクルは Codex が明示的に `$3ailoop` を再投入する。旧 tmux watcher のような常駐
  polling は使わない (AGENTS.md async delegation)。未完の worker session は委譲 marker から resume する。

## N=3 互換

N=3 isolated worker で fan-out する能力は維持 (`scripts/dry-run.ts` が fake adapter で fan-out +
serial landing を証明)。各 worker は独立 worktree で動き、landing だけ直列化する。

各 active worker には **右側 専用 tmux pane** を割り当て (§Orchestration #5/#6)、status line で
issue/provider/model/session/lifecycle-state を識別し、通常の CLI 出力を流す。pane は operator 可視化
のみで、scheduling/終了検知は durable state + receipts + event-driven termination が担う (pane
scrape/polling ではない)。dry run は fake tmux adapter で「ラベル付き右側 pane 割当・terminal 完了まで
の可観測性・cleanup が lifecycle に影響しないこと」を証明する (`lib/worker-pane.ts`, `scripts/worker-pane.ts`)。
評価中に表示経路だけが壊れた場合は、評価プロセスを中断しない。実行ペインをdetached windowへ移し、
`scripts/evaluation-pane-follow.ts` を新しい右ペインで起動してdurable JSONLへイベント駆動で再接続する。
