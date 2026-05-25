# ネットワーク公開対応: 0.0.0.0 バインド + host_guard IP 許可

## 目的
Tailscale IP (10.13.1.1) やローカルネットワークからもアクセスできるようにする。

## 変更1: crates/mycad-cli/src/view.rs

`bind_listener` のバインドアドレスを `127.0.0.1` から `0.0.0.0` に変更する。

```rust
// 変更前
tokio::net::TcpListener::bind(("127.0.0.1", port)).await

// 変更後
tokio::net::TcpListener::bind(("0.0.0.0", port)).await
```

また `run_view` で起動後に表示するメッセージにネットワーク経由でのアクセス方法を示す行を追加:

```
MyCad viewer: http://127.0.0.1:{port}/?file=...  (local)
              http://0.0.0.0:{port}  is listening on all interfaces
Press Ctrl-C to stop.
```

## 変更2: crates/mycad-api/src/router.rs

`host_guard` を、IP アドレス形式のホスト(10.13.1.1 等)を全て許可するよう拡張する。

DNS リバインディング攻撃はドメイン名を使うため、数値 IP を Host に使うアクセスは安全。

```rust
fn is_allowed_host(host_base: &str) -> bool {
    // 空文字・localhost・IPv4/IPv6 (数字・ドット・コロン・角括弧のみ) を許可
    host_base.is_empty()
        || host_base == "localhost"
        || host_base.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':' || c == '[' || c == ']')
}

// host_guard 内:
if is_allowed_host(host_base) {
    Ok(next.run(req).await)
} else {
    Err(StatusCode::FORBIDDEN)
}
```

## テスト修正

既存の `t08_static_assets_host_guard_*` テストは `Host: evil.com` → 403 を検証済み。
追加で `Host: 10.13.1.1` → 通過するテストを追加する。

## 完了条件
- `cargo build -p mycad-cli` が通る
- `cargo test -p mycad-api` が通る
