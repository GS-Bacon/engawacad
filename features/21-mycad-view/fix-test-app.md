# テスト修正: app() → app(Arc<PathBuf>) 対応

## 問題

`app()` が `app(file: Arc<PathBuf>)` に変わったため、引数なし呼び出しのテストが
全てコンパイルエラーになっている。

## 修正方針

各テストファイルの先頭に `fn test_app() -> axum::Router` ヘルパを追加し、
全ての `app()` 呼び出しを `test_app()` に置換する。

### 静的配信テスト (static_assets.rs, static_assets_edge.rs, api_fallback.rs, api_fallback_edge.rs)

これらのテストはメッシュエンドポイントを呼ばないため、ファイルパスは何でもよい。
各ファイルの import 部に以下を追加:

```rust
use mycad_api::router::app;
use std::path::PathBuf;
use std::sync::Arc;

fn test_app() -> axum::Router {
    app(Arc::new(PathBuf::from("/dev/null")))
}
```

そして各テスト内の `app()` を `test_app()` に置換する。
`make_app()` が既に定義されている場合はその内部を
`app(Arc::new(PathBuf::from("/dev/null")))` に変更する。

### メッシュテスト (mesh_api.rs)

メッシュテストは実際のファイルが必要。
fixture ファイルへの絶対パスを使う:

```rust
fn test_app_with_fixture(name: &str) -> axum::Router {
    let path = std::fs::canonicalize(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    ).expect("fixture not found");
    app(Arc::new(path))
}
```

テスト内で `app()` を呼んでいた箇所は `test_app_with_fixture("simple_box.mycad")` 等に変更。
fixture に `simple_box.mycad` が無い場合は既存の fixture ファイル名を使う。

## 確認

`cargo test --workspace` が通ること。
