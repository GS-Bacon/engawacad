---
name: 3ailoop
description: /3ai を常時自律モードで回し続ける無人ループ。tmux 2 ペイン構成 (オーケストレーター pane に watcher daemon、ワーカー pane で Claude が /3ailoop を実行) で、サイクル完了次第ただちに /clear → /3ailoop を再投入する自走モデル。停止条件 = 候補 Issue が gate:* / needs-* のみになったとき。
tools: Read, Bash, Glob, Grep, Skill
---

# /3ailoop — 自走ループ本体

EngawaCAD の Issue を **無人で連続消化** する Skill。サイクル完了ごとに `/clear` で context をリセットし、文脈劣化を構造的に防ぐ (ADR-012)。

**前提**:
- 関連 plan: `/home/bacon/.claude/plans/3ai-loop-ui-ux-jaunty-ember.md`
- 関連 ADR: [ADR-012](../../../docs/decisions/012-3ailoop-tmux-runtime.md) (cron 駆動から tmux 自走への転換)
- 関連 memory: `project-3ailoop-policy` / `project-3ailoop-implementation-style` / `project-3ailoop-known-races`
- 既存 `/3ai` Skill を **無改変** で内部利用 (Skill ツール経由で自律モード起動)
- ランタイム: tmux 2 ペイン構成。起動は `bun .claude/skills/3ailoop/scripts/loop-tmux-start.ts`、停止は `loop-tmux-stop.ts`

## 実行フロー (L-A → L-0〜L-9)

各 STEP は **bun TS スクリプト** を 1 つ呼ぶだけ。Claude は薄い orchestrator として stdout に従う (判定は TS 側)。

### L-A: tmux 自動 dispatch (#185)

`/3ailoop` を tmux session 内で素打ちしたとき、watcher 未起動なら自動で右ペイン自走モードに乗る。worker pane 内で動いている場合や非 tmux 環境ではこのステップは no-op で L-0 へ。

```bash
MODE=$(bun .claude/skills/3ailoop/scripts/loop-tmux-dispatch.ts)
case "$MODE" in
  start)
    bun .claude/skills/3ailoop/scripts/loop-tmux-start.ts \
      --claude-cmd "claude --dangerously-skip-permissions"
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

- `start`           — `$TMUX` set + watcher 未起動。`loop-tmux-start.ts` を呼ぶ ⇒ 右ペインが split で開き、claude 起動 → `/3ailoop` 自動投入 → watcher daemon spawn。**Claude 本体はここで終了** (= ユーザーが打った pane はオーケストレーター pane として解放される)
- `continue`        — `$TMUX` 未設定、または worker pane 内で /clear → /3ailoop で再投入された経路。L-0 へ進む
- `already-running` — `$TMUX` あり + watcher 生存 + 自分は worker pane 以外。手動 /3ailoop の二重実行を避けるため何もせず終了 (#185 R4-F01)

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
- 次に着手する Issue を plan.json から取り出して通知:
  ```bash
  ISSUE_N=$(bun -e 'console.log(JSON.parse(require("fs").readFileSync("features/.batch/plan.json","utf-8")).issues?.[0]?.number ?? "")')
  ISSUE_TITLE=$(bun -e 'console.log(JSON.parse(require("fs").readFileSync("features/.batch/plan.json","utf-8")).issues?.[0]?.title ?? "")')
  if [ -n "$ISSUE_N" ]; then
    bun .claude/skills/3ailoop/scripts/loop-notify.ts --kind issue-start --text "[START] #${ISSUE_N} 着手: ${ISSUE_TITLE}"
  fi
  ```

### L-4: /3ai 自律モード起動

Skill ツールで `/3ai` を **引数なし** で起動 (自律モードで動作、STEP 2.5/4 で ExitPlanMode skip、B-3/B-6 で自動判断)。

/3ai 内部で 1 Issue を消化 → 自動 commit + push (Closes #N)、自動 close。

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
