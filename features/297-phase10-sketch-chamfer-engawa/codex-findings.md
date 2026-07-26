# Codex final gate (STEP 7.5) — findings と判定

verdict (Codex 原文): **fail** (blocking=1, critical 1件)
判定 (Claude STEP 7.5-C): **critical 1件を棄却 → codex_review = passed**

## M01 (critical) — 棄却 (rejected)

- file: `crates/engawa-kernel/src/geometry/mod.rs:4`
- finding (原文): 「この tracked diff だけでは `pub mod sketch_chamfer;` の参照先本体が存在せず、さらに
  `examples_smoke.rs` / `golden_examples.rs` が参照する `examples/sketch_chamfer.engawa` と acceptance test も
  差分に入っていない。clean checkout にこの patch を適用するとコンパイルとテストが成立しない」
- suggestion (原文): 「`crates/engawa-kernel/src/geometry/sketch_chamfer.rs`、
  `crates/engawa-build/tests/sketch_chamfer_acceptance.rs`、`examples/sketch_chamfer.engawa` を
  追跡対象に含めた状態で差分を作り直す」

### 棄却理由

**指摘は実装の欠陥ではなく `dispatch-codex.ts` の diff 計算方法に起因するツール側の制約 (偽陽性) であり、
STEP 8 の `finalize-feature.ts` 経由コミットで自動的に解消される。**

1. **`git diff` が untracked ファイルを含まない**
   `.claude/skills/3ai/scripts/dispatch-codex.ts:105` は Codex への入力を
   `git diff ${baseBranch}...HEAD` で生成する。この three-dot 形式は **コミット済み差分のみ**を出力するため、
   untracked ファイルだけでなく working tree の unstaged 変更も除外される。
   実測: 本 Issue 時点の `git diff origin/HEAD...HEAD --stat` は `web/src/generated/Feature.ts` の 18 行のみ
   (gen-ts 中間コミット 78ad1b4)。Codex は実装差分をほぼ受け取っていない。

2. **3 ファイルは規約上 STEP 8 まで意図的に untracked**
   `/3ai` の禁止事項 (「`features/$ISSUE/` の手動 `git add` は行わない — 必ず `finalize-feature.ts` 経由にする」
   「`git commit`/`push` は STEP 8 以外で行わない」) により、`crates/**` 実装ファイル・新規テスト・新規 example は
   STEP 8 の squash コミットで初めて追跡対象になる。実測 `git status --short` は 3 ファイルすべて `??`。

3. **「コンパイルとテストが成立しない」は事実に反する (独立検証済み)**
   untracked ファイルを含む実際の working tree で以下を再実行し全て green を確認した。

   | 検証 | 結果 |
   |---|---|
   | `cargo test -p engawa-build --test sketch_chamfer_acceptance` | 32 passed / 0 failed |
   | `cargo test -p engawa-build --test examples_smoke` | 29 passed / 0 failed |
   | `cargo test -p engawa-format --test golden_examples` | 22 passed / 0 failed (`golden_sketch_chamfer` 含む) |
   | `ci.log` (STEP 6 時点の `cargo xtask ci`) | `=== All CI checks passed ===` / 全 `test result` で `0 failed` |

   `golden_sketch_chamfer` と `examples_smoke` の通過は、Codex が「差分に入っていない」とした
   `examples/sketch_chamfer.engawa` が実在し正しく読めていることの直接的な証拠である。

4. **Codex 自身が 3 ファイルの中身を読んで検証している**
   `codex-final.yaml.log` に以下の実行記録がある。つまり Codex は「ファイルが存在しない」とは認識しておらず、
   M01 は diff の**梱包方法**に対する指摘であって実装の欠陥報告ではない。
   - `git status --short` (3 ファイルが `??` であることを確認)
   - `git diff --no-index -- /dev/null crates/engawa-kernel/src/geometry/sketch_chamfer.rs` (実装本体を全読)
   - `git diff --no-index -- /dev/null crates/engawa-build/tests/sketch_chamfer_acceptance.rs` (acceptance test を全読)
   - `sed -n '1,80p' examples/sketch_chamfer.engawa` (example を読解)
   - `git diff --cached --stat` (index が空であることを確認)

   さらに入力に含めた `claude-self-review.md:4` は
   「実装は未コミット (working tree + untracked) の状態で `cargo xtask ci` green」と明示していた。

5. **STEP 8 で diff に正しく含まれる**
   `finalize-feature.ts` 経由の squash コミットで 3 ファイルは追跡対象となり、
   マージ後の diff は Codex の suggestion が求める状態と一致する。修正作業は不要。

### フォローアップ

このツールバグは新規ファイルを伴う全 Issue で毎回同じ偽陽性 critical を発生させる構造的欠陥のため、
別 Issue として起票済み (`bug` / `batch:skill`): **#330**
`extract-test-summary.ts` の `added_in_round`、`detect-deliverable.ts`、`check-diff-coverage.ts`、
`dispatch-glm-review.ts`、`check-spec-divergence.ts` も同じ `git diff` 前提を共有しており横断的な影響がある
(`codex-input.md` の「NOTE: total_added=0 について」は既にこの症状の回避策として存在していた)。

## dispatch-codex.ts 修正 + 再実行 (M01 の根本対処)

M01 の棄却だけでなく、`dispatch-codex.ts:105` の diff 生成ロジック自体を本 Issue 内で修正した
(`git diff base...HEAD` の three-dot → `GIT_INDEX_FILE` で実 index を汚さない一時 index を作り
`git add -N .` + `git diff <merge-base>` に変更)。回帰テスト
`.claude/skills/3ai/scripts/__tests__/dispatch-codex.test.ts::T13` を追加し、
「untracked ファイル + unstaged 変更を diff に含む」「事前に staged していた変更が破壊されない」の
両方を固定した。#330 にコメントで部分修正済みである旨を追記済み。

修正後の diff (1717行、実装本体を正しく含む) で STEP 7.5 を再実行したところ、以下 2 件の**新規**指摘が
出た (M01 の偽陽性とは異なり、今回は実装の実診断)。

## Round 2: M01' (high) — 採用・修正済み

- file: `.claude/skills/3ai/scripts/dispatch-codex.ts:118`
- finding: `git add -N .` の後始末に無条件の `git reset` を使っており、review 実行前に既に staged 済みだった
  変更まで unstage してしまう。read-only と説明しているのに index 状態を破壊する副作用がある。
- 判定: **採用**。`GIT_INDEX_FILE` を使い実 index に一切触れない方式に変更 (コピーした一時 index 上で
  `add -N` + diff を行い、`unlinkSync` で一時ファイルを削除するだけで済ませ、`git reset` 自体を撤去)。
  T13 に「事前 staged 状態が維持される」アサーションを追加して固定。

## Round 2: A01' (medium) — 採用・修正済み

- file: `crates/engawa-build/tests/sketch_chamfer_acceptance.rs:389` (旧行番号)
- finding: T12 の CRUD gate 検証部が `features[features.len() - 1]` (= 実際は `Extrude`) を insert しており、
  「fillet → chamfer 連鎖の正常系」を意図した契約を固定できていなかった。
- 判定: **採用**。`features[..2]` (CreateSketch + SketchFillet) を history に、`features[2]`
  (意図した SketchChamfer) を insert 対象に修正。修正後 `cargo test t12_chain_fillet_then_chamfer` で pass 確認済み。

## Round 3 (dispatch-codex.ts 修正後の最終再実行): verdict pass, blocking=0

修正 2 件を適用後、`cargo xtask ci` green 再確認 → dispatch-codex.ts (修正版) で STEP 7.5 を再実行し
`verdict: pass, blocking: 0` (medium 2件のみ、非 block) を得た。

- **A01 (medium, 非block)**: T10 の build 側 2段目 Chamfer が `push` (Extrude の後) で適用されており、
  CRUD 側が検証する `[CreateSketch, c1, c2, Extrude]` と同一位置になっていない。
  → 後続 Issue でのテスト強化候補として記録 (`features_build.insert(2, ...)` への変更を推奨)。
- **A02 (medium, 非block)**: `ChamferLengthTooLarge` の厳密な境界値テスト
  (`length = 1.0 - LENGTH_TOLERANCE` → Ok, `length = 1.0` → エラー) が無い。大外れ値のみで判定している。
  → 自律判断ログの A-2 (claude-self-review.md) と同内容、follow-up refactor Issue にまとめて記載する。

**最終判定: STEP 7.5 blocking=0 で完了。STEP 8 へ進む。**
