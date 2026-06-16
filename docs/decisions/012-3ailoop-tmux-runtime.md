# ADR-012: /3ailoop tmux ランタイム化 (cron 撤去)

**Date**: 2026-06-16
**Status**: Accepted
**Related**: ADR-002 (差し込み作業の扱い), ADR-006 (Issue 粒度), Issue #167-179 (/3ailoop 自走基盤の構築履歴)

---

## Context

`/3ailoop` は無人連続駆動する自走ループであり、Phase 7 以降の Issue 消化スループットを底上げする横断 foundation である。現状の実装 (Issue #167-179) は `/schedule` 経由の cron で 1 時間おきに新 Claude セッションを spawn し、「文脈リセット = 新プロセス起動 + state.json 再構築」で context 劣化を防いでいる。

**問題**:

cron 駆動には固有の時間効率上のロスがある:

- **1 サイクル < 1h** (典型ケース): 短いサイクルが連発すると、次の cron 起動まで毎回アイドル時間が発生する。`/3ai` 1 周は標準で 20-40 分のオーダーで、1h 間隔とは恒常的にミスマッチ
- **1 サイクル > 1h** (重 Issue): 前サイクルが完了する前に次の cron が走り、`loop-lock.ts` が `LOCKED` を返して 30 秒 retry × 1 でも取れなければ終了する (`features/.loop/lock.json`)。次の chance は最大 2 時間後 (= 最悪 1 サイクル丸ごと取り溢す)

cron 間隔を短くしても解決しない。「サイクル中に次の cron が走るリスク」 vs 「サイクル後のアイドル時間」のトレードオフを sliding するだけで、原因 (=「外部時刻でリセットを発火する」モデル) を取り除けない。

**Claude Code の `/clear` 公式仕様** (https://code.claude.com/docs/en/commands.md):

> `/clear` — Start a new conversation with empty context.

- 会話履歴のみクリアする
- CLAUDE.md / `~/.claude/projects/.../memory/MEMORY.md` (auto memory) / Skill 定義 / Hooks は次入力時に再ロードされる
- 同じ Claude プロセス内で context リセットを発火できる

つまり「セッションリセット = 新プロセス起動」を「セッションリセット = `/clear` 送信」に置き換えれば、サイクル完了の直後にリセットして即座に次サイクルへ移れる。

**前提**:

- 運用環境は自宅 (個人開発)。tmux を運用前提に置くことに実害はない
- 「Claude が固まる / 落ちる」リスクは現運用でも cron で救済できておらず (固まれば次サイクルも `LOCKED`)、tmux モードでも別の救済機構 (異常検知 → 通知 → 人間判断) が必要
- 既存スクリプト (`loop-lock.ts` / `loop-context-bootstrap.ts` / `loop-should-stop.ts` / `loop-cycle-record.ts` 他) は cron / tmux モードのどちらでも変更不要

---

## Decision

`/3ailoop` のランタイムを cron 駆動から **tmux 2 ペイン構成の自走モデル**に転換する。

### 1. アーキテクチャ

```
[起動 pane = オーケストレーター pane]
  └─ loop-tmux-start.ts (one-shot bootstrap)
       ├─ TMUX 環境変数チェック
       ├─ tmux new-window -d -n 3ailoop-worker
       ├─ worker pane で `claude` 起動 → capture-pane で起動完了待機
       ├─ tmux send-keys '/3ailoop' Enter で初回サイクル開始
       └─ loop-tmux-watcher.ts を nohup で daemonize → PID 記録
                  ↓
[watcher daemon = 起動 pane に常駐する Bun プロセス。Claude は乗らない]
  └─ 10 秒 polling
       ├─ state.json の recent_cycles[-1].ended_at を読む
       ├─ 前回観測値と差分があれば「サイクル完了」と判定
       ├─ loop-should-stop.ts を子プロセスで呼ぶ
       │   ├─ RC=1 (stop): loop-notify で通知 → watcher 終了
       │   └─ RC=0 (proceed): worker pane に send-keys '/clear' Enter
       │                       → 復帰確認 → '/3ailoop' Enter
       └─ 異常検知: worker pane 消失 / 45 分 state.json 更新なし → 通知して終了
```

オーケストレーター pane は **watcher daemon の常駐先**。Claude を載せないので長期アイドルでも context を消費せず、ユーザーは起動 pane を解放して別作業に使ってよい。

### 2. 再開トリガー

`features/.loop/state.json` の `recent_cycles[-1].ended_at` を 10 秒間隔で polling する。`loop-cycle-record.ts record` は L-5 で atomic write するため、watcher は race を気にせず読める。差分検知 → 「サイクル完了」と判定して停止判定 → `/clear` → `/3ailoop` の順に send-keys する。

外部時刻ではなく**サイクル完了イベントそのもの**でリセットを発火するモデルになる。これがアイドルゼロ・lock 弾きゼロを成立させる本質。

### 3. pane 特定

ワーカー pane は `tmux new-window -d -n 3ailoop-worker` で **固定 window 名** を持たせる。`tmux send-keys -t =3ailoop-worker ...` で参照できる (`=` は完全一致セレクタ)。複数 `/3ailoop` 並走 (将来) や、ユーザーが他 window を開いていても干渉しない。

### 4. lock 機構の維持

`loop-lock.ts` は **撤去せず残す**。理由:

- 手動で `/3ailoop` を打ち込むケース (デバッグ・smoke) が依然ありうる
- `/3ailoop-intake` との排他は tmux モードでも必要
- watcher は loop 自体に対する **外部からのリセット発火源** であり、lock 機構の意味と直交している

ただし `STALE_THRESHOLD_MS = 12h` は cron 撤去後はもっと短くてよい (例 2h)。これは別 Issue として切り出し、本 ADR ではスコープ外。

### 5. 移行は不可逆

cron モードは撤去する。`/3ailoop` 系の SKILL.md / RUNBOOK から CronCreate / CronDelete / CronList への参照は全削除する。「動的 sentinel モード (予備)」記述も削除。tmux モード単一の運用に統一する。

両モード併存はメンテナンスコストに見合わない (ロジック 2 系統、テスト 2 系統、トラブルシュート 2 系統)。

---

## Alternatives Considered

### (a) sentinel + `claude --continue` (現状の「動的 sentinel モード」延長)

`/3ailoop` 末尾に `<<autonomous-loop-dynamic>>` sentinel を吐き、harness 側で同セッションを継続させる方式。Claude プロセスは継続するが **context は累積する**ため、文脈劣化対策にならない。本来の目的 (= リセット) が達成できないため却下。

### (b) inotify ベース

`features/.loop/state.json` を inotify で監視し OS イベント駆動でリセットを発火する。実装は重く Linux 専用になる。watcher の Bun 実装で十分軽量なため (10 秒 polling は CPU 負荷ほぼゼロ)、複雑度に見合うメリットがない。

### (c) 現状維持 (cron)

ロスを許容する。Phase 7 以降の Issue 量が増えるほどスループット差が拡大するため、早期転換が合理的。

---

## Consequences

### 影響範囲

| 項目 | 影響 |
|---|---|
| CronCreate / CronDelete / CronList 依存 | 撤去 |
| `loop-lock.ts` / `loop-context-bootstrap.ts` / `loop-should-stop.ts` / `loop-cycle-record.ts` / 他 `loop-*` | **無改変** |
| 新規スクリプト | `loop-tmux-start.ts` / `loop-tmux-watcher.ts` / `loop-tmux-stop.ts` (`.claude/skills/3ailoop/scripts/` 配下) |
| 新規 smoke | `loop-tmux-smoke.sh` (既存 `loop-smoke-test.sh` は無改変) |
| 新規状態ディレクトリ | `features/.loop/tmux/` (`watcher.pid` / `watcher.log` / `worker.window` / `last-cycle-ended-at`) |
| SKILL.md | description / L-8 / 「動的 sentinel モード」記述差し替え |
| RUNBOOK (`docs/3ailoop-runbook.md`) | §1 起動手順差し替え、§トラブルシュート追補 |
| CI | tmux smoke は `--dry-run` のみ実行可。実 tmux smoke はローカルのみ |

### 残置リスク

- `/clear` のキーストローク送信仕様が将来の Claude Code 変更で壊れる可能性 → watcher.log で監視できる設計にしておく
- worker pane で Claude が対話的に許可プロンプトを出した場合に watcher は気付かない → 既存運用 (autonomous モード / permission allowlist) を前提
- tmux 環境必須化 → 自宅運用ゆえ問題なし、CI からは smoke は `--dry-run` のみ実行

### 移行手順

本 ADR で決定した方針を 3 Issue に分割して実装する:

1. **#180 (本 Issue)**: ADR-012 起草 (`type: foundation` / `batch:skill`)
2. **#181**: 新規 3 スクリプト + bun test + 新 smoke (`type: feature` / `batch:skill`)
3. **#182**: SKILL.md / RUNBOOK 書き換え + cron 撤去 (`type: foundation` / `batch:skill`)

ADR-006 §1 の粒度ガード (1 Issue = GLM 1 サイクル相当) に沿った直列実行。並列化はしない。

### 関連プラン

- `/home/bacon/.claude/plans/3ailoop-corn-tmux-clear-lazy-pond.md` (本 ADR の決定に至った設計検討メモ)
