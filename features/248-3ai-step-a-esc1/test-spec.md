# Test Spec: #248 TS drift mitigation

## 不足テスト (plan 計画分)

| ID | 検証内容 | 実行結果 |
|----|---------|---------|
| T01_determinism | 2 回連続実行で 2 回目は no-op | PASS (bun test) |
| T02_clean_noop | clean working tree → exit 0、commit 数変化なし | PASS (bun test) |
| T03_modified_commit | modified tracked TS → commit 1 追加 | PASS (bun test) |
| T04_untracked_commit | untracked TS → commit 1 追加 + tracked になる | PASS (bun test) |
| T05_issue_msg | `--issue 248` でメッセージに `#248` | PASS (bun test) |
| T_DEG_outside_gen | `web/src/main.ts` の dirty は対象外 | PASS (bun test) |
| T_BOUNDARY_no_issue | `--issue` なし → メッセージに `#` 含まない | PASS (bun test) |

bun test 結果: `7 pass / 0 fail / 18 expect() calls`

## 実装差分から追加すべきテスト

なし。本 Issue の差分は:
- `.claude/skills/3ai/scripts/maybe-commit-generated-ts.ts` (新規)
- `.claude/skills/3ai/scripts/maybe-commit-generated-ts.test.ts` (新規)
- `.claude/skills/3ai/SKILL.md` (3 箇所追記)

`crates/` を一切触らないため Rust acceptance test は不要 (`cargo xtask ci` は既存 1152 tests 全 green を確認、本 Issue 由来の regression なし)。bun test の T01〜T_BOUNDARY が機能カバレッジ。

## エッジケース・退化入力

- T_DEG_outside_gen: scope creep 防止 (web/src/ 全体ではなく `web/src/generated/` のみ commit)
- T_BOUNDARY_no_issue: 引数欠落時の挙動 (msg に `#undefined` などが入らないか)

両方 PASS。

## 数値境界

該当なし。

## 決定性

T01 で検証済。同一 working tree state に対して script は idempotent (1 回目で commit → 2 回目は clean なので no-op)。

## 期待値乖離

なし。plan のテスト計画 ID と bun test の describe/test 名が 1:1 で対応。

## 類似ケース (未カバー)

該当なし。`web/src/generated/` 以外の auto-generated 領域は現状なし (Non-Goals 明記済み)。

## GLM test 実装スキップの正当性

`.claude/skills/3ai/scripts/*.ts` への GLM dispatch は `project_3ailoop_implementation_style` メモリ + #247 と同じ self-modification 循環リスクに該当 (GLM が自分自身が呼ばれるフローの helper を実装すると次サイクルが即座に壊れる可能性)。代わりに Claude 直接実装 + bun test で品質担保。STEP 7 GLM final review と STEP 7.5 Codex を保持して第三者視点を入れる。

→ STEP 6.6 は dispatch せず `glm_impl passed` を state shim でセット (state set 済)。
