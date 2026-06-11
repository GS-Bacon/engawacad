## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `crates/mycad-kernel/tests/extrude_negative_direction_acceptance.rs` 削除 (4 関数 `todo!()` + `#[ignore]` のまま、本物の regression test は `extrusion.rs:783-825` インラインに既存) | untracked diagnostic stubs 12 本 (既に削除済み、本 Issue 着手時点で git ls-files --others が空) |
| `crates/mycad-kernel/tests/test_bool_probe.rs` 削除 (2 行コメントのみ "safely deleted" 自記) | 他の `#[ignore]` テスト (`#[ignore = "STEP 6 ..."]` 付きの STEP 5.5 acceptance skeleton で正当な進行中状態) |
| `.claude/skills/3ai/scripts/pre-step8-check.ts` 拡張: `crates/**/tests/*.rs` から「`#[ignore]` + `todo!()` のみ」「1-行 stub コメント」パターンを検出し warn + exit 1 | `/3ai` skill フロー自体の再設計、STEP 8 異常終了パスの根本対策 |
| 新 guard の unit test (TypeScript で fixture を作って exit 1 を検証) | #110 (closed) のインライン regression test の修正 |

## Non-Goals
- untracked diagnostic file 12 本の削除 (本 Issue 着手時点で既に削除済み、`git ls-files --others --exclude-standard crates/mycad-kernel/tests/` が空)
- `#[ignore = "STEP 6 で実装後に解除"]` 付きの正当な acceptance skeleton (STEP 5.5 で生成された進行中状態) の検出・警告
- `/3ai` skill の STEP 8 異常終了経路 (#121/#126/#127/#128/#133) の根本対策 — 本 Issue はガード追加で再発時の被害を抑えるのみ
- `extrude_negative_direction_acceptance.rs` の中身を実装し直すこと (#110 のインライン regression test で既にカバー済み、重複を残さない)
- 幾何カーネル本体の変更 (本 Issue は test ファイルと skill scripts のみ)

## 実装対象

**Issue**: #139
**影響範囲**:
- 削除: `crates/mycad-kernel/tests/extrude_negative_direction_acceptance.rs` (37 行、全 4 関数 `todo!()`)
- 削除: `crates/mycad-kernel/tests/test_bool_probe.rs` (2 行コメント)
- 拡張: `.claude/skills/3ai/scripts/pre-step8-check.ts` (62 行 → +stub 検出ロジック ~50 行)
- 新規: `.claude/skills/3ai/scripts/__tests__/pre-step8-check.test.ts` (bun test、新ロジックの単体テスト)

### `pre-step8-check.ts` の before/after

**before** (現状): `crates/` 配下の unstaged/untracked のみ検出。`todo!()` のまま放置された acceptance skeleton や 1 行 stub は通過する。

**after**:
- 既存の unstaged/untracked 検出はそのまま
- 追加: `crates/**/tests/*.rs` を走査し、以下のいずれかに合致するファイルを警告として列挙:
  - **stale todo!() stub**: ファイル内のすべての `#[test]` 関数が `#[ignore]` + 関数本体に `todo!()` のみ — かつ Issue の `__tests__` 用途と区別するため `Cargo.toml` 直下の `tests/` 限定 (integration test)
  - **1-line stub**: 非空行が ≤ 3 行で `temporary` / `removed` / `safely deleted` のいずれかを含む
- 警告検出時の挙動: stderr に列挙 + exit 1 (既存の unstaged 検出と同 exit code)
- false positive 防止: 通常テスト (`#[test]` + 実装あり)、`#[ignore]` でも body が `todo!()` 以外 (例: `assert!(...)`, `unimplemented!`) は警告しない

### 削除手順

```bash
git rm crates/mycad-kernel/tests/extrude_negative_direction_acceptance.rs
git rm crates/mycad-kernel/tests/test_bool_probe.rs
cargo test --workspace --no-run  # 依存検出・コンパイル確認
cargo test --workspace           # 既存テスト全 pass 確認
```

## 設計方針

- **決定性**: ガード検出は static text scan (regex / 行カウント) で実行順依存なし。同じファイルセット → 同じ警告。
- **エラーハンドリング**: stub 検出は警告として stderr 出力 + exit 1。既存の unstaged 検出と同じ exit code 規約に揃える。
- **derive 規約**: TypeScript のため該当なし
- **workspace.dependencies 規約**: 該当なし (bun 内で動く、追加 npm 依存なし)
- **/3ai skill 整合性**: 本ガードは STEP 8 直前にのみ走るので、STEP 5.5 で生成された `#[ignore]` + `todo!()` skeleton は STEP 6 で `todo!()` を本物実装に置換するため (or 本物実装後に `#[ignore]` を外すため)、正常フローでは STEP 8 到達時には警告対象に該当しない。本 Issue の削除対象 (#110 が close 済みなのに `todo!()` が残っている) はまさにこの不整合の症状なので、ガードが本来検知すべきパターンと一致する。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 削除確認 (regression) | `extrude_negative_direction_acceptance.rs` と `test_bool_probe.rs` 削除後 `cargo test --workspace` が green | 既存テスト全 pass、削除した 4 + 0 = 4 関数分減少 |
| T02 | guard 検出 — stale stub | fixture `tmp/_test_fixtures/all_todo.rs` (全関数が `#[ignore]` + `todo!()`) を作って pre-step8-check 実行 | exit 1 + stderr に該当ファイル列挙 |
| T03 | guard 検出 — 1-line stub | fixture `tmp/_test_fixtures/oneline.rs` (`// temporary file removed` 1 行) を作って実行 | exit 1 + stderr に該当ファイル列挙 |
| T04_boundary_legitimate | guard 偽陽性ガード | 通常 integration test (`#[test]` + assert!) は警告対象外 | exit 0 |
| T_degen_partial_stub | guard 偽陽性ガード | 一部関数が `todo!()`、一部は本実装 → 「すべての関数が stub」条件に該当しない | exit 0 (警告なし) |
| T_boundary_empty_file | guard 退化処理 | 空ファイル (0 行) → 1-line stub 条件外 | exit 0 |

T02-T_boundary_empty_file は bun test (`.claude/skills/3ai/scripts/__tests__/pre-step8-check.test.ts`) で実装。fixture は `/tmp` 配下に作り、テスト終了時に削除。

## 幾何的不変条件チェックリスト

- partition 出力: **N/A** (test ファイル削除 + skill script、幾何変更なし)
- プリミティブ face outer_loop 2D 向き: **N/A**
- flip_normals / same_sense: **N/A**
- pslg_subdivide 出向き整合: **N/A**

## /3ai フロー方針 (本 Issue 限定)

scope が「test ファイル 2 本削除 + TypeScript script 拡張」のみで `crates/**` のロジック変更は無いため、以下を簡略化する:

- **STEP 3 (GLM 多ペルソナ設計レビュー)**: **スキップ**。ADR-006 §1 粒度ガードで Light 判定 (microscope ペルソナ機械的 Issue)。SCOPE + AMBIG のみが light 推奨だが、本 Issue は scope が一意で曖昧性なし。
- **STEP 5.5 (Acceptance Test Skeleton)**: 本 Issue は新規 test 追加ではなく既存 test の削除主体なので、新規 acceptance skeleton は不要。代わりに pre-step8-check 用 TypeScript test を直接書く。
- **STEP 6 (GLM コア実装)**: **Claude 直接実行**。crates/**/tests/ の delete は guard-crates フックの `tests/` 緩和で Claude 可。`.claude/skills/3ai/scripts/` も Claude 可。GLM dispatch せず。
- **STEP 6.6 (GLM テスト実装)**: 同上、Claude 直接実行。
- **STEP 7 (GLM 最終レビュー)**: スキップ可 (scope が機械的)
- **STEP 7.5 (Codex 独立技術ゲート)**: **保持**。`/3ai` skill 本体への変更で再発防止ガードが入るので、別モデル系の独立確認は価値あり。

## 検証

- `cargo xtask ci` green (削除した 2 ファイルが他のテストに依存されていないこと)
- `bun test .claude/skills/3ai/scripts/__tests__/pre-step8-check.test.ts` で T02-T_boundary_empty_file が pass
- `bun .claude/skills/3ai/scripts/pre-step8-check.ts` を現在の repo で実行 → 既存 unstaged 検出が正常に動作 (回帰なし)
