# MyCad

オープンソースの B-rep (Boundary Representation) CAD カーネル。Rust 製。

## Features

- **B-rep カーネル**: Solid, Shell, Face, Edge, Vertex によるトポロジー表現
- **パラメトリック履歴**: Feature ベースのモデリング（操作履歴が設計の源）
- **YAML フォーマット**: 人間可読・diff 可能な `.mycad` ファイル形式
- **決定的**: 同じ入力は常に同じ出力を生成
- **テッセレーション**: B-rep からトライアングルメッシュへの変換

## Quick Start

```bash
# ビルド
cargo build --workspace

# テスト
cargo test --workspace

# CI チェック（fmt + clippy + test + build）
cargo xtask ci
```

## Project Structure

```
crates/
├── mycad-kernel/     B-rep 幾何カーネル
├── mycad-format/     .mycad ファイルのスキーマ・パーサー
├── mycad-cli/        コマンドラインインターフェース
├── mycad-viewer/     3D ビューア（開発中）
└── xtask/            ビルド自動化タスク
```

## License

MIT OR Apache-2.0
