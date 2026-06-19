# Plan: feat(phase9): Feature CRUD Delete (build + cli) — #260

## 自律判断ログ

- intent-check r2 で `aligned: yes` 取得済み
- Phase 9 Feature CRUD ops 6 件目 (最終)。本 Issue closure 後、Phase 9 build+cli 軸の Feature CRUD は揃う。Phase 9 完了は ADR-014/015/016 needs-human 解消後に別途判定
- ADR-014/015/016 確定を待たない (#256-#259 と同方針)
- 数値モデル不要

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `FeatureCrud::delete(&doc, feature_id) -> Result<Document>` build 実装 (feature を完全除去、ID-stable for 残 feature) | 他 CRUD op (Edit/Rollback/Suppress/Reorder は #256-#259) |
| `engawa entry remove <input> <feature_id>` cli サブコマンド (`--output`, `--dry-run` 対応) | ADR-014 / ADR-015 / ADR-016 改訂 (別 Issue で進行中) |
| 他 Feature が参照中の Feature を delete → 明示エラー (`EditBreaksConsumer` 再利用) | `engawa entry remove` 以外の cli 変更 |
| T01-T06 を build + cli テストで担保 | 削除された feature_id の再利用禁止 (= 呼び出し側責務、本 Issue では強制しない) |
| | suppress (#258) との semantic 区別: delete は **完全除去**、suppress は **再開可能な inert 化** |

## Non-Goals

- Delete の cascade semantics (= 依存 Feature が参照中なら明示エラーで止める、暗黙の cascade はしない)
- Delete 後の id 再利用禁止 (= 呼び出し側責務)
- Phase 9 全体完了判定 (= ADR-014/015/016 needs-human 解消後の別判定)
- `### 数値モデル` (数値判断なし)

## 実装対象

- `crates/engawa-build/src/feature_crud.rs` (FeatureCrud::delete 追加)
- `crates/engawa-cli/src/main.rs` (EntryOp::Remove + run_entry 分岐)
- `crates/engawa-build/tests/260_phase9_feature_crud_delete_acceptance.rs` (新規)
- `crates/engawa-cli/tests/260_phase9_entry_remove_cli.rs` (新規)

### `impl FeatureCrud` (crates/engawa-build/src/feature_crud.rs)

**before**: insert + edit + rollback + suppress + reorder (#255-#259 既存)

**after**: `delete` を追加:
```rust
impl FeatureCrud {
    // 既存 ...

    /// Remove the feature identified by `feature_id` from the history.
    /// 
    /// Returns Err if the feature is unknown, or if removing it would break
    /// a downstream consumer (caught by pre/post simulate compare).
    pub fn delete(
        doc: &Document,
        feature_id: &str,
    ) -> Result<Document, FeatureCrudError> {
        let idx = doc.root_component.features.iter()
            .position(|f| f.id() == feature_id)
            .ok_or_else(|| FeatureCrudError::UnknownFeatureId {
                feature_id: feature_id.to_string(),
            })?;
        // Build post-delete history (feature removed)
        let mut post: Vec<Feature> = doc.root_component.features.clone();
        post.remove(idx);
        // pre/post simulate: previously executing consumer must still execute
        let (_, _, executed_at_pre) = simulate_history(&doc.root_component.features, doc.root_component.features.len());
        let (_, _, executed_at_post) = simulate_history(&post, post.len());
        for (orig_idx, _) in doc.root_component.features.iter().enumerate() {
            if orig_idx == idx { continue; } // deleted feature itself — not a consumer to preserve
            if !executed_at_pre.contains(&orig_idx) { continue; }
            let orig_id = doc.root_component.features[orig_idx].id();
            let post_idx_opt = post.iter().position(|f| f.id() == orig_id);
            let post_idx = match post_idx_opt {
                Some(i) => i,
                None => continue, // shouldn't happen but defensive
            };
            if !executed_at_post.contains(&post_idx) {
                return Err(FeatureCrudError::EditBreaksConsumer {
                    edit_feature_id: feature_id.to_string(),
                    broken_consumer_id: orig_id.to_string(),
                    broken_consumer_at: orig_idx,
                    broken_ref: format!("delete broke this consumer (still referencing deleted feature)"),
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

**before**: Add + Edit + Rollback + Suppress + Restore + Reorder (#255-#259 既存)

**after**: `Remove` を追加:
```rust
EntryOp::Remove {
    input: PathBuf,
    feature_id: String,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long)]
    dry_run: bool,
}
```

run_entry 分岐: `FeatureCrud::delete(&doc, &feature_id)` を呼んで YAML 出力 (他 op と同型)。

## 設計方針

- **決定性**: `Vec::remove` のみ、HashMap は存在判定のみ。T01 で byte-equal assert。
- **依存検出**: 削除すると参照中の consumer が壊れるケースを `EditBreaksConsumer` で reject (= "依存 Feature が参照中なら明示エラー" 要件を pre/post simulate 比較で実装)。
- **エラー再利用**: `UnknownFeatureId` (#256) / `EditBreaksConsumer` (#256) を再利用、新 variant 不要。
- **derive 規約**: 既存どおり。
- **workspace.dependencies**: 新規依存なし。
- **suppress との semantic 区別**: delete は完全除去 (再開不可)、suppress (#258) は flag toggle で再開可能。Issue body の Non-Goals 「削除された feature 状態 (= delete とは別)」と整合。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 | 配置 |
|----|------|------|----------|------|
| T01 | 決定性 | 同一 (doc, feature_id) を 2 回 `delete` → byte-equal | assert_eq | acceptance.rs |
| T02 | 正常系 (build) | `[box_1, sphere_1, cyl_1]` → delete("sphere_1") → `[box_1, cyl_1]` | features 順序確認 | acceptance.rs |
| T03 | 正常系 (cli) | `engawa entry remove <input> sphere_1` → 出力 YAML から sphere_1 削除 | input parse + remove で expected YAML を組んで byte-equal | cli.rs |
| T04_DEG_referenced | 退化 | Extrude が参照中の CreateSketch を delete → `EditBreaksConsumer` | matches Err | acceptance.rs |
| T05_DEG_unknown_id | 退化 | 存在しない feature_id → `UnknownFeatureId` | matches Err | acceptance.rs |
| T06_BOUNDARY_last_feature | 境界 | 1 feature doc → delete → `[]` (空 feature 列) | features.len() == 0 | acceptance.rs |

退化 + 境界 ID 3 件 (`T04_DEG_referenced`, `T05_DEG_unknown_id`, `T06_BOUNDARY_last_feature`) で ADR-006 §1 要件を満たす。

## 幾何的不変条件チェックリスト

- N/A (履歴 Document 純関数変換のみ)
