# Plan: feat(phase9): Feature CRUD Suppress (build + cli) — #258

## 自律判断ログ (intent-check r2 反映 + schema 拡張方針)

- intent-check r2 で `aligned: yes` 取得済み (Out-of-Scope / Non-Goals 追記後)
- **schema 拡張**: 本 Issue は Feature enum に `suppressed: bool` フィールドを追加する必要があり、ADR-002 schema_version bump 相当の変更を含む
  - `#[serde(default, skip_serializing_if = "std::ops::Not::not")] suppressed: bool` を各 variant に追加 → 既存 YAML (suppressed なし) は default=false で parse 可能、suppressed=false の場合 YAML 出力に含まれない → **後方互換性を保つ最小拡張**
  - schema_version の bump は本 Issue では不要 (既存 YAML を valid に保つため)。`schema_version` の正式な扱いは ADR-015 (#246 needs-human) で決定後の別 Issue で扱う
- ADR-014/015/016 確定を待たない (#256/#257 と同じ方針)
- 数値モデル不要

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| Feature enum 各 variant に `suppressed: bool` (default false, skip_serializing_if not) フィールド追加 (engawa-format) | 他 CRUD op (Edit は #256、Rollback は #257、Reorder/Delete は #259/#260) |
| `FeatureCrud::suppress(&doc, feature_id, on: bool) -> Result<Document>` 実装 (engawa-build) | ADR-014 / ADR-015 / ADR-016 改訂 (別 Issue で進行中) |
| `simulate_history` で `suppressed=true` を inert (executed_at に入れず) 扱い (engawa-build) | `schema_version` bump (= ADR-015 確定後の別 Issue) |
| `engawa entry suppress <id>` / `engawa entry restore <id>` cli サブコマンド (engawa-cli) | `engawa entry suppress/restore` 以外の cli 変更 |
| `build_assembly` で suppressed feature を skip (engawa-build) | `### 数値モデル` セクション (数値判断なし) |
| 退化テスト: 依存 Feature が参照中の Feature suppress → 明示エラー | suppress 状態の TUI/viewer 可視化 (Phase 21+) |
| T01-T06 を build + cli テストで担保 | suppress の cascade semantics (= 暗黙連鎖は禁止、明示エラーで止める) |

## Non-Goals

- Suppress の atomic transaction / undo stack (= 単一 op に閉じる)
- Suppress 状態での再生成性能最適化 (Phase 10+)
- 削除された feature 状態 (= delete #260 とは別)。suppress は **再開 (restore) 可能**な inert 化
- `Document::schema_version` の bump (= ADR-015 確定後の別 Issue)

## 実装対象

- 影響クレート / ファイル:
  - `crates/engawa-format/src/feature.rs` (各 variant に `suppressed: bool` 追加)
  - `crates/engawa-build/src/feature_crud.rs` (FeatureCrud::suppress + simulate_history で suppressed skip)
  - `crates/engawa-build/src/lib.rs` (build_assembly で suppressed skip)
  - `crates/engawa-cli/src/main.rs` (EntryOp::Suppress / EntryOp::Restore + run_entry 分岐)
  - `crates/engawa-build/tests/258_phase9_feature_crud_suppress_acceptance.rs` (新規, T01/T02/T_DEG_*)
  - `crates/engawa-cli/tests/258_phase9_entry_suppress_cli.rs` (新規, T03/T04)
- 変更する型・関数のシグネチャ:
  - 各 Feature variant: `suppressed: bool` (default false) フィールド追加
  - `FeatureCrud::suppress(doc: &Document, feature_id: &str, on: bool) -> Result<Document, FeatureCrudError>`
  - `simulate_history` 内で suppressed=true を inert 扱い
  - `EntryOp::Suppress { input: PathBuf, feature_id: String, output: Option<PathBuf>, dry_run: bool }`
  - `EntryOp::Restore { input: PathBuf, feature_id: String, output: Option<PathBuf>, dry_run: bool }`

### 既存関数修正の before / after

#### `Feature` enum (crates/engawa-format/src/feature.rs)

**before** (各 variant 例: CreateBox):
```rust
CreateBox {
    id: String,
    width: f64,
    height: f64,
    depth: f64,
},
```

**after** (suppressed フィールド追加 — 全 9 variant に同様):
```rust
CreateBox {
    id: String,
    width: f64,
    height: f64,
    depth: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    suppressed: bool,
},
```

これにより:
- 既存 `.engawa` YAML (`suppressed` フィールドなし) は parse 時に default=false で読める
- suppressed=false の場合は YAML 出力に含まれず、既存 fixture との byte 互換性保持
- suppressed=true の時のみ `suppressed: true` が serialize される

#### `impl FeatureCrud` (crates/engawa-build/src/feature_crud.rs)

**before**: `insert` + `edit` + `rollback`

**after**: 上記に `suppress` を追加:
```rust
impl FeatureCrud {
    // 既存メソッド ...

    /// Set or clear the `suppressed` flag on the feature identified by `feature_id`.
    /// `on = true` → suppress, `on = false` → restore.
    ///
    /// If the feature is currently being depended on by a downstream consumer
    /// (i.e. suppress would make a consumer's refs unresolvable), returns
    /// `FeatureCrudError::EditBreaksConsumer` (= same semantic as edit).
    pub fn suppress(
        doc: &Document,
        feature_id: &str,
        on: bool,
    ) -> Result<Document, FeatureCrudError> {
        let idx = doc.root_component.features.iter()
            .position(|f| f.id() == feature_id)
            .ok_or_else(|| FeatureCrudError::UnknownFeatureId {
                feature_id: feature_id.to_string(),
            })?;
        // Build new feature with toggled suppressed flag
        let mut new_feature = doc.root_component.features[idx].clone();
        set_feature_suppressed(&mut new_feature, on);
        // Reuse check_edit_preserves_consumers semantic: pre/post simulate compare
        // (suppressed=true effectively removes feature from execution; downstream must remain valid)
        check_edit_preserves_consumers(&new_feature, &doc.root_component.features, idx)?;
        let mut next = doc.clone();
        next.root_component.features[idx] = new_feature;
        next.validate()?;
        Ok(next)
    }
}

/// Helper: set the `suppressed` field on any Feature variant.
fn set_feature_suppressed(f: &mut Feature, on: bool) {
    match f {
        Feature::CreateBox { suppressed, .. }
        | Feature::CreateCylinder { suppressed, .. }
        | Feature::CreateSphere { suppressed, .. }
        | Feature::CreateSketch { suppressed, .. }
        | Feature::Extrude { suppressed, .. }
        | Feature::ExtrudeCut { suppressed, .. }
        | Feature::Cut { suppressed, .. }
        | Feature::Fuse { suppressed, .. }
        | Feature::Intersect { suppressed, .. } => {
            *suppressed = on;
        }
    }
}
```

#### `simulate_history` (crates/engawa-build/src/feature_crud.rs)

**before** (各 variant の処理):
```rust
Feature::CreateBox { id, .. } => {
    live_bodies_at.insert(id.clone(), i);
    executed_at.insert(i);
}
```

**after** (suppressed=true なら skip):
```rust
Feature::CreateBox { id, suppressed, .. } => {
    if *suppressed { continue; }
    live_bodies_at.insert(id.clone(), i);
    executed_at.insert(i);
}
```

全 9 variant で同様 (`if *suppressed { continue; }` を冒頭に追加)。CreateSketch / Extrude / ExtrudeCut / Cut / Fuse / Intersect も含む。Extrude/ExtrudeCut 等の refs_resolve check 前に suppressed check を入れる (suppress された feature は他の executed_at にも入らず inert になる)。

#### `build_assembly` (crates/engawa-build/src/lib.rs)

**before** (features を順に処理):
```rust
for feature in &doc.root_component.features {
    // process feature
}
```

**after** (suppressed=true は skip):
```rust
for feature in &doc.root_component.features {
    if feature_is_suppressed(feature) { continue; }
    // process feature (既存)
}
```

helper `feature_is_suppressed(&Feature) -> bool` を engawa-format に追加 (Feature の inherent method として):
```rust
impl Feature {
    pub fn is_suppressed(&self) -> bool {
        match self {
            Feature::CreateBox { suppressed, .. }
            | Feature::CreateCylinder { suppressed, .. }
            | /* 全 9 variant */
            => *suppressed,
        }
    }
}
```

#### `EntryOp` enum + `run_entry` (crates/engawa-cli/src/main.rs)

**before**: Add + Edit + Rollback

**after**: Suppress + Restore を追加:
```rust
EntryOp::Suppress { input, feature_id, output, dry_run } => { /* suppress(doc, &id, true) */ }
EntryOp::Restore { input, feature_id, output, dry_run } => { /* suppress(doc, &id, false) */ }
```

## 設計方針

- **決定性**: `Vec` 直接 index 操作 + `suppressed` flag toggle のみで HashMap 非依存。T01 で byte-equal assert。
- **schema 後方互換**: `#[serde(default, skip_serializing_if = "std::ops::Not::not")]` で既存 YAML (suppressed なし) を parse 可能、suppressed=false 時 serialize で出力されない → 既存 fixture との byte 互換性保持。
- **downstream 検証**: `check_edit_preserves_consumers` を再利用 (suppress=true は feature を inert 化 → 削除と等価扱い、下流 consumer の参照壊れ検出)。`EditBreaksConsumer` を返す。
- **エラーハンドリング**: 既存 `UnknownFeatureId` + `EditBreaksConsumer` を再利用 (新 variant なし)。
- **derive 規約**: 既存 `#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]` のまま (フィールド追加のみ)。
- **workspace.dependencies**: 新規依存なし。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 | 配置 |
|----|------|------|----------|------|
| T01 | 決定性 | 同一 (doc, feature_id, true) を 2 回 `suppress` → `to_yaml()` byte-equal | assert_eq | acceptance.rs |
| T02 | 正常系 (build) | 1 feature doc → suppress(true) → `features[0].is_suppressed() == true` | true | acceptance.rs |
| T03 | restore 正常系 | suppress(true) → suppress(false) で元に戻る | features[0].is_suppressed() == false, byte-equal to original | acceptance.rs |
| T04 | 正常系 (cli, suppress) | `engawa entry suppress <input> box_1 --output out.engawa` → out.engawa に `suppressed: true` | byte-equal | `crates/engawa-cli/tests/258_phase9_entry_suppress_cli.rs` |
| T05 | 正常系 (cli, restore) | `engawa entry restore <input> box_1 --output out.engawa` → out.engawa に suppressed 行なし | byte-equal | 同上 |
| T_DEG_referenced | 退化 (build) | Extrude が参照中の CreateSketch を suppress → `EditBreaksConsumer` エラー | matches Err | acceptance.rs |
| T_DEG_unknown_id | 退化 (build) | 存在しない feature_id → `UnknownFeatureId` エラー | matches Err | acceptance.rs |

退化ケース ID 2 件 (`T_DEG_referenced`, `T_DEG_unknown_id`) で ADR-006 §1 要件を満たす。

## 幾何的不変条件チェックリスト

- N/A (本 Issue は履歴 Document 純関数変換 + simulate skip 拡張のみ。B-rep 不変条件は build 段階で検証)
