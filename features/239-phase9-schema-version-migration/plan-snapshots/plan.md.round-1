## 自律判断ログ

- Codex intent-check (`features/.batch/intent-239.yaml`) → aligned: no、reason: "Out-of-Scope / Non-Goals なし、境界が閉じていない"。Phase/scope の根本的不整合ではなく Issue body completeness の問題のため、自律モード判断で plan.md に Non-Goals を明文化して続行。
- 事前調査の発見: `schema_version: u32` フィールドと `default_schema_version()` fallback、example の `schema_version: 1` 追記、`test_schema_version_backward_compat` (T03 相当) は **既に実装済み** (commit `ade637c` / `36d183b` 等)。**本 Issue の残作業は (1) `MigrationHook` trait 定義 (2) 未知 version の明示エラー (3) T_DEG_unknown_version テスト追加 の 3 点のみ**。
- 数値判断は含まないため `### 数値モデル` セクションは N/A (intent-check #239 reason 内で「数値モデルは不要」とも明記)。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `MigrationHook` trait 定義 (`migration.rs` 新規 module) | 実 migration ロジック (v1→v2 等) の実装 |
| `Document::from_yaml` で `schema_version > CURRENT_SCHEMA_VERSION` を `FormatError::UnknownSchemaVersion` で reject | downgrade migration (新→旧) サポート |
| `FormatError` に `UnknownSchemaVersion { found: u32, current: u32 }` variant 追加 | 他レイヤ (engawa-build / engawa-cli) への schema_version 伝搬 |
| 未知 version 退化テスト (T_DEG_unknown_version) 追加 | `MigrationHook` 実装者 (具体 hook) の提供 |
| 既存 `schema_version` フィールド・fallback・examples 反映の **維持** (再変更しない) | TypeScript bindings (TS derive) や JsonSchema 出力フォーマットの変更 |

## Non-Goals

- 実 migration の v1→v2 ロジックを書かない (Phase 9 ではフィールドと trait 入口のみ)
- 他レイヤ (engawa-build / engawa-cli / engawa-viewer) への schema_version 関連 API 公開は本 Issue ではしない
- downgrade (new → old) migration trait API は定義しない
- ADR-014 が draft 段階のため、本 Issue では trait 形は ADR-014 draft が示す最小形に従う (`migrate(from, to, doc) -> Result<(), FormatError>`)
- 既存 `examples/*.engawa` の中身は変更しない (`schema_version: 1` 追記は完了済)

## 実装対象

- Issue: #239
- 影響クレート/ファイル:
  - `crates/engawa-format/src/migration.rs` (新規)
  - `crates/engawa-format/src/lib.rs` (`pub mod migration;` 追加 + `pub use migration::MigrationHook;`)
  - `crates/engawa-format/src/error.rs` (`UnknownSchemaVersion` variant 追加)
  - `crates/engawa-format/src/document.rs` (`from_yaml` / `Deserialize` で version reject ロジック追加)

### 新規 trait シグネチャ

```rust
// crates/engawa-format/src/migration.rs
use crate::document::Document;
use crate::error::FormatError;

/// .engawa schema migration hook. 実装者は from → to の Document mutation を担う。
pub trait MigrationHook {
    /// 適用元 schema version。
    fn from(&self) -> u32;
    /// 適用先 schema version。
    fn to(&self) -> u32;
    /// Document を `from` → `to` へ in-place で変換。
    fn migrate(&self, doc: &mut Document) -> Result<(), FormatError>;
}
```

### `FormatError` 変更 (error.rs)

before / after:

```rust
// before — variant 列挙の末尾に追加
#[error("RefPlane '{id}' has non-finite offset (NaN or Inf)")]
InvalidRefPlaneOffset { id: String },
```

```rust
// after
#[error("RefPlane '{id}' has non-finite offset (NaN or Inf)")]
InvalidRefPlaneOffset { id: String },

#[error("unknown schema_version {found}: this engawa-format supports up to {current}")]
UnknownSchemaVersion { found: u32, current: u32 },
```

### `Document::from_yaml` 変更 (document.rs)

before:

```rust
pub fn from_yaml(yaml: &str) -> Result<Self, FormatError> {
    let raw: RawDocument = serde_yaml::from_str(yaml)?;
    let mut doc = Document {
        schema_version: raw.schema_version,
        version: raw.version,
        root_component: raw.root_component,
    };
    if doc.root_component.ref_planes.is_empty() {
        doc.root_component.ref_planes = RefPlane::default_canonical_three();
    }
    doc.validate()?;
    Ok(doc)
}
```

after:

```rust
pub fn from_yaml(yaml: &str) -> Result<Self, FormatError> {
    let raw: RawDocument = serde_yaml::from_str(yaml)?;
    if raw.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(FormatError::UnknownSchemaVersion {
            found: raw.schema_version,
            current: CURRENT_SCHEMA_VERSION,
        });
    }
    let mut doc = Document {
        schema_version: raw.schema_version,
        version: raw.version,
        root_component: raw.root_component,
    };
    if doc.root_component.ref_planes.is_empty() {
        doc.root_component.ref_planes = RefPlane::default_canonical_three();
    }
    doc.validate()?;
    Ok(doc)
}
```

`impl<'de> Deserialize<'de> for Document` (line 87 以降) でも同じ reject ロジックを追加する (Document::deserialize 経路で構築される YAML から守るため)。

## 設計方針

- **決定性**: trait 定義のみで挙動変更なし。version reject は決定的 (`>` 比較)。
- **derive 規約**: `MigrationHook` は trait のため derive 不要。エラー variant は thiserror 経由で `Debug`。
- **エラーハンドリング**: `FormatError::UnknownSchemaVersion` に `found` (実値) と `current` (本ビルドのサポート上限) を両方含めて、ユーザーが「何を読もうとして失敗したか」を判別できるようにする。
- **workspace.dependencies**: 新しい依存追加なし。

### 数値モデル

N/A (本 Issue は version 整数判定のみ。tolerance/ε 値の判断を含まない)

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一 YAML を 2 回 `from_yaml` → 構築された `Document` が一致 | assert_eq! で field 一致 |
| T02 | 正常系 | `schema_version: 1` を含む YAML → エラーなく parse、`doc.schema_version == 1` | 既存 `test_schema_version_backward_compat` で T02 + T03 をカバー (継続維持) |
| T03 | 正常系 | `schema_version` 省略 → fallback で `doc.schema_version == 1` (既存テスト維持) | `test_schema_version_backward_compat` |
| T_DEG_unknown_version | 退化 | `schema_version: 99` の YAML → `from_yaml` が `FormatError::UnknownSchemaVersion { found: 99, current: 1 }` を返す | `matches!(err, FormatError::UnknownSchemaVersion { found: 99, .. })` |
| T_BOUNDARY_current_version | 境界 | `schema_version: 1` (= CURRENT) → 正常 parse | `doc.schema_version == 1`、エラーなし |
| T_DEG_max_u32 | 退化 | `schema_version: 4294967295` (`u32::MAX`) → `UnknownSchemaVersion { found: u32::MAX, current: 1 }` | エラー variant 一致 |
| T_TRAIT_migration_hook_signature | 正常系 | `MigrationHook` trait 実装サンプル (dummy) が compile + `from()/to()/migrate()` を呼べる | doc test or unit test (dummy impl `struct V1ToV2; impl MigrationHook for V1ToV2 { ... }`) |

実装後、`#[ignore]` を付けたスケルトンを STEP 5.5 で先置きし、STEP 6 で GLM が実装して `#[ignore]` を外す。

## 幾何的不変条件チェックリスト

- [x] N/A — partition / assemble / boolean 系 Issue ではないため (フォーマット I/O 層のみ)
