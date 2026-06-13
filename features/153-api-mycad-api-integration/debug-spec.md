# Debug Spec for #153 (Codex round 1)

## F01 (high): #[serial] はプロセス内のみ、別 integration test binary 間は直列化されない

**指摘**: `crates/mycad-api/tests/startup_log_acceptance.rs` の T01/T02 と `crates/mycad-api/tests/mycad_api_serial_acceptance.rs` の `t_boundary_parallel_safety` は別 binary なので、`#[serial(mycad_api_port_7878)]` では並列実行で EADDRINUSE が再発する。

**修正方針 (Codex 提案 + Issue 案 A の意図)**:

`serial_test` の `file_locks` feature を有効化し、port 7878 を握る全 integration test を `#[file_serial(mycad_api_port_7878)]` に置き換える。file-lock ベースなのでプロセス間 (binary 間) でも直列化される。

**やること**:

### 1. `Cargo.toml` (workspace ルート) の `[workspace.dependencies]` を修正

**before**:
```toml
serial_test = "3"
```

**after**:
```toml
serial_test = { version = "3", features = ["file_locks"] }
```

### 2. `crates/mycad-api/tests/startup_log_acceptance.rs` を修正

**before** (line ~1):
```rust
use serial_test::serial;
```

**after**:
```rust
use serial_test::file_serial;
```

**before** (各 test 関数):
```rust
#[test]
#[serial(mycad_api_port_7878)]
fn t01_startup_log() { ... }

#[test]
#[serial(mycad_api_port_7878)]
fn t02_release_bin_runs() { ... }
```

**after**:
```rust
#[test]
#[file_serial(mycad_api_port_7878)]
fn t01_startup_log() { ... }

#[test]
#[file_serial(mycad_api_port_7878)]
fn t02_release_bin_runs() { ... }
```

### 3. `crates/mycad-api/tests/mycad_api_serial_acceptance.rs` を修正

**before**:
```rust
use serial_test::serial;
use std::net::TcpListener;

#[test]
#[serial(mycad_api_port_7878)]
fn t_boundary_parallel_safety() {
    let listener = TcpListener::bind("127.0.0.1:7878")
        .expect("port 7878 should be bindable when serial_test serializes access");
    drop(listener);
}
```

**after**:
```rust
use serial_test::file_serial;
use std::net::TcpListener;

#[test]
#[file_serial(mycad_api_port_7878)]
fn t_boundary_parallel_safety() {
    let listener = TcpListener::bind("127.0.0.1:7878")
        .expect("port 7878 should be bindable when file_serial serializes access across test binaries");
    drop(listener);
}
```

## 試した修正と結果
- (初回ループのため空)

## 次にやること

1. 上記 3 ファイルを修正
2. `cargo xtask ci` 全 green を確認 (特に `cargo test -p mycad-api` で startup_log_acceptance と mycad_api_serial_acceptance が並列でも EADDRINUSE しないこと)

## 追加で書いてほしいテスト

- なし (既存テストの属性置換のみ)
