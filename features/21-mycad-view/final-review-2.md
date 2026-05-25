issues:
  - id: F01
    severity: high
    file: "crates/mycad-api/src/router.rs"
    line_hint: 26
    finding: "`.nest(\"/api/v0\", ...)` に専用 fallback が無いまま外側で `.fallback(static_handler)` を付けているため、Axum の仕様上 `/api/v0/<未知パス>` は静的 fallback を継承する。結果として `/api/v0/unknown` や `/api/v0/mesh/` が 404 JSON ではなく `index.html` の 200 になり、API 契約を壊す。"
    suggestion: "`/api/v0` 側に明示的な 404 fallback を持たせて静的 fallback を継承させない。あわせて `GET /api/v0/unknown` と `GET /api/v0/mesh/` が 404 になる統合テストを追加する。"
  - id: F02
    severity: high
    file: "crates/mycad-api/build.rs"
    line_hint: 12
    finding: "release 埋め込み判定が `web/dist/index.html` の stub sentinel しか見ておらず、`web/src`・`web/src/generated`・`web/public`・`vite.config.ts` と `web/dist` の整合を検証していない。同じコミットでも、手元に残った古い `web/dist` を埋めたバイナリと最新 `vite build` 後のバイナリが両立し、決定性を壊す。"
    suggestion: "`cargo xtask web` で frontend source/config/generated を含む build stamp/manifest を出力し、`build.rs` でそれを必須検証する。少なくとも stale `web/dist` を release build で拒否する。"
  - id: F03
    severity: low
    file: "features/20-ts-frontend-skeleton/state.json"
    line_hint: 1
    finding: "Issue 21 の viewer 実装差分に、無関係な `features/20-ts-frontend-skeleton/state.json` の `merge` 状態更新が混入している。機能差分と運用メタデータが同じ変更集合に入っており、スコープがぶれている。"
    suggestion: "この state 更新は別コミットに分離するか、viewer 実装レビュー対象から外す。"

verdict: fail