# F01 修正: /assets/* 未解決パスを 404 に戻す

## 問題

`crates/mycad-api/src/static_assets.rs` の `static_handler` が、
`/assets/...` 等の拡張子付き未解決パスに対して `index.html` を 200 で返す SPA fallback を
行っている。合意済みの T04 (`GET /assets/<実在しないファイル> → 404`) に違反し、
Vite アセット欠落・不整合を隠してしまう。

## 修正方針

`static_handler` の fallback ロジックを以下のルールに変更する:

1. パスが `/` または拡張子を持たない(`/foo`, `/bar/baz`) → `index.html` を 200(SPA ルート)
2. パスが `/assets/...` など **拡張子付き**、かつ embedded asset に存在しない → `StatusCode::NOT_FOUND`(404)
3. embedded asset に存在するパスはそのまま配信(変更なし)

判定方法例:
```rust
let path = uri.path().trim_start_matches('/');
// 拡張子の有無で SPA ルートと静的ファイルを区別
let is_file_request = std::path::Path::new(path).extension().is_some();
match WebAssets::get(if path.is_empty() { "index.html" } else { path }) {
    Some(asset) => { /* serve asset */ }
    None if is_file_request => StatusCode::NOT_FOUND.into_response(),
    None => { /* SPA fallback: serve index.html */ }
}
```

## テスト修正

`crates/mycad-api/tests/static_assets.rs` の `t04_missing_asset_falls_back` を
200 assert から **404 assert** に変更する。

## 対象ファイル

- `crates/mycad-api/src/static_assets.rs` — fallback ロジック修正
- `crates/mycad-api/tests/static_assets.rs` — t04 の assert 修正

修正後 `cargo xtask ci` が green になること。
