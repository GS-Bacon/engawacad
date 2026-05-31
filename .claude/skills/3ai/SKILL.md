---
name: 3ai
description: Claude × Codex × GLM マルチエージェント CAD 開発フロー。GitHub Issue を起点に設計(Claude)→設計レビュー(Codex)→実装+テスト(GLM)→最終レビュー(Codex)を一周する。
tools: Read, Write, Edit, Bash, Glob, Grep
---

# /3ai — マルチエージェント CAD 開発フロー

このスキルが呼ばれたら以下の手順を**必ず順番通りに**実行すること。

---

## STEP 0: プランモードへ移行

**`EnterPlanMode` を呼ぶ。**

> プランモードでも **Bash dispatch は通る**（Edit/Write 等のファイル変更のみ deny）。  
> STEP 3 の Codex 設計レビューは ExitPlanMode 前に実行すること（後ろ倒し禁止）。

---

## STEP 1: Issue 選択・作業ディレクトリ作成

```bash
gh issue list --state open
```

ROADMAP.md の現 Phase に紐づく Milestone の未着手 Issue を提案し、ユーザーに選んでもらう。  
選んだ Issue 番号を `$ISSUE_NUM`、スラッグを `$ISSUE_SLUG` とする（例: `42-make-cylinder`）。

作業ディレクトリと状態ファイルを一括生成（`features/$ISSUE_NUM-$ISSUE_SLUG/` に plan.md / rejection.md / judgment-summary.md / state.json が作られる）:

```bash
bun .claude/skills/3ai/scripts/init-feature.ts --issue $ISSUE_NUM --slug $ISSUE_SLUG
```

---

## STEP 2: 壁打ち・設計・テスト設計

ユーザーと議論しながら設計を固める。CLAUDE.md の原則を守ること:
- **推測禁止** — 不明点は `grep`/`Read` で調査してから書く
- **反論・客観的視点を提供** — 問題を多角的に検討する

`features/$ISSUE_NUM-$ISSUE_SLUG/plan.md` の各セクションに設計を記述する（セクション見出しは plan.md 内コメントを参照）。必須事項:

- **Non-Goals**: 本 Issue で実装しない項目を列挙（「該当なし」でも明記、空欄禁止）
- **設計方針**: 決定性要件・B-rep トポロジー妥当性（Euler-Poincaré V-E+F=2）・退化幾何の扱い・derive 規約・エラーハンドリング・workspace.dependencies
- **テスト計画（ID 付き）**: T01 決定性、T02〜正常系、エッジケース、golden YAML
- **幾何的不変条件チェックリスト**: Boolean/Partition/Assemble 系のみ（非該当は N/A）

---

## STEP 2.5: プラン Draft をユーザーへ平易に提示（Codex レビュー前）

プランファイルの記述が完成したら、**Codex に渡す前に**要点をユーザーへ提示する。

- 専門用語を避け、噛み砕いた言葉で
- 「何を作るか」「主要な設計判断とその理由」「テスト方針の要点」の 3 点を番号付きで簡潔に
- プランファイルの全文転記ではなく**要点の要約**

フィードバックがあればプランを修正。修正がなければ STEP 3 へ（この提示では `ExitPlanMode` を呼ばない）。

---

## STEP 3: Codex 設計レビュー（プランモード内）

ループ上限: **wrapper が自動判定**（code=3 / docs=2）。超過時は wrapper が exit 3 で終了。

### 3-A: dispatch（初回）

`adr-context.md` が必要な場合（plan が特定 ADR を参照）は dispatch 前に Claude が抜粋して `features/$ISSUE_NUM-$ISSUE_SLUG/adr-context.md` に書く。

```bash
bun .claude/skills/3ai/scripts/dispatch-codex-auto.ts \
  --issue $ISSUE_NUM \
  --mode design \
  --input <プランファイルパス> \
  --plan <プランファイルパス> \
  --state features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/design-review.md \
  --plan-snapshot-dir features/$ISSUE_NUM-$ISSUE_SLUG/plan-snapshots \
  [--adr-context features/$ISSUE_NUM-$ISSUE_SLUG/adr-context.md]
```

> `## Non-Goals` セクションがない場合、wrapper が exit 1 で停止する。プランに追記してから再実行。

### 3-B: 棄却 gate（毎 round 必須）

**完了通知を待つ（ポーリングしない）。** `design-review.md` を読んで各 issue を判定:

1. **採用** → plan を修正
2. **棄却** → `features/$ISSUE_NUM-$ISSUE_SLUG/rejection.md` に `## Round N` で追記
3. **部分採用** → plan 一部修正 + rejection.md に残り件を追記

採用数・棄却数を記録する:
```bash
bun .claude/skills/3ai/scripts/state.ts judge \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  <round番号> <adopted_count> <rejected_count>
```

`features/$ISSUE_NUM-$ISSUE_SLUG/judgment-summary.md` に採用・棄却の一覧を `## Round N` で追記する。

**全採用警告チェック**（次 round dispatch 前に必ず実行）:
```bash
bun .claude/skills/3ai/scripts/state.ts check-full-adoption-warning \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json
```
exit 1 が返った場合は停止してユーザーへ表示:
> 2 round 連続で棄却が 0 件です。scope 防衛できていますか?  
> Non-Goals に含まれる指摘や medium 以下で受容すべき指摘は棄却 log に記録してから次 round に進んでください。

### 3-C: 再 dispatch（round 2 以降）

Critical/High が残っていれば再 dispatch する（rejection.md がある場合は `--rejection` を追加）:

```bash
bun .claude/skills/3ai/scripts/dispatch-codex-auto.ts \
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

**exit 3（上限超過）フォールバック**:
```bash
bun .claude/skills/3ai/scripts/state.ts assert-critical-zero \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  features/$ISSUE_NUM-$ISSUE_SLUG/design-review.md.verdict.json
```
- **critical ≥ 1** → 停止してユーザーにエスカレーション（design-review.md と rejection.md を提示）
- **critical = 0** → Claude 裁量で受け切る: 残 high/medium/low を 1 件ずつ「採用 → plan 修正」「棄却 → rejection.md 追記」で処理 → `state.ts set ... design_review passed` → 対応内訳を報告して STEP 4 へ

### 3-D: 通過

全 Critical/High 解消後:
```bash
bun .claude/skills/3ai/scripts/state.ts set features/$ISSUE_NUM-$ISSUE_SLUG/state.json design_review passed
```

---

## STEP 4: 確定プラン提出

**`ExitPlanMode` を呼ぶ。** ユーザーに承認を求める。承認されたら STEP 5 へ。

---

## STEP 5: 作業ブランチ作成

```bash
git checkout -b cad/$ISSUE_NUM-$ISSUE_SLUG
```

---

## STEP 6: GLM-5.1 コア実装（背景実行・自動エスカレーション付き）

**目的**: コア機能の実装 + plan T01〜のうち決定性・正常系の最小テスト。  
**ループ定数**: `GLM_MAX_LOOPS=3`（通常試行上限）、`ESC_MAX_LOOPS=1`（debug-spec 付き再 dispatch 上限）

### 6-A: コア実装ループ（最大 3 回）

```bash
bun .claude/skills/3ai/scripts/dispatch-glm.ts \
  --agent .claude/skills/3ai/agents/glm-implementer.md \
  --plan-file <プランファイルパス> \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result-file features/$ISSUE_NUM-$ISSUE_SLUG/glm-result.json \
  --mode core \
  --max-turns 60
# debug-spec 付き再 dispatch の場合は --debug-spec features/$ISSUE_NUM-$ISSUE_SLUG/debug-spec.md を追加

bun .claude/skills/3ai/scripts/state.ts inc features/$ISSUE_NUM-$ISSUE_SLUG/state.json phases.core_impl.glm_runs
```

**`run_in_background: true` で起動し、完了通知を待つ（ポーリングしない）。**

### 6-B: 結果判定と早期エスカレーション判定

`glm-result.json` を読んで:

- `status: success` かつ `ci_passed: true` → `state.ts set ... core_impl passed` して **STEP 6.5 へ**
- `status: failed` の場合:
  - 今回と前回の `error_pattern` を比較
  - **2 連続同一 error_pattern** または **通常試行が GLM_MAX_LOOPS 回** → **6-C へ**
  - それ以外（新しいエラー）→ 6-A に戻って次の試行

### 6-C: Claude デバッグアシスト（debug-spec 作成）

`features/$ISSUE_NUM-$ISSUE_SLUG/ci.log` と `crates/**` の関連ファイルを **Read** して根本原因を分析する。  
`features/$ISSUE_NUM-$ISSUE_SLUG/debug-spec.md` を **Write** する（セクション: **仮説 / 関連ファイル / 修正方針 / 追加で書いてほしいテスト**）。  
6-A に戻り `--debug-spec` 付きで dispatch（ESC_MAX_LOOPS=1 のため 1 回まで）。

### 6-D: 終結判定

debug-spec 付き再 dispatch でも `status: failed` のままなら停止してユーザーにエスカレーション。`debug-spec.md` はユーザーが手動改稿して再投入できる。Anthropic claude への自動フォールバック禁止。

---

## STEP 6.5: Claude がテスト仕様書（test-spec.md）を作成

**ゲート:**
```bash
bun .claude/skills/3ai/scripts/state.ts assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json core_impl
```

Claude が以下を実行する（crates/** の **Read のみ**）:

1. `git diff main..HEAD` で実装差分を読む
2. plan のテスト計画 ID 表と突き合わせ、未実装のものを特定する
3. 実装差分を見て「plan に書いていなかったが生じた分岐・ケース」を特定する
4. `features/$ISSUE_NUM-$ISSUE_SLUG/test-spec.md` を **Write** する（セクション: **不足テスト（plan 計画分） / 実装差分から追加すべきテスト / エッジケース・退化入力 / 数値境界 / 決定性**）

---

## STEP 6.6: GLM-5.1 テスト実装（背景実行）

**ゲート:** `features/$ISSUE_NUM-$ISSUE_SLUG/test-spec.md` が存在すること。

```bash
bun .claude/skills/3ai/scripts/dispatch-glm.ts \
  --agent .claude/skills/3ai/agents/glm-test-implementer.md \
  --plan-file <プランファイルパス> \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result-file features/$ISSUE_NUM-$ISSUE_SLUG/glm-test-result.json \
  --mode test \
  --test-spec features/$ISSUE_NUM-$ISSUE_SLUG/test-spec.md \
  --max-turns 60

bun .claude/skills/3ai/scripts/state.ts inc features/$ISSUE_NUM-$ISSUE_SLUG/state.json phases.test_impl.glm_runs
```

**`run_in_background: true` で起動し、完了通知を待つ。**

`glm-test-result.json` を読んで:
- `status: success` かつ `ci_passed: true` → `state.ts set ... glm_impl passed`
- `status: failed` → 6-C と同じ手順（debug-spec を test モード向けに作成）で最大 1 回再 dispatch。それでも失敗なら停止・エスカレーション

---

## STEP 7: Codex 最終レビュー（背景実行・完了通知）

**ゲート:**
```bash
bun .claude/skills/3ai/scripts/state.ts assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json glm_impl
```

ループ上限: **wrapper が自動判定**（code=2 / docs=1）。超過時は wrapper が exit 3 で終了。

**dispatch 前に test-summary.json を生成する:**
```bash
bun .claude/skills/3ai/scripts/extract-test-summary.ts \
  --ci-log features/$ISSUE_NUM-$ISSUE_SLUG/ci.log \
  --output features/$ISSUE_NUM-$ISSUE_SLUG/test-summary.json
```

```bash
bun .claude/skills/3ai/scripts/dispatch-codex-auto.ts \
  --issue $ISSUE_NUM \
  --mode final \
  --state features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/final-review.md \
  --test-summary features/$ISSUE_NUM-$ISSUE_SLUG/test-summary.json
```

**完了通知を待つ。** `final-review.md.verdict.json` の `blocking` が 0 かつ `verdict: pass` なら:
```bash
bun .claude/skills/3ai/scripts/state.ts set features/$ISSUE_NUM-$ISSUE_SLUG/state.json final_review passed
```

Critical/High があれば GLM 修正 dispatch → Codex 再レビュー（ループ +1）。

**exit 3（上限超過）フォールバック**:
```bash
bun .claude/skills/3ai/scripts/state.ts assert-critical-zero \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  features/$ISSUE_NUM-$ISSUE_SLUG/final-review.md.verdict.json
```
- **critical ≥ 1** → 停止してユーザーにエスカレーション
- **critical = 0**、blocking が **docs-only**（コードファイル変更を伴わない）→ Claude 裁量で受け切る: 残 high/medium を直接修正（docs への Edit/Write）または棄却 → `cargo xtask ci` green 確認 → `state.ts set ... final_review passed` → 内訳報告して STEP 8 へ
- **critical = 0**、blocking に **code 系**が含まれる → 停止してユーザーにエスカレーション

---

## STEP 8: 確定・squash マージ

**ゲート:**
```bash
bun .claude/skills/3ai/scripts/state.ts assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json final_review
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
bun .claude/skills/3ai/scripts/state.ts set features/$ISSUE_NUM-$ISSUE_SLUG/state.json merge passed
```

---

## 禁止事項（常に守ること）

- `crates/**` を自分（Claude）が直接 Edit/Write **しない** — guard-crates フックが deny する（Read は可）
- `claude -p` ワーカーを spawn **しない**（課金制約: Z.AI/Codex は可、Anthropic claude は不可）
- GLM が詰まっても Anthropic claude へ自動フォールバック**しない**
- dispatch 完了をポーリング**しない** — 背景実行 + 完了通知で受け取る
- git commit/push は STEP 8 以外で行わない
