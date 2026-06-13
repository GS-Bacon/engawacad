## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| SKILL.md STEP 7.5-C の `--raise-at 3` → `--raise-at 6` に変更 | `state.ts inc` 自体のロジック改修 (現状の `--raise-at` 引数で対応可能) |
| SKILL.md STEP 7.5-D 見出し・本文の `codex_loops > 2` → `codex_loops > 5` に変更 | 案 B (同系統指摘の文字列類似度ベース判定) — 実装複雑、運用検証なし |
| 自律モード時の「3 round 連続 blocking」検出ユーザー確認レール追加 (案 C) | Codex agent prompt 改訂 — 出力品質は高いまま維持したい |

## Non-Goals

- Codex 出力の文字列類似度判定ロジック実装
- `state.ts` への新規サブコマンド追加
- 他 STEP の閾値変更 (本 Issue は STEP 7.5 のみ)

## 実装対象

`.claude/skills/3ai/SKILL.md` のみ:
- STEP 7.5-C: `--raise-at 3` → `--raise-at 6` / 「`codex_loops` 上限 2」→「`codex_loops` 上限 5」
- STEP 7.5-C 末尾に自律モード追加判定の記述 (3 round 連続 blocking で停止点)
- STEP 7.5-D 見出しの `codex_loops > 2` → `codex_loops > 5`

新規スクリプト・新規テストファイルは作成しない。

## 設計方針

- 案 C (ハイブリッド: 上限緩和 + 連続 blocking でユーザー確認) を採用
- 5 round 上限の根拠: #145 実例で 5 round 必要だった (r1=hi+md, r2=cr, r3=hi+md, r4=hi, r5=pass)
- 3 round 連続 blocking 条件は最小限: 上限 5 までに完了する余地を残しつつ、明らかな loop 兆候 (3 連続 fail) で停止

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 文書 | SKILL.md に `--raise-at 6` が存在 | grep でヒット |
| T02 | 文書 | SKILL.md に `codex_loops 上限 5` が存在 | grep でヒット |
| T03 | 文書 | SKILL.md に `3 round 連続 blocking` の自律モード判定が存在 | grep でヒット |
| T_boundary_old_threshold_removed | 境界 | 旧 `--raise-at 3` や `上限 2` が STEP 7.5-C/D セクションに残っていない | grep で `--raise-at 3` (STEP 7.5 内) がヒットしない |

## 幾何的不変条件チェックリスト

- [ ] N/A (3ai SKILL.md 数値変更。Boolean/Partition/Assemble 非関連)
