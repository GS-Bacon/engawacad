# /3ailoop & /3ailoop-intake RUNBOOK

EngawaCAD の自走ループ /3ailoop と要望投入受け口 /3ailoop-intake の **運用手順書**。
初回起動から定常運用、トラブル対応、終了までを集約する。

関連:
- 設計プラン: `/home/bacon/.claude/plans/3ai-loop-ui-ux-jaunty-ember.md`
- ランタイム ADR: [ADR-012](decisions/012-3ailoop-tmux-runtime.md) (cron 駆動から tmux 自走への転換)
- memory: `project-3ailoop-policy` / `project-3ailoop-intake-policy` / `project-3ailoop-known-races` / `project-3ailoop-implementation-style`
- 実装 Issue: #167 (基盤) → #168 (hardening v2) → #169 (hardening v3) → #170 (コア) → #171 (観測性) → #172 (safety net) → #173 (Phase 切替) → #174 (SKILL) → #175 (intake) → #176 (運用、本 RUNBOOK) → #179 (Discord 通知) → #180 (ADR-012) → #181 (tmux ランタイム)

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

### 1-3. tmux session 準備 + 自走起動 (ADR-012)

ADR-012 により ランタイムは **tmux 2 ペイン構成の自走モデル**。cron は使わない。

```bash
# 1) tmux session を起動 (既存があれば attach、なければ新規)
tmux new-session -A -s engawa

# 2) session 内で start.ts を実行 (オーケストレーター pane で)
bun .claude/skills/3ailoop/scripts/loop-tmux-start.ts
```

`loop-tmux-start.ts` は以下を行う:
1. TMUX 環境変数を検査 (session 外なら exit 2)
2. 既存 worker pane (`3ailoop-worker` window) の重複を検査 (R4-F03 対策)
3. `tmux new-window -d -n 3ailoop-worker` でワーカー pane 生成
4. ワーカー pane で `claude` を起動 → capture-pane で起動完了を待機 (デフォルト 60s タイムアウト、`LOOP_TMUX_BOOT_TIMEOUT_SEC` で上書き可)
5. ワーカー pane に `/3ailoop` を送信し初回サイクル開始
6. `loop-tmux-watcher.ts` を detached プロセスとして spawn、PID を `features/.loop/tmux/watcher.pid` に記録

成功すると stdout に `{ ok: true, window: "3ailoop-worker", watcher_pid: <PID> }` が出る。

watcher daemon は 10 秒間隔で `features/.loop/state.json` を polling し、サイクル完了を検知したら `loop-should-stop.ts` を呼び:
- RC=0 (proceed) → ワーカー pane に `/clear` 送信 → 復帰確認 (capture-pane) → `/3ailoop` 再送
- RC=1 (stop) → Discord 通知 + watcher 自身も終了

### 1-4. 5 cycle 立会い

各 cycle 完了後に dashboard と watcher.log を目視:
- 想定外の pause がないか
- token 使用量が予想範囲か
- 起票された子 Issue (split-batch) が妥当か
- ADR draft が立った場合 gate:adr-review に正しく入っているか
- watcher の挙動: `tail -f features/.loop/tmux/watcher.log` で baseline 確立 → 差分検知 → send-keys の流れが見える

異常があれば §4-3 の停止手順を参照。

### 1-5. 定常運用へ移行

5 cycle 異常なし → そのまま自走を継続。memory に「初回 5 cycle 監視済み」を記録。tmux 自走モデルではアイドル時間ゼロ・lock 弾きゼロで動き続けるため、cron のように間隔を調整する必要はない。

---

## 2. 定常運用の確認ポイント

`features/.dashboard.md` を **気が向いたとき** (数日に 1 度でも OK、累積俯瞰型で過去把握できる) 開く:

| セクション | 何を見るか |
|---|---|
| Current Status | running / paused / Pause reason |
| 24h Activity | Closed / Merged / 起票 / 新 ADR / token |
| Pending Gates | gate:human-feel / gate:adr-review の Issue 一覧 |
| Needs-Human Backlog | 失敗退避 Issue + failure count |
| Cumulative Stats | 累積トータル、token 10億 (CUM_PAUSE) 近い? |
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

- `check-issue-granularity.ts` (d6f103b で `dispatch-codex-intent.ts` から移行) の aligned:no が 3 回累積 = 要望と Issue の方向ズレ
- Issue 本文を見直す / ユーザー意図を再確認 / 必要なら起票し直す
- 解決後:
```bash
bun .claude/skills/3ailoop/scripts/loop-intent-guard.ts reset --issue <N>
```

### 3-5. token 累積閾値 (10億, CUM_PAUSE) 超過

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

### 4-3. tmux 自走の停止 / 再起動

```bash
# 通常停止 (watcher 停止 → worker pane kill → lock release)
bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts

# debug: window を残したまま watcher だけ止める
bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts --keep-window

# dry-run で動作確認
bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts --dry-run
```

異常時 (3 cycle 連続 pause など)、loop-cycle-record / watcher が自動で worker を kill することはない (= 設計上停止は人間判断のみ。安全側)。停止後は `loop-tmux-start.ts` を再実行すれば自走再開。

### 4-4. worker pane が異常終了している

```bash
# tmux list-windows で確認
tmux list-windows

# 3ailoop-worker が無くなっていたら watcher は loop-tmux-worker-gone を通知して既に
# 自分も終了している。features/.loop/tmux/watcher.log で確認:
tail -20 features/.loop/tmux/watcher.log

# 復旧: PID file が残っていたら掃除して再起動
bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts  # PID 掃除のため
bun .claude/skills/3ailoop/scripts/loop-tmux-start.ts
```

### 4-5. watcher daemon が固まった / state.json が更新されない

`features/.loop/state.json` の `recent_cycles[-1].ended_at` が `LOOP_TMUX_STUCK_MIN` (デフォルト 45 分) 以上更新されないと、watcher は `loop-tmux-stuck` 通知を出して自滅する。

ただし #209 修正以降、worker pane の foreground command (`tmux display -p '#{pane_current_command}'`) が `bash`/`zsh`/`sh`/`fish`/`dash` 以外のとき (= claude / bun / cargo / node 等が走っている間) は 10s poll の heartbeat で `lastActivityAt` が更新される。長尺 cycle (例 15h) でも自爆しなくなり、stuck 閾値は「素の shell プロンプトで真に idle になってから 45 分」の意味になる。watcher.log の `heartbeat: pane_cmd=... active=... idle=...min` 行で実際の判定経過を追跡できる。

復旧:
```bash
# 1) watcher.log で原因確認
tail -40 features/.loop/tmux/watcher.log

# 2) ワーカー pane に attach して Claude の状態を確認 (フリーズ / 許可プロンプト待ち等)
tmux select-window -t =3ailoop-worker

# 3) 必要なら手動で介入 → /3ailoop を再投入
# 4) 介入できなければ stop → start で再起動
bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts
bun .claude/skills/3ailoop/scripts/loop-tmux-start.ts
```

### 4-6. /clear の送信が効かない (Claude Code 仕様変更)

ADR-012 で前提とした `/clear` のキーストローク送信仕様が壊れた場合の対応:

```bash
# 1) watcher.log で「send-keys: /clear」の後に Claude の応答が無いことを確認
tail -40 features/.loop/tmux/watcher.log

# 2) ワーカー pane に attach して /clear を手動入力して挙動確認
tmux select-window -t =3ailoop-worker

# 3) loop-tmux-stop → 手動で /3ailoop を 1 サイクルずつ回す縮退運用に切り替える
bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts
# その後はワーカー pane で /3ailoop を人手で投入し、1 サイクルごとに /clear を打つ
```

cron / `/schedule` ベースの fallback は ADR-012 で撤去済みのため使用しない。

### 4-7. /3ailoop と /3ailoop-intake の同時起動衝突

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
# 一時停止 (watcher は残して止める)
# 全 open Issue に手動で needs-human ラベル → loop-should-stop が pause を返す
# → watcher が loop-stop を通知して自分も終了する

# 完全停止 (watcher kill + worker pane kill + lock release)
bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts

# debug 用: worker pane を残して watcher だけ止める
bun .claude/skills/3ailoop/scripts/loop-tmux-stop.ts --keep-window
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
