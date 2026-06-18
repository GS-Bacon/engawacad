---
name: 3ai
description: Claude × Codex × GLM マルチエージェント CAD 開発フロー。GitHub Issue を起点に設計(Claude)→設計レビュー(GLM多ペルソナ)→実装+テスト(GLM)→最終レビュー(GLM)→独立技術レビュー(Codex)を一周する。
tools: Read, Write, Edit, Bash, Glob, Grep
---

# /3ai — マルチエージェント CAD 開発フロー

このスキルが呼ばれたら以下の手順を**必ず順番通りに**実行すること。

---

## 引数解釈（最初に判定）

- `/3ai --issue N` → **単一 Issue モード**。STEP 0 へ直行（既存フロー、変更なし）。  
- `/3ai`（引数なし）→ **自律バッチモード**（承認待ちなし・無人進行）。STEP B へ。`plan.json.batch_arg === null` がスイッチ。  
- `/3ai --batch fixes` → **対話バッチモード**（bug+batch:* に絞る、従来どおりユーザー確認あり）。STEP B へ。  
- `/3ai --batch phase` → **対話バッチモード**（現 Phase milestone の type:feature に絞る、従来どおりユーザー確認あり）。STEP B へ。  

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
- **`intent_check_required: true`** → Codex intent-check を実行:
  ```bash
  bun .claude/skills/3ai/scripts/dispatch-codex-intent.ts \
    --issue $N \
    --result features/.batch/intent-$N.yaml
  ```
  `aligned: yes` → 続行。`aligned: no` → Claude が ROADMAP/ADR でスコープ整合を再確認し、推奨スコープに調整して続行。ただし *Phase/スコープ自体の根本的不整合*（例: 別 Phase 向け Issue など）と判断したら推奨実装せず停止してユーザーにエスカレーション。`aligned: skip (split-detector parent)` → **yes と等価扱い** (#235: body に `loop-split-detector で分割される想定` か label `splittable` のある起点 Issue は intent-check を skip し、STEP 3 で `split_proposal` 経路に乗せる)。

**対話モード（`batch_arg !== null`）**:

- **`needs-review` ラベルあり / ambiguous** → ユーザーに要件を確認。解決したら続行、解決しなければバッチから除外する
- **`intent_check_required: true`** → Codex intent-check を実行し `aligned: no` → ユーザーと相談し、スコープ修正またはバッチから除外する

**B-3 完了後の残 Issue は無人で自動進行する。**

### B-4: グループ・Issue ループ（auto 実行）

`plan.json` の `groups[].order` 順、各グループ内は `deps` 依存順に Issue を処理する:

1. `main` ブランチ上でクリーンな状態を確認する
2. `bun .claude/skills/3ai/scripts/init-feature.ts --issue $N --slug $SLUG` を実行する  
   （`features/$N-$SLUG/` が既存の場合はスキップ → 完了済みとして continue）
3. `flow: full` の Issue → 既存 STEP 1（ディレクトリ作成済み、Issue 確定のみ）→ STEP 2 → 2.5 → STEP 3（GLM 設計レビュー収束）まで実行した後:
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
6. **STEP 7.5**: Codex 独立技術ゲート  
   - `keep_codex_gate: true`（= batch:kernel）→ **STEP 7.5 を保持する**（幾何不変量リスクが高いため）  
   - `keep_codex_gate: false`（= それ以外の light）→ **STEP 7.5 をスキップし B-6 横断レビューに集約する**  
   - **state shim**: 7.5 をスキップする Issue は STEP 7 末で以下を実行し STEP 8 の assert ゲートを通す:  
     ```bash
     bun .claude/skills/3ai/scripts/state.ts set \
       features/$N-$SLUG/state.json codex_review passed
     ```
7. **STEP 8**: squash マージ + `finalize-feature.ts`（逐次実行のため単一ツリーで安全）

### B-6: 横断 Codex レビュー（全グループ完了後）

`batch_start_sha`（plan.json に記録）を base に全バッチコミットを一括レビューする:

```bash
bun .claude/skills/3ai/scripts/dispatch-codex.ts \
  --mode review \
  --base <batch_start_sha> \
  --instruction .claude/skills/3ai/agents/codex-final-reviewer.md \
  --result features/.batch/codex-crosscut.yaml
```

- **自律モード（`batch_arg === null`）の場合**: `blocking ≥ 1` でも Claude が自己判断で修正して続行する:
  1. 各 critical/high 指摘を「採用」「棄却」「partial」に分類し理由を記録する
  2. 採用した指摘: F01→F02 dispatch（crates/src/ は GLM、その他は Claude が直接修正）
  3. 修正後 `cargo xtask ci` green を確認し、Codex 再レビューを実施する（`--result features/.batch/codex-crosscut-r2.yaml`）
  4. 再レビューで `blocking == 0` になれば完了。再び `blocking ≥ 1` の場合は同ループを繰り返す（上限 2 回）
  5. 2 回ループ後も critical ≥ 1 が残る場合のみユーザーにエスカレーション; critical = 0 なら Claude 裁量で受け切る
- **対話モード（`batch_arg !== null`）**: `blocking ≥ 1` → ユーザーに報告（バッチ全体の自動ループはせずエスカレーション）
- medium/low のみ → `features/.batch/codex-crosscut-findings.md` に記録

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

**`EnterPlanMode` を呼ぶ。**

> プランモードでも **Bash dispatch は通る**（Edit/Write 等のファイル変更のみ deny）。  
> STEP 3 の GLM 設計レビューは ExitPlanMode 前に実行すること（後ろ倒し禁止）。

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

1. 全ペルソナの `review-*-rN.yaml` を読む
2. **重複除去**: 同一論点を複数ペルソナが指摘している場合、最も高 severity の 1 件に統合
3. 各 issue を Claude が判定:
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
exit 1 が返った場合（2 round 連続で全指摘を棄却）は **停止してユーザーにエスカレーション**:
> 2 round 連続で全 GLM 指摘を棄却しています。設計そのものに問題がある可能性があります。  
> rejection.md を提示します。設計を見直してから再開してください。

**全採用警告チェック**（次 round dispatch 前に必ず実行）:
```bash
bun .claude/skills/3ai/scripts/state.ts check-full-adoption-warning \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json
```
exit 1 が返った場合は停止してユーザーへ表示:
> 2 round 連続で棄却が 0 件です。scope 防衛できていますか?  
> Non-Goals に含まれる指摘や medium 以下で受容すべき指摘は棄却 log に記録してから次 round に進んでください。

### 3-E: 収束判定・再 dispatch

- **全 Critical/High が 0** かつ **2 round 連続で C/H = 0** → STEP 3-F へ
- **Critical/High が残る** → 3-C に戻って再 dispatch（N++）
- **design_loops が上限超過** → Critical の数を確認:
  - critical = 0: Claude 裁量で残 high/medium を「採用→修正」「棄却→rejection.md」で処理 → 3-F へ
  - critical ≥ 1: 停止してユーザーにエスカレーション

### 3-F: 通過

```bash
bun .claude/skills/3ai/scripts/state.ts set \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json design_review passed
```

---

## STEP 4: 確定プラン提出（唯一の承認点）

> **自律バッチモード（引数なし起動, `batch_arg === null`）では本 STEP の `ExitPlanMode` をスキップし、確定 plan の要点を情報共有として表示するのみ（待たない）。B-4 の手順に従ってそのまま STEP 5 へ進む。**  
> 対話バッチモード（`batch_arg !== null`）および単一 Issue モードでは下記のとおり唯一の承認点として維持する。

**`ExitPlanMode` を呼ぶ。これがフロー全体で唯一の承認点（対話/単一 Issue モード）。**

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
7. `bun .claude/skills/3ai/scripts/state.ts set features/$ISSUE_NUM-$ISSUE_SLUG/state.json acceptance_skeleton passed`

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

`features/$ISSUE_NUM-$ISSUE_SLUG/ci.log` と `crates/**` の関連ファイルを **Read** して根本原因を分析する。  
`features/$ISSUE_NUM-$ISSUE_SLUG/debug-spec.md` を **追記** する（2 回目以降は既存内容を保持して `試した修正と結果` のチェックをつける）。セクション: **仮説 / 関連ファイル / 修正方針 / 試した修正と結果 / 次にやること / 追加で書いてほしいテスト**

**自律モード（`batch_arg === null`）では Claude が自力で debug-spec を追記してよい**。`ESC_MAX_LOOPS=3` までの再 dispatch ループ内なら都度追記が前提（過去の保守解釈「ESC 越権で書けない」は誤り — #225 で明文化）。

6-A に戻り `--debug-spec` 付きで dispatch（ESC_MAX_LOOPS=3 のため最大 3 回まで）。

### 6-D: ESC_MAX_LOOPS 到達時の adversarial review

ESC 試行 3 回でも `status: failed` のまま → Codex 3 ペルソナ adversarial review にエスカレートする:

```bash
bun .claude/skills/3ai/scripts/escalate-glm-adversarial.ts \
  --issue $ISSUE_NUM \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG
RC=$?
```

ペルソナ: **architect** (既存 invariant との整合で refute) / **contrarian** (実装案を refute、棄却案の利点を強調) / **migration** (テスト互換性で refute)。refute デフォルト。

判定 (exit code + stdout JSON):
- `RC=0` (kind=continue, 全 approved): Claude が debug-spec.md にさらに新しい仮説を追記し、6-A に戻って **追加** dispatch する（ESC counter は継続、`ESCALATION_TOKEN_CAP=200_000` 越えで自動退避）
- `RC=3` (kind=needs_human, reason=refute): 1 ペルソナでも refute → escalate スクリプトが Issue に `needs-human` 自動付与 + `raise-issue-on-failure.ts` を呼んで起票 → loop は他 Issue に進む
- `RC=3` (kind=needs_human, reason=token_cap): per-Issue 累積 token 200k 越え → 同じく `needs-human` 自動退避

`debug-spec.md` はユーザーが手動改稿して再投入できる。Anthropic claude への自動フォールバックは禁止。

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
   - **乖離検出時**: test-spec.md に「## 期待値乖離」セクションを追加 + **停止してユーザーへエスカレーション**（STEP 6.6 に進まない）
   - **乖離なし時**: 手順 5 へ進む
5. `features/$ISSUE_NUM-$ISSUE_SLUG/test-spec.md` を **Write** する（セクション: **不足テスト（plan 計画分） / 実装差分から追加すべきテスト / エッジケース・退化入力 / 数値境界 / 決定性**）

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

## STEP 7: GLM 最終レビュー（背景実行・完了通知）

**ゲート:**
```bash
bun .claude/skills/3ai/scripts/state.ts assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json glm_impl
```

**dispatch 前に test-summary.json を生成する:**
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

`final-review.yaml.verdict.json` を読んで `blocking` が 0 かつ `verdict: pass` なら:
```bash
bun .claude/skills/3ai/scripts/state.ts set features/$ISSUE_NUM-$ISSUE_SLUG/state.json final_review passed
```

Critical/High があれば GLM 修正 dispatch → GLM final 再レビュー（ループ +1、上限 2）。

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

**ゲート:**
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

### 7.5-B: Codex 技術レビュー dispatch (3 ペルソナ並列, #231)

```bash
bun .claude/skills/3ai/scripts/dispatch-codex-3persona.ts \
  --instruction .claude/skills/3ai/agents/codex-final-reviewer.md \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/codex-final.yaml \
  --extra-input features/$ISSUE_NUM-$ISSUE_SLUG/codex-input.md
```

3 ペルソナ (architect / contrarian / migration) を `Promise.all` で並列実行し、各 persona の指摘を統合した `codex-final.yaml` と `codex-final.yaml.verdict.json` を生成する。1 つでも persona が critical/high を返せば aggregate `blocking >= 1` (#231)。`base` は `origin/HEAD` から自動検出し、`git diff <base>...HEAD` を各 Codex 呼び出しに渡す。

per persona の生 yaml は `codex-final-architect.yaml` / `codex-final-contrarian.yaml` / `codex-final-migration.yaml` に書き出され、統合 yaml の issue id には persona プレフィックス (`A-` / `C-` / `M-`) が付与される。

### 7.5-C: 判定（`codex-final.yaml.verdict.json` を読む）

**`blocking == 0`**（critical/high なし）の場合:
```bash
bun .claude/skills/3ai/scripts/state.ts set \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json codex_review passed
```
medium/low の指摘があれば `features/$ISSUE_NUM-$ISSUE_SLUG/codex-findings.md` に記録のみ（非 block）→ **STEP 8 へ**。

**`blocking >= 1`**（critical/high あり）の場合:
```bash
bun .claude/skills/3ai/scripts/state.ts inc \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json codex_loops \
  --raise-at 6 \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --step "STEP 7.5 codex_review"
```
`codex-final.yaml` の critical/high 指摘を `features/$ISSUE_NUM-$ISSUE_SLUG/debug-spec.md` に転記し、GLM 実装へ再 dispatch（指摘内容がコア実装なら `--mode core`、テスト関連なら `--mode test`）→ `cargo xtask ci` green 確認 → **7.5-A に戻って Codex 再レビュー**（`codex_loops` 上限 5、#152 で上限緩和）。

**自律モード (`batch_arg === null`) の追加判定**: `codex_loops` が 3 以上で **3 round 連続 blocking ≥ 1** が続いた場合 (前 3 round の `codex-final-r*.yaml` をすべて確認して critical/high 残存)、**ユーザーに「続行 or scope 切り出し」を確認** する停止点を設ける。同系統の指摘が無限ループする兆候のため。

### 7.5-D: ループ上限超過フォールバック（`codex_loops > 5`、#152 で上限緩和）

```bash
bun .claude/skills/3ai/scripts/state.ts assert-critical-zero \
  features/$ISSUE_NUM-$ISSUE_SLUG/state.json \
  features/$ISSUE_NUM-$ISSUE_SLUG/codex-final.yaml.verdict.json
```

- **critical ≥ 1** → 停止してユーザーにエスカレーション。
- **critical = 0** かつ残 high が docs-only（コードファイル変更を伴わない）→ Claude 裁量で受け切る: 残 high/medium を直接修正（docs への Edit/Write）または棄却 → `cargo xtask ci` green 確認 → `state.ts set ... codex_review passed` → 内訳報告して STEP 8 へ。
- **critical = 0** だが code 系 high が残る → 停止してユーザーにエスカレーション。

---

## STEP 8: 確定・squash マージ

**ゲート:**
```bash
bun .claude/skills/3ai/scripts/state.ts assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json codex_review
```

```bash
# crates/ の unstaged/untracked ファイルを検出（git add 漏れ防止）
bun .claude/skills/3ai/scripts/pre-step8-check.ts \
  --auto-raise --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG
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
bun .claude/skills/3ai/scripts/finalize-feature.ts --issue $ISSUE_NUM --slug $ISSUE_SLUG
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
