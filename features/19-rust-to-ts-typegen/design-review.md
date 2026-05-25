issues:
  - id: R01
    severity: high
    section: "実装対象 > workspace 依存追加 + MSRV 引き上げ"
    finding: "`[workspace.package].rust-version` を `1.88` に上げるだけでは不十分。現状の member `Cargo.toml` はどれも `rust-version.workspace = true` を持っておらず、MSRV が各 crate に継承されない。加えて `rust-toolchain.toml` は実際には `channel = \"stable\"` で、設計書の『stable=1.92 で既に充足』前提とも一致していない。"
    suggestion: "全 member crate に `rust-version.workspace = true` を追加するか各 crate で `rust-version = \"1.88\"` を明示し、必要なら `rust-toolchain.toml` も実際の検証対象に合わせて更新すること。"
  - id: R02
    severity: high
    section: "実装対象 > xtask gen-ts サブコマンド / アーキテクチャ整合性"
    finding: "各公開型に `#[ts(export, export_to = \"../../web/src/generated\")]` を埋め込む案は、`mycad-format` と `mycad-kernel` のコア型が `web/` のディレクトリ構造を直接知る設計になる。これは型定義をフロント配置に結合し、層分離と再利用性を崩す。"
    suggestion: "出力先パスは `xtask` 側の export ドライバか専用 export モジュール/crate に閉じ込め、コア crate の型定義から `web/src/generated` を参照しない構成にすること。"
  - id: R03
    severity: medium
    section: "実装対象 > workspace 依存追加 + MSRV 引き上げ / 決定性"
    finding: "`ts-rs` の API と生成結果を『pin した 12.0.1』前提で exact golden 比較まで行う設計なのに、依存指定は `version = \"12\"` で 12.x に浮動している。patch 更新で export API 名や出力整形が変わると、設計が期待する再現性とテスト安定性が崩れる。"
    suggestion: "12.0.1 前提で進めるなら `=12.0.1` まで固定するか、patch 更新を許容するなら golden を全文一致ではなく意味的アサーション中心に切り替えること。"
  - id: R04
    severity: medium
    section: "実装対象 > mycad-api transport DTO / アーキテクチャ整合性"
    finding: "新規 `crates/mycad-api/src/transport.rs` に `MeshRequest` と `ErrorResponse` を同居させる案は、`CLAUDE.md` の『1概念1ファイル。重要な型は `mod.rs` から `pub use`』規約と矛盾している。"
    suggestion: "`mesh_request.rs` と `error_response.rs` に分割し、`lib.rs` か `transport/mod.rs` を薄い再エクスポート層にすること。"
  - id: R05
    severity: medium
    section: "テスト計画"
    finding: "`EntityRef` は『どの root からも推移参照されないので明示 root に含める』と設計で特別扱いしているのに、テスト計画には `EntityRef.ts` の生成有無や内容を直接検証するケースがない。T06 では最も抜けやすい explicit root の取りこぼしを検出できない。"
    suggestion: "`EntityRef.ts` の存在と exact/golden 内容を確認する専用テストを追加し、必要なら生成ファイル一覧自体も固定すること。"

verdict: fail