# Plan: feat(phase9): Feature CRUD Reorder (build + cli) — #259

## 自律判断ログ

- intent-check r2 で `aligned: yes` 取得済み
- T_BOUNDARY_self semantics 確定 (Issue body Out-of-Scope より): `feature_id == before_id` → **no-op (元 Document をそのまま返す Ok)**、エラーにはしない
- ADR-014/015/016 確定を待たない (#256-#258 と同方針)
- 数値モデル不要

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `FeatureCrud::reorder(&doc, feature_id, before_id) -> Result<Document>` build 実装 (feature_id を before_id の直前に移動、ID-stable) | 他 CRUD op (Edit/Rollback/Suppress は #256-#258、Delete は #260) |
| `engawa entry reorder <input> <feature_id> --before <before_id>` cli サブコマンド (`--output`, `--dry-run` 対応) | ADR-014 / ADR-015 / ADR-016 改訂 (別 Issue で進行中) |
| 依存サイクル発生時はエラー (= pre/post simulate 比較で `EditBreaksConsumer` 検出) | `engawa entry reorder` 以外の cli 変更 |
| T01-T06 を build + cli テストで担保 | atomic / multi-feature 同時移動 (Non-Goals) |

## Non-Goals

- Reorder の atomic transaction / multi-feature 同時移動 (= 単一 feature_id を 1 つの before_id 前に挿入する単純 op)
- 依存サイクル発生時の自動解消 (= 明示エラーで止める、解消は呼び出し側責務)
- T_BOUNDARY_self (feature_id == before_id) の期待値は **no-op (元 Document をそのまま返す Ok)** で固定 (エラーにはしない)
- `### 数値モデル` (数値判断なし)

## 実装対象

- `crates/engawa-build/src/feature_crud.rs` (FeatureCrud::reorder 追加)
- `crates/engawa-cli/src/main.rs` (EntryOp::Reorder + run_entry 分岐)
- `crates/engawa-build/tests/259_phase9_feature_crud_reorder_acceptance.rs` (新規)
- `crates/engawa-cli/tests/259_phase9_entry_reorder_cli.rs` (新規)

### `impl FeatureCrud` (crates/engawa-build/src/feature_crud.rs)

**before**: `insert` + `edit` + `rollback` + `suppress` (#255-#258 既存)

**after**: `reorder` を追加 (注: `simulate_history(features: &[Feature], up_to: usize) -> (HashMap<String,usize>, HashMap<String,usize>, HashSet<usize>)` を再利用。executed_at は HashSet<usize> なので `.contains(&idx)` で参照可能。`post.iter().position(...).unwrap()` の unwrap は安全 — orig_id は post = `doc.features.clone() + remove + insert` の結果に必ず存在するため):
```rust
impl FeatureCrud {
    // 既存メソッド ...

    pub fn reorder(
        doc: &Document,
        feature_id: &str,
        before_id: &str,
    ) -> Result<Document, FeatureCrudError> {
        if feature_id == before_id {
            return Ok(doc.clone()); // T_BOUNDARY_self
        }
        let from_idx = doc.root_component.features.iter()
            .position(|f| f.id() == feature_id)
            .ok_or_else(|| FeatureCrudError::UnknownFeatureId {
                feature_id: feature_id.to_string(),
            })?;
        let before_idx = doc.root_component.features.iter()
            .position(|f| f.id() == before_id)
            .ok_or_else(|| FeatureCrudError::UnknownFeatureId {
                feature_id: before_id.to_string(),
            })?;
        let mut post: Vec<Feature> = doc.root_component.features.clone();
        let moved = post.remove(from_idx);
        let insert_at = if before_idx > from_idx { before_idx - 1 } else { before_idx };
        post.insert(insert_at, moved);
        // pre/post simulate: previously-executing consumer must still execute
        let (_, _, executed_at_pre) = simulate_history(&doc.root_component.features, doc.root_component.features.len());
        let (_, _, executed_at_post) = simulate_history(&post, post.len());
        for (orig_idx, _) in doc.root_component.features.iter().enumerate() {
            if !executed_at_pre.contains(&orig_idx) { continue; }
            let orig_id = doc.root_component.features[orig_idx].id();
            let post_idx = post.iter().position(|f| f.id() == orig_id).unwrap();
            if !executed_at_post.contains(&post_idx) {
                return Err(FeatureCrudError::EditBreaksConsumer {
                    edit_feature_id: feature_id.to_string(),
                    broken_consumer_id: orig_id.to_string(),
                    broken_consumer_at: orig_idx,
                    broken_ref: format!("reorder broke this consumer (cycle or producer moved past consumer)"),
                });
            }
        }
        let mut next = doc.clone();
        next.root_component.features = post;
        next.validate()?;
        Ok(next)
    }
}
```

### `EntryOp` enum + `run_entry` (crates/engawa-cli/src/main.rs)

**before**: Add + Edit + Rollback + Suppress + Restore (#255-#258 既存)

**after**: `Reorder` を追加:
```rust
EntryOp::Reorder {
    input: PathBuf,
    feature_id: String,
    /// id of the feature to move this one immediately before
    #[arg(long)]
    before: String,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long)]
    dry_run: bool,
}
```

run_entry 分岐: `FeatureCrud::reorder(&doc, &feature_id, &before)` を呼んで YAML 出力 (他 op と同型)。

## 設計方針

- **決定性**: `Vec::remove` + `Vec::insert` は決定的、HashMap は存在判定のみで出力に影響なし。T01 で byte-equal assert。
- **T_BOUNDARY_self**: feature_id == before_id を最初に検知して `Ok(doc.clone())` で early-return → no-op。
- **依存サイクル / consumer break 検出**: post-reorder の `simulate_history` を pre と比較し、previously executing consumer が post で inert なら `EditBreaksConsumer` で reject。これにより (a) reorder で producer が consumer の後ろに移動するケース、(b) 暗黙のサイクル発生ケース の両方を検出。
- **エラー再利用**: `UnknownFeatureId` (#256 追加) / `EditBreaksConsumer` (#256 追加) を再利用、新 variant 不要。
- **derive 規約**: 既存どおり (追加 type なし)。
- **workspace.dependencies**: 新規依存なし。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 | 配置 |
|----|------|------|----------|------|
| T01 | 決定性 | 同一 (doc, feature_id, before_id) を 2 回 `reorder` → byte-equal | assert_eq | acceptance.rs |
| T02 | 正常系 (build) | `[box_1, sphere_1, cyl_1]` → reorder("cyl_1", "sphere_1") → `[box_1, cyl_1, sphere_1]` | features 順序確認 | acceptance.rs |
| T03 | 正常系 (cli) | `engawa entry reorder <input> cyl_1 --before sphere_1` → 出力 YAML の順序確認 | input parse + reorder で expected YAML を組んで byte-equal | cli.rs |
| T04_BOUNDARY_self | 境界 | reorder("box_1", "box_1") → no-op (元 doc と byte-equal) | byte-equal to original | acceptance.rs |
| T05_DEG_circular | 退化 | B が A を参照 (A=CreateSketch, B=Extrude with sketch=A) の状態で A を B の後に移動 → `EditBreaksConsumer` | matches Err | acceptance.rs |
| T06_DEG_unknown_id | 退化 | 存在しない feature_id / before_id → `UnknownFeatureId` | matches Err | acceptance.rs |

退化 + 境界 ID 3 件 (`T04_BOUNDARY_self`, `T05_DEG_circular`, `T06_DEG_unknown_id`) で ADR-006 §1 要件を満たす。

## 幾何的不変条件チェックリスト

- N/A (履歴 Document 純関数変換のみ)
