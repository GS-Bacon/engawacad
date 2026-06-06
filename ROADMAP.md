# MyCad Roadmap

このドキュメントはユーザーから見た機能単位でフェーズを定義する。技術選定の理由は [`docs/decisions/`](docs/decisions/) の ADR に記録する。

各フェーズの実装タスクは [GitHub Issues](https://github.com/GS-Bacon/mycad/issues) で管理し、1 フェーズ = 1 Milestone に対応する。Issue は着手するフェーズのものだけを作成する (空想 Issue は作らない)。マイルストーン運用ルール (type ラベル・Phase 完了判定・差し込み作業の扱い) は [ADR-002](docs/decisions/002-roadmap-management.md) を参照。

---

## Phase 0: 基盤 ✅

**外から見た成果**: 無し (内部スケルトンのみ)

**完了条件**:
- `cargo xtask ci` が通る (fmt → clippy → test → build)
- B-rep トポロジー型 (`Solid`/`Shell`/`Face`/`Loop`/`Edge`/`HalfEdge`/`Vertex`) が実装済み
- YAML ベースの `.mycad` フォーマット (`Document` / `Feature` / `Component`) が実装済み
- `make_cuboid` → `tessellate_solid` のパイプラインが動作し、決定性テストが通る

**状態**: 完了 (2025)

---

## Phase 1: `.mycad` から STL を出力できる ✅

**外から見た成果**: コマンドラインから `.mycad` ファイルを STL に変換できる

```bash
mycad export examples/simple_box.mycad -o box.stl
```

**完了条件**:
- `mycad export <input.mycad> -o <output.stl>` が動作する
- 出力 STL が Blender / MeshLab 等で読み込める
- `examples/simple_box.mycad` が変換できる (CreateBox のみ)

**Issues**: [Milestone: Phase 1](https://github.com/GS-Bacon/mycad/milestone/2)

**状態**: 完了 (2026-05-23)

---

## Phase 2: Web ベースビューア基盤 ✅

**前提 ADR**: [ADR-003](docs/decisions/003-viewer-and-app-architecture.md)

**外から見た成果**: ブラウザで `.mycad` の形状を 3D でリアルタイム確認できる

```bash
mycad view examples/simple_box.mycad   # ローカルサーバを起動してブラウザを開く
```

**完了条件**:
- `mycad view <input.mycad>` がローカルサーバを起動しブラウザを自動オープンする
- ブラウザ上で box がマウスで回転・ズームできる
- Rust API 層 (HTTP サーバ) と TS フロント (Three.js) の骨組みが動作する
- Rust → TS 型自動生成パイプラインが確立している

**備考**: Tauri によるネイティブデスクトップアプリ化・server モードでの self-hosted multi-user 利用は将来の発展。Issue 化は当該 Phase 着手時。

**Issues**: [Milestone: Phase 2](https://github.com/GS-Bacon/mycad/milestone/3)

**状態**: 完了 (2026-05-26)

---

## Phase 3: 円柱・球・押し出しが作れる ✅

**前提 ADR**: [ADR-005](docs/decisions/005-topological-naming.md) — `Extrude` (スケッチ→ソリッド) 着手前に決定必須

**外から見た成果**: box 以外の基本形状を `.mycad` で記述できる

**完了条件**:
- `CreateCylinder` / `CreateSphere` が `.mycad` から geometry 生成まで動作する
- `Extrude` (スケッチ → ソリッド) が動作する
- Phase 1 の `export` と Phase 2 の `view` で確認できる

**状態**: 完了 (2026-05-27)

---

## ✅ Phase 4: Boolean 演算ができる

**外から見た成果**: 形状の足し引きを `.mycad` で記述できる

**完了条件**:
- `Cut` / `Fuse` / `Intersect` が `.mycad` から動作する
- 演算結果が `export` / `view` で確認できる

**備考**: 曲面同士の Boolean は交線として自由曲線を、フィレット・面取りは自由曲面を必要とする (交線は一般に円・直線でない)。自由曲面/NURBS は専用 Phase を立てず、本 Phase が要求する範囲から漸進的に導入する。数値モデル (トレラント vs 厳密) もここで決定する。詳細は ADR-004 参照。

**状態**: 完了 (2026-06-04)

---

## ✅ Phase 5: アセンブリと部品参照

**外から見た成果**: 複数の部品を組み合わせ、標準ライブラリ部品を参照できる

**完了条件**:
- `stdlib://` 参照が解決される
- Component 階層の transform が正しく適用される
- `examples/assembly.mycad` が動作する

**状態**: 完了 (2026-06-06)

---

## Phase 6: STEP export (低優先度)

**外から見た成果**: CAD 業界標準の STEP フォーマットで出力できる

```bash
mycad export model.mycad -o model.step
```

**完了条件**:
- `mycad export ... -o <output.step>` が動作する
- 出力 STEP が主要 CAD ソフトで読み込める

**備考**: STEP (ISO 10303) は仕様規模が大きく Rust エコシステムも限定的なため、Phase 1 で STL パイプラインを確立してから着手する。詳細は ADR-002 参照。
