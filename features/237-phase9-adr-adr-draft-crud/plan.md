## 自律判断ログ

- Issue body は `docs/decisions/014-*.md` を作る指示だが、ADR-014 は既に `014-component-refplane-isolation.md` で使用済み (commit済、#207 で gate:adr-review 中)。**ADR 番号を 014→015 に繰り上げる**: 新規ファイル `docs/decisions/015-phase9-design-foundations.md`。
- Issue body の "ADR-014" 表記は本 plan で "ADR-015" に読み替える。Issue 本体タイトルとの不整合は merge commit メッセージで明示する。
- intent-check 不要 (light flow, `intent_check_required: false`)
- light flow: STEP 2'/5/5.5/6/6.5/6.6/7、STEP 7.5 skip (keep_codex_gate=false の state shim 適用)

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `docs/decisions/015-phase9-design-foundations.md` を draft 状態で commit | ADR 内の決定を **実装する** こと (実装は #239/#240/#241/#242/#243 で各々) |
| Feature CRUD (Edit/Roll back/Suppress/Reorder/Delete/Insert) API 抽象の Decision Matrix (Options A/B/C + Trade-off + 採用前提崩壊 trigger) | Feature CRUD の具体 API シグネチャ確定 (実装 Issue で詰める、ADR は方向性のみ) |
| Variable の 2 段スコープ (Document / Sketch) 名前空間定義 + `${var_name}` 構文採択理由 | Variable 評価エンジンの実装 (#241) |
| `schema_version` 命名規則 + migration hook trait 形 (`migrate(from, to, doc) -> Result<(), FormatError>`) | migration ドライバの実装 (本 Phase より後) |
| 品質基盤 (proptest / criterion / cargo-fuzz / cargo-llvm-cov / Playwright) の選定理由と最小 setup 範囲 | 各ツールの実セットアップ (#243) |
| 既存 ADR (-005 / -007 / -013) との関係の明文化 | 既存 ADR の改訂 |
| 親 #194 の plan.md 分割計画書との整合確認 | #194 の plan.md 自体の修正 |
| `gate:adr-review` ラベル付与 → ADR-013 auto-accept フロー (Decision Matrix lint + Codex 3 ペルソナ) で approved | 人間レビューを要求すること (auto-accept フロー前提) |

## Non-Goals

- ADR 内の決定を本 Issue で実装しない (純粋に方針文書)
- Variable 評価エンジンの実装、Feature CRUD API の確定、migration driver の実装、品質基盤ツールの実セットアップ — いずれも別 Issue
- 既存 ADR (-001 〜 -014) の改訂
- ADR-013 auto-accept フロー自体の修正

## 実装対象

- Issue: #237
- 影響ファイル:
  - `docs/decisions/015-phase9-design-foundations.md` (新規)
  - `crates/engawa-format/tests/adr_015_phase9_doc_acceptance.rs` (新規、ADR file 構造の smoke test)

## 設計方針

- **ADR スタイル**: 既存 ADR (013/014) と同じ heading 構造を使う:
  ```
  # ADR-015: <title>

  **Date**: 2026-06-18
  **Status**: Accepted
  **Related**: ADR-002, ADR-006, ADR-013, ADR-014
  **Resolves**: Issue #237

  ---

  ## Context
  ## Decision (Phase 9 設計基盤 4 項目)
    ### 1. Feature CRUD API 抽象
    ### 2. Variable 2 段スコープ
    ### 3. schema_version + MigrationHook
    ### 4. 品質基盤ツール選定
  ## Decision Matrix
    | 項目 | Option A | Option B | Option C | 採用 | Trade-off | 採用前提崩壊 trigger |
  ## Trade-off
  ## 採用前提崩壊 trigger
  ## 既存 ADR との関係
  ## Open Questions
  ```
- **Decision Matrix lint 準拠**: `loop-adr-decision-matrix-lint.ts` が Options A/B/C / Trade-off / 採用前提崩壊 trigger / 既存 ADR 関係を必須セクションとして lint する。これら 4 セクションを必ず含める。

### 数値モデル

N/A (ADR draft のため数値判断なし)

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | ADR file を 2 回 read → 内容が一致 | `assert_eq!(content_a, content_b)` |
| T_DOC_required_sections | 正常系 | ADR file が必須セクション (`## Context`, `## Decision`, `## Decision Matrix`, `## Trade-off`, `## 採用前提崩壊 trigger`, `## 既存 ADR との関係`) を含む | 各 `## <name>` が file content に存在 |
| T_DOC_decision_matrix_has_options | 正常系 | Decision Matrix セクションに Options A/B/C のいずれも記載がある | grep で Option A/B/C が見つかる |
| T_DEG_file_missing | 退化 | file path が存在することを check | path::Path::new(...).exists() == true |
| T_BOUNDARY_status_accepted | 境界 | Status は `Accepted` または `Proposed` のいずれか | content に `**Status**: Accepted` or `**Status**: Proposed` が見つかる |

## 幾何的不変条件チェックリスト

- [x] N/A — ADR draft Issue
