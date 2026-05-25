issues:
  - id: R01
    severity: high
    section: "設計方針 > embed のビルド依存問題と対策"
    finding: "`build.rs` の説明に `cargo:rerun-if-changed` が無く、埋め込み元の `web/dist` が crate 外にあるため、フロント資産を更新しても `mycad-api`/`mycad-cli` の再コンパイルが走らず、release バイナリへ古いアセットや初回スタブを埋め込む恐れがある"
    suggestion: "`build.rs` で `web/dist/` を明示的に `rerun-if-changed` 対象にし、可能なら埋め込み対象を `OUT_DIR` にコピーして Cargo 依存を明確化すること"
  - id: R02
    severity: high
    section: "テスト計画"
    finding: "提案された `cargo xtask ci` は debug の `cargo build --workspace` までしか通さず、`PROFILE=release` 分岐の build guard と release 時の `rust-embed` 実埋め込み経路を自動検証していない。R01 対策の成否を CI で保証できない"
    suggestion: "`cargo xtask ci` か別タスクで `cargo xtask web` の後に `cargo build -p mycad-cli --release` を必須化し、release 経路で `GET /` が実アセットを返す smoke テストも追加すること"
  - id: R03
    severity: medium
    section: "テスト計画 > 正常系"
    finding: "`T03` は `/` の `index.html` しか見ておらず、viewer 起動に必須な `/assets/...` の実在ファイル配信成功を自動検証していない。`static_handler` のパス解決や MIME 設定を誤っても CI で見逃す"
    suggestion: "`index.html` から実際の `script src` を抽出して `GET` し、200 と妥当な JavaScript content-type を確認するテストを追加すること"
  - id: R04
    severity: medium
    section: "設計方針 > 決定性"
    finding: "`bind_listener` が `AddrInUse` 時に `127.0.0.1:0` へフォールバックするため、`mycad view <same file>` の公開 URL は同一入力でも実行ごとに変わりうる。設計文は決定性を `build_view_url` 単体に限定しており、コマンド全体の非決定的挙動を見落としている"
    suggestion: "決定性を重視するなら既定動作はポート競合をエラーにするか、固定順序のポート探索にすること。少なくとも『ポート占有時は URL が非決定になる』ことを設計上明記すること"

verdict: fail