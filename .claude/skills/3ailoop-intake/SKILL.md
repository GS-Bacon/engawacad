---
name: 3ailoop-intake
description: ユーザー要望を Plan モード壁打ちで Issue 化する受け口。重複→意図→Phase→粒度→gate→label の順に検証し、合意した内容を gh issue create で起票する。loop と排他、ROADMAP に該当 Phase なしは needs-phase で保留。
tools: Read, Bash, Glob, Grep
---

# /3ailoop-intake — 要望投入の壁打ち Skill

ユーザー要望を loop が消化できる「精査済み Issue」に変換する。Plan モード壁打ちでユーザー意図を確認しながら、6 つの TS スクリプトで重複/Phase/粒度/gate/label を検証する。

**前提**:
- 関連 plan: `/home/bacon/.claude/plans/3ai-loop-ui-ux-jaunty-ember.md`
- 関連 memory: `project-3ailoop-implementation-style` / `project-3ailoop-known-races`
- loop と排他: `features/.batch/lock` を共有

## 引数

`/3ailoop-intake "<要望文>"` で起動。要望文は ユーザーが Skill 起動時に渡す。

## 実行フロー (I-0〜I-10)

### I-0: lock 取得

```bash
TOKEN=$(bun .claude/skills/3ailoop/scripts/loop-lock.ts acquire --owner intake)
```

- 取得失敗 (loop 進行中) → stderr に LOCKED → ユーザーに「loop 終了待ちか中断するか」を確認 → 中断なら終了

### I-1: Plan モードに入る

EnterPlanMode を呼ぶ。以降の I-2〜I-8 は壁打ち (Read-only + Bash dispatch のみ)。

### I-2: 重複チェック

```bash
bun .claude/skills/3ailoop-intake/scripts/intake-dedup.ts --query "<要望文>" --limit 5
```

- stdout の Top-N をユーザーに提示
- ユーザー判断:
  - **別件**: I-3 へ続行
  - **既存に追記**: 既存 Issue にコメントで要望を追記して終了 (lock release)
  - **取消**: 終了 (lock release)

### I-3: 意図確認

Claude が要望を 3 行で再要約 → ユーザーに「この理解で OK?」と確認。NG なら聞き直して I-3 を繰り返す。OK なら確定した意図を `/tmp/intake-intent-<timestamp>.md` に保存して I-4 へ。

### I-4: Phase 推定

```bash
bun .claude/skills/3ailoop-intake/scripts/intake-phase-estimate.ts --intent /tmp/intake-intent-*.md
```

- 結果 `phase: N` → 該当 Phase milestone に紐づける予定として記録
- 結果 `phase: "needs-phase"` → I-7 で `needs-phase` ラベルを付与する方針

### I-5: 粒度判定

```bash
bun .claude/skills/3ailoop-intake/scripts/intake-granularity.ts --intent /tmp/intake-intent-*.md
```

- `ok: true` → I-6 へ
- `ok: false` → `split_suggestion` をユーザーに提示し、「分割するか / 1 件で起票するか」を判断。分割なら I-3 へ戻り個別意図化

### I-6: gate 判定

```bash
GATE=$(bun .claude/skills/3ailoop-intake/scripts/intake-gate-detector.ts --intent /tmp/intake-intent-*.md)
```

- `gate_human_feel: true` → gate:human-feel をラベルに含める予定。ユーザーに mock 画像/markdown 添付を促す

### I-7: ラベル提案 + lint pass

```bash
LABELS=$(bun .claude/skills/3ailoop-intake/scripts/intake-label-suggest.ts \
  --intent /tmp/intake-intent-*.md --gate "$GATE")
```

- スクリプト内部で lint-issue-labels.ts を必ず通す (exit 0 強制)
- 出力 CSV をユーザーに確認 (差分があれば修正)

### I-8: intent-check は省略

memory `project_3ailoop_implementation_style.md` 準拠: skill トラックは ROADMAP Phase 完了条件評価軸と構造的に整合しないため、Codex intent-check は省略する。

### I-9: ExitPlanMode → 起票

ExitPlanMode を呼びユーザー承認を取る。承認後:

```bash
gh issue create \
  --title "<要約タイトル>" \
  --body-file /tmp/intake-intent-*.md \
  --label "$LABELS"
```

起票直後に lint 確認:

```bash
bun .claude/skills/3ai/scripts/lint-issue-labels.ts --issue $NEW_ISSUE_NUM
```

(exit 0 必須)

### I-10: 経緯保存 + lock release

```bash
bun .claude/skills/3ailoop-intake/scripts/intake-record.ts append \
  --issue $NEW_ISSUE_NUM --stage created --data '{"labels":"...","phase":N}'

bun .claude/skills/3ailoop/scripts/loop-lock.ts release --token "$TOKEN"
```

`features/.intake/issue-<N>.yaml` に I-2〜I-9 各 stage の結果を時系列で保存。

## 重複時の運用ルール

`intake-dedup` で類似度の高い Issue が見つかったとき:
- **必ずユーザー判断**を仰ぐ (自動マージ・自動別件化はしない、plan 方針 11)

## ROADMAP 外要望

`intake-phase-estimate` が `needs-phase` を返した場合:
- I-7 で `needs-phase` ラベルを CSV に含めて起票 (loop は触らない)
- 人間が ROADMAP 改訂時にまとめてレビュー (plan 方針 12)

## 関連スクリプト

`.claude/skills/3ailoop-intake/scripts/`:
- `intake-dedup.ts` — gh issue list + jaccard 類似度
- `intake-phase-estimate.ts` — ROADMAP Phase 突合
- `intake-granularity.ts` — 長さ・連結語・包括語の heuristic
- `intake-gate-detector.ts` — UI/UX キーワード検出
- `intake-label-suggest.ts` — type/batch/gate 推定 + lint pass
- `intake-record.ts` — features/.intake/issue-<N>.yaml に経緯保存
