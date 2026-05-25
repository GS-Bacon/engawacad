# セキュリティ修正: ファイルパスをサーバ State に移動、ブラウザ/ネットワークに非公開

## 目的
`?file=` クエリパラメータを廃止し、`mycad view` 起動時に指定したファイルパスを
axum State としてサーバ内部に閉じ込める。ブラウザ/ネットワーク上にファイルパスを
一切露出しない。

## 変更1: crates/mycad-api/src/handler.rs

`get_mesh` の引数を `Query<MeshRequest>` から `State<Arc<PathBuf>>` に変更する。

```rust
use std::path::PathBuf;
use std::sync::Arc;
use axum::extract::State;

pub(crate) async fn get_mesh(
    State(file): State<Arc<PathBuf>>,
) -> Result<Json<TriangleMesh>, ApiError> {
    let path = file.as_path();
    // is_relative チェック不要 (CLI が canonicalize 済み)
    let doc = Document::from_path(path)?;
    // ... 残りは既存と同じ
}
```

`MeshRequest` の import/使用はすべて削除する。

## 変更2: crates/mycad-api/src/router.rs

`app()` → `app(file: Arc<PathBuf>) -> Router` に変更。State を注入する。

```rust
pub fn app(file: Arc<PathBuf>) -> Router {
    let api = Router::new()
        .route("/mesh", get(get_mesh))
        .fallback(api_not_found)
        .with_state(file);  // State を /api/v0 内に閉じ込める

    Router::new()
        .nest("/api/v0", api)
        .fallback(static_handler)
        .layer(middleware::from_fn(host_guard))
}
```

## 変更3: crates/mycad-api/src/main.rs (standalone binary)

standalone の `mycad-api` バイナリは開発用途のみ。
コマンドライン引数 `--file <path>` を追加して起動時にファイルを指定できるようにする。
引数なしで起動した場合はエラーメッセージを表示して終了する。

```rust
// 例
let file = std::env::args().nth(1)
    .map(std::path::PathBuf::from)
    .and_then(|p| std::fs::canonicalize(p).ok())
    .expect("Usage: mycad-api <path-to-file.mycad>");
let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
axum::serve(listener, app(Arc::new(file))).await.unwrap();
```

## 変更4: crates/mycad-cli/src/view.rs

`axum::serve` の引数を `app()` から `app(Arc::new(abs_path.clone()))` に変更する。

```rust
axum::serve(listener, mycad_api::router::app(Arc::new(abs_path.clone())))
    .await
    .map_err(|e| format!("server error: {e}"))
```

## 変更5: テスト修正

`crates/mycad-api/tests/mesh_api.rs` など `?file=` クエリを使っているテストを修正:
- `app()` → `app(Arc::new(PathBuf::from("fixtures/...")))` 等に変更
- fixture の絶対パスは `std::fs::canonicalize` で取得するか、test 内で組み立てる

## 完了条件
- `cargo build --workspace` が通る
- `cargo test --workspace` が通る
- `MeshRequest` の import エラーなし
