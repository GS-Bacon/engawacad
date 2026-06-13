## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `.claude/skills/3ai/agents/glm-implementer.md` の Rust/MyCad 規約に「acceptance skeleton 不過剰拡張」セクション追加 | 案 B (GLM Write tool への技術的ガード) — 実装複雑 |
| `.claude/skills/3ai/agents/glm-test-implementer.md` にも同等の制約を追加 (STEP 6.6 で再発防止) | 案 C (過去 Codex 指摘の自動フィードバック蓄積) — スキル基盤再設計が必要 |
| ≤30 行目安、関数追加禁止、retry/wait helper 独自実装禁止、kill thread/timeout 必須を明文化 | プロンプト効果の定量検証 (運用で評価) |

## Non-Goals

- agent prompt の他セクションのリファクタ (本 Issue では skeleton 制約追加のみ)
- glm-implementer のロール定義変更
- glm-implementer 以外のレビュアー prompt 変更

## 実装対象

- `.claude/skills/3ai/agents/glm-implementer.md`: `### スコープ厳守` の直後に `### acceptance skeleton の不過剰拡張` セクションを追加
- `.claude/skills/3ai/agents/glm-test-implementer.md`: `### 本体コードの変更制限` の直後に同等の制約セクションを追加

新規スクリプト・新規テストファイルは作成しない (prompt 改訂のみ)。

## 設計方針

- **エラーハンドリング**: 該当なし (prompt 改訂)
- **derive 規約**: 該当なし
- **workspace.dependencies**: 該当なし

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 文書 | glm-implementer.md に「acceptance skeleton の不過剰拡張」セクションが存在 | grep でヒット |
| T02 | 文書 | glm-test-implementer.md にも同等の制約セクションが存在 | grep でヒット |
| T_boundary_kill_thread_keyword | 境界 | 両 agent prompt に「kill thread / timeout」または「deadlock 防止」キーワードが含まれる | grep でヒット |

## 幾何的不変条件チェックリスト

- [ ] N/A (3ai agent prompt 改訂。Boolean/Partition/Assemble 非関連)
