## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `[workspace.dependencies]` に `serial_test` crate を追加 | mycad-api 起動時 port の CLI 引数化 (案 B) |
| `crates/mycad-api/Cargo.toml` の `[dev-dependencies]` に `serial_test = { workspace = true }` 追加 | TcpListener::bind("127.0.0.1:0") による動的 port 割り当て |
| `startup_log_acceptance.rs` の port_guard (Mutex) を削除し各 test 関数に `#[serial(mycad_api_port_7878)]` 属性付与 | mycad-api 本体ロジック変更 |
| 既存 T01 / T02 テストが引き続き green を維持 | static_assets / static_assets_edge など mock-based test の改修 (port 7878 を spawn しないため対象外) |

## Non-Goals

- 案 B (port 動的割り当て) は API 仕様変更を伴うため別 Issue
- 他の integration test ファイル (mock-based 系) の serial 化 (今は不要)
- e2e_api_scenarios.rs 等の改修 (spawn しないため衝突しない)

## 実装対象

<!-- Issue: #153 -->
<!-- 影響ファイル -->
- `Cargo.toml` (workspace ルート): `[workspace.dependencies]` に `serial_test = "3"` を追加
- `crates/mycad-api/Cargo.toml`: `[dev-dependencies]` に `serial_test = { workspace = true }` を追加
- `crates/mycad-api/tests/startup_log_acceptance.rs`: port_guard ヘルパ削除、`use serial_test::serial`、各 test 関数に `#[serial(mycad_api_port_7878)]` を付与

### 1. workspace Cargo.toml

`[workspace.dependencies]` セクションに以下を追加 (アルファベット順の位置に挿入):

```toml
serial_test = "3"
```

### 2. `crates/mycad-api/Cargo.toml`

`[dev-dependencies]` セクションに以下を追加:

```toml
serial_test = { workspace = true }
```

### 3. `crates/mycad-api/tests/startup_log_acceptance.rs`

**before** (port_guard ヘルパ部分):
```rust
use std::sync::{Mutex, OnceLock};

fn port_guard() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
}
```

**after**: port_guard 関数を削除。`Mutex` / `OnceLock` の use 文も startup_log_acceptance.rs 内で他で使われていないなら削除。

**before** (各 test 関数の冒頭):
```rust
#[test]
fn t01_debug_binary_logs_listening() {
    let _lock = port_guard().lock().unwrap_or_else(|e| e.into_inner());
    // ... 残りはそのまま
}
```

**after**:
```rust
use serial_test::serial;

#[test]
#[serial(mycad_api_port_7878)]
fn t01_debug_binary_logs_listening() {
    // port_guard().lock() 行を削除
    // ... 残りはそのまま
}
```

T02 (`t02_release_binary_accepts_tcp`) も同様に `#[serial(mycad_api_port_7878)]` を付与し、port_guard().lock() 行を削除する。

## 設計方針

- **決定性**: serial_test は test 実行順序を直列化するだけで結果には影響しない。決定性は維持される。
- **B-rep トポロジー妥当性**: 該当なし (API integration test の改修)。
- **derive 規約**: 該当なし。
- **エラーハンドリング**: 該当なし (test での panic 維持)。
- **workspace.dependencies**: `serial_test = "3"` を追加 (Issue 提案通り)。バージョンは crates.io 最新の 3.x stable を採用。

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_existing | 回帰 (継承) | 既存 `t01_debug_binary_logs_listening` が `#[serial]` 適用後も green | 既存 assertion |
| T02_existing | 回帰 (継承) | 既存 `t02_release_binary_accepts_tcp` が `#[serial]` 適用後も green | 既存 assertion |
| T_boundary_parallel_safety | 境界 (確認) | `cargo test -p mycad-api --test startup_log_acceptance` を 2 回連続実行しても EADDRINUSE が出ないことを CI ログで確認 | 全 test pass |

注: T01/T02 は既存テストの維持確認。新規 test は不要。T_boundary は退化/境界 ID 要件を満たすための実行手順の記録 (専用 #[test] 関数追加ではなく、CI ログによる検証)。

## 幾何的不変条件チェックリスト

- [ ] N/A (API integration test の改修。Boolean/Partition/Assemble 系の不変条件は影響しない)
