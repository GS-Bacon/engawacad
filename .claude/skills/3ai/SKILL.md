---
name: 3ai
description: Claude × Codex × GLM マルチエージェント CAD 開発フロー。GitHub Issue を起点に設計(Claude)→設計レビュー(GLM多ペルソナ)→実装+テスト(GLM)→最終レビュー(GLM)→独立技術レビュー(Codex)を一周する。
tools: Read, Write, Edit, Bash, Glob, Grep
---

# /3ai — マルチエージェント CAD 開発フロー

このスキルが呼ばれたら以下の手順を**必ず順番通りに**実行すること。

---

## 引数解釈（最初に判定）

- `/3ai --issue N` → **単一 Issue モード** (対話)。STEP 0 へ直行。承認待ちあり (STEP 4 ExitPlanMode 実行)。
- `/3ai --issue N --autonomous` → **単一 Issue 自律モード** (#320 Phase D-1 対応)。STEP 0 へ直行、STEP 4 ExitPlanMode skip + 各種 escalation は Opus 4.7 subagent 判定に委譲。**watcher fan-out (L-3.5) が使う経路**。
- `/3ai`（引数なし）→ **自律バッチモード**（承認待ちなし・無人進行）。STEP B へ。`plan.json.batch_arg === null` がスイッチ。
- `/3ai --batch fixes` → **対話バッチモード**（bug+batch:* に絞る、従来どおりユーザー確認あり）。STEP B へ。
- `/3ai --batch phase` → **対話バッチモード**（現 Phase milestone の type:feature に絞る、従来どおりユーザー確認あり）。STEP B へ。

### 自律モード判定 (STEP 内で参照)

以下のいずれかを満たすとき **自律モード**:
1. `--batch` 付き起動で `batch_arg === null` (引数なし起動、自律バッチ)
2. `--issue N --autonomous` (単一 Issue 自律)

---

## 🚨 AUTONOMOUS MODE 禁止事項 (#323 systemic rule)

**自律モードでは以下を絶対に守る**。SKILL.md の個別 STEP に記述が無くても、このルールが優先する。

### 1. `AskUserQuestion` を絶対に呼ばない

曖昧場面 (branch 衝突、cargo warning 解釈、git rebase conflict、tests flake、any judgment call) に遭遇したら:

**3 段階判定** (順に試す):
1. **Claude 自力判断**: plan.md / Non-Goals / ADR / 直近 commit log / test 出力を Read/grep して自己判断。判断根拠を `features/$N-$SLUG/plan.md` の「自律判断ログ」セクションに append
2. **Opus 4.7 subagent 委譲**: 上記で決まらない場合、Agent tool で subagent 起動:
   ```
   Agent(
     subagent_type: "general-purpose",
     model: "opus",
     description: "<judgment 内容>",
     prompt: "<状況説明 + 選択肢 + 判断基準>"
   )
   ```
   subagent の返答に従って進行
3. **needs-human 退避**: 上記でも決まらない = 本当に人間判断必要:
   ```bash
   bun .claude/skills/3ai/scripts/raise-issue-on-failure.ts \
     --step "<現 STEP>" --feature-dir features/$N-$SLUG \
     --error-summary "<状況説明>"
   ```
   起票後、**その場で /3ai 終了**。worker pane は idle に戻る (watcher が次 Issue を fan-out)。

### 2. Plan mode に入ったら即 `ExitPlanMode`

Plan mode は承認プロンプトを構造的に含む。autonomous mode ではまず入らないよう努める。入ってしまった場合は情報共有として plan を stdout に流し、即 ExitPlanMode を呼ぶ (これは承認取得ではなく mode 遷移だけの用途)。

### 3. 個別 STEP の記述と衝突した場合

各 STEP に「ユーザーへエスカレーション」「停止して確認」と書いてあっても、autonomous mode ではこの systemic rule が優先する = 3 段階判定に流す。SKILL.md 個別記述は対話モード用のフォールバック。

### 4. なぜこのルールが必要か

Claude Code (worker pane の Claude) は SKILL.md に書いてない曖昧場面で AskUserQuestion を **自主的に呼ぶ**。これは runtime 動作で SKILL.md の個別 STEP 追記では防げない (無限モグラ叩き、#320/#322 で観測)。**冒頭に systemic rule を置くことで Claude の判断傾向自体を変える**。

### 5. 例外

- Bash 実行の permission prompt は `--dangerously-skip-permissions` で既に bypass 済
- 本ルールは AskUserQuestion tool 呼び出しに限る
- MCP tool の連携で外部からユーザー確認要求が来る場合は別扱い (現状該当なし)

### 6. #324 git stash 禁止 (autonomous mode)

**autonomous mode の worker では git stash を呼ばない**。

理由:
- git stash はリポジトリ全体で共有される (worktree 単位で isolate されない)
- N=3 worker が並列で走る場合、他 worker の stash が自 worktree に降ってくる race がある (2026-07-26 に worker-1 で観測)
- rebase / reset で意図しない変更が反映されるリスク

代替:
- 一時退避したい場合は **branch を切って commit**、後で cherry-pick or 破棄
- rebase 中断時は `git rebase --abort` (stash 不要)
- 汚染された作業木の回復は `git reset --hard HEAD` (自分の branch HEAD に戻すだけ)

---

自律モードで skip / 委譲される STEP (個別記述):
- STEP 4 ExitPlanMode → skip (情報共有のみ、待たない)
- STEP 6.5 期待値乖離エスカレ → Opus 4.7 subagent 判定に委譲 (memory: [[opus-delegation-for-implementation]])
- STEP 6-D adversarial refute エスカレ → Opus 4.7 subagent 判定に委譲
- STEP 7 critical エスカレ → Opus 4.7 subagent 判定に委譲

---

## STEP B: バッチ実行（バッチモード専用）

> 引数なし、または `--batch` 付きで呼ばれた場合のみ実行する。`--issue N` の場合は STEP 0 へ直行。

### 自律スイッチ

B-1 で生成した `plan.json` の `batch_arg` フィールドで挙動を切り替える:

- **`batch_arg === null`（引数なし起動）→ 自律モード**: B-2 slug ログ表示後に承認を待たず自動進行する。B-3 では Claude が自力で要件を詰めて推奨実装で続行する。B-4 の full Issue では STEP 4 の `ExitPlanMode` をスキップし情報共有のみ行う。止まるのは **B-3 で自力解決できない曖昧さ** と **各種 escalation ゲート** のみ。
- **`batch_arg !== null`（`--batch fixes/phase`）→ 対話モード**: 従来どおり B-2 着手確認・B-3 ユーザー確認・B-4 full の STEP 4 `ExitPlanMode` をすべて維持する。

### B-1: 実行プラン生成

```bash
bun .claude/skills/3ai/scripts/batch-select.ts [--batch fixes|phase]
```

`features/.batch/plan.json` を読み込む。`groups` が空なら「対象 Issue が見つかりませんでした」を報告して終了する。

`batch_start_sha`（plan.json に含まれる）は B-6 横断レビューで使う。

### B-2: プラン提示（情報共有 + slug 確認）

`plan.json` の内容を平易な言葉でユーザーに提示する:
- 選択した tier・グループ一覧・各 Issue の `flow`（light/full）・`gate`（auto/pause）・実行順序
- 導出した slug を Issue ごとに**必ず一覧表示する**（間違いに気づけるようログを省かない）

**自律モード（`batch_arg === null`）**: slug・順序を表示したら承認を待たず直ちに B-3 へ進む。slug は表示した値を自動採用（後から変えられない）。`ExitPlanMode` も着手確認も行わない。

**対話モード（`batch_arg !== null`）**: slug の修正があれば **ここで一括受付**する（後から変えられない）。全件 `flow: light` のバッチはここで一括着手確認を行う（`ExitPlanMode` なし、承認=ユーザーの返答）。

### B-3: pause の front-load（実装前に人間介在を集約）

`gate: pause` または `intent_check_required: true` の Issue がある場合、実装ループ前にまとめて処理する。

**自律モード（`batch_arg === null`）**:

- **`needs-review` ラベルあり / ambiguous** → ユーザーに確認する前に **Claude が ROADMAP・関連 ADR・関連 Issue・既存コードを Read/grep して要件を確定する**。確定できた場合は *推奨設計* で続行し、曖昧点と採った判断を `features/$N-$SLUG/plan.md` の冒頭に「自律判断ログ」として明記する。どうしても確定できない（情報が存在しない / 複数選択肢が等価で根拠なし）場合のみ停止してユーザーにエスカレーション。
- **`intent_check_required: true`** → **Claude 内製の粒度チェック** を実行 (Codex 呼び撤廃、ADR-006 §1 の粒度チェックリストを決定的ルールで判定):
  ```bash
  bun .claude/skills/3ai/scripts/check-issue-granularity.ts \
    --issue $N \
    --result features/.batch/intent-$N.yaml
  ```
  出力 yaml は Codex intent-check 時代のフォーマットと互換 (`aligned: yes/no/skip` + optional `reason:` + optional `split_proposal:`)。判定ルールは type 軸ラベル / In-Scope 記載 / enhancement 禁止 / タイトル内機能列挙 3+ の 4 種類。

  `aligned: yes` → 続行。`aligned: no` → **intent-check yaml に `split_proposal:` セクションが含まれているかで分岐**:
  - **`split_proposal:` あり** (タイトル内で 3 個以上の機能が列挙された粒度過大ケース) → auto-split ルートに乗せる:
    ```bash
    bun .claude/skills/3ailoop/scripts/loop-split-detector.ts process \
      --review-yaml features/.batch/intent-$N.yaml --parent-issue $N
    ```
    → 親 Issue に `blocked-by-split` + 子 Issue 起票 → 本 Issue は skip して次 Issue へ。次サイクルで子 Issue が phase-feature tier で pick 可能になる。
  - **`split_proposal:` なし** (type 軸ラベル欠如 / In-Scope 欠如 / enhancement 使用等の refute) → Claude が Issue を修正 (ラベル追加 / In-Scope 表追記) して続行。ただし *Phase/スコープ自体の根本的不整合* と判断したら推奨実装せず停止してユーザーにエスカレーション。
  - `aligned: skip (split-detector parent)` → **yes と等価扱い** (#235: body に `loop-split-detector で分割される想定` か label `splittable` のある起点 Issue は粒度チェックを skip し、STEP 3 で `split_proposal` 経路に乗せる)。

**対話モード（`batch_arg !== null`）**:

- **`needs-review` ラベルあり / ambiguous** → ユーザーに要件を確認。解決したら続行、解決しなければバッチから除外する
- **`intent_check_required: true`** → Claude 内製の粒度チェック (`check-issue-granularity.ts`) を実行し `aligned: no` → ユーザーと相談し、スコープ修正またはバッチから除外する

**B-3 完了後の残 Issue は無人で自動進行する。**

### B-4: グループ・Issue ループ（auto 実行）

`plan.json` の `groups[].order` 順、各グループ内は `deps` 依存順に Issue を処理する:

1. `main` ブランチ上でクリーンな状態を確認する
2. `bun .claude/skills/3ai/scripts/init-feature.ts --issue $N --slug $SLUG` を実行する  
   （`features/$N-$SLUG/` が既存の場合はスキップ → 完了済みとして continue）
3. `flow: full` の Issue → 既存 STEP 1（ディレクトリ作成済み、Issue 確定のみ）→ STEP 2 → 2.5 → STEP 3（GLM 設計レビュー収束）→ **STEP 3.5（Codex 独立設計 gate、1 persona × ループなし）** まで実行した後:
   - **自律モード（`batch_arg === null`）**: STEP 4 の `ExitPlanMode` をスキップ。代わりに STEP 2.5 と同じ「確定 plan の要点を情報共有として表示（待たない）」を行い、そのまま STEP 5 へ進む。
   - **対話モード（`batch_arg !== null`）**: 従来どおり STEP 4 `ExitPlanMode`（唯一の承認点）を実行し、承認後に STEP 5〜8 を自動進行する。
4. `flow: light` の Issue → **B-5 へ**

### B-5: Lightweight フロー（機械的 Issue）

**スキップ**: STEP 2（壁打ち）/ STEP 2.5 / STEP 3（GLM 多ペルソナ設計レビュー全体）/ STEP 4（ExitPlanMode）

**実行（順序通り）**:

1. **light STEP 2'**: Claude が `features/$N-$SLUG/plan.md` を自動生成する（壁打ちなし）。  
   必須セクション: `## In-Scope / Out-of-Scope` 表、`## Non-Goals`、実装対象、テスト計画 ID 表（normal ≥1 + `_degen_/_boundary_` ガード行）
2. **STEP 5**: ブランチ作成 (`git checkout -b cad/$N-$SLUG`)
3. **STEP 5.5**: Acceptance Test Skeleton 作成（**保持** — 偽陽性ガード、機械的 Issue でも省略しない）
4. **STEP 6 / 6.5 / 6.6**: GLM コア実装 → test-spec 作成 → GLM テスト実装（既存フローをそのまま実行）
5. **STEP 7**: GLM 最終レビュー
6. **STEP 7.5**: Codex 独立技術ゲート (1 persona × ループなし)
   - `keep_codex_gate: true`（= batch:kernel）→ **STEP 7.5 を保持する**（幾何不変量リスクが高いため）  
   - `keep_codex_gate: false`（= それ以外の light）→ **STEP 7.5 をスキップし B-6 Claude spot check で拾う**（Codex 呼びは行わない）  
   - **state shim**: 7.5 をスキップする Issue は STEP 7 末で以下を実行し STEP 8 の assert ゲートを通す:  
     ```bash
     bun .claude/skills/3ai/scripts/state.ts set \
       features/$N-$SLUG/state.json codex_review passed
     ```
7. **STEP 8**: squash マージ + `finalize-feature.ts`（逐次実行のため単一ツリーで安全）

### B-6: 横断 Claude spot check（全グループ完了後）

Codex 削減改修 Task 5 で **Codex 呼びを廃止**。個別 Issue の Codex 独立ゲート (STEP 7.5) 済みなので、バッチ全体横断で新規発見できる領域は少ないため、Claude が commit range を summarize して spot check する軽量経路に置き換える。

`batch_start_sha`（plan.json に記録）を base に:

```bash
# 1) commit range を要約
git log --oneline "$BATCH_START_SHA..HEAD"
git diff --stat "$BATCH_START_SHA..HEAD"

# 2) 気になる箇所を Read で spot check (crates/**、tests/**、examples/**)
#    - 新規 pub API のシグネチャに違和感がないか
#    - Boolean/Partition 系で退化ケースの防衛が抜けていないか
#    - golden YAML の差分が意図通りか
```

**Claude の判断で必要と感じた場合のみ**、手動で個別 Issue の STEP 7.5 を再走 (`dispatch-codex.ts --mode review`) してよい。デフォルトは Codex 呼びなしで進む。

- 気になる指摘があれば `features/.batch/crosscut-findings.md` に記録して次バッチに引き継ぐ
- 明確なバグと判断したら `bug` ラベル + `batch:*` 継承で新規 Issue を起票 (`raise-issue-on-failure.ts` の呼び出し規約に従う)

---

### B-7: バッチ終端 reconciliation（**必須**・全グループ + B-6 完了後）

> B-6 が終わったら**必ずここを実行する**。任意でも dry-run でもない。

**1. 成果物回収（intake/batch 含む）**
```bash
# --sweep は .intake/.batch の共有成果物も一括回収する（B-2 是正後の拡張動作）
bun .claude/skills/3ai/scripts/finalize-feature.ts --sweep
```
- 未コミット成果物が 0 件なら no-op（冪等）。

**2. 解消済み auto-raise Issue の close**
```bash
# sweep モードで全 feature の raised_issues を走査し、対応 step が passed なら close する
bun .claude/skills/3ai/scripts/resolve-issues.ts --sweep
```
- dry-run で事前確認してから本番実行してもよい:
  `bun .claude/skills/3ai/scripts/resolve-issues.ts --sweep --dry-run`

**3. ROADMAP/マイルストーン突合（ADR-002 手続き）**
- 現 Phase の `type: feature` Issue が**全 closed** かどうかを確認する。
- 全 closed なら: ROADMAP の当 Phase を `🚧` → `✅` に更新 + マイルストーンを close。
- 未 closed が残るなら何もしない（次バッチへ持ち越し）。

---

## STEP 0: プランモードへ移行

### 対話モード (`--issue N` 単独 or `--batch fixes/phase`)

**`EnterPlanMode` を呼ぶ。**

> プランモードでも **Bash dispatch は通る**（Edit/Write 等のファイル変更のみ deny）。
> STEP 3 の GLM 設計レビューは ExitPlanMode 前に実行すること（後ろ倒し禁止）。

### 🚨 autonomous モード (`--issue N --autonomous` or 引数なし) — #328

**`EnterPlanMode` を呼ばない**。理由:

- Plan mode 中は Edit/Write 制限、STEP 5+ で解除に `ExitPlanMode` が必須
- `ExitPlanMode` 呼び出し自体が Claude Code の runtime で **"Would you like to proceed?" 承認 UI を必ず表示**する (autonomous mode でも回避不可、実観測 2026-07-26 worker-1 #297)
- SKILL.md instruction では suppress できない (Claude Code built-in behavior)

代替: **通常モードのまま STEP 1-8 を実行**。STEP 2 の plan.md 執筆は Write tool で直接ファイル作成 (Plan mode 不要)。STEP 3-D 等の判断は AUTONOMOUS MODE 禁止事項 (§1-6) に従って進める。

対話モードでは Plan mode + ExitPlanMode 承認が「唯一のユーザー承認点」として機能する。autonomous mode はこの承認点自体を持たない設計 (代わりに Opus 4.7 subagent 委譲 + needs-human 退避で判断)。

---

## STEP 1: Issue 選択・作業ディレクトリ作成

```bash
gh issue list --state open
```

ROADMAP.md の現 Phase に紐づく Milestone の未着手 Issue を提案し、ユーザーに選んでもらう。  
選んだ Issue 番号を `$ISSUE_NUM`、スラッグを `$ISSUE_SLUG` とする（例: `42-make-cylinder`）。  
`--issue N` 指定時（バッチから呼ばれる場合を含む）はこの一覧表示と対話選択を飛ばし、N を `$ISSUE_NUM` に使う。

作業ディレクトリと状態ファイルを一括生成（`features/$ISSUE_NUM-$ISSUE_SLUG/` に plan.md / rejection.md / judgment-summary.md / state.json が作られる）:

```bash
bun .claude/skills/3ai/scripts/init-feature.ts --issue $ISSUE_NUM --slug $ISSUE_SLUG
```

### STEP 1-A: 自動起票 Issue 検出時の前回 ci.log 必須調査 (#150)

Issue タイトルに `[自動起票]` が含まれる、または body に `*このIssueは raise-issue-on-failure.ts により自動起票されました。*` が含まれる場合は **必ず以下を実行する**:

1. Issue body の `feature-dir: features/N-slug/` から元の作業ディレクトリを抽出する
2. **必須**: その feature-dir の `ci.log` を読み、`FAILED` / `error:` / `panicked at` / `error[E` などのキーワードでエラー周辺を確認する (`grep -nE '(FAILED|error:|panicked at|error\[E)' features/N-slug/ci.log | tail -20`)
3. plan.md の「根本原因」セクションは **その grep 結果 + 関連コードを読んで自分の言葉で書く**。Issue 本文の `error_summary` を鵜呑みにしない
4. 起票元 Issue の本文 `## ci.log 失敗周辺 (抜粋)` セクション (本 Issue #150 で `raise-issue-on-failure.ts` が出力するようになった) も併読すると効率的

これにより「Issue 本文の誤診断を信じて plan を作り、後段の CI で真因判明 → やり直し」の構造的再発を防ぐ。

---

## STEP 2: 壁打ち・設計・テスト設計

ユーザーと議論しながら設計を固める。CLAUDE.md の原則を守ること:
- **推測禁止** — 不明点は `grep`/`Read` で調査してから書く
- **反論・客観的視点を提供** — 問題を多角的に検討する

`features/$ISSUE_NUM-$ISSUE_SLUG/plan.md` の各セクションに設計を記述する（セクション見出しは plan.md 内コメントを参照）。必須事項:

- **In-Scope / Out-of-Scope 表**: 本 Issue でやること / やらないことを表形式で明記（GLM SCOPE ペルソナが存在を検証する）
- **Non-Goals**: 本 Issue で実装しない項目を列挙（「該当なし」でも明記、空欄禁止）
- **設計方針**: 決定性要件・B-rep トポロジー妥当性（Euler-Poincaré V-E+F=2）・退化幾何の扱い・derive 規約・エラーハンドリング・workspace.dependencies
- **数値モデル** (Phase 4/6+ のみ必須、不要なら削除): tolerance/ε 値・退化判定基準・ADR-004 準拠方針
- **テスト計画（ID 付き）**: T01 決定性、T02〜正常系、エッジケース、golden YAML
- **幾何的不変条件チェックリスト**: Boolean/Partition/Assemble 系のみ（非該当は N/A）
- **既存関数を編集する場合**: plan.md の「実装対象」または「実装順序」セクションに、**修正箇所ごとに before / after コードスニペット**を含めること。新規ファイル・新規関数の追加のみで完結する Issue では不要。

---

## STEP 2.5: プラン Draft をユーザーへ情報共有（GLM レビュー前）

プランファイルの記述が完成したら、要点をユーザーへ**情報共有として**提示する（承認は STEP 4 の `ExitPlanMode` で取るため、ここでは不要）。

- 専門用語を避け、噛み砕いた言葉で
- 「何を作るか」「主要な設計判断とその理由」「テスト方針の要点」の 3 点を番号付きで簡潔に
- プランファイルの全文転記ではなく**要点の要約**

ユーザーから任意のフィードバックがあれば反映する。フィードバックの有無にかかわらず、常に STEP 3 へ進む。`ExitPlanMode` は呼ばない。

---

## STEP 3: GLM 設計レビュー（多ペルソナ・収束ループ）

**ループ上限**: Issue サイズで決定 — light=3, standard=5, heavy=10。  
`design_loops` カウンタが上限を超えたらユーザーに確認する。

### 3-A: plan 前提チェック

plan.md に `## In-Scope / Out-of-Scope` と `## Non-Goals` が存在するか確認。
なければ追記してから次へ（`## In-Scope / Out-of-Scope` がない場合 GLM SCOPE が critical を出す）。

`adr-context.md` が必要な場合（plan が特定 ADR を参照）は dispatch 前に Claude が抜粋して `features/$ISSUE_NUM-$ISSUE_SLUG/adr-context.md` に書く。

### 3-B: ペルソナ一覧確定

基本: `scope` / `invariant` / `ambig`  
以下の条件で `numeric` を追加:
- Phase 4/6+ の Issue、または
- `plan.md` に `### 数値モデル` セクションが存在する、または
- Codex intent-check で「数値判断含む」と判定された場合

### 3-C: GLM ペルソナ並列 dispatch（`run_in_background: true` で各ペルソナを起動）

ラウンド番号 N = `design_loops` に 1 を加えた値:
```bash
bun .claude/skills/3ai/scripts/state.ts inc \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json design_loops
```

**plan スナップショット保存**:
```bash
mkdir -p features/$ISSUE_NUM-$ISSUE_SLUG/plan-snapshots
cp <プランファイルパス> features/$ISSUE_NUM-$ISSUE_SLUG/plan-snapshots/plan.md.round-N
```

各ペルソナを **並列バックグラウンド** で起動（`run_in_background: true`）:
```bash
bun .claude/skills/3ai/scripts/dispatch-glm-review.ts \
  --persona scope \
  --issue $ISSUE_NUM --round N \
  --input <プランファイルパス> \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/review-scope-rN.yaml \
  [--adr-context features/$ISSUE_NUM-$ISSUE_SLUG/adr-context.md] \
  [--rejection features/$ISSUE_NUM-$ISSUE_SLUG/rejection.md] \
  [--judgment-summary features/$ISSUE_NUM-$ISSUE_SLUG/judgment-summary.md] \
  [--plan-snapshot-dir features/$ISSUE_NUM-$ISSUE_SLUG/plan-snapshots]

# 同様に --persona invariant, ambig [, numeric] を並列起動
```

**全ペルソナの完了通知を待つ（ポーリングしない）。**

### 3-D: 集約・棄却 gate（毎 round 必須）

**#315 Phase B: Opus 4.7 委譲**（worker pane は Sonnet 5 化のため、判断が重い本 STEP は Agent tool で Opus 4.7 subagent に委譲する）:

```
Agent(
  subagent_type: "general-purpose",
  model: "opus",
  description: "STEP 3-D: GLM 指摘を採用/棄却",
  prompt: "以下の GLM 4 persona review 出力を読み、各 issue を採用/棄却/部分採用に判定してください..."
)
```

`model: "opus"` は Anthropic の Opus 4.7 (`claude-opus-4-7`) に解決される。Opus 4.8 は tool call 破壊のため禁止 (memory: opus-4-8-banned)。Sonnet 5 で走ると Non-Goals 防衛が甘くなり STEP 3 loop 回数が増える。

subagent の返答フォーマット: 各 issue の `{finding_id, verdict: adopted|rejected|partial, reason, plan_edit_diff}` を JSON で返させる。orchestrator (Sonnet 5) がそれを受けて plan.md 修正 / rejection.md 追記を実行する。

---

1. 全ペルソナの `review-*-rN.yaml` を読む
2. **重複除去**: 同一論点を複数ペルソナが指摘している場合、最も高 severity の 1 件に統合
3. 各 issue を Opus 4.7 subagent が判定:
   - **採用** → plan を直接修正
   - **棄却** → `features/$ISSUE_NUM-$ISSUE_SLUG/rejection.md` に `## Round N` で追記
   - **部分採用** → plan 一部修正 + rejection.md に残り件を追記

採用数・棄却数を記録:
```bash
bun .claude/skills/3ai/scripts/state.ts judge \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  N <adopted_count> <rejected_count>
```

`features/$ISSUE_NUM-$ISSUE_SLUG/judgment-summary.md` に採用・棄却の一覧を `## Round N` で追記する。

**early-stop チェック**（次 round dispatch 前に必ず実行）:
```bash
bun .claude/skills/3ai/scripts/state.ts check-early-stop \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json
```
exit 1 が返った場合（2 round 連続で全指摘を棄却）:

**#322 対話モード**: **停止してユーザーにエスカレーション**:
> 2 round 連続で全 GLM 指摘を棄却しています。設計そのものに問題がある可能性があります。
> rejection.md を提示します。設計を見直してから再開してください。

**#322 autonomous モード**: **Opus 4.7 subagent 判定に委譲** (--autonomous or batch_arg === null):
```
Agent(
  subagent_type: "general-purpose",
  model: "opus",
  description: "STEP 3-D early-stop 判定",
  prompt: "2 round 連続で全 GLM 指摘を棄却した。rejection.md と plan.md を読み、以下の 3 択で判定してください: (a) 収束続行 (棄却根拠が妥当) (b) round 追加実行 (念のため) (c) needs-human 退避 (設計そのものに問題)。判定根拠を返してください。"
)
```
subagent の返答が (a) → 3-F 進行、(b) → 3-C 再 dispatch、(c) → `raise-issue-on-failure.ts` で needs-human 起票して次 Issue へ。

**全採用警告チェック**（次 round dispatch 前に必ず実行）:
```bash
bun .claude/skills/3ai/scripts/state.ts check-full-adoption-warning \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json
```
exit 1 が返った場合:

**#322 対話モード**: 停止してユーザーへ表示:
> 2 round 連続で棄却が 0 件です。scope 防衛できていますか?
> Non-Goals に含まれる指摘や medium 以下で受容すべき指摘は棄却 log に記録してから次 round に進んでください。

**#322 autonomous モード**: Opus 4.7 subagent が rejection.md / Non-Goals を読み、scope 防衛不足なら次 round dispatch 前に指摘を棄却リストに追加、防衛済みなら round 続行。

### 3-E: 収束判定・再 dispatch

- **全 Critical/High が 0** かつ **2 round 連続で C/H = 0** → STEP 3-F へ
- **Critical/High が残る** → 3-C に戻って再 dispatch（N++）
- **design_loops が上限超過** → Critical の数を確認:
  - critical = 0: Claude 裁量で残 high/medium を「採用→修正」「棄却→rejection.md」で処理 → 3-F へ
  - critical ≥ 1: **#322 対話モード**: 停止してユーザーにエスカレーション / **autonomous モード**: Opus 4.7 subagent が残 critical を「plan 修正で吸収 / needs-human 退避」で判定

### 3-F: 通過

```bash
bun .claude/skills/3ai/scripts/state.ts set \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json design_review passed
```

---

## STEP 3.5: Codex 独立設計 gate (1 persona × ループなし)

**目的**: GLM 多ペルソナ (STEP 3) が Z.AI 同系モデルの分身であることによる相関盲点を、別モデル系 (Codex) の 1 発 gate で破る。実装フェーズに入る前に設計段階で発見することで、実装コストを削減する。

**ゲート:**
```bash
bun .claude/skills/3ai/scripts/state.ts assert \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json design_review
```

### 3.5-A: Codex 独立ゲート dispatch (1 persona, 1 呼び, ループ制御なし)

```bash
bun .claude/skills/3ai/scripts/dispatch-codex-design.ts \
  --plan-file <プランファイルパス> \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/codex-design.yaml
```

内部で `plan.md` + `adr-context.md` + `judgment-summary.md` + `rejection.md` (存在するもの) を連結して Codex 1 persona (`codex-design-reviewer.md`) に投げる。`PRIOR JUDGMENTS` ブロックが存在する場合、Codex は Claude の判定 (採用/棄却) を独立に再検証する (agent 定義の「STEP 3.5 独立 gate モード」参照)。

**Codex CLI エラー時のフォールバック** (usage limit / rate limit / exit != 0 等):
- 1 発 gate なのでリトライしない
- **スキップを後払い台帳に記録** (Codex 回復後に L-5.9 collector がまとめて後追いレビュー):
  ```bash
  bun .claude/skills/3ai/scripts/record-codex-skip.ts \
    --issue $ISSUE_NUM --slug $ISSUE_SLUG --step 3.5 --reason "<usage-limit / exit!=0 の概要>"
  ```
- 続けて `state.ts set ... codex_design passed` を実行して STEP 4 に進む
- 相関盲点破りが 1 サイクル欠けるが、STEP 7.5 の実装後 gate が保険として機能する

### 3.5-B: 判定 (`codex-design.yaml.verdict.json` を読む)

**`blocking == 0`** (critical/high なし) の場合:
```bash
bun .claude/skills/3ai/scripts/state.ts set \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json codex_design passed
```
medium/low の指摘があれば `features/$ISSUE_NUM-$ISSUE_SLUG/codex-design-findings.md` に記録のみ (非 block) → **STEP 4 へ**。

**`blocking >= 1`** (critical/high あり) の場合:

**#315 Phase B: Opus 4.7 委譲**（Codex 独立指摘の採用/棄却は相関盲点破りの最重要ポイント、Sonnet 5 で判断させると gate が形骸化する。Agent tool で Opus 4.7 subagent に委譲する）:

```
Agent(
  subagent_type: "general-purpose",
  model: "opus",
  description: "STEP 3.5-B: Codex 指摘採用/棄却",
  prompt: "Codex が別モデル系として plan.md をレビューし critical/high の指摘を出した。各指摘を採用/棄却/partial で判定し理由を記録..."
)
```

1. Opus 4.7 subagent が各 critical/high 指摘を **採用 / 棄却 / partial** に分類し理由を記録:
   - 採用 → plan.md を直接修正
   - 棄却 → `rejection.md` に `## STEP 3.5 Codex` セクションで追記
   - partial → 一部修正 + rejection.md に残件追記
2. `judgment-summary.md` にも `## STEP 3.5 Codex` セクションで採用/棄却の内訳を追記
3. **Codex 再呼びなし**。修正版のまま STEP 4 に進む (相関盲点は STEP 7.5 が保険)
4. state を passed に倒す:
   ```bash
   bun .claude/skills/3ai/scripts/state.ts set \
     features/$ISSUE_NUM-$ISSUE_SLUG/state.json codex_design passed
   ```
5. **例外: critical で ADR 判断が必要** (Phase 全体の設計方針を疑う必要) と subagent が判断した場合のみユーザーへエスカレーション

### 3.5-C: needs-human 判定 (Codex エラー / critical で判断保留)

- Codex 呼びは 1 発 gate。再 dispatch はしないため `pause-streak-tracker` の `codex-design-gate` カテゴリは inc しない。
- ADR 判断保留での needs-human は Task 手動、`pause-streak-tracker` 経由ではない。

---

## STEP 4: 確定プラン提出（唯一の承認点）

> **🚨 autonomous モード (`--issue N --autonomous` or `batch_arg === null`) では `ExitPlanMode` を絶対に呼ばない (#328)**。理由: そもそも STEP 0 で Plan mode に入っていないため exit 不要。加えて ExitPlanMode 呼び出し自体が Claude Code の承認 UI を必ず表示するため、autonomous mode の設計に反する。plan.md の要点を情報共有として stdout に出すのみ (待たない)、そのまま STEP 5 へ進む。
> 対話バッチモード（`batch_arg !== null`）および単一 Issue 対話モード (`--issue N` 単独) では Plan mode に入っている前提で下記の唯一の承認点として維持する。

**対話モード**: **`ExitPlanMode` を呼ぶ。これがフロー全体で唯一の承認点。**

GLM レビュー反映後の plan を提示しユーザーに承認を求める。

**承認後は STEP 5〜STEP 8 を自動進行する。** ユーザー介入が発生するのは失敗時エスカレーションのみ（STEP 6-D、STEP 6.5 期待値乖離、STEP 7 critical、STEP 7.5 critical）。

---

## STEP 5: 作業ブランチ作成

```bash
git checkout -b cad/$ISSUE_NUM-$ISSUE_SLUG
```

---

## STEP 5.5: Acceptance Test Skeleton 作成（Claude が書く）

**目的**: plan.md のテスト計画（T01〜）を元に `#[ignore]` 付きスケルトンを先行して置くことで、「テストなしで CI グリーン」という偽陽性を排除する。

**配置先**: `crates/<crate>/tests/<feature>_acceptance.rs`（integration test 限定）
> ⚠️ inline `#[cfg(test)] mod tests` への書き込みはパスベース guard では緩和できないため対象外。inline test の追加は STEP 6 で GLM 担当。

1. `features/$ISSUE_NUM-$ISSUE_SLUG/plan.md` のテスト計画 ID 表を読む

   **退化/境界ケース ID チェック**: テスト計画 ID 表に退化・境界ケース専用の ID が最低 1 件あるか確認する:
   ```bash
   grep -qiE '_degen_|_boundary_|_degenerate_|T_DEG' \
     features/$ISSUE_NUM-$ISSUE_SLUG/plan.md \
     || echo "⚠️  WARNING: 退化/境界ケース ID が plan.md のテスト計画表に見当たりません。最低 1 件追加してからスケルトンを生成してください。"
   ```
   警告が出た場合は plan.md のテスト計画 ID 表に退化/境界テスト ID（例: `T02_degen_zero_length_edge`）を追記してから次へ進む（強制ではないが省略禁止）。

2. 対象クレートの `crates/<crate>/tests/<feature>_acceptance.rs` を **Write** する（guard の `tests/` 緩和により Claude が直接書ける）:

```rust
#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t01_determinism() { todo!() }

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t02_build_manifold_euler() { todo!() }
// テスト計画の全 ID 分を列挙する（関数名は t<NN>_<内容> に合わせる）
```

3. `cargo test --workspace 2>&1 | head -20` でスケルトンがコンパイルエラーなく通ることを確認（ignored は OK）

4. **バグ修正 Issue の「再現ファースト」確認**（Issue に `bug` ラベルがある場合のみ、feature/chore は不要）:

   バグ再現テスト（テスト計画の T01 または「repro/regression/現象確認」に相当する関数）について:
   - **`todo!()` のままにしない**: バグの現象を直接アサートするテストボディを Claude が記述する。  
     例: naked edge バグなら `assert_eq!(count_naked_edges(&mesh), 0)` など、バグが直ると通過し直らないと失敗する assertion を書く。
   - **`#[ignore]` を外して `cargo test -p <crate> <test_fn_name>` を実行し、テストが `FAILED` になることを確認する**（= バグが再現できた）。
     - `FAILED` になる → バグを正しく捕捉している。`#[ignore = "STEP 6 で修正後に解除"]` を付け直して次へ進む。
     - `FAILED` にならない（通過する / `todo!()` のまま panic する）→ テストがバグを捕捉できていない。テストボディを修正してから再確認する。
   - この確認なしに `#[ignore]` を付けたままにしてはいけない（GLM が「テストと修正を同時に書いてどちらも通す」という偽陽性を防ぐため）。

6. **examples/ smoke テストチェック**: plan.md または Issue の変更対象を確認し、`examples/*.engawa` を新規追加・変更する場合は `crates/engawa-build/tests/examples_smoke.rs` にも対応エントリを追加する:
   - 新規追加: 新しい関数を追加し `smoke(include_str!("../../../examples/<file>.engawa"))` を呼ぶ
   - 変更のみ（既存 example の修正）: 既存テストがあれば追加不要
   - このステップで追加したテストを `cargo test -p engawa-build --test examples_smoke` で確認する
   - 現時点で build が通らないことが既知の場合は `#[ignore = "known bug: #<N>"]` を付ける

7. **#313 Phase A: regression test 削除検知** — 過去バグの再発防止テスト (`crates/**/tests/regression_*.rs`) を silently 削除していないか差分検査:
   ```bash
   bun .claude/skills/3ai/scripts/lint-regression-preserved.ts
   RC=$?
   ```
   - `RC=0`: 削除なし、次へ
   - `RC=1`: **blocking**。削除された regression test を復元するか、意図的削除なら plan.md の該当セクションに「Regression test 削除: <理由>」を明記してから再実行
   - `RC=2`: 環境エラー (git repo でない / base ref 不明)、ユーザーへエスカレ

8. `bun .claude/skills/3ai/scripts/state.ts set features/$ISSUE_NUM-$ISSUE_SLUG/state.json acceptance_skeleton passed`

---

## STEP 6: GLM-5.1 コア実装（背景実行・自動エスカレーション付き）

**目的**: コア機能の実装 + plan T01〜のうち決定性・正常系の最小テスト。  
**ループ定数**: `GLM_MAX_LOOPS=3`（通常試行上限）、`ESC_MAX_LOOPS=3`（debug-spec 付き再 dispatch 上限）、`ESCALATION_TOKEN_CAP=200_000`（per-Issue 累積 token 上限。`escalate-glm-adversarial.ts` の tracker が `features/.loop/glm-escalation/<issue>.json` で永続化する。1 round あたり 90k 推定なので約 2 round で限界）

**ゲート:**
```bash
bun .claude/skills/3ai/scripts/state.ts assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json acceptance_skeleton
```

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

# TS drift mitigation (#248): GLM が新規 Rust 型を追加した場合、web/src/generated/ の
# dirty を intermediate commit して `cargo xtask ci` の drift check を通す。
# clean なら no-op。STEP 8 の squash で 1 commit に集約される。
bun .claude/skills/3ai/scripts/maybe-commit-generated-ts.ts --issue $ISSUE_NUM
```

**`run_in_background: true` で起動し、完了通知を待つ（ポーリングしない）。**

**GLM 実装条件**: `tests/<feature>_acceptance.rs` のスケルトンをコンパイルエラーなく維持しつつ、実装完了したテスト関数から `#[ignore]` を外すこと。

### 6-B: 結果判定と早期エスカレーション判定

`glm-result.json` を読んで:

- `status: success` かつ `ci_passed: true` → `state.ts set ... core_impl passed` して **STEP 6.5 へ**
- `status: failed` の場合:
  - 今回と前回の `error_pattern` を比較
  - **2 連続同一 error_pattern** または **通常試行が GLM_MAX_LOOPS 回** → **6-C へ**
  - それ以外（新しいエラー）→ 6-A に戻って次の試行

### 6-C: Claude デバッグアシスト（debug-spec 作成・追記）

**#315 Phase B: Opus 4.7 委譲**（CI ログ + crates/** の根本原因分析は Sonnet 5 では推論力が足りない場面が多い。Agent tool で Opus 4.7 subagent に委譲する）:

```
Agent(
  subagent_type: "general-purpose",
  model: "opus",
  description: "STEP 6-C: debug-spec 追記",
  prompt: "以下の ci.log と関連コードを読み、失敗の根本原因を分析して debug-spec.md に仮説/修正方針を追記..."
)
```

subagent は `features/$ISSUE_NUM-$ISSUE_SLUG/ci.log` と `crates/**` の関連ファイルを **Read** して根本原因を分析する。  
`features/$ISSUE_NUM-$ISSUE_SLUG/debug-spec.md` を **追記** する（2 回目以降は既存内容を保持して `試した修正と結果` のチェックをつける）。セクション: **仮説 / 関連ファイル / 修正方針 / 試した修正と結果 / 次にやること / 追加で書いてほしいテスト**

**自律モード（`batch_arg === null`）では Opus 4.7 subagent が自力で debug-spec を追記してよい**。`ESC_MAX_LOOPS=3` までの再 dispatch ループ内なら都度追記が前提（過去の保守解釈「ESC 越権で書けない」は誤り — #225 で明文化）。

6-A に戻り `--debug-spec` 付きで dispatch（ESC_MAX_LOOPS=3 のため最大 3 回まで）。

### 6-D: ESC_MAX_LOOPS 到達時の adversarial review

ESC 試行 3 回でも `status: failed` のまま → **GLM 3 ペルソナ** adversarial review にエスカレートする (Codex 3 persona から移行、Codex 削減改修 Task 3):

```bash
bun .claude/skills/3ai/scripts/escalate-glm-adversarial.ts \
  --issue $ISSUE_NUM \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG
RC=$?
```

ペルソナ: **architect** (`glm-adversarial-architect.md` / 既存 invariant / API 契約で refute) / **contrarian** (`glm-adversarial-contrarian.md` / 採用方針を refute、代替案の優位性を提示) / **migration** (`glm-adversarial-migration.md` / 既存テスト互換で refute)。refute デフォルト。GLM (Z.AI 経由 claude -p) を 3 persona 並列で spawn。

**なぜ Codex から GLM に移行したか**: STEP 7.5-B の Codex 3 persona と名前空間が衝突していた (両者とも architect/contrarian/migration)。設計思想 (skill description) は「Codex = 独立技術レビューの 1 箇所」であり、実装フェーズ中間の adversarial review は同系モデル (GLM) で十分。7.5 の独立ゲート 1 発と役割を明確に分離する。

判定 (exit code + stdout JSON):
- `RC=0` (kind=continue, 全 approved): Claude が debug-spec.md にさらに新しい仮説を追記し、6-A に戻って **追加** dispatch する（ESC counter は継続、`ESCALATION_TOKEN_CAP=200_000` 越えで自動退避）
- `RC=3` (kind=needs_human, reason=refute): 1 ペルソナでも refute → escalate スクリプトが Issue に `needs-human` 自動付与 + `raise-issue-on-failure.ts` を呼んで起票 → loop は他 Issue に進む
- `RC=3` (kind=needs_human, reason=token_cap): per-Issue 累積 token 200k 越え → 同じく `needs-human` 自動退避

`debug-spec.md` はユーザーが手動改稿して再投入できる。Anthropic claude への自動フォールバックは禁止 (Z.AI 経由の GLM は禁止対象外)。

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

3.5. **類似ケース追加チェック（バグ修正 Issue のみ）**:
   修正したバグと同じ根本原因を持ちうる構造的類似ケースを確認し、未カバーなら test-spec.md に追記する。
   1. 修正した関数・モジュールを `grep -r` で探し、同じコードパスを通る他の呼び出しパターンや入力の組み合わせを列挙する
   2. 各パターンに対応するテストが既存コードに存在するか `cargo test --list 2>&1 | grep <keyword>` で確認する
   3. 未カバーの類似ケースを後述の test-spec.md に **`## 類似ケース（未カバー）`** セクションとして追記する  
      例: boolean_box_cut を修正したなら `boolean_box_void` / `boolean_intersect_box_cyl` / `boolean_fuse_box_cyl` 等で同じ問題が起きていないかを確認する
   4. 既存テストが `#[ignore]` であれば「修正で解除できる可能性がある」として test-spec.md に記録する

4. **期待値乖離チェック**: 以下を実行し、plan の T ID 期待値と実装の assertion 値が一致するか確認する:
   ```bash
   bun .claude/skills/3ai/scripts/check-spec-divergence.ts \
     --plan-file <プランファイルパス> \
     --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG
   ```
   出力を読み、各 T ID で plan の期待値と実装 assertion が一致するか Claude が判定する。
   - **乖離検出時**:
     - **#322 対話モード**: test-spec.md に「## 期待値乖離」セクションを追加 + **停止してユーザーへエスカレーション**（STEP 6.6 に進まない）
     - **#322 autonomous モード**: Opus 4.7 subagent が乖離を「(a) 実装 assertion が正・plan 期待値を修正 / (b) plan 期待値が正・実装を修正 / (c) 本質的乖離で needs-human 退避」の 3 択で判定:
       ```
       Agent(subagent_type: "general-purpose", model: "opus",
             description: "STEP 6.5 期待値乖離判定",
             prompt: "plan の T ID 期待値と実装 assertion の乖離を 3 択で判定...")
       ```
       subagent 返答が (a) → plan.md 修正 / (b) → 実装差戻し (STEP 6.x) / (c) → `raise-issue-on-failure.ts` で needs-human 起票
   - **乖離なし時**: 手順 5 へ進む
5. `features/$ISSUE_NUM-$ISSUE_SLUG/test-spec.md` を **Write** する（セクション: **不足テスト（plan 計画分） / 実装差分から追加すべきテスト / エッジケース・退化入力 / 数値境界 / 決定性**）

6. **#318 Phase C: Property test 存在確認** (Phase 11+ or Boolean/Tessellation Issue で blocking、他は warn):

```bash
bun .claude/skills/3ai/scripts/check-proptest-required.ts \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --phase $PHASE_NUM
RC=$?
```

- `RC=0`: OK (T_PROP_ ID あり or 非必須)、STEP 6.6 へ
- `RC=1`: **blocking** (Phase 11+ or Boolean-系で T_PROP_ 不足)。Claude が test-spec.md に `T_PROP_<invariant>` を最低 1 件追記してから再実行
- `RC=2`: 環境エラー (test-spec.md 未存在 = STEP 6.5 未実行)

Boolean / Tessellation を触る Issue は Phase 10 でも必須。Property test は Euler-Poincaré / manifold / 決定性の不変量検証に必須。

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

## STEP 6.6.5: 3 並列 dispatch (#317 Phase D-2)

STEP 6.6 完了後、**STEP 6.7 / 7 / 7.5 は独立した 3 種類の final review** なので **3 並列 dispatch → barrier で合流** する。逐次実行に比べ wall-clock を短縮 (1 Issue あたり ~5-10 分削減見込み)。

### 並列 dispatch パターン

```bash
# 3 種類の final review を並列起動 (Sonnet 5 が orchestrator)

# 6.7: Opus 4.7 subagent (Agent tool)
Agent(
  subagent_type: "general-purpose",
  model: "opus",
  description: "STEP 6.7: 3 観点 self-review",
  prompt: "..."
) &   # background

# 7: GLM final review dispatch
bun .claude/skills/3ai/scripts/dispatch-glm-review.ts \
  --persona final --issue $ISSUE_NUM \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/final-review.yaml \
  --test-summary features/$ISSUE_NUM-$ISSUE_SLUG/test-summary.json &

# 7.5: Codex final gate dispatch
bun .claude/skills/3ai/scripts/dispatch-codex.ts \
  --mode final-review --issue $ISSUE_NUM \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/codex-final.yaml &

wait   # barrier: 全 3 完了待ち
```

(Agent tool は shell の `&` ではなく `run_in_background: true` で発行、GLM/Codex dispatch は shell `&`)

### barrier 合流時の結果集約

3 者完了後、以下の優先順で判定:

1. **STEP 6.7 critical/high 検出** → 実装ループ (STEP 6.x) に戻す。**STEP 7 と 7.5 の結果は破棄**する (実装が変わるので再 review が必要)
2. **STEP 7 critical/high 検出** → GLM 修正 dispatch → GLM final 再レビュー (ループ +1、上限 2)。STEP 7.5 結果は保持 (次 loop で使う)
3. **STEP 7.5 critical/high 検出** → Codex は 1 発 gate なので再 dispatch なし。Claude (Opus 4.7 委譲) が採用/棄却判定
4. **全て pass** → STEP 8 (merge pane へ enqueue) へ

### Test-summary の準備

STEP 7 と 7.5 は `test-summary.json` を入力とするため、並列 dispatch 前に生成しておく:

```bash
bun .claude/skills/3ai/scripts/extract-test-summary.ts \
  --ci-log features/$ISSUE_NUM-$ISSUE_SLUG/ci.log \
  --output features/$ISSUE_NUM-$ISSUE_SLUG/test-summary.json
```

### 逐次実行 (fallback)

3 並列を試して問題が出たら、fallback として逐次実行に戻せるように STEP 6.7 / 7 / 7.5 の従来記述は残す。並列化は最適化であって semantics は逐次と同じ。

---

## STEP 6.7: Claude self-review (#280 — Codex 往復削減 shift-left)

**並列 dispatch (#317 Phase D-2)**: 本 STEP は STEP 7 / 7.5 と 3 並列で起動される。barrier 合流時に critical/high が検出されたら STEP 6.x へ戻し、7 / 7.5 の結果は破棄する。

**ゲート:** `features/$ISSUE_NUM-$ISSUE_SLUG/glm-self-review.md` が存在すること
(STEP 6 の GLM core 実装が完了直前に出力)。

Codex 7.5 で走る 3 観点 (architect / contrarian / migration) と同じ観点を
**Codex を呼ぶ前に Claude が自己適用**する。7.5 は 1 persona × ループなしの 1 発 gate
なので、6.7 で 3 観点をシフトレフト完了させることが 7.5 通過率を上げる鍵となる。

**#315 Phase B: Opus 4.7 委譲**（Sonnet 5 で self-review すると Codex 7.5 finding 率が上がりコスト増。judgment 力の高い Opus 4.7 で shift-left を効かせる）:

```
Agent(
  subagent_type: "general-purpose",
  model: "opus",
  description: "STEP 6.7: 3 観点 self-review",
  prompt: "git diff main..HEAD の実装差分と glm-self-review.md を読み、architect/contrarian/migration の 3 観点で GLM が見落とした弱点を探して claude-self-review.md に記録..."
)
```

subagent が実行する手順:

1. `git diff main..HEAD` で実装差分を読む (CI green の最終状態)
2. `glm-self-review.md` を読み、GLM が認めた弱点を把握
3. 以下の 3 観点で **GLM が見落としている弱点**を探す:
   - **architect**: 既存 invariant / API 契約 / B-rep トポロジー保証を破る変更
   - **contrarian**: 採用方針の反論可能性 / 直前 Issue や同 Phase の defensive
     semantics 退化
   - **migration**: 既存テスト互換 / 後方互換性 / public API 破壊
4. 結果を `features/$ISSUE_NUM-$ISSUE_SLUG/claude-self-review.md` に Write
   (フォーマットは glm-self-review.md と同じ 3 観点セクション)
5. 重要度判定:
   - **critical/high** が 1 件以上 → STEP 6.x に戻して GLM 再実装 dispatch
     (debug-spec として claude-self-review.md を渡す)
   - **medium のみ** → claude-self-review.md に記録、STEP 7 へ進む
   - **弱点なし** → claude-self-review.md に "## 結論\n弱点検出なし" と書いて
     STEP 7 へ進む

「弱点なし」を頻発するなら critical thinking 不足。最低 1 件は仮説を出して
GLM 実装が反証できるか考えること。Codex 7.5 finding 率を下げるのが目的。

---

## STEP 7: GLM 最終レビュー（背景実行・完了通知）

**並列 dispatch (#317 Phase D-2)**: 本 STEP は STEP 6.7 / 7.5 と 3 並列で起動される。barrier 合流時、7 だけで critical/high 検出時は GLM 修正 dispatch ループに入る。6.7 critical/high なら本 STEP の結果は破棄される。

**ゲート:**
```bash
bun .claude/skills/3ai/scripts/state.ts assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json glm_impl
```

**dispatch 前に test-summary.json を生成する** (並列起動前に必須):
```bash
bun .claude/skills/3ai/scripts/extract-test-summary.ts \
  --ci-log features/$ISSUE_NUM-$ISSUE_SLUG/ci.log \
  --output features/$ISSUE_NUM-$ISSUE_SLUG/test-summary.json
```

**GLM final レビュアーを起動** (`run_in_background: true`):
```bash
bun .claude/skills/3ai/scripts/dispatch-glm-review.ts \
  --persona final \
  --issue $ISSUE_NUM \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/final-review.yaml \
  --test-summary features/$ISSUE_NUM-$ISSUE_SLUG/test-summary.json
```

**完了通知を待つ（ポーリングしない）。**

**#318 Phase C: diff coverage 検査** (GLM final review と並行 or 直前で実行、Phase 11+ で blocking):

```bash
bun .claude/skills/3ai/scripts/check-diff-coverage.ts \
  --phase $PHASE_NUM \
  --threshold 70
RC=$?
```

- `RC=0`: OK (閾値以上 or Phase 10 以下の warn / cargo-llvm-cov 未 install の warn)
- `RC=1`: **blocking** (Phase 11+ かつ実装差分 line coverage < 70%)。テスト追加を要求

Phase 10 現在は cargo-llvm-cov 未導入のため常に warn + exit 0。Phase 11 の品質基盤導入後に blocking として機能する。

`final-review.yaml.verdict.json` を読んで `blocking` が 0 かつ `verdict: pass` なら:
```bash
bun .claude/skills/3ai/scripts/state.ts set features/$ISSUE_NUM-$ISSUE_SLUG/state.json final_review passed
```

**dispatch_error 分岐 (#262)**: verdict.json が `dispatch_error: true` または `verdict: "error"` の場合は GLM がレビュー出力を生成できなかった (max-turns 到達 / claude CLI 非ゼロ終了 / verdict 行不在) ことを意味する。**fix-dispatch ループに入らない** (再 dispatch しても同様に失敗するため):

```bash
bun .claude/skills/3ai/scripts/raise-issue-on-failure.ts \
  --step "STEP 7 GLM final review dispatch_error" \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --error-summary "GLM final review が dispatch_error で終了 (reason: <verdict.json の reason>)"

# STEP 7.5 gate (final_review assert) を通すため、Claude 裁量で final_review を passed に倒す。
# Codex 独立 gate (STEP 7.5) が真の最終判定を担う前提。
bun .claude/skills/3ai/scripts/state.ts set \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json final_review passed
```

起票・state 更新後は **STEP 7.5 (Codex 独立技術ゲート) に進める**。STEP 7.5 で blocking=0 なら次へ、critical/high が出たらそちらの通常エスカレーション経路に従う。`needs-human` 退避は行わない (STEP 7.5 が独立 review 軸を担うため)。

Critical/High があれば (= `blocking >= 1` かつ `verdict != "error"`) GLM 修正 dispatch → GLM final 再レビュー（ループ +1、上限 2）。

**#315 Phase B: Opus 4.7 委譲**（critical エスカレ判定は「ユーザーに投げるか / Claude 裁量で棄却するか」の重要判断。Sonnet 5 では甘くなりがちなので Agent tool で Opus 4.7 subagent に委譲する）:

```
Agent(
  subagent_type: "general-purpose",
  model: "opus",
  description: "STEP 7: critical エスカレ判定",
  prompt: "以下の GLM final review critical/high findings と Codex final review (7.5) の結果を読み、ユーザーエスカレするか裁量棄却するかを判定..."
)
```

`raise-issue-on-failure.ts` を呼ぶ or `state.ts set ... final_review passed` に倒すかは subagent の判定に従う。

**ループ上限超過フォールバック** (`final_loops` が 2 を超えた場合):
```bash
bun .claude/skills/3ai/scripts/state.ts assert-critical-zero \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  features/$ISSUE_NUM-$ISSUE_SLUG/final-review.yaml.verdict.json
```
- **critical ≥ 1** → 停止してユーザーにエスカレーション
- **critical = 0**、blocking が **docs-only**（コードファイル変更を伴わない）→ Claude 裁量で受け切る: 残 high/medium を直接修正（docs への Edit/Write）または棄却 → `cargo xtask ci` green 確認 → `state.ts set ... final_review passed` → 内訳報告して STEP 8 へ
- **critical = 0**、blocking に **code 系**が含まれる → 停止してユーザーにエスカレーション

---

## STEP 7.5: Codex 独立技術最終ゲート

**目的**: 実装者 GLM とレビュアー GLM が同系であることによる相関盲点を、別モデル系 (Codex/gpt-5.4) の独立視点で破る。diff 全体を技術的観点でレビューし、critical/high は merge 前にブロックする。

**並列 dispatch (#317 Phase D-2)**: 本 STEP は STEP 6.7 / 7 と 3 並列で起動される。barrier 合流時、7.5 だけで critical/high 検出時は Opus 4.7 subagent が採用/棄却判定 (Codex は 1 発 gate なので再 dispatch なし)。6.7 critical/high なら本 STEP の結果は破棄される。

**ゲート**: 3 並列 dispatch では `glm_impl` の gate assert のみ、`final_review` gate は barrier 合流後に判定する:
```bash
bun .claude/skills/3ai/scripts/state.ts assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json glm_impl
```

(逐次 fallback モードでは以下を使う:)
```bash
bun .claude/skills/3ai/scripts/state.ts assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json final_review
```

### 7.5-A: テストサマリ + Non-Goals を Codex 入力に整形

`build-codex-input.ts` で `codex-input.md` を生成する（TEST SUMMARY + Non-Goals + known ignored tests を注入）:

```bash
bun .claude/skills/3ai/scripts/build-codex-input.ts \
  --plan-file features/$ISSUE_NUM-$ISSUE_SLUG/plan.md \
  --test-summary features/$ISSUE_NUM-$ISSUE_SLUG/test-summary.json \
  --ci-log features/$ISSUE_NUM-$ISSUE_SLUG/ci.log \
  --output features/$ISSUE_NUM-$ISSUE_SLUG/codex-input.md
```

### 7.5-B: Codex 技術レビュー dispatch (1 persona × ループなし)

```bash
bun .claude/skills/3ai/scripts/dispatch-codex.ts \
  --mode review \
  --instruction .claude/skills/3ai/agents/codex-final-reviewer.md \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/codex-final.yaml \
  --extra-input features/$ISSUE_NUM-$ISSUE_SLUG/codex-input.md
```

**単一 persona 1 呼び**。従来の 3 persona 並列 (architect/contrarian/migration) は、`codex-final-reviewer.md` の「3 観点統合チェックリスト」セクションで **1 persona 内でカバー** させる。3 観点は Claude self-review (STEP 6.7) で既にシフトレフト済みなので、Codex は 1 発 gate で十分。`base` は `origin/HEAD` から自動検出し `git diff <base>...HEAD` を渡す。

**Codex CLI エラー時のフォールバック** (usage limit / rate limit / exit != 0 等):
- 1 発 gate なのでリトライしない
- **スキップを後払い台帳に記録** (Codex 回復後に L-5.9 collector がまとめて後追いレビュー):
  ```bash
  bun .claude/skills/3ai/scripts/record-codex-skip.ts \
    --issue $ISSUE_NUM --slug $ISSUE_SLUG --step 7.5 --reason "<usage-limit / exit!=0 の概要>"
  ```
- 続けて `state.ts set ... codex_review passed` を実行して STEP 8 に進む
- 相関盲点破りが 1 サイクル欠けるが、次サイクルの回帰テストで担保 (STEP 3.5 で既に 1 回 gate 済み)

### 7.5-C: 判定（`codex-final.yaml.verdict.json` を読む）

**`blocking == 0`**（critical/high なし）の場合:
```bash
bun .claude/skills/3ai/scripts/state.ts set \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json codex_review passed
```
medium/low の指摘があれば `features/$ISSUE_NUM-$ISSUE_SLUG/codex-findings.md` に記録のみ（非 block）→ **STEP 8 へ**。

**`blocking >= 1`**（critical/high あり）の場合 (**Codex 再呼びなし**、1 発 gate ポリシー):
1. `codex-final.yaml` の critical/high 指摘を `features/$ISSUE_NUM-$ISSUE_SLUG/debug-spec.md` に転記
2. GLM 実装へ再 dispatch (指摘内容がコア実装なら `--mode core`、テスト関連なら `--mode test`)
3. `cargo xtask ci` green を確認
4. **Codex 再レビューは行わない**。修正が指摘 id に対応しているかは Claude が `codex-final.yaml` の issue id と修正 diff を突き合わせて確認する
5. `state.ts set ... codex_review passed` → **STEP 8 へ**

**修正しきれない (Claude が判断保留する必要がある) critical 指摘のみ**、停止してユーザーへエスカレーション。この場合は `raise-issue-on-failure.ts` で起票してから報告する。

### 7.5-D: (削除)

従来の `codex_loops` カウンタ + 5 loops フォールバック分岐は、1 発 gate 化により廃止。ループ制御が消えたため `state.ts inc ... codex_loops` の呼び出しも不要。

---

## STEP 8: 確定・squash マージ

**ゲート:**
```bash
bun .claude/skills/3ai/scripts/state.ts assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json codex_review
```

```bash
# TS drift mitigation (#248): generated TS が dirty なら intermediate commit
bun .claude/skills/3ai/scripts/maybe-commit-generated-ts.ts --issue $ISSUE_NUM
# crates/ の unstaged/untracked ファイルを検出（git add 漏れ防止）
bun .claude/skills/3ai/scripts/pre-step8-check.ts \
  --auto-raise --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG

# #318 Phase C: fuzz corpus 更新確認 (YAML schema 変更時のみ効く、fuzz/ 未セットアップなら warn)
bun .claude/skills/3ai/scripts/lint-fuzz-corpus-updated.ts
FUZZ_RC=$?
# FUZZ_RC=1 (blocking): YAML schema に新 enum variant/field を追加したのに fuzz/corpus/parser/ に seed が無い
# → Claude が Feature の最小 example を fuzz corpus に追加してから再実行
# FUZZ_RC=0: OK (schema 未変更 or seed 追加済み or fuzz/ 未セットアップ warn)

cargo xtask ci   # 最終 green 確認
```

### STEP 8 分岐: loop-mode vs 単体 mode (#321 Phase D-1 補完)

worktree 内 (`$PWD` が `/home/bacon/worktrees/wN`) かつ worker-registry にエントリがあれば **loop-mode**、それ以外は **単体 mode** (従来直接 push)。

```bash
LOOP_WORKTREE_BASE=${LOOP_WORKTREE_BASE:-/home/bacon/worktrees}
WORKER_ID=""
if [[ "$PWD" == "$LOOP_WORKTREE_BASE"/* ]] && [ -f features/.loop/worker-registry.json ]; then
  # worktree path から worker-id を registry で逆引き
  WORKER_ID=$(bun -e '
    const r = JSON.parse(require("fs").readFileSync("features/.loop/worker-registry.json","utf-8"));
    const pwd = process.env.PWD;
    for (const [id, w] of Object.entries(r.workers)) {
      if (w.worktree === pwd) { console.log(id); process.exit(0); }
    }
    process.exit(1);
  ' 2>/dev/null || true)
fi
```

**loop-mode (worker-id 検出時)** — merge pane 経由で serial 化 (直接 push しない):

```bash
if [ -n "$WORKER_ID" ]; then
  # 1. worktree branch にコミット (現在 branch にそのまま)
  git add -A
  git commit -m "feat: <内容の一行要約>

<詳細（任意）>

Closes #$ISSUE_NUM

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
  COMMIT_SHA=$(git rev-parse HEAD)

  # 2. worker-registry を merging に遷移
  bun .claude/skills/3ailoop/scripts/loop-worker-registry.ts mark-merging \
    --worker-id "$WORKER_ID"

  # 3. merge-queue に enqueue (merge pane が拾って rebase → CI → push → close → release)
  bun .claude/skills/3ailoop/scripts/loop-tmux-merge-dispatcher.ts --enqueue \
    --issue "$ISSUE_NUM" \
    --worker-id "$WORKER_ID" \
    --worktree "$PWD" \
    --commit-sha "$COMMIT_SHA"

  # 4. state を merge_enqueued に倒す (finalize は merge pane 側)
  bun .claude/skills/3ai/scripts/state.ts set \
    features/$ISSUE_NUM-$ISSUE_SLUG/state.json merge enqueued
  bun .claude/skills/3ai/scripts/finalize-feature.ts \
    --issue $ISSUE_NUM --slug $ISSUE_SLUG
  # worker Claude session は STEP 8 完了で idle 待機に戻る (watcher が次 Issue を fan-out)
fi
```

**単体 mode (worker-id なし = 通常 /3ai --issue N 起動 or ローカル開発)** — 従来通り直接 push:

```bash
if [ -z "$WORKER_ID" ]; then
  git checkout main
  git merge --squash cad/$ISSUE_NUM-$ISSUE_SLUG
  git commit -m "feat: <内容の一行要約>

<詳細（任意）>

Closes #$ISSUE_NUM

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
  git push origin main
  git branch -d cad/$ISSUE_NUM-$ISSUE_SLUG
  bun .claude/skills/3ai/scripts/state.ts set features/$ISSUE_NUM-$ISSUE_SLUG/state.json merge passed
  bun .claude/skills/3ai/scripts/finalize-feature.ts --issue $ISSUE_NUM --slug $ISSUE_SLUG
fi
```

> `finalize-feature.ts` は `features/$ISSUE_NUM-$ISSUE_SLUG/` を git に追加してコミットする。
> 既に追跡済み・差分なしの場合は何もしない（冪等）。
> 取りこぼし一括回収: `bun .claude/skills/3ai/scripts/finalize-feature.ts --sweep [--dry-run]`

---

## 禁止事項（常に守ること）

- `crates/**` を自分（Claude）が直接 Edit/Write **しない** — guard-crates フックが deny する（Read は可）
- `claude -p` ワーカーを spawn **しない**（課金制約: Z.AI/Codex は可、Anthropic claude は不可）
- GLM が詰まっても Anthropic claude へ自動フォールバック**しない**
- dispatch 完了をポーリング**しない** — 背景実行 + 完了通知で受け取る
- git commit/push は STEP 8 以外で行わない（`finalize-feature.ts` の commit は STEP 8 の一部として許可）
  - **例外**: `web/src/generated/` の auto-generated TS は STEP 6 以降 `maybe-commit-generated-ts.ts` 経由の intermediate commit を許容 (#248)。STEP 8 squash で最終 1 commit に集約される。
- `features/$ISSUE/` の手動 `git add` は行わない — 必ず `finalize-feature.ts` 経由にする

---

## エラー検知・自動 Issue 起票（常に守ること）

**フロー中にエラー・トラブルが発生したら `raise-issue-on-failure.ts` で GitHub Issue を即時起票する。**  
エスカレーションしてユーザーに報告する際は、必ず起票してから報告する。

```bash
bun .claude/skills/3ai/scripts/raise-issue-on-failure.ts \
  --step "STEP X-Y <内容>" \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --error-summary "<エラーの概要（1〜3行）>" \
  [--result-file features/$ISSUE_NUM-$ISSUE_SLUG/glm-result.json]
```

**対処方針（優先順位）:**
1. **修正できるなら**: Issue 起票 → その場で修正 → フロー続行（方向議論は後回しでよい）
2. **修正方針が不明なら**: Issue 起票 → ユーザーへエスカレーション

**起票タイミング:**
- STEP 6-D: ESC_MAX_LOOPS=3 到達 → Codex adversarial で refute、または per-Issue token cap 200k 越え → `escalate-glm-adversarial.ts` が起票 + `needs-human` 自動付与
- STEP 6.5: 期待値乖離検出 → 起票してユーザーへ
- STEP 7 ループ上限超過 (critical ≥ 1) → 起票して修正またはユーザーへ
- STEP 7.5 ループ上限超過 → 起票して修正またはユーザーへ
- B-3 ambiguous が解決しない場合 → 起票してからバッチから除外

**補助チェックスクリプト（各 STEP で活用）:**
- `check-dispatch-result.ts --result <json> --auto-raise --feature-dir <dir> --step <name>` — dispatch 結果の status/ci_passed を確認（STEP 6-B, 6.6, 7 後）。`--auto-raise` を付けるとエラー時に自動起票する
- `pre-step8-check.ts [--feature-dir <dir>]` — STEP 8 直前に crates/ の unstaged/untracked を検出。exit 1 で停止のみ（自動起票しない。繰り返し発生する場合は手動で `raise-issue-on-failure.ts` を呼ぶこと）
- `resolve-issues.ts --feature-dir <dir> [--dry-run]` — state.json の raised_issues[] を読み、対応 step が passed なら Issue を close する（B-7 の手順 2 で使用）
- `resolve-issues.ts --sweep [--dry-run]` — 全 feature を走査して解消済み auto-raise Issue を一括 close する
- `build-codex-input.ts --plan-file ... --test-summary ... --output ...` — STEP 7.5-A で使用（Non-Goals を自動注入）
- `lint-test-semantics.ts` — STEP 6.6 後に BooleanOp 命名不整合をチェック
- `lint-issue-labels.ts --labels "<csv>" | --issue <N>` — Issue 起票直前/直後にラベル組み合わせを検証 (type 軸 + batch:* 必須)。**プランモードで `gh issue create` する場合は必ず起票直後に `--issue <N>` で検証して exit 0 を確認すること** (CLAUDE.md / ADR-006 §1)
