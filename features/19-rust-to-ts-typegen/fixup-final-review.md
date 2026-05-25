## Codex 最終レビュー指摘修正

以下 3 点を修正すること。

---

### F01: test_ts_derive_backward_compat を golden 比較に強化

**対象**: `crates/mycad-format/src/document.rs` の `test_ts_derive_backward_compat` テスト

**問題**: 現在は `from_path → to_yaml → from_yaml → to_yaml` の roundtrip が安定するか (yaml1==yaml2) しか確認していない。
`#[derive(TS)]` が `serde` 属性に影響した場合の検出には不十分。

**修正内容**:
- `examples/simple_box.mycad` に対し、hardcoded な golden YAML 文字列を用意し、
  `Document::from_path(path).unwrap().to_yaml().unwrap()` がその golden と一致することを assert する。
- golden は実際に `cargo test` を実行して `to_yaml()` の出力を確認し、その出力を static として貼る。
- roundtrip 比較 (yaml1==yaml2) は残しても良い。golden 比較を **追加** する形で。

**手順**:
1. 一時的に `println!("{}", doc.to_yaml().unwrap())` を追加して `cargo test -- test_ts_derive_backward_compat --nocapture` を実行し、simple_box.mycad の canonical YAML を取得する。
2. そのまま golden として static str に貼り付ける。
3. `println!` は削除してから完成版のテストにする。

---

### F02: T01/T02/T05/T07/T08 の assertion を強化

**対象**: `crates/mycad-api/tests/mesh_api.rs`

**T01 (t01_normal_box), T02 (t02_normal_cylinder)**:
- `serde_json::Value` の使用をやめ、`mycad_kernel::tessellation::TriangleMesh` に deserialize する。
- `mesh.positions.len() > 0` と `mesh.indices.len() % 3 == 0` を assert。
- TriangleMesh は `use mycad_kernel::tessellation::TriangleMesh;` でインポート。
- `mycad-api/Cargo.toml` の `[dev-dependencies]` に `mycad-kernel = { workspace = true }` が必要なら追加。

**T05 (t05_invalid_extension)**:
- 現在は status code のみ。
- body を `ErrorResponse` に parse し、`err.error` が空でないことと、"extension" / ".mycad" / "Invalid" のいずれかを含むことを assert。

**T07 (t07_unsupported_feature)**:
- 現在は status code のみ。
- body を `ErrorResponse` に parse し、`err.error` が空でないことを assert。

**T08 (t08_degenerate_dimension)**:
- 現在は status code のみ。
- body を `ErrorResponse` に parse し、`err.error` が空でないことを assert。

注: `ErrorResponse` は既に mesh_api.rs に `#[derive(Deserialize)] struct ErrorResponse { error: String }` として定義されている。追加定義は不要。

---

### F03: xtask TS export テストの並列実行 flakiness 解消

**対象**: `crates/xtask/src/main.rs` の `#[cfg(test)]` モジュール

**問題**: `export_to_temp_dir()` が `std::env::set_var("TS_RS_EXPORT_DIR", ...)` を呼んでおり、
cargo test の並列実行で複数テストが同じ環境変数を奪い合い、t01_determinism が flaky になる。

**修正内容**: テストモジュール先頭に static Mutex を追加し、`export_to_temp_dir()` 呼び出し前に取得する:

```rust
use std::sync::{Mutex, OnceLock};
static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn export_to_temp_dir() -> PathBuf {
    let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    // ... 既存のコード
}
```

---

## 完了条件
1. 上記 3 点を修正する
2. `cargo test --workspace` が全 green
3. `cargo clippy --workspace -- -D warnings` が通る
4. `cargo fmt --all -- --check` が通る
5. 結果を glm-result.json に書き出す: { "status": "success|failed", "ci_passed": true|false, "summary": "...", "failed_reason": "..." }

## 禁止
- git commit/push は行わない
