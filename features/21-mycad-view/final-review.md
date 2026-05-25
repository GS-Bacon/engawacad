issues:
  - id: F01
    severity: high
    file: "crates/mycad-api/src/static_assets.rs"
    line_hint: 35
    finding: "未知の `/assets/...` 要求を SPA ルート扱いして `index.html` を 200 で返している。合意済み T04 (`GET /assets/<実在しないファイル> -> 404`) に反し、Vite の JS/CSS アセットが欠落・不整合でも HTML を返して破損を隠してしまう。`crates/mycad-api/tests/static_assets.rs` もこの誤挙動を 200 で固定している。"
    suggestion: "SPA fallback は実アセットではないルートに限定し、`/assets/*` あるいは拡張子付きの未解決パスは `StatusCode::NOT_FOUND` を返す。あわせて `t04_missing_asset_falls_back` を 404 assert に修正する。"
  - id: F02
    severity: medium
    file: "crates/mycad-api/build.rs"
    line_hint: 15
    finding: "release 埋め込み判定が `index.html` の stub sentinel 有無だけで、`.gitignore` 対象の `web/dist/` が現在の `web/src` / `web/public` / `web/src/generated` / `vite.config.ts` に対応しているかを検証していない。結果として、同じコミットでも手元の古い `web/dist` を埋め込んだ別バイナリを生成できる。"
    suggestion: "`cargo xtask web` で build stamp/manifest を出力し `build.rs` で検証するか、少なくとも frontend source/config/generated の freshness を `web/dist` と照合して stale dist の release build を拒否する。"

verdict: fail