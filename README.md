# EngawaCAD

オープンソースの B-rep (Boundary Representation) CAD カーネル。Rust 製。

## Features

- **B-rep カーネル**: Solid, Shell, Face, Edge, Vertex によるトポロジー表現
- **パラメトリック履歴**: Feature ベースのモデリング（操作履歴が設計の源）
- **YAML フォーマット**: 人間可読・diff 可能な `.engawa` ファイル形式
- **決定的**: 同じ入力は常に同じ出力を生成
- **テッセレーション**: B-rep からトライアングルメッシュへの変換

## Quick Start

```bash
# ビルド
cargo build --release

# .engawa ファイルを STL に変換
./target/release/engawa export examples/simple_box.engawa -o box.stl
```

### Development

```bash
# テスト
cargo test --workspace

# CI チェック（fmt + clippy + test + build）
cargo xtask ci
```

## Roadmap

開発はフェーズ単位で進めています。現状のフェーズと今後の予定は [`ROADMAP.md`](ROADMAP.md) を参照してください。

- ✅ Phase 0: 基盤 (B-rep カーネル + YAML フォーマット)
- ✅ Phase 1: `.engawa` から STL を出力
- ⏳ Phase 2 以降: 3D Viewer / Boolean 演算 / アセンブリ / STEP export

## Project Structure

```
crates/
├── engawa-kernel/     B-rep 幾何カーネル
├── engawa-format/     .engawa ファイルのスキーマ・パーサー
├── engawa-cli/        コマンドラインインターフェース
├── engawa-viewer/     3D ビューア（開発中）
└── xtask/            ビルド自動化タスク
```

## License

MIT OR Apache-2.0
