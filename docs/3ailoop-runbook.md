# /3ailoop & /3ailoop-intake RUNBOOK

EngawaCAD の自走ループ /3ailoop と要望投入受け口 /3ailoop-intake の **運用手順書**。
初回起動から定常運用、トラブル対応、終了までを集約する。

関連:
- 設計プラン: `/home/bacon/.claude/plans/3ai-loop-ui-ux-jaunty-ember.md`
- memory: `project-3ailoop-policy` / `project-3ailoop-intake-policy` / `project-3ailoop-known-races` / `project-3ailoop-implementation-style`
- 実装 Issue: #167 (基盤) → #168 (hardening v2) → #169 (hardening v3) → #170 (コア) → #171 (観測性) → #172 (safety net) → #173 (Phase 切替) → #174 (SKILL) → #175 (intake) → #176 (運用、本 RUNBOOK)

---

## 1. 初回起動手順

### 1-1. Smoke test

```bash
bash .claude/skills/3ailoop/scripts/loop-smoke-test.sh
```

L-0〜L-9 全 PASS を確認 (/3ai 自律起動 L-4 は skip された smoke 限定)。
成果物:
- `features/.loop/state.json` (cycle=1)
- `features/.loop/decisions.log.md`
- `features/.dashboard.md` (7 セクション)

### 1-2. Dashboard 目視確認

```bash
cat features/.dashboard.md
```

Current Status / Loop-actionable open issues / Pending Gates / Needs-Human Backlog を確認。

### 1-3. CronCreate 設定 (初回 5 cycle、1 時間間隔)

```text
# Claude Code で /schedule を起動して以下のように設定:
スケジュール: 1 時間ごと (cron: 0 */1 * * *)
起動内容: /3ailoop
期間: 最初の 5 cycle (= 5 時間)
```

詳細は `/schedule` Skill のドキュメント参照。
CronCreate は **人間が起動** する (cron job の所有者明示のため)。

### 1-4. 5 cycle 立会い

各 cycle 完了後に dashboard を目視:
- 想定外の pause がないか
- token 使用量が予想範囲か
- 起票された子 Issue (split-batch) が妥当か
- ADR draft が立った場合 gate:adr-review に正しく入っているか

異常があれば:
```bash
# CronDelete で停止
# 該当 cycle の features/.loop/state.json と features/.dashboard.md を確認
```

### 1-5. 定常運用へ移行

5 cycle 異常なし → CronCreate を **2 時間間隔** (cron: 0 */2 * * *) に更新。memory に「初回 5 cycle 監視済み」を記録。

---

## 2. 定常運用の確認ポイント

`features/.dashboard.md` を **気が向いたとき** (数日に 1 度でも OK、累積俯瞰型で過去把握できる) 開く:

| セクション | 何を見るか |
|---|---|
| Current Status | running / paused / Pause reason |
| 24h Activity | Closed / Merged / 起票 / 新 ADR / token |
| Pending Gates | gate:human-feel / gate:adr-review の Issue 一覧 |
| Needs-Human Backlog | 失敗退避 Issue + failure count |
| Cumulative Stats | 累積トータル、token 100M 近い? |
| Recent Activity | 直近 20 件のサイクル結果 |
| Decision Log | ADR draft / Issue 分割 / 退避 / Phase 切替 履歴 |

---

## 3. Pause 時の対応

### 3-1. gate:human-feel の Issue が残っている

- 該当 Issue を gh で開く
- viewer / 既存実装で視覚確認 (acceptance criteria 照合)
- 承認: ラベル削除 → loop が次サイクルで pick
- 差し戻し: コメントに修正要求 → ラベル維持

```bash
gh issue edit <N> --remove-label gate:human-feel
```

### 3-2. gate:adr-review の Issue

- 紐づく ADR ファイル (`docs/decisions/NNN-*.md`) を読む
- accepted: ADR の `Status` を `Accepted` に書換 + commit、Issue に accepted コメント、ラベル削除
- rejected: ADR を revert する commit、Issue に rejected コメント

### 3-3. needs-human (失敗退避)

- `features/.loop/failure-streak/<N>.json` で failure count 確認
- 該当 Issue の最新 `features/<N>-*/ci.log` / `debug-spec.md` を読む
- 問題を手動で分析 → 対処 (修正 commit / Issue 分割 / 取り下げ)
- 解決後:
```bash
bun .claude/skills/3ailoop/scripts/loop-failure-tracker.ts reset --issue <N>
```

### 3-4. needs-intent-review

- `dispatch-codex-intent.ts` の aligned:no が 3 回累積 = 要望と Issue の方向ズレ
- Issue 本文を見直す / ユーザー意図を再確認 / 必要なら起票し直す
- 解決後:
```bash
bun .claude/skills/3ailoop/scripts/loop-intent-guard.ts reset --issue <N>
```

### 3-5. token 累積閾値 (100M) 超過

- `features/.loop/state.json` の `cumulative.token_*` を確認
- 想定範囲なら閾値を上げる (`loop-token-meter.ts` の `CUM_PAUSE` 定数編集)
- 想定外なら原因調査 (どの dispatch が大量消費したか log で追跡)

---

## 4. トラブル時の復旧

### 4-1. lock stale (異常終了で残った lock)

```bash
# 状態確認
bun .claude/skills/3ailoop/scripts/loop-lock.ts status

# 12h 経過なら自動 stale 回収されるはず。即時復旧したい場合:
bun .claude/skills/3ailoop/scripts/loop-lock.ts release --force
```

### 4-2. state.json 破損

```bash
# 破損ファイルをバックアップして削除
mv features/.loop/state.json features/.loop/state.json.broken

# 次サイクルで自動初期化される
```

cumulative は失われるが、recent_cycles は再構築可能。

### 4-3. CronDelete で停止

```bash
# /schedule で該当 cron を削除、または:
# CronList で確認 → 該当 ID を CronDelete
```

異常時 (3 cycle 連続 pause など)、loop-cycle-record が自動 CronDelete することはない (= 設計上 cron 停止は人間判断のみ。安全側)。

### 4-4. /3ailoop と /3ailoop-intake の同時起動衝突

- intake が「lock 取得失敗」で停止する設計
- 30s 待って retry、それでもダメなら手動 release --force (慎重に)
- 通常は loop の cycle 完了 (数分〜数十分) を待つだけ

---

## 5. /3ailoop-intake の使い方

```text
/3ailoop-intake "<要望文>"
```

- Plan モードに自動で入る
- 重複候補がでたら判断
- Phase 推定結果を確認
- 粒度判定で NG なら分割
- gate 判定で UI/UX なら mock を Issue 本文に貼付
- label 提案を確認 (修正可)
- ExitPlanMode で承認 → 起票

Phase 着手時の mock セッション運用は memory `project-3ailoop-intake-policy` 参照。

---

## 6. 既知の race / 受容項目

memory `project-3ailoop-known-races` 参照。lock の stale takeover race (rmSync 無条件削除) や state.json plain RMW 等、実装に既知の race condition があるが実運用での発生確率は低い。初回 5 cycle 立会いで挙動観察し対処方針確定する。

---

## 7. ループ終了 / 一時停止

```bash
# 一時停止 (cron は残す)
# 全 open Issue に手動で needs-human ラベル → loop-should-stop が pause を返す

# 完全停止 (cron 削除)
# /schedule または CronDelete
```

---

## 8. Discord 通知 (任意)

`/3ailoop` の各 L ステップから 1 行通知を Discord Webhook へ流せる。設定は **env だけ**、未設定なら通知は silent skip され loop の動作には一切影響しない。

### 8-1. セットアップ

1. Discord 側で対象のフォーラムチャンネルに対し Webhook を作成し URL を取得。
2. 流したい既存スレッド(投稿)の **スレッド ID** を控える (スレッド右クリック → 「リンクをコピー」の末尾の数値、または Discord 開発者モードでコピー)。
3. URL の末尾に `?thread_id=<スレッドID>` を付けて 1 本の URL に結合する。
4. `.claude/settings.local.json` (git ignore 済み) の `env` に追加:
   ```json
   {
     "env": {
       "DISCORD_WEBHOOK_URL": "https://discord.com/api/webhooks/<id>/<token>?thread_id=<thread_id>"
     }
   }
   ```
   - フォーラムチャンネル全体に投稿したい (= 通知ごとに新スレッドを作る) 場合は別仕様 (本実装は thread_id 集約のみ対応)。
5. 動作確認:
   ```bash
   bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind smoke --text "[TEST] notify wiring OK"
   ```
   stderr に何も出ず exit 0、Discord スレッドに 1 行届けば成功。

### 8-2. 通知される主なイベント

| L | kind | 例 |
|---|---|---|
| L-2 | `loop-stop` | `[STOP] loop paused — gate-only` |
| L-3 末尾 | `issue-start` | `[START] #45 着手: <title>` |
| L-5 | `issue-done` | `[DONE] #45 closed (cycle 13, 4 commits)` |
| L-5.6 | `issue-raised-adr` | `[NEW] #82 raised: gate:adr-review for docs/decisions/...` |
| L-5.7 | `issue-raised-split` | `[NEW] #83 raised (split of #45)` |
| L-5.8 | `intent-guard` | `[WARN] #45 needs-intent-review (aligned:no ×3)` |
| L-7 | `needs-human` | `[WARN] #N needs-human (failure streak 3)` |
| L-7.5 | `token-limit` | `[STOP] token limit reached — paused` |

### 8-3. 無効化 / 一時停止

- 完全停止: `.claude/settings.local.json` の `env.DISCORD_WEBHOOK_URL` を削除 (または空文字)。次サイクルから skip。
- 一時止めだけしたい: シェルで `unset DISCORD_WEBHOOK_URL` してから loop を起動。

### 8-4. 失敗時の挙動

- env 未設定 → stderr に `[notify] DISCORD_WEBHOOK_URL not set, skip`、exit 0。
- HTTP error / timeout / 401 / 404 → stderr に `[notify] notify-failed (kind=...): ...`、exit 0。loop は止まらない。
- 429 (rate limit) → `Retry-After` を 1 回だけ尊重して再送、それでも失敗なら諦める。

通知の取りこぼしは設計上許容 (Discord 通知は障害の検知手段であり、唯一の障害源にはしない方針)。確実に追いたいなら `features/.dashboard.md` と `features/.loop/decisions.log.md` を見る。
