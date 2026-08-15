---
name: 3ailoop-plan
description: Codex plans the selected Issue into a plan.md. Applies the ADR-006 granularity guard (check-issue-granularity), records non-goals and scope, and prepares the contract the worker will implement. Codex is the sole planner; no external model writes the plan.
allowed-tools: Read, Write, Edit, Bash, Glob, Grep
---

# $3ailoop-plan — plan an Issue with Codex

選ばれた Issue を Codex が `features/<N>-<slug>/plan.md` に落とす。

## 手順

1. Issue 本文・関連 ADR・Non-Goals・直近 cycle log を読む。
2. ADR-006 (`docs/decisions/006-issue-decomposition.md`) の粒度ガードを適用:
   ```bash
   bun .claude/skills/3ai/scripts/check-issue-granularity.ts --issue-draft <draft.md> --result <intent.yaml>
   ```
   (`aligned: no` なら `split_proposal` で分割するか、type/scope を修正して再チェック。)
3. plan.md に「実装方針 / 想定touchpoint / テスト方針 / Non-Goals / 受け入れ条件」を書く。
4. Issue全体を、単独で合否判定できるdelivery sliceへ分ける。各sliceは原則として:
   - 1つの観測可能な成果
   - 1つのfocused test family
   - おおむね1〜4個のprimary touchpoint（原子的変更に必要なら超過可）
   - そのsliceだけのNon-Goals
   を持つ。ファイル数だけで機械的に分割しない。
5. 最初のslice contractを`features/<N>-<slug>/slices/<NN>-<name>.md`へ書き、L-3へ渡す。
   後続sliceは直前の実diffと検証結果をCodexが確認してから確定する。
6. 各sliceの隣に`<NN>-<name>.requirements.json`を書く。`scope` (`tiny|small|medium`)、
   `primary_touchpoints`、`task_class` (`bugfix|multi-crate|diagnose|focused-test|general`)、
   `ambiguity` (`low|material-choice`)、`negative_tests_required`を記録し、モデル選定に渡す。

## Slice contract

workerへ渡すのは次だけにする。

- **Objective**: 今回得る成果を1文で示す。
- **Done when**: workerとCodexが同じ方法で確認できる合格条件。
- **Boundary**: 今回扱わない成果、外部操作、安全境界。
- **Starting context**: 必読ファイルと直前sliceの短いhandoff。会話履歴や全調査ログは渡さない。
- **Handoff**: local commit、receipt、実行したtest、未検証事項。

共通ハーネスは安全・終了状態・receiptだけを固定する。モデル固有の編集手順はplanへ書かず、評価で得た
短いtask profileとslice requirementsの適合だけでタスクサイズ・種類を調整する。

正確な編集順、コマンド順、関数構成、行単位の解法は、壊れやすい操作に必要な場合を除いて
指定しない。想定touchpointは探索の入口でありallowlistではない。workerはsliceの目的内で自由に
調査・編集・テストし、material choiceだけを`needs_supervisor`で相談する。

## 権限

- **Codex が唯一の planner**。外部モデル (worker) は plan を書かない。
- plan は worker に渡す契約。worker は実装詳細は自由に選べるが (contract §"Orchestration and authority" #3)、
  material choice で block したら `needs_supervisor` receipt を出す。
- CodexはIssue全体の順序とslice境界を決める。workerのslice内の行動順は管理しない。

粒度ガードは既存の決定的 script を再利用する (新 runtime は /3ai 依存しないが、純粋 lint script は
安全なのでそのまま使う)。
