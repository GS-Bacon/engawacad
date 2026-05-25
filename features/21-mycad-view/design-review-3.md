issues:
  - id: R01
    severity: high
    section: "設計方針 > 決定性"
    finding: "`bind_listener` の固定順次スキャンが `u16` 上限を考慮しておらず、`--port 65535` 近傍で `port+1` 以降の計算が debug では panic、release では wrap になり得る。公開 URL が profile 依存になり、同一入力で同一出力を壊す"
    suggestion: "`checked_add` で探索範囲を計算し、65535 を超える場合は明示エラーにする。`65535` 近傍と 16 ポート全占有のテストを追加する"
  - id: R02
    severity: high
    section: "実装対象 > crates/xtask"
    finding: "`cargo xtask web` を『vite build のみ』にすると、Rust 側の型変更後でも `web/src/generated` を再生成せずに `dist` を作れてしまう。設計書が release 前提コマンドとして案内している経路自体が stale な API 契約を埋め込める"
    suggestion: "`cargo xtask web` は少なくとも `gen-ts` を先行実行し、生成物に差分があれば fail させた上で `vite build` を行う"
  - id: R03
    severity: medium
    section: "設計方針 > embed のビルド依存問題と対策"
    finding: "`build.rs` の release ガードは『stub かどうか』しか見ておらず、`web/src`・`web/public`・`vite.config.*` 変更後に古い `web/dist` が残っているケースは検出できない。`rerun-if-changed=../../web/dist` だけでは release artifact とソースツリーの整合が保証されない"
    suggestion: "`cargo xtask web` が build stamp/manifest を出力し `build.rs` でそれを検証するか、少なくとも frontend source/config/generated を freshness 判定に含めて stale `dist` を release で拒否する"

verdict: fail