---
name: 3ailoop-worker-qualify
description: Discover, screen, fully qualify, retire, and replenish the 3ailoop free-cloud worker pool. Use initially, when fewer than three qualified workers remain, or after expiry, authentication failure, rate limiting, persistent harness failure, or quality degradation. OpenCode fixed models are evaluated first; local models, Gemini, paid/unknown-cost models, and automatic routers are forbidden.
---

# $3ailoop-worker-qualify

認定済み無料workerを品質基準を下げずに最大3件維持する。通常サイクルでは呼ばず、初回、pool不足、失効・障害・品質低下時だけ使う。

## 手順

1. 現在のpoolを確認する。

   ```bash
   bun .agents/skills/3ailoop/scripts/model-select.ts
   ```

   qualifiedが利用可能なら通常処理へ戻る。3件未満なら補充評価を続ける。

2. OpenCode providerだけから候補を取得する。

   ```bash
   bun .agents/skills/3ailoop/scripts/model-discover.ts
   ```

3. 未評価候補を同一の小型Rust課題で足切りする。長時間実行なので `$async-agent-delegation` を使い、
   `visible-command.ts`を通して右ペインへイベント要約を表示する。モデル本文は表示せず、無加工streamはlogへ保存する。

   ```bash
   bun .agents/skills/3ailoop/scripts/model-eval-screen.ts \
     features/.loop/model-eval/screen-v1
   ```

   評価はloopback限定の`opencode serve`とSSE eventで管理する。screen report version 5は本番と同じschema v1 receipt契約を使い、旧`test_command`形式を拒否する。`session.status=retry` / `session.error`から
   429・`Free usage exceeded`・upstream 5xxを標準出力待ちより先に検出する。rate limitは即時再試行せず、異なる2モデルで同じlimitを確認したら
   provider cooldownとして残りをskipする。初回無応答・502/503/504・connectionだけ、汚れていない新規fixtureで1回再試行する。
   初回model event、出力後idle、絶対上限を別々に記録し、infraは能力不合格にせず`inconclusive`とする。
   tracked / staged / committed / untrackedの完全差分とreceipt申告を一致させ、scope driftをfail-closedにする。
   Rust fixtureの`Cargo.lock`はbaseline生成時に作成・commitし、grader自身の生成物をworker差分へ混入させない。

4. 足切り合格モデルだけを固定suiteで本評価する。

   ```bash
   bun .agents/skills/3ailoop/scripts/model-eval-qualify.ts \
     features/.loop/model-eval/screen-v1/results.json \
     features/.loop/model-eval/qualification-v1
   ```

   suite v3では全実装課題へ任意の`$ask-codex`能力を渡す。相談を指示・加点せず、相談なしまたは相談後の最終deliveryで評価する。
   safety違反・scope逸脱・本番schema receipt差分不一致は即失格。実装3課題中2件以上とproduction contract準拠を必須とする。
   相談が発生した場合だけ固定のCodex reference decisionを同じsessionへ1回返す。相談前のscope内WIP editは許容し、scope逸脱はresumeせず失格とする。
   同一の共通ハーネスで比較した後、task結果から短い`task_profile`を生成する。モデル別の長いpromptや手順は作らない。
   本評価中のinfra障害はその候補を直ちに`inconclusive`で停止し、汚れたfixtureへ再試行しない。既存qualifiedの評価・statusを
   `inconclusive`で上書きしない。429だけavailability観測として加算し、後日のevent-triggered再評価へ送る。

5. `features/.loop/model-registry.json` のqualifiedを最大3件にする。3件未満でも不合格モデルを昇格させない。1件以上あれば通常運用を開始できる。
   task成功数を最優先し、同点ならCodex相談回数、所要時間、identityの順に選ぶ。相談の文章品質や相談有無は独立得点にしない。
   `operating_mode`（autonomous/assisted）、相談回数、相談後完遂数を評価記録へ保存する。
   v3評価中はtop-level registry suite v2を信頼し続け、v3 qualifiedが1件以上得られた場合だけsuite v3を有効化する。

6. rate limit・失効・品質低下を記録し、該当モデルを選定対象から外す。provider全体の制限と確認できた場合は同providerの全モデルへ適用する。

7. free期間終了時は次を実行してretirement reportを残す。

   ```bash
   bun .agents/skills/3ailoop/scripts/model-retire.ts \
     --identity <provider/model> --reason '<reason>' --write-report
   ```

## 境界

- 自動ルーターを候補にしない。provider/modelを固定する。
- OpenCode候補が残る間は外部providerを調査しない。
- account作成、規約同意、billing情報、credential不足はユーザーへ戻す。
- 全qualifiedが利用不能なら停止する。Codex・GLM・有料モデルへ自動fallbackしない。
- 評価fixture以外のworktree、default branch、GitHubを変更しない。
- raw model本文はJSONL logにだけ保存し、集計`results.json`へ埋め込まない。

詳細な採点、registry、再評価条件は [references/policy.md](references/policy.md) を読む。
