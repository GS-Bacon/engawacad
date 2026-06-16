---
name: 3ailoop
description: /3ai を常時自律モードで回し続ける無人ループ。CronCreate 経由で各サイクル新セッション起動、文脈劣化対策で memory/plan/state/ROADMAP を毎回 read。停止条件 = 候補 Issue が gate:* / needs-* のみになったとき。
tools: Read, Bash, Glob, Grep, Skill
---

# /3ailoop — 自走ループ本体

EngawaCAD の Issue を **無人で連続消化** する Skill。各サイクルが新セッションで起動し、文脈劣化を構造的に防ぐ。

**前提**:
- 関連 plan: `/home/bacon/.claude/plans/3ai-loop-ui-ux-jaunty-ember.md`
- 関連 memory: `project-3ailoop-policy` / `project-3ailoop-implementation-style` / `project-3ailoop-known-races`
- 既存 `/3ai` Skill を **無改変** で内部利用 (Skill ツール経由で自律モード起動)

## 実行フロー (L-0〜L-9)

各 STEP は **bun TS スクリプト** を 1 つ呼ぶだけ。Claude は薄い orchestrator として stdout に従う (判定は TS 側)。

### L-0: lock 取得

```bash
TOKEN=$(bun .claude/skills/3ailoop/scripts/loop-lock.ts acquire --owner loop)
```

- 取得失敗 (intake 進行中など) → `LOCKED: ...` が stderr に出る → **30 秒待ってから 1 度だけ retry**、それでもダメなら終了 (sentinel emit せず)
- 取得成功 → `$TOKEN` を保持

### L-1: Context bootstrap (文脈再構築)

```bash
bun .claude/skills/3ailoop/scripts/loop-context-bootstrap.ts
```

stdout に Current Phase / MEMORY index / dashboard / 直近 cycle / open Issue 内訳が出る。
**Claude は出力を確認して** 直近の運用ルール (memory) を再認識すること。

### L-2: 停止条件チェック

```bash
bun .claude/skills/3ailoop/scripts/loop-should-stop.ts
RC=$?
```

- `RC=0` (proceed): 続行 → L-2.5 へ
- `RC=1` (stop): pause 理由を `loop-cycle-record record --pause-reason "<reason>"` で記録し、`loop-lock release --token $TOKEN` してから終了 (sentinel emit せず)

### L-2.5: Phase 完了処理

```bash
bun .claude/skills/3ailoop/scripts/loop-phase-close-check.ts apply
RC=$?
```

- `RC=0` (apply 成功 = ROADMAP/milestone 更新済 or 完了不要): L-3 へ
- `RC=1` (未完): そのまま L-3 へ (Phase 進行中なら通常 Issue を消化)

### L-3: バッチ選定 (loop モード)

```bash
bun .claude/skills/3ai/scripts/batch-select.ts --loop
```

- `features/.batch/plan.json` に loop 用プラン生成
- split-batch tier 最優先、gate/needs-* / blocked-by-split は除外

### L-4: /3ai 自律モード起動

Skill ツールで `/3ai` を **引数なし** で起動 (自律モードで動作、STEP 2.5/4 で ExitPlanMode skip、B-3/B-6 で自動判断)。

/3ai 内部で 1 Issue を消化 → 自動 commit + push (Closes #N)、自動 close。

### L-5: サイクル記録

```bash
bun .claude/skills/3ailoop/scripts/loop-cycle-record.ts record
```

state.json に追記、recent_cycles[] と cumulative 更新。

### L-5.5: Decision log 追記 (条件付き)

サイクル中に重要判断 (ADR draft / Issue 分割 / 失敗退避) があれば手動で:

```bash
bun .claude/skills/3ailoop/scripts/loop-decision-log.ts append --kind <kind> --message "..."
```

### L-5.6: ADR pause detector (#178 指摘 6 配線)

このサイクル中に /3ai が新規 ADR を作成していれば gate:adr-review Issue を起票:

```bash
bun .claude/skills/3ailoop/scripts/loop-adr-pause-detector.ts scan
```

(`features/.loop/last-adr-scan-sha` の marker を使って前回 scan 以降のみ検出)

### L-5.7: split-detector 実行 (各 feature の review yaml)

直近サイクルで /3ai が生成した feature dir があれば、その review yaml に split_proposal が
含まれているか確認:

```bash
for review_yaml in features/*/codex-final.yaml; do
  [ -f "$review_yaml" ] || continue
  PARENT=$(basename "$(dirname "$review_yaml")" | awk -F- '{print $1}')
  bun .claude/skills/3ailoop/scripts/loop-split-detector.ts process \
    --review-yaml "$review_yaml" --parent-issue "$PARENT" 2>&1 | tail -5 || true
done
```

(split_proposal なしの review は no-op で終了)

### L-5.8: intent-guard (aligned:no カウント)

/3ai の intent-check 結果 (`features/*/intent-check.yaml`) に `aligned: no` があれば inc:

```bash
for ic in features/*/intent-check.yaml; do
  [ -f "$ic" ] || continue
  if grep -q '^aligned:\s*no' "$ic"; then
    PARENT=$(basename "$(dirname "$ic")" | awk -F- '{print $1}')
    bun .claude/skills/3ailoop/scripts/loop-intent-guard.ts inc --issue "$PARENT" 2>&1 | tail -2
  fi
done
```

### L-6: Dashboard 更新

```bash
bun .claude/skills/3ailoop/scripts/loop-dashboard.ts
```

`features/.dashboard.md` を累積俯瞰型で上書き。

### L-7: 失敗ループ検出

直近サイクルで Issue N が失敗していたら:

```bash
bun .claude/skills/3ailoop/scripts/loop-failure-tracker.ts inc --issue N
```

N=3 で `needs-human` 退避 (gh edit 内部)。

### L-7.5: Token 閾値チェック

```bash
bun .claude/skills/3ailoop/scripts/loop-token-meter.ts check
RC=$?
```

- `RC=0`: 続行
- `RC=1` (累積 100M 超): `loop-cycle-record record --pause-reason "token threshold"` で記録 → release → 終了

### L-8: Sentinel emit

- **CronCreate モード**: 何も出さず終了 (次 cron が起動)
- **動的 sentinel モード** (予備): stdout に `<<autonomous-loop>>` を出して終了

### L-9: Lock 解放

```bash
bun .claude/skills/3ailoop/scripts/loop-lock.ts release --token "$TOKEN"
```

## ADR/分割の自動検出 (毎サイクル末尾、L-6 と並行)

サイクル中に /3ai が新規 ADR を draft したら:

```bash
bun .claude/skills/3ailoop/scripts/loop-adr-pause-detector.ts scan --depth 5
```

→ `gate:adr-review` Issue を起票 (次 cycle の L-2 で pause 判定される)

GLM/Codex review が `split_proposal` を出力していたら:

```bash
bun .claude/skills/3ailoop/scripts/loop-split-detector.ts process \
  --review-yaml features/<feature>/codex-final.yaml \
  --parent-issue <N>
```

→ 親 `blocked-by-split` + 子起票 (次 cycle の L-3 で split-batch tier 最優先で pick)

## 既知の race / 受容項目

memory `project-3ailoop-known-races` に詳細。loop-lock の stale takeover race (CAS rename 未実装) / state.json plain RMW / 同 issue 並列 inc / TTL 12h と renew 不整合 / future timestamp 検証なし — 実運用での発生確率は低いが、初回 5 cycle 立会い (Issue #8) で挙動観察し対処方針を確定する。

## 関連スクリプト

`.claude/skills/3ailoop/scripts/`:
- `loop-lock.ts` (#167-169) — atomic dir + session token + stale 12h
- `loop-context-bootstrap.ts` (#170) — 各サイクル冒頭の文脈再構築
- `loop-should-stop.ts` (#170) — gate/needs-* のみで pause 判定
- `loop-cycle-record.ts` (#171) — サイクル集計を state.json に追記
- `loop-dashboard.ts` (#171) — features/.dashboard.md 累積俯瞰型生成
- `loop-token-meter.ts` (#171) — token 閾値 (累積 100M / 24h 5M)
- `loop-decision-log.ts` (#171) — 重要判断の時系列追記
- `loop-failure-tracker.ts` (#172) — 連続失敗 N=3 で needs-human
- `loop-adr-pause-detector.ts` (#172) — 新規 ADR で gate:adr-review
- `loop-split-detector.ts` (#172) — split_proposal で親 blocked + 子起票
- `loop-intent-guard.ts` (#172) — aligned:no N=3 で needs-intent-review
- `loop-phase-close-check.ts` (#173) — Phase 完了条件検証 + ROADMAP/milestone 更新

`.claude/skills/3ai/scripts/`:
- `batch-select.ts --loop` (#170) — loop モード (split-batch tier 最優先 + 共通 exclude)
- `state.ts` (#167-169) — failure_streak 個別ファイル化 (`features/.loop/failure-streak/<N>.json`)
- `lint-issue-labels.ts` (#167-169) — 既知ラベル厳格化
- `dispatch-codex.ts --mode review` — Issue #8 累積 review で使用
