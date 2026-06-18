# Debug Spec (STEP 7.5 r1 → r2 — Codex 3 ペルソナ採用指摘の修正)

## 概要

Codex 3 ペルソナ独立技術ゲートで 2 round 分の指摘を統合し fix する。codex_loops=1 → 2 へ。

---

## R1 修正 (済) — 4 fix 採用

- ✅ MigrationHook trait シグネチャを `migrate(from, to, doc)` 1 メソッド形に統一
- ✅ `default_schema_version` を `INITIAL_SCHEMA_VERSION = 1` 固定で deserialize 専用に分離
- ✅ `FormatError` に `#[non_exhaustive]` 付与
- ✅ T01 determinism を deep equality (`to_yaml()` 比較) に強化

---

## R2 修正 (本ファイル) — 3 採用 / 1 棄却

### Fix R2-1: `schema_version` 検査を 2 段階 deserialize 経路に変える (A-F01 high)

`crates/engawa-format/src/document.rs`:

**現状の問題**:
```rust
pub fn from_yaml(yaml: &str) -> Result<Self, FormatError> {
    let raw: RawDocument = serde_yaml::from_str(yaml)?;  // ← Feature.type が
    if raw.schema_version > CURRENT_SCHEMA_VERSION {     //   未知の変種を含むと
        return Err(...);                                  //   ここに到達する前に
    }                                                     //   YAML error で落ちる
    ...
}
```

**修正方針**: untyped `serde_yaml::Value` でまず `schema_version` だけ抜き取り、`> CURRENT` を判定してから `RawDocument` への typed deserialize に進む。

**Before** (`from_yaml`):
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

**After**:
```rust
pub fn from_yaml(yaml: &str) -> Result<Self, FormatError> {
    // Stage 1: untyped peek to enforce schema_version contract before typed parsing.
    let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;
    let peeked_version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(INITIAL_SCHEMA_VERSION);
    if peeked_version > CURRENT_SCHEMA_VERSION {
        return Err(FormatError::UnknownSchemaVersion {
            found: peeked_version,
            current: CURRENT_SCHEMA_VERSION,
        });
    }

    // Stage 2: typed deserialize (now we know the schema is one we support).
    let raw: RawDocument = serde_yaml::from_value(value)?;
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

`impl<'de> Deserialize<'de> for Document` (line 87 以降) も同じ 2-stage 化:

```rust
impl<'de> Deserialize<'de> for Document {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_yaml::Value::deserialize(deserializer)?;
        let peeked_version = value
            .get("schema_version")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(INITIAL_SCHEMA_VERSION);
        if peeked_version > CURRENT_SCHEMA_VERSION {
            return Err(serde::de::Error::custom(format_args!(
                "unknown schema_version {}: this engawa-format supports up to {}",
                peeked_version, CURRENT_SCHEMA_VERSION
            )));
        }
        let raw: RawDocument = serde_yaml::from_value(value).map_err(serde::de::Error::custom)?;
        let mut doc = Document {
            schema_version: raw.schema_version,
            version: raw.version,
            root_component: raw.root_component,
        };
        if doc.root_component.ref_planes.is_empty() {
            doc.root_component.ref_planes = RefPlane::default_canonical_three();
        }
        doc.validate().map_err(serde::de::Error::custom)?;
        Ok(doc)
    }
}
```

**追加テスト** (inline + acceptance 両方):

```rust
/// T_FUTURE_unknown_feature_type: schema_version > CURRENT で、現行 Component が
/// deserialize できない future Feature.type を含むペイロードでも、
/// UnknownSchemaVersion で reject されること (2-stage 経路の回帰テスト)。
#[test]
fn t_future_unknown_feature_type_reject() {
    let yaml = "\
schema_version: 99
version: 0.1.0
root_component:
  name: Future
  features:
    - type: create_hyperspace_warp
      id: hw_1
      foo: 42
      bar: 7.0
";
    let result = Document::from_yaml(yaml);
    match result {
        Err(FormatError::UnknownSchemaVersion { found, current }) => {
            assert_eq!(found, 99);
            assert_eq!(current, CURRENT_SCHEMA_VERSION);
        }
        other => panic!("expected UnknownSchemaVersion, got {other:?}"),
    }
}
```

### Fix R2-2: `MigrationHook` の doc comment を訂正 (C-F01 medium)

`crates/engawa-format/src/migration.rs`:

**Before** (line 11-15):
```rust
/// Migration hook for converting documents between schema versions.
///
/// Implementers provide the logic to transform a `Document` from an older schema version
/// to a newer one in-place. The migration is applied after deserialization and before
/// validation.
```

**After**:
```rust
/// Migration hook for converting documents between schema versions.
///
/// Implementers provide the logic to transform a `Document` from an older schema version
/// to a newer one in-place. This trait defines the migration *contract*; the actual driver
/// (loader that selects and invokes hooks during deserialization) will be added in a
/// follow-up Issue. Currently, no API automatically applies registered hooks.
```

### Fix R2-3: テスト const 化 (C-F02 low)

`crates/engawa-format/tests/schema_version_migration_acceptance.rs`:

- `use engawa_format::document::CURRENT_SCHEMA_VERSION;` を import に追加 (現状は `engawa_format::{Document, FormatError, MigrationHook}` のみ)
- `t_boundary_current_version` の `schema_version: 1` を `schema_version: {current}` に変更し、assertion も `CURRENT_SCHEMA_VERSION` 経由
- `t_deg_unknown_version_99` / `t_deg_max_u32_version` の `assert_eq!(current, 1)` を `assert_eq!(current, CURRENT_SCHEMA_VERSION)` に変更

inline (`document.rs` 内 `tests` mod) でも同様に `CURRENT_SCHEMA_VERSION` 直接参照可能なので合わせる (`super::CURRENT_SCHEMA_VERSION`)。

### M-F01 は棄却 (autonomous 判断)

> 「`UnknownSchemaVersion` variant 追加 + `#[non_exhaustive]` 付与」が source-breaking、semver-major にすべき。

**棄却理由**:
- 本 Issue は phase 9 の format 拡張で、ADR-014 (draft 中) が breaking 変更を許容する前提 (新 trait + 新 variant)
- `#[non_exhaustive]` を **外す** と次回 variant 追加でまた exhaustive match を壊す。今回の breaking を呑む代わりに forward-looking compat を獲得するのが r1 M-F02 採用の趣旨
- `engawa-api/src/error.rs` の `From<FormatError>` は本 issue 内で追従済み (CI green)
- M-F01 は r1 M-F02 と直接矛盾する指摘で、両方同時に採用することは構造的に不可能

`rejection.md` に転記する。

---

## 修正後の検証

1. `cargo xtask ci` が green
2. `cargo test -p engawa-format --test schema_version_migration_acceptance` が新規追加 `t_future_unknown_feature_type_reject` を含めて pass
3. `cargo test -p engawa-format --lib` で inline test pass (`t_future_unknown_feature_type_reject` inline 版も追加)
4. `cargo test -p engawa-api` で `From<FormatError>` 経路が壊れていない

## 試した修正と結果

- [x] R1: MigrationHook trait 1-method 化 — Codex r2 で A-F01/C-F02/M-F01 解消
- [x] R1: `INITIAL_SCHEMA_VERSION` 分離 — Codex r2 で C-F01 解消
- [x] R1: `#[non_exhaustive]` 付与 — Codex r2 で M-F02 解消 (M-F01 で再指摘されたが棄却)
- [x] R1: T01 deep equality — Codex r2 で M-F03 解消
- [ ] R2 fix 3 件は本 dispatch で適用予定

## 次にやること

GLM に `--mode core` で再 dispatch、本 debug-spec.md (R2 セクション) を渡す。
