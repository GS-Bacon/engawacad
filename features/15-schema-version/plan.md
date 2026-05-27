# Issue #15 実装プラン: Document に schema_version 追加

## 目標

`.mycad` Document に `schema_version: u32` を追加し、将来の migration 土台を確立する。

---

## 変更 1: `crates/mycad-format/src/document.rs`

### struct 定義前に追加

```rust
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}
```

### `Document` struct を以下に変更

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct Document {
    /// Format schema version. Increment when the .mycad file format changes in a breaking way.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Kernel version that created this document.
    pub version: String,
    /// The root component (assembly or single part).
    pub root_component: Component,
}
```

### `Document::new()` を変更

```rust
pub fn new(name: &str) -> Self {
    Self {
        schema_version: CURRENT_SCHEMA_VERSION,
        version: env!("CARGO_PKG_VERSION").to_string(),
        root_component: Component::new(name),
    }
}
```

### テスト変更

**golden 文字列の更新** (`test_ts_derive_backward_compat`):
```
let golden = "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Simple Box\n  features:\n  - type: create_box\n    id: box_1\n    width: 10.0\n    height: 20.0\n    depth: 30.0\n";
```

**golden 文字列の更新** (`test_extruded_rect_yaml_golden`):
```rust
static GOLDEN: &str = concat!(
    "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Extruded Rect\n  features:\n",
    // ...以降は変更なし
```
つまり先頭に `"schema_version: 1\n"` を追記するだけ。

**後方互換テスト追加** (`test_ts_derive_backward_compat` モジュール or 別関数):

```rust
#[test]
fn test_schema_version_backward_compat() {
    // schema_version フィールドがない古い形式の YAML でも読めること
    let old_yaml = "version: 0.1.0\nroot_component:\n  name: Old\n  features: []\n";
    let doc = Document::from_yaml(old_yaml).expect("old yaml should parse");
    assert_eq!(doc.schema_version, 1, "missing schema_version defaults to 1");
    // 再シリアライズすると schema_version: 1 が出力されること
    let yaml = doc.to_yaml().unwrap();
    assert!(yaml.starts_with("schema_version: 1\n"), "re-serialized yaml must include schema_version");
}
```

---

## 変更 2: `examples/*.mycad` 5 ファイル全て

各ファイル先頭行の `version:` の**前に** `schema_version: 1` を追記する。

対象:
- `examples/simple_box.mycad`
- `examples/cylinder.mycad`
- `examples/sphere.mycad`
- `examples/extruded_rect.mycad`
- `examples/assembly.mycad`

例 (`simple_box.mycad`):
```yaml
schema_version: 1
version: "0.1.0"
root_component:
  name: "Simple Box"
  ...
```

---

## 変更 3: `crates/xtask/src/main.rs` の DOCUMENT_GOLDEN 更新

`schema_version` フィールドを ts-rs が生成する TS 型に含める必要がある。

**手順**:
1. 変更 1 を適用後に `cargo test -p xtask 2>&1 | head -80` を実行し、実際のテスト失敗メッセージから `actual` (実際の生成内容) を確認する。
2. `DOCUMENT_GOLDEN` をその実際の出力に合わせて更新する。

期待される追加内容（`schema_version` フィールドが TS 型の先頭に現れる）:
```typescript
export type Document = {
/**
 * Format schema version. Increment when the .mycad file format changes in a breaking way.
 */
schema_version: number,
/**
 * Kernel version that created this document.
 */
version: string,
/**
 * The root component (assembly or single part).
 */
root_component: Component, };
```

---

## 完了条件

1. `cargo xtask ci` が green
2. `cargo run -p mycad-cli -- export examples/simple_box.mycad -o /tmp/box.stl` が成功（後方互換確認: ファイルに schema_version があっても正常動作）
3. git commit / push は行わない（オーケストレーターが行う）
