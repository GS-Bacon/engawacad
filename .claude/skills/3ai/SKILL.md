---
name: 3ai
description: Claude × Codex × GLM マルチエージェント CAD 開発フロー。GitHub Issue を起点に設計(Claude)→設計レビュー(Codex)→実装+テスト(GLM)→最終レビュー(Codex)を一周する。
tools: Read, Write, Edit, Bash, Glob, Grep
---

# /3ai — マルチエージェント CAD 開発フロー

このスキルが呼ばれたら以下の手順を**必ず順番通りに**実行すること。
普段のプランモードとは独立したフローである。

---

## STEP 0: プランモードへ移行

**`EnterPlanMode` を呼ぶ。** 以降 STEP 4 まではプランモード内で動作する。

> **重要 — プランモード内でも STEP 1・STEP 3 の Bash dispatch は必ず実行する。**
> harness のプランモードは「read-only except plan file」と指示するが、これが deny するのは Edit/Write 等のファイル変更ツールであって、**Bash サブプロセス起動は通る**。
> `mkdir features/...`・`state.sh`・`dispatch-codex.sh`（Codex 設計レビュー）はすべて Bash であり、プランモード内でそのまま走る。
> したがって **STEP 3 の Codex 設計レビューを「ExitPlanMode 後に回す」と後ろ倒ししてはならない。** プランファイル記述完了後、プランモード内で Codex レビューを回して Critical/High を潰し、その後で STEP 4 の `ExitPlanMode` を呼ぶ（STEP 3→4 の順序厳守）。
> ファイル変更で deny されるのは `crates/**` への Edit/Write のみ（`guard-crates.sh` フック）。Read や `features/` 配下への Write は通過する。

---

## STEP 1: Issue 選択・作業ブランチ作成

```bash
gh issue list --state open
```

ROADMAP.md の現 Phase に紐づく Milestone の未着手 Issue を提案し、ユーザーに選んでもらう。
選んだ Issue 番号を `$ISSUE_NUM`、スラッグを `$ISSUE_SLUG` とする（例: `42-make-cylinder`）。

作業ディレクトリと状態ファイルを作成（後で使う）:
```bash
mkdir -p features/$ISSUE_NUM-$ISSUE_SLUG
bash .claude/skills/3ai/scripts/state.sh init features/$ISSUE_NUM-$ISSUE_SLUG/state.json $ISSUE_NUM $ISSUE_SLUG
```

---

## STEP 2: 壁打ち・設計・テスト設計

ユーザーと議論しながら設計を固める。CLAUDE.md の原則を守ること:
- **推測禁止** — 不明点は `grep`/`Read` で調査してから書く
- **反論・客観的視点を提供** — 問題を多角的に検討する

プランファイルに以下を記述する（`features/$ISSUE_NUM-$ISSUE_SLUG/plan.md` と対応させる）:

```markdown
## Non-Goals
本 Issue では実装しない項目を箇条書きで列挙する。該当なしの場合も明示的に `- 該当なし` と書くこと（空欄禁止）。
- (例) フル退化検出: #34 で対応予定
- (例) 既存テストの全 golden 更新: スコープ外

## 実装対象
- Issue: #NNN
- 影響クレート/ファイル: (具体パス列挙)
- 変更する型・関数のシグネチャ

## 設計方針
- 決定性要件: (IdGenerator の使い方、同一入力→同一出力の保証方法)
- B-rep トポロジー妥当性: (Euler-Poincaré が成立するか: V - E + F = 2)
- 退化幾何の扱い: (ゼロ長エッジ、面積ゼロ面などの排除・エラー条件)
- derive 規約: Debug/Clone/Serialize/Deserialize (+JsonSchema が必要か)
- エラーハンドリング: thiserror の使い方
- workspace.dependencies 規約

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一入力を2回実行し全 ID・座標が一致 | assert_eq! |
| T02 | 正常系 | ... | ... |
| TXX | エッジケース | 退化入力 / 境界数値 | エラーまたは正常処理 |
| TXX | golden YAML | .mycad → rebuild → 再出力が一致 | assert_eq! |

## 幾何的不変条件チェックリスト（Boolean/Partition/Assemble 系のみ。該当しない場合は "N/A" と明記）
- [ ] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか
- [ ] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか
- [ ] flip_normals / same_sense の意味論が明確か（頂点順を変えるか vs 法線だけ変えるか）
- [ ] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか
```

---

## STEP 2.5: プラン Draft をユーザーへ平易に提示（Codex レビュー前）

プランファイルの記述が完成したら、**Codex に渡す前に**プランの要点をユーザーへ提示する。

提示の形式（CLAUDE.md の /3ai トーン規約に従う）:
- 専門用語を避け、噛み砕いた言葉で説明する
- 「何を作るか」「主要な設計判断とその理由」「テスト方針の要点」の 3 点を番号付きで簡潔に
- プランファイルの全文転記ではなく、**要点の要約**を提示する

提示後、ユーザーからフィードバックがあればプランを修正する。
修正がなければそのまま STEP 3 へ進む（この提示では `ExitPlanMode` を呼ばない）。

---

## STEP 3: Codex 設計レビュー（プランモード内）

プランファイルへの記述が完成したら、Codex にレビューを委託する。
ループ上限: **wrapper が自動判定**（code=3 / docs=2）。超過時は wrapper が exit 3 で終了。

### 3-A: dispatch（初回）

`adr-context.md` が必要な場合（plan が特定の ADR を参照している場合）は、dispatch 前に Claude が該当 ADR の背景・決定セクションを抜粋して `features/$ISSUE_NUM-$ISSUE_SLUG/adr-context.md` に書く。

```bash
bash .claude/skills/3ai/scripts/dispatch-codex-auto.sh \
  --issue $ISSUE_NUM \
  --mode design \
  --input <プランファイルパス> \
  --plan <プランファイルパス> \
  --state features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/design-review.md \
  --plan-snapshot-dir features/$ISSUE_NUM-$ISSUE_SLUG/plan-snapshots \
  [--adr-context features/$ISSUE_NUM-$ISSUE_SLUG/adr-context.md]
```

> **注意**: `--plan` に `## Non-Goals` セクションがない場合、wrapper が exit 1 で停止する。  
> プランに Non-Goals を追記してから再実行すること（「該当なし」の場合は `- 該当なし` と明記）。

### 3-B: 棄却 gate（毎 round 必須）

**完了通知を待つ（ポーリングしない）。** `design-review.md` を読んで各 issue を判定する:

1. **採用** → plan を修正する
2. **棄却** → `features/$ISSUE_NUM-$ISSUE_SLUG/rejection.md` に追記する:
   ```markdown
   ## Round N
   - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）>
   ```
3. **部分採用** → plan 一部修正 + `rejection.md` に「残りの N 点は棄却」として追記する

採用数・棄却数を記録する:
```bash
bash .claude/skills/3ai/scripts/state.sh judge \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  <round番号> <adopted_count> <rejected_count>
```

**judgment-summary.md を更新する**（round 2 以降の dispatch で Codex に渡される）:
`features/$ISSUE_NUM-$ISSUE_SLUG/judgment-summary.md` に以下の形式で追記する:
```markdown
## Round N
- R01: 採用 → plan の「決定性要件」節に IdGenerator 制約を明記した
- R02: 棄却 → Non-Goals に記載済みの「退化検出」の蒸し返し
- R03: 部分採用 → epsilon 比較は採用。HalfEdge 循環チェックは後続 issue (#35) に委譲
```

**全採用警告チェック**（次 round dispatch 前に必ず実行）:
```bash
bash .claude/skills/3ai/scripts/state.sh check-full-adoption-warning \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json
```
exit 1 が返った場合は **停止して次のメッセージをユーザーへ表示**:
> 2 round 連続で棄却が 0 件です。scope 防衛できていますか?  
> Non-Goals に含まれる指摘や medium 以下で受容すべき指摘は棄却 log に記録してから次 round に進んでください。

### 3-C: 再 dispatch（round 2 以降）

Critical/High が残っていれば再 dispatch する（rejection.md がある場合は `--rejection` を追加）:

```bash
bash .claude/skills/3ai/scripts/dispatch-codex-auto.sh \
  --issue $ISSUE_NUM \
  --mode design \
  --input <プランファイルパス> \
  --plan <プランファイルパス> \
  --state features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/design-review.md \
  --plan-snapshot-dir features/$ISSUE_NUM-$ISSUE_SLUG/plan-snapshots \
  --rejection features/$ISSUE_NUM-$ISSUE_SLUG/rejection.md \
  --judgment-summary features/$ISSUE_NUM-$ISSUE_SLUG/judgment-summary.md \
  [--adr-context features/$ISSUE_NUM-$ISSUE_SLUG/adr-context.md]
```

wrapper が **exit 3（上限超過）** で終了した場合は以下のフォールバック手順を実行する:

1. `features/$ISSUE_NUM-$ISSUE_SLUG/design-review.md.verdict.json` の `severity_counts.critical` を確認する
2. **critical が 1 件以上** → 停止してユーザーにエスカレーション。design-review.md と rejection.md を提示し、判断を仰ぐ
3. **critical が 0 件** → Claude 裁量で受け切る:
   - 残った high/medium/low を 1 件ずつ「採用 → plan を直す」「棄却 → rejection.md に追記」で処理する
   - 全件処理し終えたら `state.sh set` で通過扱いにする
   - 受けた・棄却した内訳をユーザーに 1 メッセージで報告してから STEP 4 へ

判定ヘルパー（critical = 0 なら exit 0）:
```bash
bash .claude/skills/3ai/scripts/state.sh assert-critical-zero \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  features/$ISSUE_NUM-$ISSUE_SLUG/design-review.md.verdict.json
```

### 3-D: 通過

全 Critical/High 解消後（中断している medium/low は棄却 log に記録済みであることを確認）:
```bash
bash .claude/skills/3ai/scripts/state.sh set features/$ISSUE_NUM-$ISSUE_SLUG/state.json design_review passed
```

---

## STEP 4: 確定プラン提出

**`ExitPlanMode` を呼ぶ。** ユーザーに承認を求める。
承認されたら STEP 5 へ進む。

---

## STEP 5: 作業ブランチ作成

```bash
git checkout -b cad/$ISSUE_NUM-$ISSUE_SLUG
```

---

## STEP 6: GLM-5.1 コア実装（背景実行・自動エスカレーション付き）

**目的**: コア機能の実装 + plan T01〜のうち決定性・正常系の最小テストのみ。エッジケーステストは STEP 6.6 で行う。

**ループ定数**: `GLM_MAX_LOOPS=3`（通常試行上限）、`ESC_MAX_LOOPS=1`（debug-spec 付き再 dispatch 上限）

**自分（Claude）は crates/** を Edit/Write しない**（guard-crates フックが deny する）。
crates/** の Read は許可（debug-spec 作成のための根本原因分析に使う）。

### 6-A: コア実装ループ（最大 3 回）

各試行で以下を実行する:

```bash
bash .claude/skills/3ai/scripts/dispatch-glm.sh \
  --agent .claude/skills/3ai/agents/glm-implementer.md \
  --plan-file <プランファイルパス> \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result-file features/$ISSUE_NUM-$ISSUE_SLUG/glm-result.json \
  --mode core \
  --max-turns 60
# debug-spec 付き再 dispatch の場合は --debug-spec features/$ISSUE_NUM-$ISSUE_SLUG/debug-spec.md を追加

bash .claude/skills/3ai/scripts/state.sh inc features/$ISSUE_NUM-$ISSUE_SLUG/phases.core_impl.glm_runs
```

**`run_in_background: true` で起動し、完了通知を待つ（ポーリングしない）。**

### 6-B: 結果判定と早期エスカレーション判定

`glm-result.json` を読んで:

- `status: success` かつ `ci_passed: true` → 以下を実行して **STEP 6.5 へ**:
  ```bash
  bash .claude/skills/3ai/scripts/state.sh set features/$ISSUE_NUM-$ISSUE_SLUG/state.json core_impl passed
  ```

- `status: failed` の場合:
  - 今回の `error_pattern` と前回の `error_pattern` を比較する。
  - **2 連続同一 error_pattern** または **通常試行が GLM_MAX_LOOPS 回に達した** → **6-C へ**
  - それ以外（新しいエラーに変わっている）→ 6-A に戻って次の試行を行う

### 6-C: Claude デバッグアシスト（debug-spec 作成）

以下を行う（**コード差分そのものは書かない**。修正は GLM が担当）:

1. `features/$ISSUE_NUM-$ISSUE_SLUG/ci.log` と `crates/**` の関連ファイルを **Read** して根本原因を分析する
2. `features/$ISSUE_NUM-$ISSUE_SLUG/debug-spec.md` を **Write** する。構成:
   ```markdown
   ## 仮説
   （根本原因の仮説を 1〜3 行で）

   ## 関連ファイル
   （疑わしいファイルとその該当箇所）

   ## 修正方針
   （「何をどう変えるべきか」の仕様。コード差分ではなく意図を記述）

   ## 追加で書いてほしいテスト
   （根本原因を再現・検証するためのテストケース）
   ```
3. 6-A に戻り `--debug-spec features/$ISSUE_NUM-$ISSUE_SLUG/debug-spec.md` 付きで dispatch する（ESC_MAX_LOOPS=1 のため、この再 dispatch は 1 回まで）

### 6-D: 終結判定

debug-spec 付き再 dispatch でも `status: failed` のままなら:
- **停止してユーザーにエスカレーション**
- `debug-spec.md` は `features/$ISSUE_NUM-$ISSUE_SLUG/` に残る。ユーザーが手動で改稿して再投入できる
- Anthropic claude への自動フォールバック禁止

---

## STEP 6.5: Claude がテスト仕様書（test-spec.md）を作成

**ゲート: core_impl が passed であることを確認。**
```bash
bash .claude/skills/3ai/scripts/state.sh assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json core_impl
```

Claude が以下を実行する（crates/** の **Read のみ**。Edit/Write は guard-crates が deny する）:

1. `git diff main..HEAD` で実装差分を読む
2. plan のテスト計画 ID 表（T01〜）と突き合わせ、未実装のものを特定する
3. 実装差分を見て「plan に書いていなかったが生じた分岐・ケース」を特定する
4. 以下の構成で `features/$ISSUE_NUM-$ISSUE_SLUG/test-spec.md` を Write する:

```markdown
## 不足テスト（plan 計画分）
- T03「..」: <plan からの再掲、期待挙動を明記>

## 実装差分から追加すべきテスト
- TX1「..」: <差分を見て気付いたケース>

## エッジケース・退化入力
- TX2 「..」（ゼロ長エッジ / 面積ゼロ / coincident vertices 等）

## 数値境界
- TX3 「..」（f64::MAX / f64::MIN_POSITIVE / NaN / Inf 等）

## 決定性
- TX4 「..」（同一入力 100 回反復 / ラウンドトリップ等）
```

---

## STEP 6.6: GLM-5.1 テスト実装（背景実行）

**ゲート: `features/$ISSUE_NUM-$ISSUE_SLUG/test-spec.md` が存在すること。**

```bash
bash .claude/skills/3ai/scripts/dispatch-glm.sh \
  --agent .claude/skills/3ai/agents/glm-test-implementer.md \
  --plan-file <プランファイルパス> \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result-file features/$ISSUE_NUM-$ISSUE_SLUG/glm-test-result.json \
  --mode test \
  --test-spec features/$ISSUE_NUM-$ISSUE_SLUG/test-spec.md \
  --max-turns 60

bash .claude/skills/3ai/scripts/state.sh inc features/$ISSUE_NUM-$ISSUE_SLUG/phases.test_impl.glm_runs
```

**`run_in_background: true` で起動し、完了通知を待つ（ポーリングしない）。**

`glm-test-result.json` を読んで:
- `status: success` かつ `ci_passed: true` → `state.sh set ... glm_impl passed` に進む
- `status: failed` の場合は 6-C と同じ手順（debug-spec を test モード向けに作成）で最大 1 回再 dispatch。それでも失敗なら停止・エスカレーション

CI 通過後:
```bash
bash .claude/skills/3ai/scripts/state.sh set features/$ISSUE_NUM-$ISSUE_SLUG/state.json glm_impl passed
```

---

## STEP 7: Codex 最終レビュー（背景実行・完了通知）

**ゲート: glm_impl が passed であることを確認。未通過なら STEP 6.6 へ戻ること。**
```bash
bash .claude/skills/3ai/scripts/state.sh assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json glm_impl
```

ループ上限: **wrapper が自動判定**（code=2 / docs=1）。超過時は wrapper が exit 3 で終了。

**dispatch 前に test-summary.json を生成する:**
```bash
bash .claude/skills/3ai/scripts/extract-test-summary.sh \
  --ci-log features/$ISSUE_NUM-$ISSUE_SLUG/ci.log \
  --output features/$ISSUE_NUM-$ISSUE_SLUG/test-summary.json
```

```bash
bash .claude/skills/3ai/scripts/dispatch-codex-auto.sh \
  --issue $ISSUE_NUM \
  --mode final \
  --state features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/final-review.md \
  --test-summary features/$ISSUE_NUM-$ISSUE_SLUG/test-summary.json
```

**完了通知を待つ。** `final-review.md.verdict.json` の `blocking` が 0 かつ `verdict: pass` なら:
```bash
bash .claude/skills/3ai/scripts/state.sh set features/$ISSUE_NUM-$ISSUE_SLUG/state.json final_review passed
```

Critical/High があれば:
1. GLM 修正ディスパッチ（dispatch-glm.sh を再実行、フォーカスは指摘箇所のみ）
2. Codex 再レビュー（ループ +1）
3. wrapper が **exit 3（上限超過）** で終了したら **停止・エスカレーション**

---

## STEP 8: 確定・squash マージ

**ゲート: final_review が passed であることを確認。未通過なら STEP 7 へ戻ること。**
```bash
bash .claude/skills/3ai/scripts/state.sh assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json final_review
```

```bash
cargo xtask ci   # 最終 green 確認
git checkout main
git merge --squash cad/$ISSUE_NUM-$ISSUE_SLUG
git commit -m "feat: <内容の一行要約>

<詳細（任意）>

Closes #$ISSUE_NUM

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
git push origin main
git branch -d cad/$ISSUE_NUM-$ISSUE_SLUG
bash .claude/skills/3ai/scripts/state.sh set features/$ISSUE_NUM-$ISSUE_SLUG/state.json merge passed
```

---

## 禁止事項（常に守ること）

- `crates/**` を自分（Claude）が直接 Edit/Write **しない** — guard-crates フックが deny する（Read は可）
- `glm`（Z.AI ラッパー）以外の `claude -p` ワーカーを spawn **しない**（課金制約）
- GLM が詰まっても Anthropic claude へ自動フォールバック**しない**
- dispatch 完了をポーリング**しない** — 背景実行 + 完了通知で受け取る
- git commit/push は STEP 8 以外で行わない
