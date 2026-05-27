# 最終レビュー修正: F01-F04 フォーカス修正

`cargo xtask ci` は green 済み。以下の 4 点のみを修正してください。プラン外の変更は一切禁止。

---

## F01: tower dev-dep に `features = ["util"]` 追加

**ファイル**: `crates/mycad-api/Cargo.toml`

`[dev-dependencies]` の tower を以下に変更:
```toml
tower = { workspace = true, features = ["util"] }
```

---

## F02 (最重要): Host ヘッダ検証 middleware + T06 修正

### router.rs に loopback-only middleware を追加

**ファイル**: `crates/mycad-api/src/router.rs`

`app()` に axum middleware を追加して `Host` ヘッダが `127.0.0.1:3000` / `localhost:3000` / `127.0.0.1` / `localhost` 以外なら `403 Forbidden` を返すようにする。

実装方針:
- `axum::middleware::from_fn` を使い、`Host` ヘッダを確認する非同期関数を定義する。
- `Host` ヘッダが無いか、loopback 以外の値なら `StatusCode::FORBIDDEN` を `Response` として返す。
- middleware を `Router::layer` で全ルートに適用する。

```rust
// router.rs のイメージ（実装は GLM が判断）
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;

async fn host_guard<B>(req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
    let host = req.headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let host_base = host.split(':').next().unwrap_or("");
    if matches!(host_base, "127.0.0.1" | "localhost" | "") {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

pub fn app() -> Router {
    Router::new()
        .nest("/api/v0", Router::new().route("/mesh", get(get_mesh)))
        .layer(middleware::from_fn(host_guard))
}
```

### T06 の期待値を 403 に修正

**ファイル**: `crates/mycad-api/tests/mesh_api.rs`

`t06_host_header_rebinding` を以下に変更:
- コメント「Without explicit host validation middleware...」を削除
- `assert_eq!(resp.status(), StatusCode::OK)` を `assert_eq!(resp.status(), StatusCode::FORBIDDEN)` に変更

---

## F03: ApiError を thiserror ベースに変更

**ファイル**: `crates/mycad-api/Cargo.toml`

`[dependencies]` に追加:
```toml
thiserror = { workspace = true }
```

**ファイル**: `crates/mycad-api/src/error.rs`

`ApiError` を `#[derive(Debug, thiserror::Error)]` に変更。各バリアントに `#[error("...")]` を付ける。手書き `Display` impl を削除。`std::error::Error` は thiserror が自動実装するので不要。

---

## F04: MeshQuery を非公開に変更

**ファイル**: `crates/mycad-api/src/handler.rs`

`pub struct MeshQuery` → `struct MeshQuery`(pub を外す)。内部専用型なので非公開が正しい。

---

## 完了条件

- 上記4点のみを変更する
- `cargo xtask ci` が green(全テスト通過、clippy -D warnings 通過、fmt check 通過)
- T06 が `StatusCode::FORBIDDEN` を返すことを確認
