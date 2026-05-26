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
> ファイル変更で deny されるのは `crates/**` への Edit/Write のみ（`guard-crates.sh` フック）。

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
```

---

## STEP 3: Codex 設計レビュー（プランモード内）

プランファイルへの記述が完成したら、Codex にレビューを委託する。
ループ上限: **wrapper が自動判定**（code=3 / docs=2）。超過時は wrapper が exit 3 で終了。

```bash
# プランファイルをまとめてレビュー用 input として使う
bash .claude/skills/3ai/scripts/dispatch-codex-auto.sh \
  --issue $ISSUE_NUM \
  --mode design \
  --input <プランファイルパス> \
  --plan <プランファイルパス> \
  --state features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/design-review.md
```

**完了通知を待つ（ポーリングしない）。** 結果ファイルを読み、Critical/High があれば:
1. プランを改訂
2. 上記コマンドを再実行
3. wrapper が **exit 3（上限超過）** で終了したら **停止してユーザーにエスカレーション**

全 Critical/High 解消後:
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

## STEP 6: GLM-5.1 実装（背景実行・完了通知）

GLM に実装+テスト+`cargo xtask ci` green ループを**全て内部で**回させる。
**自分（Claude）は crates/** を編集しない**（フックが deny する）。

```bash
bash .claude/skills/3ai/scripts/dispatch-glm.sh \
  --agent .claude/skills/3ai/agents/glm-implementer.md \
  --plan-file <プランファイルパス> \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result-file features/$ISSUE_NUM-$ISSUE_SLUG/glm-result.json \
  --max-turns 80
```

**`run_in_background: true` で起動し、完了通知を待つ。**

`glm-result.json` を読んで確認:
- `status: success` かつ `ci_passed: true` → 以下を実行して STEP 7 へ:
  ```bash
  bash .claude/skills/3ai/scripts/state.sh set features/$ISSUE_NUM-$ISSUE_SLUG/state.json glm_impl passed
  ```
- `status: failed` → **停止してユーザーにエスカレーション**（Anthropic claude への自動フォールバック禁止）

---

## STEP 7: Codex 最終レビュー（背景実行・完了通知）

**ゲート: glm_impl が passed であることを確認。未通過なら STEP 6 へ戻ること。**
```bash
bash .claude/skills/3ai/scripts/state.sh assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json glm_impl
```

ループ上限: **wrapper が自動判定**（code=2 / docs=1）。超過時は wrapper が exit 3 で終了。

```bash
bash .claude/skills/3ai/scripts/dispatch-codex-auto.sh \
  --issue $ISSUE_NUM \
  --mode final \
  --state features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/final-review.md
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

- `crates/**` を自分（Claude）が直接 Edit/Write **しない** — フックが deny する
- `glm`（Z.AI ラッパー）以外の `claude -p` ワーカーを spawn **しない**（課金制約）
- GLM が詰まっても Anthropic claude へ自動フォールバック**しない**
- dispatch 完了をポーリング**しない** — 背景実行 + 完了通知で受け取る
- git commit/push は STEP 8 以外で行わない
