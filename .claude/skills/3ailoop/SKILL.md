---
name: 3ailoop
description: /3ai を常時自律モードで回し続ける無人ループ。tmux (2+N) pane 構成 (N=3 default、LOOP_MAX_WORKERS=1 で従来 2 pane): orchestrator + merge dispatcher + worker × N (各 worktree)。watcher が fan-out で Issue を割当、worker が並列 /3ai、merge pane が serial rebase → CI → push。サイクル完了ごとに /clear → /3ai --issue N。停止条件 = 候補 Issue が gate:* / needs-* のみになったとき。
tools: Read, Bash, Glob, Grep, Skill
---

# /3ailoop — 自走ループ本体 (N=3 並列 default)

EngawaCAD の Issue を **無人で並列消化** する Skill。サイクル完了ごとに `/clear` で context をリセットし、文脈劣化を構造的に防ぐ (ADR-012)。Phase D-1 で N=3 tmux worker pane 並列化に拡張 (#316)、backward compat として N=1 の従来 2 pane モードも保持。

**前提**:
- 関連 plan: `/home/bacon/.claude/plans/3ai-loop-ui-ux-jaunty-ember.md`
- 関連 ADR: [ADR-012](../../../docs/decisions/012-3ailoop-tmux-runtime.md) (cron 駆動から tmux 自走への転換)
- 関連 memory: `project-3ailoop-policy` / `project-3ailoop-implementation-style` / `project-3ailoop-known-races`
- 既存 `/3ai` Skill を **無改変** で内部利用 (Skill ツール経由で自律モード起動)
- ランタイム: tmux 2 ペイン構成。起動は `bun .claude/skills/3ailoop/scripts/loop-tmux-start.ts`、停止は `loop-tmux-stop.ts`

## 実行フロー (L-A → L-0〜L-9)

各 STEP は **bun TS スクリプト** を 1 つ呼ぶだけ。Claude は薄い orchestrator として stdout に従う (判定は TS 側)。

### L-A: tmux 自動 dispatch (#185, #316 Phase D-1)

`/3ailoop` を tmux session 内で素打ちしたとき、watcher 未起動なら自動で (2+N) pane 自走モードに乗る (N=3 default、`LOOP_MAX_WORKERS=1` で従来 2-pane モード)。worker pane 内で動いている場合や非 tmux 環境ではこのステップは no-op で L-0 へ。

```bash
MODE=$(bun .claude/skills/3ailoop/scripts/loop-tmux-dispatch.ts)
case "$MODE" in
  start)
    bun .claude/skills/3ailoop/scripts/loop-tmux-start.ts \
      --claude-cmd "claude --dangerously-skip-permissions --model sonnet"
    exit 0
    ;;
  already-running)
    echo "/3ailoop はすでに別ペインの worker で走っています。何もせず終了します。"
    exit 0
    ;;
  continue)
    : # 通常の L-0〜L-9 へ進む
    ;;
esac
```

- `start`           — `$TMUX` set + watcher 未起動。`loop-tmux-start.ts` を呼ぶ ⇒:
  - **N=1 (backward compat)**: 右ペインが split で開き、claude 起動 → `/3ailoop` 自動投入 → watcher daemon spawn
  - **N>=2 (default 3)**: merge pane + worker × N の (2+N) pane split → 各 worker に worktree 割当 (`git worktree add /home/bacon/worktrees/wN HEAD`) → claude 起動 → worker-registry に登録 → watcher daemon spawn (fan-out mode)
  - どちらも **Claude 本体はここで終了** (= ユーザーが打った pane はオーケストレーター pane として解放される)
- `continue`        — `$TMUX` 未設定、または worker pane 内で /clear → /3ai --issue N で再投入された経路。L-0 へ進む
- `already-running` — `$TMUX` あり + watcher 生存 + 自分は worker pane 以外。手動 /3ailoop の二重実行を避けるため何もせず終了 (#185 R4-F01)

**tmux 構成 (N>=2)**:

```
tmux window
├─ orchestrator pane          (ユーザーが素打ちした pane、start.ts 終了後は解放)
├─ merge pane                 (loop-tmux-merge-dispatcher.ts --watch が常駐)
├─ worker-1 pane (worktree A) Claude Sonnet 5、watcher の指示で /3ai --issue N を実行
├─ worker-2 pane (worktree B) 同上
└─ worker-3 pane (worktree C) 同上
```

- worker-registry.json (`features/.loop/worker-registry.json`) が各 worker の state (idle/busy/merging/error) と current_issue を管理
- watcher daemon が空 worker を検出して次の Issue を assign + `/clear`+`/3ai --issue N` を send-keys で投入

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

### L-1.5: ADR auto-accept rescan (#252)

L-2 (should-stop) の前に滞留中の `gate:adr-review` Issue を rescan する。これがないと
両 LLM 不在 / regen 中断で滞留した gate Issue が永遠に解消されず、L-2 で `batch-select 0`
→ pause → 次サイクル冒頭でまた同じ判定…という構造的デッドロックに陥る (cycle 32-34 実観測)。

```bash
bun .claude/skills/3ailoop/scripts/loop-adr-pause-detector.ts scan --auto-accept --depth 1
```

`--auto-accept` 指定時は新規 ADR scan に加えて **既存 gate:adr-review Issue の rescan** が
自動で走る (`--no-rescan-stale` で無効化可)。各滞留 Issue は #251 GLM fallback を含む
auto-accept チェーンに再投入され、3 persona 全 approved なら gate 削除 + close、refute
1 件以上なら regen_required (`adr-regen-count/<adr>.json` の regen_count を inc)、
cap 越え (3 回) で `needs-human` 退避。

これにより滞留 gate が毎サイクル進行する: approve なら解消、refute 連続なら確定的に
needs-human に移行 (= 人間判断待ち、ただし他の actionable Issue を block しない)。

### L-1.6: Phase 完了処理 (#271 で L-2.5 から移動)

L-2 (should-stop) の前に Phase 完了判定 + 移行を実施する。L-2.5 にあった頃は
「Phase の最終子 Issue が closed → 次サイクル冒頭 L-2 で actionable=0 → pause →
phase-close-check に永遠に到達しない」という構造的デッドロックがあった (cycle 46
で実観測、144 連続 pause で watcher halt)。L-1.6 に前倒しすることで、
最終子 closed の直後サイクルで ROADMAP ✅ + milestone close + current_phase 進行が
発火し、L-2 では既に next phase の Issue が actionable として見える。

```bash
bun .claude/skills/3ailoop/scripts/loop-phase-close-check.ts apply
RC=$?
```

- `RC=0` (apply 成功 = ROADMAP/milestone 更新済 or 完了不要): L-2 へ
- `RC=1` (未完): そのまま L-2 へ (Phase 進行中なら通常 Issue を消化)

phase-close-check は内部で **split parent auto-close** (#271) も実施する:
- Phase N milestone の open type:feature Issue で `blocked-by-split` ラベル付き、
  かつ `parent-blocked-by-split:N` 子 Issue が全 closed なら、親を auto-close
- これがないと #194/#195/#206 系の split 親が永遠に open で Phase 完了判定が失敗する

**#319 Phase E: 3 系統 adversarial audit** (Phase 11/14/17/20 完了時のみ、旧 Fable 5 監査から置換):

`loop-phase-close-check.ts apply` が Phase 11/14/17/20 の完了処理を成功させた直後、内部で `loop-phase-audit.ts --phase N` を invoke する (関数名 `invokePhaseAudit`)。

`loop-phase-audit.ts` の動作:
1. 直近 3 Phase (N-2..N) の ADR 変更 / 実装 diff / cycle-journal をまとめて `features/.loop/phase-audit-N/input.md` に集約
2. **3 系統独立 adversarial dispatch** (並列):
   - **Opus 4.7** (Anthropic 系): Anthropic 系 audit + 集約役
   - **Codex 3 persona** (OpenAI 系): architect / contrarian / migration
   - **GLM 3 persona** (Z.AI 系): impl-detail 系
3. **barrier** で 3 系統完了待ち
4. **集約** (Opus 4.7): 各系統の findings を dedupe + severity 統一 → critical/high/medium/low に分類
5. **Issue 起票**: critical (即修正、loop 消化) / high, medium (defer:phase-N+1) / low (record only)。上限 5
6. **ログ**: `features/.loop/phase-audit-log.jsonl` に append

**現状 (MVP)**: 3 系統 dispatch は **pure-script stub** (`feedback_3ai_billing` の Anthropic claude -p 禁止に配慮)。Phase 11 approaching 時に実 LLM wiring に切り替える TODO を code コメントに明記。

**Fable 5 が使えない理由**: [[fable-5-banned]] — Opus 4.8 auto-fallback あり (tool call 破壊 [[opus-4-8-banned]])。3 系統独立 adversarial は Fable 5 単一よりも correlation blind spot 破りの観点で強い。

### L-1.65: Phase retrospective (#315)

L-1.6 で Phase 完了が確定した cycle のみ発火。Phase 11/14/17/20 は 3 系統 adversarial audit (別枠) と重複するため **skip**、それ以外の Phase で loop 運用メトリクスを自動振り返り。

```bash
# L-1.6 の apply が成功して Phase N が closed になった場合のみ
if [ "$PHASE_CLOSED" = "true" ] && ! [[ "$CLOSED_PHASE" =~ ^(11|14|17|20)$ ]]; then
  bun .claude/skills/3ailoop/scripts/loop-phase-retrospective.ts --phase "$CLOSED_PHASE"
  # dry-run で確認したい場合は --dry-run を追加
fi
```

集計する 4 メトリクス (すべて pure-script、LLM 呼び出しなし):
1. **pause 率**: cycle-journal.log から `pause_reason != null` の割合と top 5 reasons
2. **needs-human 発生数**: 該当 Phase 期間中に raised された Issue で `needs-human` 付与された数
3. **codex-skip 累積**: `features/.loop/codex-skips.jsonl` の created / resolved / pending
4. **Skill 改修投資効果**: `.claude/skills/*` の commit 前後の cycle 平均時間 delta

判定ルール:
- pause 率 > 30% → 「pause 率が高い」Issue 起票
- needs-human >= 3 → 「needs-human 多発」Issue 起票
- codex-skip pending >= 5 → 「Codex skip 滞留」Issue 起票
- Skill 改修後に cycle 時間悪化 → 「Skill 改修後の劣化」Issue 起票
- 常に: 「retro(phaseN): 概要」baseline Issue を 1 件起票

Issue 起票上限は 5 件 (概要 1 + 4 topic)。全て `type: foundation, batch:skill`、milestone 無し。

### L-1.7: Phase seeder (#315)

L-1.6 で current_phase が bump された cycle のみ発火。ROADMAP.md の Phase N 完了条件を読んで起点 Issue を自動起票し、Phase 昇格直後の「actionable=0 で loop 停止」を防ぐ (Phase 10 で 2026-07-01 → 07-06 の 5 日間停止を招いた構造的問題)。

```bash
# L-1.6 の apply が current_phase を N → N+1 に進めた場合のみ
if [ "$PHASE_BUMPED" = "true" ]; then
  bun .claude/skills/3ailoop/scripts/loop-phase-seeder.ts --phase "$NEW_PHASE"
fi
```

動作:
1. ROADMAP.md の `## Phase N: ...` セクションから完了条件を M 個抽出
2. 各条件を Issue draft に展開 (pure-script templating、LLM 未使用)
3. `check-issue-granularity.ts` で全 draft 検証 (in-process import)
4. 粒度 OK → `gh issue create --milestone "Phase N: <title>"` で起票
5. 粒度 NG → `split_proposal` を消費して子 draft に展開 (1 round 限定)、それでも NG なら `needs-human` フラグ付きでスキップ
6. `lint-issue-labels.ts` で label 検証 (in-process)
7. ログ: `features/.loop/phase-seeder-log.jsonl` に append

**設計上の注意**:
- ✅ Phase (`## ✅ Phase N`) を対象にすると exit 2 で拒否 (誤って完了済み Phase を seed しないため)
- Milestone は事前に存在している前提 (L-1.6 の close 処理で next phase milestone は残る)
- 生成される Issue の body は minimal templating。詳細は Claude が STEP 2 で plan.md に落とす

### L-2: 停止条件チェック

```bash
bun .claude/skills/3ailoop/scripts/loop-should-stop.ts
RC=$?
```

- `RC=0` (proceed): 続行 → L-3 へ
- `RC=1` (stop): pause 理由を `loop-cycle-record record --pause-reason "<reason>"` で記録し、`bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind loop-stop --text "[STOP] loop paused — <reason>"` で通知してから、`loop-lock release --token $TOKEN` してから終了 (sentinel emit せず)

**#284: pause で skip した Issue があれば `--pause-issue` と `--pause-category` を併用する**:

```bash
bun .claude/skills/3ailoop/scripts/loop-cycle-record.ts record \
  --pause-reason "$REASON" \
  --pause-issue "274,275,276" \
  --pause-category "intent-aligned-no"
```

- `--pause-issue` は CSV (skip した Issue 番号)。ADR 番号は **入れない** (混在で偽カウントになる)
- `--pause-category` は kebab-case の英数文字列 (例: `intent-aligned-no` / `codex-usage-limit` / `adr-not-finalized` / `scope-cut-error`)
- 各 Issue × Category で連続 3 回到達したとき `needs-human` 自動付与 + コメント投稿 (pause-streak tracker)
- Issue が close (= 前進) すると、その Issue の全 category streak は record 内で自動リセットされる

### L-3: バッチ選定 (loop モード)

```bash
bun .claude/skills/3ai/scripts/batch-select.ts --loop
```

- `features/.batch/plan.json` に loop 用プラン生成
- split-batch tier 最優先、gate/needs-* / blocked-by-split は除外
- **#316 Phase D-1: crate_group 出力** — 各 group に `crate_groups: [{id, issues, parallel_safe, reason}]` が付与される。同 crate group は serial、別 crate group は parallel dispatch 可能

次に着手する Issue の取り出し方は tmux モードで異なる:

**N=1 (backward compat)**: 従来通り 1 Issue pop:
```bash
ISSUE_N=$(bun -e 'console.log(JSON.parse(require("fs").readFileSync("features/.batch/plan.json","utf-8")).issues?.[0]?.number ?? "")')
ISSUE_TITLE=$(bun -e 'console.log(JSON.parse(require("fs").readFileSync("features/.batch/plan.json","utf-8")).issues?.[0]?.title ?? "")')
if [ -n "$ISSUE_N" ]; then
  bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind issue-start --text "[START] #${ISSUE_N} 着手: ${ISSUE_TITLE}"
fi
```

**N>=2**: L-3.5 の fan-out で watcher が代行 (このステップでは plan.json 生成のみ、Issue の pop は watcher 側の責務)。

### L-3.5: Fan-out to N workers (#316 Phase D-1、N>=2 のみ)

watcher daemon が polling で発火する処理 (Claude orchestrator は直接実行しない):

1. worker-registry.json を読み、`state: idle` の worker を列挙
2. plan.json の `crate_groups` をフラット化して `worker-queue.jsonl` (未実装で in-memory) に enqueue
   - `parallel_safe: true` group → 全 Issue を並列 enqueue OK
   - `parallel_safe: false` group → group 内は同時 in-flight 上限 1
3. 各 idle worker に 1 Issue を assign:
   ```bash
   bun .claude/skills/3ailoop/scripts/loop-worker-registry.ts assign --worker-id worker-N --issue $ISSUE_N
   ```
4. 該当 worker pane に slash command を投入:
   ```bash
   tmux send-keys -t <worker-N-pane_id> "/clear" Enter
   tmux send-keys -t <worker-N-pane_id> "/3ai --issue $ISSUE_N --autonomous" Enter
   # #320: --autonomous 必須。/3ai --issue N 単独は対話モード (STEP 4 ExitPlanMode 承認待ち)
   ```

N=1 モードでは L-3.5 は no-op、L-4 に進む。

### L-4: /3ai 自律モード起動

**N=1**: Skill ツールで `/3ai` を **引数なし** で起動 (自律モードで動作、STEP 2.5/4 で ExitPlanMode skip、B-3/B-6 で自動判断)。/3ai 内部で 1 Issue を消化 → 自動 commit + push (Closes #N)、自動 close。

**N>=2**: 各 worker pane で並列に `/3ai --issue N` が起動される (watcher が L-3.5 で送信済み)。各 worker は独立 worktree で動作、STEP 8 で main に直接 push せず、代わりに:

1. `bun loop-worker-registry.ts mark-merging --worker-id worker-N`
2. `bun loop-tmux-merge-dispatcher.ts --enqueue --issue N --worker-id worker-N --worktree <path> --commit-sha $(git rev-parse HEAD)`
3. worker pane は次の Issue 割当を待つ (idle 状態に遷移)

merge pane が serial 処理:
- merge lock 取得 → rebase → cargo xtask ci → push → Issue close → worker release

### L-4.5: Barrier + per-worker cycle-record (#316 Phase D-1、N>=2 のみ)

watcher daemon が polling で発火:

- merge pane の `merge-history.jsonl` を tail して完了した Issue を検出
- 各完了 Issue ごとに L-5 (cycle-record) を発火 (worker 単位ではなく Issue 単位)
- 全 worker 完了を待つ barrier ではなく、**完了順に発火** (先に終わった Issue から dashboard 更新)

### L-5: サイクル記録

```bash
RECORD_JSON=$(bun .claude/skills/3ailoop/scripts/loop-cycle-record.ts record)
echo "$RECORD_JSON"
```

state.json に追記、recent_cycles[] と cumulative 更新。stdout の JSON から closed Issue を取り、各 1 通通知:

```bash
echo "$RECORD_JSON" | bun -e '
  const e = JSON.parse(require("fs").readFileSync(0, "utf-8"));
  for (const n of (e.closed ?? [])) {
    console.log(`#${n}\t${e.cycle}\t${e.merged_commits}`);
  }
' | while IFS=$'\t' read -r N C M; do
  bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind issue-done --text "[DONE] ${N} closed (cycle ${C}, ${M} commits)"
done
```

**#313 Phase A: workspace.dependencies 集約 lint** (CLAUDE.md の依存管理原則を機械検査):

```bash
WD_OUT=$(bun .claude/skills/3ai/scripts/lint-workspace-deps.ts 2>&1)
WD_RC=$?
echo "$WD_OUT" | tail -20
if [ $WD_RC -ne 0 ]; then
  # 非 blocking: warn を Discord に流すのみ (loop は継続)
  VIOLATIONS=$(echo "$WD_OUT" | grep -c "^ERROR:")
  bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind cruft --text "[WARN] workspace.dependencies 非集約 dep 検出 (詳細は loop 出力)"
fi
```

このステップは warn のみ (blocking gate 化は Phase C 以降で判断)。

### L-5.5: Decision log 追記 (条件付き)

サイクル中に重要判断 (ADR draft / Issue 分割 / 失敗退避) があれば手動で:

```bash
bun .claude/skills/3ailoop/scripts/loop-decision-log.ts append --kind <kind> --message "..."
```

### L-5.6: ADR auto-accept フロー (ADR-013)

このサイクル中に /3ai が新規 ADR を作成していれば `gate:adr-review` Issue を起票し、続けて
自動 review chain (Decision Matrix lint + Multi-LLM Review + 暴走防止 cap) を走らせる:

```bash
ADR_JSON=$(bun .claude/skills/3ailoop/scripts/loop-adr-pause-detector.ts scan --auto-accept)
echo "$ADR_JSON"
```

(`features/.loop/last-adr-scan-sha` の marker を使って前回 scan 以降のみ検出。
滞留 gate Issue の rescan は L-1.5 で先行実行されるため、ここでは主に新規 ADR draft の処理)

`--auto-accept` モードでは各 ADR について `loop-adr-auto-accept.ts` を呼び出し、以下を実施:

1. **Decision Matrix lint** (`loop-adr-decision-matrix-lint.ts`) — Options A/B/C / Trade-off / 採用前提崩壊 trigger / 既存 ADR 関係を機械判定。歪み #2 修正で **sensitive topic + orphan check** を追加 (schema/format/topology/boolean を扱う ADR は Related 行に他 ADR 引用必須、本文中の ADR-NNN は Related 行に挙げる)
2. **Cross-ADR 意味整合 precheck** (`loop-adr-cross-ref-check.ts`, 歪み #2 第 1 弾) — LLM 1 呼び出し (Codex → GLM fallback) で「引用すべき過去 accepted ADR」「矛盾する既存 ADR」「ROADMAP 将来 Phase 構想との整合」を点検、misaligned なら missing_refs/conflicts/suggestions を regen にフィードバック
3. **Multi-LLM Adversarial Review** — Codex を architect / contrarian / migration の 3 ペルソナで並列実行、refute デフォルト
4. **Refute overrider** (`loop-adr-refute-overrider.ts`, 歪み #2 第 2 弾) — refute が出た場合のみ起動。各 refute を「真の矛盾 (keep)」「字義解釈 (override)」で 1 LLM judge。全 refute override 可能なら accept ルート昇格、1 つでも keep なら regen 維持
5. **暴走防止 cap** (`loop-adr-regen-tracker.ts`) — 再生成 3 回越え or 1 ADR 350k token 越えで `needs-human` 退避 (元 200k、cross-ref + overrider 追加で引き上げ)

3 ペルソナ全員 approved (または全 refute が override 可) なら `gate:adr-review` を削除して Issue を close (auto-accept)。
cross-ref misaligned / 1 ペルソナでも keep_refute なら regen_required (呼び元 = 次サイクルの /3ai が draft 再生成)。
cap 越えなら `gate:adr-review` 削除 + `needs-human` 付与 で loop は他 Issue に進む。

旧運用 (手動 gate:adr-review pause) に戻すには `--auto-accept` を外す。fallback として gate ラベルと
人間判定経路は残してある (ADR-002 ラベル運用)。

新規に起票した gate Issue を通知 (JSON 出力以外の "no new ADRs" 行は skip):

```bash
echo "$ADR_JSON" | bun -e '
  let raw = require("fs").readFileSync(0, "utf-8").trim();
  let obj; try { obj = JSON.parse(raw); } catch { process.exit(0); }
  for (const c of (obj.created ?? [])) {
    if (c.issue) console.log(`${c.issue}\t${c.adr}`);
  }
' | while IFS=$'\t' read -r N ADR; do
  bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind issue-raised-adr --text "[NEW] #${N} raised: gate:adr-review for ${ADR}"
done
```

### L-5.7: split-detector 実行 (各 feature の review yaml)

直近サイクルで /3ai が生成した feature dir があれば、その review yaml に split_proposal が
含まれているか確認:

```bash
for review_yaml in features/*/codex-final.yaml; do
  [ -f "$review_yaml" ] || continue
  PARENT=$(basename "$(dirname "$review_yaml")" | awk -F- '{print $1}')
  SPLIT_OUT=$(bun .claude/skills/3ailoop/scripts/loop-split-detector.ts process \
    --review-yaml "$review_yaml" --parent-issue "$PARENT" 2>&1 || true)
  echo "$SPLIT_OUT" | tail -5
  # JSON 末尾行を抽出して子 Issue ごとに通知
  echo "$SPLIT_OUT" | bun -e '
    const lines = require("fs").readFileSync(0,"utf-8").trim().split("\n");
    for (let i = lines.length - 1; i >= 0; i--) {
      const l = lines[i].trim();
      if (!l.startsWith("{")) continue;
      try {
        const o = JSON.parse(l);
        if (!o.ok || !Array.isArray(o.children)) break;
        for (const c of o.children) {
          if (c.child) console.log(`${c.child}\t${o.parent}`);
        }
        break;
      } catch {}
    }
  ' | while IFS=$'\t' read -r CHILD PARENT_N; do
    bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind issue-raised-split --text "[NEW] #${CHILD} raised (split of #${PARENT_N})"
  done
done
```

(split_proposal なしの review は no-op で終了)

### L-5.8: intent-guard (aligned:no カウント) + intent-based auto-split

/3ai の intent-check 結果 (`features/*/intent-check.yaml` および `features/.batch/intent-*.yaml`) に `aligned: no` があるとき、**`split_proposal:` の有無で分岐**する。

- **`split_proposal:` あり** (Codex が粒度違反を検出して分割案を併記) → `loop-split-detector.ts` に流して親 `blocked-by-split` + 子起票。intent-guard は inc しない (auto-split で構造的解決したため)。
- **`split_proposal:` なし** → 従来通り intent-guard inc (連続 3 回で needs-intent-review)。

```bash
process_intent_yaml() {
  local ic="$1"
  local parent_hint="$2"  # dir 由来 / file 由来のヒント
  [ -f "$ic" ] || return
  if ! grep -q '^aligned:\s*no' "$ic"; then return; fi

  if grep -q '^split_proposal:' "$ic"; then
    # auto-split ルート
    local PARENT="$parent_hint"
    SD_OUT=$(bun .claude/skills/3ailoop/scripts/loop-split-detector.ts process \
      --review-yaml "$ic" --parent-issue "$PARENT" 2>&1 || true)
    echo "$SD_OUT" | tail -5
    # 子 Issue ごとに 1 通通知
    echo "$SD_OUT" | bun -e '
      const lines = require("fs").readFileSync(0,"utf-8").trim().split("\n");
      for (let i = lines.length - 1; i >= 0; i--) {
        const l = lines[i].trim();
        if (!l.startsWith("{")) continue;
        try {
          const o = JSON.parse(l);
          if (!o.ok || !Array.isArray(o.children)) break;
          for (const c of o.children) { if (c.child) console.log(`${c.child}\t${o.parent}`); }
          break;
        } catch {}
      }
    ' | while IFS=$'\t' read -r CHILD PARENT_N; do
      bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind issue-raised-split --text "[NEW] #${CHILD} raised (intent-check split of #${PARENT_N})"
    done
  else
    # 従来: intent-guard inc
    IG_OUT=$(bun .claude/skills/3ailoop/scripts/loop-intent-guard.ts inc --issue "$parent_hint" 2>&1)
    echo "$IG_OUT" | tail -2
    if echo "$IG_OUT" | grep -q "needs-intent-review added"; then
      CNT=$(echo "$IG_OUT" | grep -oE "reached [0-9]+" | grep -oE "[0-9]+" | head -1)
      bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind intent-guard --text "[WARN] #${parent_hint} needs-intent-review (aligned:no ×${CNT:-3})"
    fi
  fi
}

# feature-dir 経由 (features/N-slug/intent-check.yaml)
for ic in features/*/intent-check.yaml; do
  [ -f "$ic" ] || continue
  PARENT=$(basename "$(dirname "$ic")" | awk -F- '{print $1}')
  process_intent_yaml "$ic" "$PARENT"
done

# batch 経由 (features/.batch/intent-N.yaml) — B-3 実行時に生成される
for ic in features/.batch/intent-*.yaml; do
  [ -f "$ic" ] || continue
  PARENT=$(basename "$ic" .yaml | sed 's/^intent-//')
  process_intent_yaml "$ic" "$PARENT"
done
```

**#284 補足**: `loop-intent-guard.ts` は intent-check.yaml が **生成された** ケース専用カウンタ。
ADR 滞留など intent-check 自体に到達しない pause は L-2 stop / L-3 batch pick で skip されるため、
intent-guard は inc されない。同等の状況を捕捉する経路として **L-2 stop 時に `loop-cycle-record record`
で `--pause-issue` + `--pause-category` を渡す** ことで `pause-streak tracker` が連続検出する
(両者は並行運用)。

### L-5.9: Codex skip 回収 (後払いレビュー)

Codex gate (STEP 3.5 / 7.5) が usage-limit 等でスキップされ台帳 (`features/.loop/codex-skips.jsonl`) に積まれたエントリを、Codex 回復後にまとめて後追いレビューする。1 サイクル最大 2 件、usage-limit 検出で即中断 (残りは次サイクル):

```bash
bun .claude/skills/3ailoop/scripts/loop-codex-skip-collector.ts
```

fail-safe 設計で常に RC=0 (処理結果は stdout の JSON ログ)。blocking findings が出たエントリは `bug` Issue を自動起票 (元 batch 継承 or `batch:kernel`) し、`resolved_at` を書き込んで `codex-review-deferred` ラベルを除去する。merge commit が無いエントリ (WIP 凍結等) は no-merge-commit として close 扱い。

### L-6: Dashboard 更新

```bash
bun .claude/skills/3ailoop/scripts/loop-dashboard.ts
```

`features/.dashboard.md` を累積俯瞰型で上書き。

**#313 Phase A: cruft トレンド snapshot + render** (`#[ignore]` / `#[allow(clippy::)]` / regression test file 数の推移):

```bash
# 1. 現在の cruft 数を features/.loop/cruft-trend.jsonl に append
bun .claude/skills/3ailoop/scripts/dashboard-cruft-trend.ts snapshot

# 2. dashboard 末尾に「## Cruft Trend」セクションを append (3 snapshot 前との差分)
CRUFT_MD=$(bun .claude/skills/3ailoop/scripts/dashboard-cruft-trend.ts render)
printf '\n---\n\n%s\n' "$CRUFT_MD" >> features/.dashboard.md
```

累積傾向を可視化するだけで blocking はしない。`#[ignore]` / `#[allow(clippy::)]` が silently 増え続ける「loop 自体が bad state を生む」構造の早期検知に使う。

### L-7: 失敗ループ検出

直近サイクルで Issue N が失敗していたら:

```bash
FT_OUT=$(bun .claude/skills/3ailoop/scripts/loop-failure-tracker.ts inc --issue N 2>&1)
echo "$FT_OUT"
```

N=3 で `needs-human` 退避 (gh edit 内部)。threshold 越え時は通知:

```bash
if echo "$FT_OUT" | grep -q "needs-human"; then
  STREAK=$(echo "$FT_OUT" | grep -oE "[0-9]+" | head -1)
  bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind needs-human --text "[WARN] #N needs-human (failure streak ${STREAK:-3})"
fi
```

### L-7.5: Token 閾値チェック

```bash
bun .claude/skills/3ailoop/scripts/loop-token-meter.ts check
RC=$?
```

- `RC=0`: 続行
- `RC=1` (累積 10億 超): `loop-cycle-record record --pause-reason "token threshold"` で記録 → `bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind token-limit --text "[STOP] token limit reached (累積 10億 超) — paused"` で通知 → release → 終了

### L-8: サイクル末尾 (tmux 自走モード)

何も出さず終了する。次サイクルは tmux 自走モデルが起動する:

- オーケストレーター pane の watcher daemon (`loop-tmux-watcher.ts`) が `state.json` の `recent_cycles[-1].ended_at` を polling し、L-5 で更新された差分を検知する
- 停止判定 (`loop-should-stop.ts`) が proceed (RC=0) を返せば、watcher はワーカー pane に `tmux send-keys '/clear'` → 復帰確認 → `tmux send-keys '/3ailoop'` を順に流し込む
- ADR-012 で決定した通り、cron / sentinel は使用しない

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
- `loop-token-meter.ts` (#171) — token 閾値 (累積 10億 pause / 24h 100M warning)
- `loop-decision-log.ts` (#171) — 重要判断の時系列追記
- `loop-failure-tracker.ts` (#172) — 連続失敗 N=3 で needs-human
- `loop-pause-streak-tracker.ts` (#284) — Issue × Category 2 軸の連続 pause カウンタ。L-5 の `loop-cycle-record record --pause-issue --pause-category` から呼ばれ N=3 で needs-human
- `loop-adr-pause-detector.ts` (#172, #190) — 新規 ADR で gate:adr-review、`--auto-accept` で review chain 連動
- `loop-adr-decision-matrix-lint.ts` (#191, ADR-013) — ADR draft の必須セクションを機械 lint
- `loop-adr-auto-accept.ts` (#190, ADR-013) — Decision Matrix lint + Multi-LLM Review + auto-accept フロー本体
- `loop-adr-regen-tracker.ts` (#192, ADR-013) — 再生成 cap 3 + token 上限 200k の永続化トラッカー
- `loop-split-detector.ts` (#172) — split_proposal で親 blocked + 子起票
- `loop-intent-guard.ts` (#172) — aligned:no N=3 で needs-intent-review
- `loop-phase-close-check.ts` (#173) — Phase 完了条件検証 + ROADMAP/milestone 更新
- `loop-notify.ts` — 各 L ステップから Discord Webhook へ 1 行通知 (env `DISCORD_WEBHOOK_URL` 未設定で silent skip、失敗しても loop は止めない)
- `loop-tmux-dispatch.ts` (#185) — `$TMUX` と watcher PID 状態から `start` / `continue` を判定し L-A で分岐する
- `loop-tmux-start.ts` (#181, #185) — tmux 環境チェック → ワーカー pane を `tmux split-window -h` で現 window の右に split → pane_id 取得 → Claude 起動 → 初回 `/3ailoop` 投入 → watcher daemon spawn
- `loop-tmux-watcher.ts` (#181, #185) — `state.json` の `recent_cycles[-1].ended_at` を 10s polling、差分検知で pane_id 経由の `/clear` → `/3ailoop` を send-keys。worker pane 消失 / 45min フリーズ / tmux 一時障害 3 連続失敗で安全側に停止
- `loop-tmux-stop.ts` (#181, #185) — watcher PID kill (SIGTERM → 5s → SIGKILL) → worker pane `/exit` → `tmux kill-pane` → pane が確実に消えた場合のみ lock release

`.claude/skills/3ai/scripts/`:
- `batch-select.ts --loop` (#170) — loop モード (split-batch tier 最優先 + 共通 exclude)
- `state.ts` (#167-169) — failure_streak 個別ファイル化 (`features/.loop/failure-streak/<N>.json`)
- `lint-issue-labels.ts` (#167-169) — 既知ラベル厳格化
- `dispatch-codex.ts --mode review` — Issue #8 累積 review で使用
