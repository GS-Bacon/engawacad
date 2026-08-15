---
name: 3ailoop-worker-run
description: Launch or resume a free cloud worker on the Issue's plan inside an isolated worktree. Async, event-driven delegation — emits a delegation marker and ends the turn; no polling, sleep-loops, or repeated state checks. The worker implements, tests, makes a local commit, and emits a receipt (completed / needs_supervisor / failed / rate_limited). On needs_supervisor, Codex decides and the same worker session resumes.
allowed-tools: Read, Bash, Glob, Grep
---

# $3ailoop-worker-run — run/resume the worker

適格 free worker に1つのdelivery sliceを委譲する。**async, event-driven, polling なし**
(AGENTS.md "External AI delegation")。

## Worker autonomy

- CodexはObjective / Done when / Boundary / Starting contextだけを固定する。
- workerはassigned worktreeとslice境界の中で、調査順、編集方法、必要ファイル、focused testを自由に選ぶ。
- workerへ逐次手順、コマンド列、行単位の解法を渡さない。途中の通常判断にCodex承認を要求しない。
- workerへ任意の`$ask-codex`能力を渡す。local evidenceを確認してもmaterial choiceが残り、誤判断の手戻りが大きい場合に自発的に使える。相談は必須フェーズではない。
- receipt形式、実行command、ローカルで確認できる事実は相談せず、決定的script/validatorへ従う。通常の相談予算は1 sliceあたり2回。
- scope拡張、外部権限、安全性の問題は相談予算に関係なく`needs_supervisor`で止める。
- 1 slice完了ごとにlocal commit + receiptを出して終了する。次sliceはCodexレビュー後、同じworktreeの
  状態を引き継いだ**新しいmodel session**へ渡す。会話contextは引き継がない。
- `needs_supervisor`の回答だけは、相談を出した同じsessionへresumeする。

## 手順

1. isolated worktree を作る (assigned worktree のみ):
   ```bash
   # via cli/git-real.ts addWorktree (detach at base)
   ```
2. worker-registry に worker を登録 + Issue を assign (CAS `tryAssign` で expectedGeneration を渡す)。
3. 委譲 marker を発行して **即座に turn を終える**:
   ```bash
   bun .agents/skills/3ailoop/scripts/run-worker.ts \
     --issue <N> --worker-id worker-1 --model opencode/<model> \
     --worktree <path> --plan features/<N>-<slug>/slices/<NN>-<name>.md \
     --requirements features/<N>-<slug>/slices/<NN>-<name>.requirements.json \
     --expected-generation <generation-read-before-selection> \
     [--registry-path features/.loop/model-registry.json]
   ```
   `--model` は **qualified-model registry に存在し、かつ現在も選定可能** (active / zero-cost /
   toolcall / 未期限切れ / re-eval 不要) でなければならない。`$3ailoop-worker-select` が選定した identity を
   そのまま渡すこと。未登録・probation・retired・期限切れ・rate-limit 済みの model は `model-not-selectable`
   で拒否され、委譲 marker は出ない (policy #6)。launch の実行可能コマンドは registry entry の `harness` に
   従う (OpenCode は `opencode run`、qualified external candidate はその resumable CLI)。
   scriptは`$async-agent-delegation`へ直接queueし、stdoutの`marker`は
   `[agent-delegation:<uuid>]`になる。これを最終メッセージに1回含めてturnを終了する。
   `launch` が worker を起動する **実行可能な非対話コマンド** であり、job descriptor
   は `features/.loop/worker-queue/<marker>.json` にも書き込まれる (AGENTS.md async-delegation)。
4. Codex の turn-complete 通知から、`$async-agent-delegation` (AGENTS.md "External AI delegation") が
   job descriptor の `launch.command` を `launch.cwd` (= assigned isolated worktree) で **非対話・model pinned**
   で起動する。worker は worktree 内で実装 → `launch` に渡した prompt に従い `receipt_path` へ `receipt.json`
   を書いて **終了通知** (success / rate-limit / abnormal のいずれも終了通知として扱う)。
   `opencode-worker-terminal.ts`がJSONLから実session IDとcommand auditを取得し、`type:error`、malformed、
   session欠落、receipt欠落、schema不正をCLI exit 0でもfail-closedにする。identityとemitted_atは自己申告でなく
   ハーネスがreceiptへ結ぶ。`forbidden_operations`は実行した違反の自己申告で、違反なしは`[]`。
   receipt直前にuntrackedを含む完全差分を確認し、意図しない生成物を除去してから`changed_files`を確定する。
   - **operator 可視化 (§Orchestration #5/#6)**: 起動前に job descriptor の `pane` contract に従い、
     この worker 専用の **右側 tmux pane** を割り当てる。`pane-render.ts`は通常出力を
     RUNNING / STEP / LOOK / EDIT / TEST / COMMIT / RETRY / ERROR / verdict の短いイベント表示へ変換する。
     model本文・hidden reasoning・credential・任意command本文は表示せず、無加工streamはstateが示すlogへ保存する。
     pane は operator 用であり、scheduling/終了検知は durable state + receipts + event-driven termination で行う
     (pane scrape / polling には依存しない)。
5. Codex は終了通知を受け取り、**保存済み終了コード/状態/ログを 1 度だけ確認**して `$3ailoop-verify` へ。
   terminal 後、worker の pane は既定で決定的に release する (lifecycle へ影響しない cleanup)。
   出力はstateが示すlog fileへ残る。デバッグ時に限り`visible-command.ts --retain-pane`または
   `LOOP_RETAIN_WORKER_PANE=1`で明示的に残せるが、通常運用では使わない。

## resume (consultation)

worker が任意の`$ask-codex`として`needs_supervisor` receipt を出したとき:
- receipt.consultation { question, options, recommendation } を Codex が読む。
- Codex が decision を決定し、consultations.jsonl に answer を書く (lib/consult.ts `answerConsultation`)。
- harnessが観測したreceipt.session_id、相談recordのid、Codex回答fileを別々に渡し、**同じ worker session** を再開する。

  ```bash
  bun .agents/skills/3ailoop/scripts/run-worker.ts \
    <通常の起動引数> \
    --resume-session <receipt.session_id> \
    --consult-id <consultation-record-id> \
    --decision-file <codex-answer.txt>
  ```

相談IDをOpenCode session IDとして流用しない。3引数の一部だけを渡したresumeはfail-closedになる。作業途中のscope内editは保持してよいが、回答前のscope逸脱・禁止操作は許可しない。

通常のslice継続にはこのresume経路を使わない。完了済みsliceの大きな会話contextを次sliceへ持ち込まず、
Codexがreceipt/diff/testから短いStarting contextを作って新sessionへ渡す。

## 禁止 (lib/safety.ts が監査)

worker は assigned worktree 内のみ。以下は検出次第 即 fail:
- `git push` / `gh <resource> <write-verb>` (issue close, pr merge, milestone edit, …)
- default-branch への land 試行
- shared stash
- 別 worktree の編集
- `git config credential.*`

詳細は [../3ailoop/references/runtime.md](../3ailoop/references/runtime.md) の Async delegation / Worker receipt 節。
