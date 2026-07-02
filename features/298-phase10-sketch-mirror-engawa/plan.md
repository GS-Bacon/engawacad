## In-Scope / Out-of-Scope
<!-- ADR-006 §plan.md 必須セクション。GLM SCOPE ペルソナが存在を検証する。 -->
| In-Scope | Out-of-Scope |
|----------|--------------|
| (本 Issue で実装するもの) | (明示的に除外するもの) |

## Non-Goals
<!-- Out-of-Scope と同内容でも重複 OK。dispatch-codex-auto.ts の guard が参照する。 -->
<!-- スコープ外を必ず列挙。該当なしの場合も "- 該当なし" と書くこと（空欄禁止）。 -->
<!-- 例: - フル退化検出: #34 で対応予定 -->

## 実装対象
<!-- Issue: #NNN -->
<!-- 影響クレート/ファイル: (具体パス列挙) -->
<!-- 変更する型・関数のシグネチャ -->
<!-- 既存関数の修正がある場合は STEP ごとに before/after スニペットを明記。新規追加のみの場合は不要。 -->

## 設計方針
<!-- 決定性要件: IdGenerator の使い方、同一入力→同一出力の保証方法 -->
<!-- B-rep トポロジー妥当性: Euler-Poincaré V - E + F = 2 が成立するか -->
<!-- 退化幾何の扱い: ゼロ長エッジ、面積ゼロ面などの排除・エラー条件 -->
<!-- derive 規約: Debug/Clone/Serialize/Deserialize (+JsonSchema が必要か) -->
<!-- エラーハンドリング: thiserror の使い方 -->
<!-- workspace.dependencies 規約 -->

### 数値モデル (Phase 4 / 6+ で必須、不要なら削除)
<!-- ADR-006 §plan.md 必須セクション。GLM NUMERIC ペルソナが存在を検証する。 -->
<!-- tolerance: ε_snap = (値) — 点の同一判定 -->
<!-- tolerance: ε_len  = (値) — エッジ長の退化判定 -->
<!-- tolerance: ε_area = (値) — 面積ゼロ判定 -->
<!-- ADR-004 準拠方針: tolerant / exact のどちらを採用するか -->

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一入力を2回実行し全 ID・座標が一致 | assert_eq! |
| T02 | 正常系 | ... | ... |

## 幾何的不変条件チェックリスト
<!-- Boolean/Partition/Assemble 系の Issue のみ記述。非該当は各項目を "N/A" に書き換えること。 -->
- [ ] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか
- [ ] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか
- [ ] flip_normals / same_sense の意味論が明確か（頂点順を変えるか vs 法線だけ変えるか）
- [ ] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか
