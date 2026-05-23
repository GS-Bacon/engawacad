# MyCad — Claude Code ガイドライン

## AI作業ルール

- 推測による作業を禁止。必ず検索や調査を行い、事実に基づいて作業すること。
- プランモードではユーザーの許可があるまでプランファイルの作成を禁止。
- プランモードではまず議論や壁打ちを行い、適宜質問や反論を通じて客観的な視点から議論すること。

## セッション開始時のチェックリスト

- `ROADMAP.md` を読み、現在の Phase と完了条件を把握する
- `gh issue list --state open` で open issue を確認する
- ユーザーが特定 Issue を指定しない場合は、現在 Phase の Milestone に紐づく未着手 Issue を提案する
- Issue に着手する際は commit message に `Closes #N` を含めてメインブランチへ直接 push する (branch・PR は不要)
- ただし PR を作る場合は作成直後にセルフマージしてブランチを削除する

## Project Overview

MyCadはRust製のオープンソースB-rep (Boundary Representation) CADカーネル。
設計原則: **決定性** (同じ入力は常に同じ出力)、**パラメトリック履歴** (feature historyが真実の源)、**YAML-based format** (人間可読・diff可能)。

## Project Map

```
crates/
├── mycad-kernel/     B-rep幾何カーネル
│   ├── brep/         トポロジー (Solid, Shell, Face, Loop, Edge, HalfEdge, Vertex)
│   ├── geometry/     幾何型 (Point, Vec3, Plane, Curve, Surface) + 共有数学ユーティリティ
│   ├── primitives/   プリミティブ生成 (make_cuboid)
│   └── tessellation/ メッシュ化 (tessellate_solid → TriangleMesh)
├── mycad-format/     .mycadファイルのYAMLスキーマ・パーサー
│   ├── document.rs   Document (トップレベル)
│   ├── feature.rs    Feature enum (操作履歴の1ステップ)
│   └── component.rs  Component (設計階層)
├── mycad-cli/        CLIバイナリ (mycad)
├── mycad-viewer/     3Dビューア (未実装)
└── xtask/            ビルド自動化タスク
```

## Build & Test Commands

```bash
cargo build --workspace          # 全クレートビルド
cargo test --workspace           # 全テスト実行
cargo clippy --workspace -- -D warnings  # lint
cargo fmt --all -- --check       # フォーマットチェック
cargo xtask ci                   # 上記すべてを順次実行
```

## Conventions

- **決定性**: すべてのEntityIDは `IdGenerator` で決定的に生成。テストでは決定性を検証すること。
- **依存管理**: 新しい依存は必ず `[workspace.dependencies]` に追加し、各クレートは `{ workspace = true }` で参照。
- **derive規約**: 公開型には `Debug, Clone, Serialize, Deserialize` を付与。スキーマ生成が必要な型は `JsonSchema` も追加。
- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
- **モジュール構造**: 1概念1ファイル。重要な型は `mod.rs` から `pub use` で再エクスポート。
- **テスト**: 単体テストは `#[cfg(test)] mod tests` でインライン。統合テストは `tests/` ディレクトリ。

## Architecture Principles

- **Index-based topology**: `Solid` 内のトポロジーエンティティはフラット配列に格納し、インデックスで参照（ポインタ不使用）。
- **Feature history = source of truth**: `mycad-format` の Feature 列が設計の真実の源。B-rep は Feature から再生成される。
- **Kernel has zero rendering deps**: カーネルはレンダリング非依存。`TriangleMesh` を生成し、ビューアが消費する。
- **Flat crates layout**: `crates/` 直下にフラットに配置。深いネストは作らない。
