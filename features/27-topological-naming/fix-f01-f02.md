# 修正指示 — F01/F02: ComponentRef と EntityRef の不正値排除

## F01: ComponentRef::File("stdlib://...") を validate で拒否

### 問題
`ComponentRef::File("stdlib://something")` は現在の validate_component を素通りし、
YAML に書かれると reload 時に StdLib に化けて round-trip が壊れる。

### 修正 (crates/mycad-format/src/document.rs)

`validate_component` 内の reference チェックに追加:
```rust
if let Some(ref_val) = &component.reference {
    match ref_val {
        ComponentRef::StdLib(path) if path.is_empty() => {
            return Err(FormatError::InvalidReference {
                value: format!("stdlib://"),
                reason: "stdlib reference must have a non-empty path after 'stdlib://'",
            });
        }
        ComponentRef::File(path) if path.is_empty() => {
            return Err(FormatError::InvalidReference {
                value: String::new(),
                reason: "reference must not be empty",
            });
        }
        ComponentRef::File(path) if path.starts_with("stdlib://") => {
            return Err(FormatError::InvalidReference {
                value: path.clone(),
                reason: "file reference must not start with 'stdlib://'",
            });
        }
        _ => {}
    }
}
```

テスト追加:
```rust
#[test]
fn t17_file_with_stdlib_prefix_rejected() {
    let mut doc = Document::new("Test");
    doc.root_component.reference = Some(ComponentRef::File("stdlib://foo".into()));
    assert!(matches!(doc.to_yaml().unwrap_err(), FormatError::InvalidReference { .. }));
}
```

## F02: EntityRef 公開構築バイパス問題

### 問題
`EntityRef::Named { feature_id: "bad;id".into(), kind: ..., role: ... }` のように
直接 variant を構築すると不正値でも Serialize/canonical_name が受け付けてしまう。

### 修正方針: try_new + Serialize に validate gate を追加

#### 1. validated constructor の追加 (crates/mycad-format/src/feature.rs)
```rust
impl EntityRef {
    pub fn try_named(
        feature_id: impl Into<String>,
        kind: EntityKind,
        role: impl Into<String>,
    ) -> Result<Self, FormatError> {
        let v = EntityRef::Named { feature_id: feature_id.into(), kind, role: role.into() };
        v.validate()?;
        Ok(v)
    }

    pub fn try_derived(
        kind: EntityKind,
        op: impl Into<String>,
        from: Vec<EntityRef>,
        selector: impl Into<String>,
    ) -> Result<Self, FormatError> {
        let v = EntityRef::Derived { kind, op: op.into(), from, selector: selector.into() };
        v.validate()?;
        Ok(v)
    }
}
```

#### 2. Serialize に validate gate を追加
`#[derive(Serialize)]` を削除し、手書き impl で validate を先走らせる:

```rust
impl Serialize for EntityRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate().map_err(serde::ser::Error::custom)?;
        // derive が生成するのと同等の内部タグ付き enum をシリアライズ
        // 既存の serde tag = "ref" 形式を保持するため、Shadow enum を使う:
        #[derive(Serialize)]
        #[serde(tag = "ref", rename_all = "snake_case")]
        enum Shadow<'a> {
            Named { feature_id: &'a str, kind: EntityKind, role: &'a str },
            Derived { kind: EntityKind, op: &'a str, from: &'a [EntityRef], selector: &'a str },
        }
        match self {
            EntityRef::Named { feature_id, kind, role } =>
                Shadow::Named { feature_id, kind: *kind, role }.serialize(serializer),
            EntityRef::Derived { kind, op, from, selector } =>
                Shadow::Derived { kind: *kind, op, from, selector }.serialize(serializer),
        }
    }
}
```

#### 3. テスト追加
```rust
#[test]
fn t_serialize_rejects_invalid_directly_constructed() {
    // 直接 variant 構築でも Serialize が validate を通す
    let invalid = EntityRef::Named {
        feature_id: "bad;id".into(),
        kind: EntityKind::Face,
        role: "top".into(),
    };
    let result = serde_yaml::to_string(&invalid);
    assert!(result.is_err(), "Serialize of invalid EntityRef must fail");
}
```

## 完了条件
- `cargo test -p mycad-format` 全通過 (t17, serialize rejection test 含む)
- `cargo build --workspace` 全通過
- `cargo xtask ci` green
- **注意**: Shadow enum が Serialize を derive する際に `from: &'a [EntityRef]` を扱えない場合は、
  代わりに `from: Vec<EntityRef>` のクローン、または Shadow の Derived に `from: &'a Vec<EntityRef>` を
  使うなど適宜調整すること。
