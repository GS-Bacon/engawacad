# Test Spec: #247 GLM reviewer 層境界注入

## 不足テスト (plan 計画分)

| ID | 検証内容 | 実行結果 |
|----|----------|---------|
| T02_files_inject | 6 reviewer prompt 全てに「クレート層境界」見出しが注入されている (heading 1 + 末尾 bullet で計 2 occurrence) | PASS: 全 6 ファイル = 2 |
| T03_scope_rule_appended | 6 ファイルの `### スコープ規律` に「層越境指摘禁止」bullet が追加されている | PASS: `grep -l ... \| wc -l` = 6 |
| T04_yaml_format_unchanged | `## 出力フォーマット（厳守）` セクションは無変更 | PASS: 全 6 ファイル = 1 occurrence |
| T_DEGEN_no_collateral | 既存の PRIOR REJECTIONS / PRIOR JUDGMENTS / SCOPE DEFENSE 規律文言は削除されていない | PASS: scope=3, invariant=3, ambig=4, numeric=3, final=1, assembly=1 (改訂前と同一値) |
| T_BOUNDARY_no_idgen_for_format | invariant.md に「engawa-format には `IdGenerator` は存在しない」明示文がある | PASS: `grep -F` でマッチ確認済み |

## 実装差分から追加すべきテスト

なし。Rust crate を一切触らないため `cargo test --workspace` の追加テストは不要。`cargo xtask ci` は markdown 変更が他テストに影響しないことの regression として既に green (テスト実行ログ → features/247-3ai-step-d-check/ci.log)。

## エッジケース・退化入力

なし (prompt 改訂は input 概念がない静的 markdown)。

## 数値境界

該当なし。

## 決定性

該当なし (静的 markdown ファイル、ランダム要素なし)。

## 期待値乖離

なし。plan の T ID 期待値はすべて shell コマンド出力で検証済み (上表の「実行結果」列)。GLM test 実装 (STEP 6.6) は **dispatch せず Claude 直接検証で完了**。理由:

1. `.claude/skills/3ai/agents/glm-reviewer-*.md` は GLM 自身の system prompt であり、GLM 自身に「自分の system prompt を改訂したテストを書け」と dispatch すると self-modification の循環 (memory `project_3ailoop_implementation_style`) になる
2. T02-T_BOUNDARY は markdown 構造の grep 検証で完結し、Rust test harness が不要

→ STEP 6.6 はスキップし `glm_impl passed` を state shim でセットする。STEP 7 (GLM final review) は通常通り走らせて第三者視点を保つ。STEP 7.5 (Codex) も保持。

## 類似ケース (未カバー)

`codex-design-reviewer.md` / `codex-final-reviewer.md` には同等のセクションがまだ無い。これは Non-Goals で「別 Issue で扱う」と明示済み。本サイクル内では起票しないが、次回 Codex reviewer false alarm が観察されたら同様の手順で別 Issue 起票して扱う。
