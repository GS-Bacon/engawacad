# EngawaCAD Roadmap

このドキュメントはユーザーから見た機能単位でフェーズを定義する。技術選定の理由は [`docs/decisions/`](docs/decisions/) の ADR に記録する。

各フェーズの実装タスクは [GitHub Issues](https://github.com/GS-Bacon/engawacad/issues) で管理し、1 フェーズ = 1 Milestone に対応する。Issue は着手するフェーズのものだけを作成する (空想 Issue は作らない)。マイルストーン運用ルール (type ラベル・Phase 完了判定・差し込み作業の扱い) は [ADR-002](docs/decisions/002-roadmap-management.md) を参照。

---

## Phase 0: 基盤 ✅

**外から見た成果**: 無し (内部スケルトンのみ)

**完了条件**:
- `cargo xtask ci` が通る (fmt → clippy → test → build)
- B-rep トポロジー型 (`Solid`/`Shell`/`Face`/`Loop`/`Edge`/`HalfEdge`/`Vertex`) が実装済み
- YAML ベースの `.engawa` フォーマット (`Document` / `Feature` / `Component`) が実装済み
- `make_cuboid` → `tessellate_solid` のパイプラインが動作し、決定性テストが通る

**状態**: 完了 (2025)

---

## Phase 1: `.engawa` から STL を出力できる ✅

**外から見た成果**: コマンドラインから `.engawa` ファイルを STL に変換できる

```bash
engawa export examples/simple_box.engawa -o box.stl
```

**完了条件**:
- `engawa export <input.engawa> -o <output.stl>` が動作する
- 出力 STL が Blender / MeshLab 等で読み込める
- `examples/simple_box.engawa` が変換できる (CreateBox のみ)

**Issues**: [Milestone: Phase 1](https://github.com/GS-Bacon/engawacad/milestone/2)

**状態**: 完了 (2026-05-23)

---

## Phase 2: Web ベースビューア基盤 ✅

**前提 ADR**: [ADR-003](docs/decisions/003-viewer-and-app-architecture.md)

**外から見た成果**: ブラウザで `.engawa` の形状を 3D でリアルタイム確認できる

```bash
engawa view examples/simple_box.engawa   # ローカルサーバを起動してブラウザを開く
```

**完了条件**:
- `engawa view <input.engawa>` がローカルサーバを起動しブラウザを自動オープンする
- ブラウザ上で box がマウスで回転・ズームできる
- Rust API 層 (HTTP サーバ) と TS フロント (Three.js) の骨組みが動作する
- Rust → TS 型自動生成パイプラインが確立している

**備考**: Tauri によるネイティブデスクトップアプリ化・server モードでの self-hosted multi-user 利用は将来の発展。Issue 化は当該 Phase 着手時。

**Issues**: [Milestone: Phase 2](https://github.com/GS-Bacon/engawacad/milestone/3)

**状態**: 完了 (2026-05-26)

---

## Phase 3: 円柱・球・押し出しが作れる ✅

**前提 ADR**: [ADR-005](docs/decisions/005-topological-naming.md) — `Extrude` (スケッチ→ソリッド) 着手前に決定必須

**外から見た成果**: box 以外の基本形状を `.engawa` で記述できる

**完了条件**:
- `CreateCylinder` / `CreateSphere` が `.engawa` から geometry 生成まで動作する
- `Extrude` (スケッチ → ソリッド) が動作する
- Phase 1 の `export` と Phase 2 の `view` で確認できる

**状態**: 完了 (2026-05-27)

---

## ✅ Phase 4: Boolean 演算ができる

**外から見た成果**: 形状の足し引きを `.engawa` で記述できる

**完了条件**:
- `Cut` / `Fuse` / `Intersect` が `.engawa` から動作する
- 演算結果が `export` / `view` で確認できる

**備考**: 曲面同士の Boolean は交線として自由曲線を、フィレット・面取りは自由曲面を必要とする (交線は一般に円・直線でない)。自由曲面/NURBS は専用 Phase を立てず、本 Phase が要求する範囲から漸進的に導入する。数値モデル (トレラント vs 厳密) もここで決定する。詳細は ADR-004 参照。

**状態**: 完了 (2026-06-04)

---

## ✅ Phase 5: アセンブリと部品参照

**外から見た成果**: 複数の部品を組み合わせ、標準ライブラリ部品を参照できる

**完了条件**:
- `stdlib://` 参照が解決される
- Component 階層の transform が正しく適用される
- `examples/assembly.engawa` が動作する

**状態**: 完了 (2026-06-06)

---

## ✅ Phase 6: 対話編集の背骨 — 面を選んで押出/押出カット

**前提 ADR**: [ADR-008](docs/decisions/008-interactive-editing-increment.md)

**外から見た成果**: ブラウザでモデルの平面をクリック選択し、押出または押出カットで形状を足し引きでき、即座に再描画される

```
ブラウザ上で面をクリック → 深さを指定 → 押出 or 押出カット → 形状が更新される
```

**完了条件**:
- `TriangleMesh` が三角形→面の安定参照 (`EntityRef`) を保持し、API 経由でフロントに渡る
- Feature を 1 個追加する書き込みエンドポイントが動作し、再テッセレーション結果を返す
- ブラウザで平面をクリック選択 → 押出/押出カット実行 → 結果が反映される
- `Extrude`(add) と `ExtrudeCut`(remove) が別 Feature として `.engawa` に積まれる
- 決定性テストが通り、`engawa view` で確認できる

**備考**: ビューアの回転品質・カメラ制御の改善も本 Phase の Milestone Issue として着手する (`type: refactor`/`bug`、完了判定の対象外)。

**状態**: 完了 (2026-06-07)

---

## Phase 7: スケッチ描画（正準平面） ✅

**外から見た成果**: ブラウザで正準平面（xy/xz/yz）上に線分スケッチを描き、押出/押出カットできる

**完了条件**:
- ブラウザ上のスケッチキャンバスで線分を描いてプロファイルを作れる
- 描いたスケッチから `Extrude` / `ExtrudeCut` を実行できる
- 作成したスケッチが `.engawa` に `CreateSketch` Feature として記録される

**サンプル**: [`examples/sketch_via_refplane.engawa`](examples/sketch_via_refplane.engawa) — Front 参照平面上に矩形を描いて押し出す最小例。サンプル一覧は [`examples/README.md`](examples/README.md)。

**状態**: 完了 (2026-06-16)

---

## Phase 8: モデル面上のスケッチ

**外から見た成果**: モデルの任意の平面上にスケッチを描き、造形を積み上げられる

**完了条件**:
- モデル面を選択してスケッチ平面を設定できる
- その平面上でスケッチを描いて押出/押出カットできる
- トポロジカル命名（ADR-005）によりモデル再生成後も面参照が安定する

---

## Phase 8 以降の総括 — 自律実装期と UI 期

Phase 8 から先は **自律実装期 (Phase 9-20)** と **UI 期 (Phase 21-24)** の二段構成。自律実装期は CLI + YAML + 3ailoop で消化可能な「Feature 列で操作できる機能」の拡充に集中し、UI 期突入前に Quality + Refactor Pass (Phase 12 / 16 / 20) で品質基盤を固める。詳細な再設計の経緯と機能ユニバース (Solidworks / Fusion / Onshape 統合) は `/home/bacon/.claude/plans/phase8-3ailoop-intake-phase-fluttering-blossom.md` 参照。

---

## Phase 9: 履歴編集 + 変数 + CLI 拡張 + 品質基盤入口

**外から見た成果**: 作った Feature を後から編集・並べ替え・サプレスでき、変数で形を駆動できる

**完了条件**:
- Feature CRUD (Edit / Roll back / Suppress / Reorder / Delete / Insert) が `.engawa` から動作する
- Document 全域 + Sketch 内の 2 段スコープ Variable / Equation が動作する
- `engawa entry add/edit/remove/reorder/suppress` 統一 CLI が利用できる
- `schema_version` フィールドと migration hook の入口が `engawa-format` に入っている
- 品質基盤 (proptest / criterion / cargo-fuzz / cargo-llvm-cov / Playwright) の最小 setup と main の bench baseline が取得済み

**前提 ADR**: 新規 (履歴 CRUD 抽象 + Variable スコープ + schema_version + 品質基盤、CLI 命名規約)

---

## Phase 10: スケッチ基本曲線拡張

**外から見た成果**: 線分以外のスケッチ要素 (円・弧・楕円・矩形・多角形・Slot・Conic) を描ける

**完了条件**:
- 上記スケッチ要素が `.engawa` で記述・描画可能
- スケッチ編集 (Trim / Extend / Offset / Sketch Fillet / Sketch Chamfer / Mirror / Pattern) が動作

---

## Phase 11: 拘束ソルバ 基礎

**外から見た成果**: スケッチに幾何拘束・寸法拘束を付与してパラメトリックに駆動できる

**完了条件**:
- 幾何拘束 (Horizontal / Vertical / Coincident / Collinear / Parallel / Perpendicular / Tangent / Equal / Midpoint / Fix / Concentric / Symmetric / Pierce / Mirror / Merge points) が動作
- 寸法拘束 (Linear / Aligned / Angular / Radial / Diameter / Driven) が動作
- Over/Under-constrained 検出が動作
- ソルバの決定性テスト緑

**前提 ADR**: 新規 (拘束ソルバ ライブラリ方針、ADR-004 tolerant model との整合)

---

## Phase 12: Quality + Refactor Pass 1

**外から見た成果**: Phase 9-11 で蓄積した複雑さの整理と、品質基盤の本格運用 (機能追加なし)

**完了条件**:
- 直近 Phase の `needs-human` 退避バグ集中修正
- proptest / fuzz / bench リグレッション検知 / coverage 閾値 / Visual Regression Testing 本実装
- 最新ベストプラクティス調査 ADR (Pass 1)
- viewer (TypeScript 側) もテスト網羅・bench 対象

---

## Phase 13: 拘束ソルバ 拡張

**外から見た成果**: Spline (Fit/Control point) と高度な拘束 (Smooth/Curvature/Path length/Baseline/Ordinate) が使える

**完了条件**:
- Spline (Fit point / Control point) が描画・拘束可能
- Smooth (G2) / Curvature / Normal / On-curve / Coradial / Same length / Same radius が動作
- 寸法 (Baseline / Ordinate / Arc length / Path length) が動作
- Equation Driven Curve が動作

---

## Phase 14: 拡張形状操作

**外から見た成果**: Revolve / Sweep / Loft / Pattern / Mirror / Helix で複雑な形状を作れる

**完了条件**:
- Revolve / Sweep / Loft が `.engawa` から動作
- Pattern (Linear / Circular / Curve) と Mirror が動作
- Helix が動作 (Thread/Coil 自体は将来候補)

**前提 ADR**: 新規 (スケッチ → ソリッド変換規約)

---

## Phase 15: 自由曲面・面処理

**外から見た成果**: Fillet / Chamfer / Shell / Draft と、Surface workspace (Patch / Boundary / Knit / Trim / Thicken) が使える

**完了条件**:
- Fillet (Constant / Variable / Chord / Full Round) と Chamfer (Equal / Distance-distance / Angle) が動作
- Shell / Draft / Offset Face / Scale (Uniform/Non-uniform) が動作
- Surface workspace (Patch / Boundary / Ruled / Knit / Trim / Untrim / Extend / Thicken / Mid Surface) が動作
- ADR-004 で予告した自由曲面の本格導入

**前提 ADR**: 新規 (自由曲面 NURBS/Bezier 表現、面処理の数値安定性)

---

## Phase 16: Quality + Refactor Pass 2

**外から見た成果**: Phase 13-15 (拘束拡張・拡張形状・自由曲面) の整理と品質確認 (機能追加なし)

**完了条件**: Phase 12 と同じ必須項目 + 最新ベストプラクティス調査 ADR (Pass 2)

---

## Phase 17: アセンブリ Mate 基礎

**外から見た成果**: 部品同士を Mate / Joint で接合し、サブアセンブリと干渉検査ができる

**完了条件**:
- Standard Mate (Coincident / Parallel / Perpendicular / Tangent / Concentric / Lock / Distance / Angle) が動作
- Joint (Rigid / Revolute / Slider / Cylindrical / Pin-Slot / Planar / Ball) が動作
- Mate Connector / Joint Origin / As-built joint が動作
- Sub-assembly / Component Pattern (Linear / Circular) / Mirror Component が動作
- Interference / Clearance detection が動作

**前提 ADR**: 新規 (アセンブリ Mate データモデル、Mate Connector 表現)

---

## Phase 18: アセンブリ機構拘束

**外から見た成果**: Cam / Gear / Screw / Belt-Chain などの機構的接合を表現できる

**完了条件**:
- Mechanical Mate (Cam / Slot / Hinge / Gear / Rack-Pinion / Screw / Universal Joint / Belt-Chain) が動作
- Motion Link / Drive Joints / Joint Limits / Contact Set が動作

---

## Phase 19: データ連携

**外から見た成果**: STEP / IGES / DXF/DWG / GLTF / USDZ の入出力と、`.engawa` スキーマの版管理ができる

**完了条件**:
- STEP I/O ライブラリ調査 ADR → 実装 (重さに応じて縮退判断、最低 AP203 export)
- IGES export、DXF/DWG (sketch) export、GLTF/USDZ (Web 表示用) export
- Schema versioning + migration の本実装

**前提 ADR**: 新規 (STEP I/O ライブラリ調査結果と縮退方針)

---

## Phase 20: Quality + Refactor Pass 3 + UI 期前ゲート

**外から見た成果**: UI 期突入の最終ゲート。Phase 12/16 の品質手法全て + UI 期に向けた整理

**完了条件**:
- Phase 12 の必須項目 (proptest / fuzz / bench / coverage / Visual Regression / 最新調査 ADR)
- CLI コマンド体系の安定化レビュー (UI 期で wrap する前提で命名・引数・出力形式を統合)
- API ドキュメント自動生成体制 (rustdoc + CLI help → markdown)
- E2E シナリオテスト (全 examples の export ハッシュ一致 + Visual Regression)
- サンプルファイル群の完備 (全 Phase の代表 `.engawa` が examples/ にある)

**このゲートを通過しないと UI 期 (Phase 21+) に入らない**。

---

## Phase 21-24: UI 期

Phase 20 ゲート通過後に詳細を再計画する。現時点では項目名のみ:

- **Phase 21**: 編集 GUI 基盤 (Feature tree UI / inline params / 押出・押出カット分離など UI 細部)
- **Phase 22**: スケッチ拘束 GUI (拘束追加 / 寸法入力 / Over-constrained 視覚化)
- **Phase 23**: アセンブリ Mate GUI + Inspect (Mate ダイアログ / Measure / Section / Curvature 解析)
- **Phase 24**: Tauri デスクトップ化 (ローカル完結ネイティブアプリ)

---

## 将来候補（番号未確定・粗く列挙）

着手する Phase の Issue のみ作成する（空想 Issue 禁止 / ADR-002）。

- **2D Drawing**: 投影図・寸法記入・図面テンプレート
- **T-Spline (Form workspace)**: 有機的自由曲面のスカルプティング
- **Cloud collab / Multi-user editing**: Onshape 風の同時編集 (Versions/Branches ネイティブ管理含む — 現状は `.engawa` YAML + git で代替)
- **Custom Feature DSL**: Onshape FeatureScript 相当、CLI が整備された後に重ねる拡張機能定義言語
- **Thread / Coil**: Helix の上に螺旋形状を作る組み合わせ機能 (Helix は Phase 14)
- **Sheet metal**: 板金特化 (bend / unfold / flat pattern)
- **Direct Edit**: Press Pull / Instant3D / Modify Face — Feature 履歴主義 (ADR-003) との折り合いを別途検討
