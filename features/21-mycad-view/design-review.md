issues:
  - id: R01
    severity: high
    section: "設計方針 > embed のビルド依存問題と対策"
    finding: "`rust-embed` の release 埋め込み内容が git 管理外の `web/dist` の事前状態に依存しており、同じコミットでも『stub だけを埋めた binary』と『実フロントを埋めた binary』の両方が生成されうる。計画は注意書きで回避する前提だが、現行 `cargo xtask ci` も Rust build を `vite build` より先に実行するため、この非決定性と配布破綻を自動で防げない。"
    suggestion: "`web/dist` 生成を release/package 経路の前提として機械的に強制すること。例: `xtask` で `vite build` 後に release build を行う、または `build.rs` で release 時に stub しか無い場合は失敗させる。"
  - id: R02
    severity: medium
    section: "テスト計画 > host_guard"
    finding: "新規 static fallback に対する `Host` 検証テストが無い。既存 `/api/v0/mesh` の 403 テストだけでは、`.fallback(static_handler)` の配線ミスで静的ルートだけ localhost 制約を外しても検出できない。"
    suggestion: "`GET /` または `/assets/...` に `Host: evil.com` を付けた統合テストを追加し、静的配信も API と同じ guard 下にあることを固定すること。"
  - id: R03
    severity: medium
    section: "テスト計画 > 静的配信"
    finding: "T03 は `GET /` の 200 と `content-type` しか見ないため、`index.html` が空、常に stub、または誤った HTML を返していても通る。今回の中核契約である『未ビルド時は frontend not built 案内、ビルド済みなら viewer HTML』を自動検証できていない。"
    suggestion: "stub 経路では body に `frontend not built` を含むことを assert し、可能なら web build 後の smoke test で返却 HTML が実アセット参照を持つことも確認すること。"

verdict: fail