Reading additional input from stdin...
OpenAI Codex v0.121.0 (research preview)
--------
workdir: /home/bacon/mycad
model: gpt-5.4
provider: openai
approval: never
sandbox: read-only
reasoning effort: xhigh
reasoning summaries: none
session id: 019e5dac-0790-7bb1-a206-adf6c3cfcd2b
--------
user
# Codex 設計レビュアー（MyCad CAD カーネル専用）

あなたは Rust 製 B-rep CAD カーネル「MyCad」の設計ドキュメントをレビューする専門家です。
stdin に渡された設計・テスト計画ドキュメントを読み、以下の観点で問題を指摘してください。

## レビュー観点

### 1. 決定性（MyCad の核心）
- `IdGenerator` を使った決定的 ID 生成になっているか
- 同一入力で必ず同一出力（同一 ID・同一座標）が保証されているか
- 非決定的要素（HashMap のイテレーション順、thread_rng など）が混入していないか

### 2. B-rep トポロジー妥当性
- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
- Loop/Shell/Solid の入れ子構造が一貫しているか

### 3. 退化幾何の扱い
- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
- 数値的境界（浮動小数点の epsilon 比較）の方針が明記されているか

### 4. API・型設計
- `Debug, Clone, Serialize, Deserialize` が付与されているか（JsonSchema が必要な型は追記）
- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
- ライブラリエラーには `thiserror` を使用しているか
- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）

### 5. テスト計画の充足性
- 決定性反復テスト（T0x 系）があるか
- Euler-Poincaré 検証テストがあるか
- 退化入力エッジケースが十分にあるか
- golden YAML ラウンドトリップテストがあるか

### 6. アーキテクチャ整合性
- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
- Feature history = source of truth の原則と矛盾しないか
- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか

---

## 出力フォーマット（厳守）

```yaml
issues:
  - id: R01
    severity: critical  # critical | high | medium | low
    section: "設計方針 > 決定性"
    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
    suggestion: "IdGenerator::next() に置き換えること"
  - id: R02
    severity: medium
    ...

verdict: pass  # pass | fail
# fail = Critical または High が1件以上ある
```

`issues` が空の場合は `issues: []` と書く。
**このフォーマット以外の出力は禁止。前置きや後置きの文章は書かない。**

<stdin>
# Issue #18: API 層スケルトン (mycad-api / axum)

## Context

Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。

既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。

## 実装対象

- 新規クレート `crates/mycad-api/`(lib + bin 構成)
- Issue: #18 / Milestone: Phase 2

### 影響ファイル

| ファイル | 変更 |
|----------|------|
| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
| `crates/mycad-api/Cargo.toml` | 新規。既存規約(`version.workspace = true` 等)に従う。`[[bin]]` で binary 名 `mycad-api` |
| `crates/mycad-api/src/lib.rs` | 新規。Router 構築関数 `pub fn app() -> Router`、ハンドラ、`ApiError` |
| `crates/mycad-api/src/main.rs` | 新規。`#[tokio::main]`、`127.0.0.1:3000` バインド |
| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` による統合テスト |

### シグネチャ

```rust
// lib.rs
pub fn app() -> axum::Router;

#[derive(serde::Deserialize)]
struct MeshQuery { file: String }

async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;

enum ApiError { NotFound, BadExtension(String), Format(String), Build(String), Tessellate(String) }
impl IntoResponse for ApiError { /* status + {"error": "..."} */ }
```

## 設計方針

### 再利用(新規ロジックを書かない)
- `Document::from_path(&Path) -> Result<Document, FormatError>`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
- `build_solid_from_features(&[Feature], &mut IdGenerator) -> Result<Solid, KernelError>`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ対応。
- `tessellate_solid_with(&Solid, &TessellationOptions) -> Result<TriangleMesh, TessellationError>`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
- `TriangleMesh`(同 mod.rs:16)は `Serialize` 済み → `Json(mesh)` でそのまま返却。
- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き、末尾を `Ok(Json(mesh))` に置換した形。

### エンドポイント / バージョニング
- `GET /api/v0/mesh?file=<path>`。`/api/v0/` prefix を `Router::nest("/api/v0", ...)` で表現し将来バージョン追加に備える。
- クエリは `file` のみ(v0 スコープ)。テッセレーション解像度は default 固定。

### パス安全性(多層防御 — 任意位置の .mycad 読取とセキュリティを両立)
1. **CWD ジェイルを設けない** → 任意パスの `.mycad` を読める(要望)。
2. **`.mycad` 拡張子限定**(`Document::from_path` が既に強制)→ `/etc/passwd` 等の機密ファイル読取を遮断。
3. **`std::fs::canonicalize` + 実ファイル存在確認** → 正規化し、無ければ 404、ディレクトリ等は 400。
4. **`127.0.0.1` のみバインド**(`0.0.0.0` 不可)→ ネットワークから到達不可。
5. **Host ヘッダ検証ミドルウェア** → `Host` が `127.0.0.1:3000` / `localhost:3000` 以外なら 403。DNS リバインディングで悪意サイトがブラウザ経由で叩くのを遮断。

### エラーハンドリング
- `FormatError`/`KernelError`/`TessellationError` は `Serialize` 非対応 → `ApiError` enum に `.to_string()` で集約し、`IntoResponse` で status + `{"error": "..."}` を返す。
- status マッピング: ファイル不在 → 404 / 非.mycad 拡張子・不正パラメータ → 400 / 未対応サーフェス・複数フィーチャ等 → 422 / その他 → 500。

### 依存・規約
- `[workspace.dependencies]` に追加(前例ゼロなので新規登録): `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot` 用)。各クレートは `{ workspace = true }` で参照。
- derive 規約・`thiserror`・`version.workspace = true` 等の既存慣例を踏襲。
- clippy `-D warnings` 通過必須(未使用 import 等に注意)。

## テスト計画

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 正常系 | `GET /api/v0/mesh?file=examples/simple_box.mycad` を oneshot で叩く | 200、body を `TriangleMesh` に deser でき `positions`/`indices` が非空、`indices.len() % 3 == 0` |
| T02 | 決定性 | 同一リクエストを 2 回実行 | 返る JSON バイト列が完全一致 |
| T03 | 404 | 存在しない `file=examples/nope.mycad` | 404、body `{"error": ...}` |
| T04 | 400 | 非 `.mycad` 拡張子 `file=Cargo.toml` | 400(InvalidExtension) |
| T05 | 403 | `Host: evil.com` ヘッダ付与 | 403(Host 検証) |
| T06 | 422 | 複数/未対応フィーチャ `file=examples/assembly.mycad` | 422 系エラー(build/tessellate 失敗を JSON で返す) |

> oneshot 実行は相対パス基準が必要なため、テストは crate ルートからの相対 `examples/...` ではなくワークスペースルートからの絶対パスを `env!("CARGO_MANIFEST_DIR")` 起点で組み立てる(`crates/mycad-api` から `../../examples/...`)。

## 検証(end-to-end)

```bash
cargo xtask ci                      # fmt → clippy(-D warnings) → test --workspace → build
cargo run -p mycad-api --bin mycad-api &   # 127.0.0.1:3000 で起動
curl -s 'http://127.0.0.1:3000/api/v0/mesh?file=examples/simple_box.mycad' | head -c 300
# → TriangleMesh JSON(positions/normals/indices)が返ること
curl -s -o /dev/null -w '%{http_code}\n' 'http://127.0.0.1:3000/api/v0/mesh?file=examples/nope.mycad'  # 404
```

## 3ai フロー上の注記

- 本プラン承認(ExitPlanMode)**直後**に、harness プランモード制約で実行できなかった次を順に行う(GLM 実装 STEP 6 の前):
  1. `features/18-api-skeleton/` 作成 + `state.sh init`
  2. このプラン内容を `features/18-api-skeleton/plan.md` に転記
  3. Codex 設計レビュー(STEP 3、上限 3 回)→ Critical/High 解消
- その後 STEP 5(ブランチ `cad/18-api-skeleton`)→ STEP 6(GLM 実装、`crates/**` は Claude 編集禁止)→ STEP 7(Codex 最終レビュー)→ STEP 8(squash マージ, `Closes #18`)。
</stdin>
codex
設計ドキュメントを基準に、実リポジトリの型・規約・既存フローと突き合わせて問題点を拾います。まず関連クレートと参照先の実装を確認します。
exec
/bin/bash -lc 'rg --files /home/bacon/mycad/crates | rg "mycad-(cli|format|build|kernel)|Cargo.toml$|mod.rs$|document.rs$|main.rs$|tessellation"' in /home/bacon/mycad
 succeeded in 0ms:
/home/bacon/mycad/crates/mycad-format/Cargo.toml
/home/bacon/mycad/crates/mycad-cli/Cargo.toml
/home/bacon/mycad/crates/mycad-format/CLAUDE.md
/home/bacon/mycad/crates/xtask/Cargo.toml
/home/bacon/mycad/crates/mycad-cli/src/main.rs
/home/bacon/mycad/crates/mycad-format/src/lib.rs
/home/bacon/mycad/crates/mycad-format/src/feature.rs
/home/bacon/mycad/crates/mycad-format/src/component.rs
/home/bacon/mycad/crates/mycad-format/src/error.rs
/home/bacon/mycad/crates/mycad-format/src/document.rs
/home/bacon/mycad/crates/xtask/src/main.rs
/home/bacon/mycad/crates/mycad-cli/tests/export.rs
/home/bacon/mycad/crates/mycad-build/Cargo.toml
/home/bacon/mycad/crates/mycad-build/src/lib.rs
/home/bacon/mycad/crates/mycad-build/tests/feature_dispatcher.rs
/home/bacon/mycad/crates/mycad-kernel/Cargo.toml
/home/bacon/mycad/crates/mycad-viewer/Cargo.toml
/home/bacon/mycad/crates/mycad-kernel/src/lib.rs
/home/bacon/mycad/crates/mycad-kernel/CLAUDE.md
/home/bacon/mycad/crates/mycad-kernel/src/primitives/mod.rs
/home/bacon/mycad/crates/mycad-kernel/src/primitives/cuboid.rs
/home/bacon/mycad/crates/mycad-kernel/src/primitives/cylinder.rs
/home/bacon/mycad/crates/mycad-kernel/src/error.rs
/home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs
/home/bacon/mycad/crates/mycad-kernel/src/tessellation/stl.rs
/home/bacon/mycad/crates/mycad-kernel/benches/tessellation.rs
/home/bacon/mycad/crates/mycad-kernel/tests/cuboid_roundtrip.rs
/home/bacon/mycad/crates/mycad-kernel/src/brep/mod.rs
/home/bacon/mycad/crates/mycad-kernel/src/brep/topology.rs
/home/bacon/mycad/crates/mycad-kernel/src/geometry/curve.rs
/home/bacon/mycad/crates/mycad-kernel/src/geometry/math.rs
/home/bacon/mycad/crates/mycad-kernel/src/geometry/mod.rs
/home/bacon/mycad/crates/mycad-kernel/src/geometry/surface.rs

exec
/bin/bash -lc 'rg -n "struct IdGenerator|pub struct IdGenerator|run_export|build_solid_from_features|tessellate_solid_with|struct TriangleMesh|enum FormatError|pub enum FormatError|JsonSchema|thiserror|workspace.dependencies|mod.rs|HalfEdge|Euler|Serialize|Deserialize" -S Cargo.toml crates' in /home/bacon/mycad
 succeeded in 0ms:
Cargo.toml:18:[workspace.dependencies]
Cargo.toml:30:thiserror = "2"
crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
crates/mycad-format/src/feature.rs:1:use schemars::JsonSchema;
crates/mycad-format/src/feature.rs:2:use serde::{Deserialize, Serialize};
crates/mycad-format/src/feature.rs:5:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/feature.rs:14:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/component.rs:3:use serde::{Deserialize, Serialize};
crates/mycad-format/src/component.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
crates/mycad-format/src/component.rs:12:    /// Rotation as Euler angles in degrees [rx, ry, rz].
crates/mycad-format/src/component.rs:70:impl Serialize for ComponentRef {
crates/mycad-format/src/component.rs:71:    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
crates/mycad-format/src/component.rs:76:impl<'de> Deserialize<'de> for ComponentRef {
crates/mycad-format/src/component.rs:77:    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
crates/mycad-format/src/component.rs:83:impl schemars::JsonSchema for ComponentRef {
crates/mycad-format/src/component.rs:89:        <String as schemars::JsonSchema>::json_schema(gen)
crates/mycad-format/src/component.rs:96:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
crates/mycad-format/src/error.rs:1:use thiserror::Error;
crates/mycad-format/src/error.rs:4:pub enum FormatError {
crates/mycad-format/src/document.rs:3:use schemars::JsonSchema;
crates/mycad-format/src/document.rs:4:use serde::{Deserialize, Serialize};
crates/mycad-format/src/document.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/document.rs:25:    /// Serialize to YAML string.
crates/mycad-format/src/document.rs:30:    /// Deserialize from YAML string.
crates/mycad-kernel/CLAUDE.md:14:- すべてのEdgeは正確に2つのHalfEdgeを持つ
crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
crates/mycad-kernel/CLAUDE.md:17:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
crates/mycad-kernel/CLAUDE.md:24:3. `primitives/mod.rs` に `mod` と `pub use` を追加
crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
crates/mycad-build/src/lib.rs:6:pub fn build_solid_from_features(
crates/mycad-cli/src/main.rs:2:use mycad_build::build_solid_from_features;
crates/mycad-cli/src/main.rs:5:use mycad_kernel::tessellation::{tessellate_solid_with, to_ascii_stl, TessellationOptions};
crates/mycad-cli/src/main.rs:39:            if let Err(msg) = run_export(&input, &output, segments) {
crates/mycad-cli/src/main.rs:47:fn run_export(
crates/mycad-cli/src/main.rs:55:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
crates/mycad-cli/src/main.rs:62:        tessellate_solid_with(&solid, &opts).map_err(|e| format!("failed to tessellate: {e}"))?;
crates/mycad-kernel/src/error.rs:1:use thiserror::Error;
crates/mycad-kernel/src/tessellation/mod.rs:8:use serde::{Deserialize, Serialize};
crates/mycad-kernel/src/tessellation/mod.rs:10:use thiserror::Error;
crates/mycad-kernel/src/tessellation/mod.rs:17:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/tessellation/mod.rs:18:pub struct TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:49:#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
crates/mycad-kernel/src/tessellation/mod.rs:86:    tessellate_solid_with(solid, &TessellationOptions::default())
crates/mycad-kernel/src/tessellation/mod.rs:90:pub fn tessellate_solid_with(
crates/mycad-kernel/src/tessellation/mod.rs:445:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:446:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:525:    /// (public fields bypass new() clamping, so tessellate_solid_with must guard at point of use).
crates/mycad-kernel/src/tessellation/mod.rs:534:        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
crates/mycad-kernel/src/brep/topology.rs:4:use serde::{Deserialize, Serialize};
crates/mycad-kernel/src/brep/topology.rs:11:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:20:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:21:pub struct HalfEdge {
crates/mycad-kernel/src/brep/topology.rs:33:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:45:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:54:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:68:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:79:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:83:    pub half_edges: Vec<HalfEdge>,
crates/mycad-kernel/src/brep/topology.rs:138:        self.half_edges.push(HalfEdge {
crates/mycad-kernel/src/brep/topology.rs:185:pub struct IdGenerator {
crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
crates/mycad-kernel/src/primitives/cylinder.rs:195:            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
crates/mycad-build/tests/feature_dispatcher.rs:1:use mycad_build::build_solid_from_features;
crates/mycad-build/tests/feature_dispatcher.rs:17:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
crates/mycad-kernel/src/geometry/curve.rs:3:use serde::{Deserialize, Serialize};
crates/mycad-kernel/src/geometry/curve.rs:6:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/geometry/surface.rs:3:use serde::{Deserialize, Serialize};
crates/mycad-kernel/src/geometry/surface.rs:17:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/geometry/mod.rs:6:use serde::{Deserialize, Serialize};
crates/mycad-kernel/src/geometry/mod.rs:15:#[derive(Debug, Clone, Serialize, Deserialize)]

codex
参照先は揃っていました。次に、決定性・トポロジー・依存規約に直接関わる実装を読み、計画とのズレを詰めます。
exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/brep/topology.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::Point;
use serde::{Deserialize, Serialize};

/// Unique identifier for topological entities.
/// Generated deterministically by creation order within a feature.
pub type EntityId = u64;

/// A vertex in B-rep — a point in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub id: EntityId,
    pub point: Point,
}

/// A half-edge — one side of an edge, used within a loop (wire).
/// Half-edges are oriented: they go from `start_vertex` to the next
/// half-edge's start vertex.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalfEdge {
    pub id: EntityId,
    /// Index of the start vertex in the parent solid's vertex list.
    pub start_vertex: usize,
    /// Index of the edge this half-edge belongs to.
    pub edge: usize,
    /// Whether this half-edge follows the edge's natural direction.
    pub forward: bool,
}

/// An edge — a curve segment bounded by two vertices.
/// Each edge has exactly two half-edges (one for each adjacent face).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EntityId,
    /// Indices of the two bounding vertices in the parent solid's vertex list.
    pub vertices: [usize; 2],
    /// The geometric curve this edge lies on.
    pub curve: Curve,
    /// Parameter range [t_start, t_end] on the curve.
    pub t_range: [f64; 2],
}

/// A loop (wire) — a closed sequence of half-edges forming a boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loop {
    pub id: EntityId,
    /// Ordered indices of half-edges in the parent solid's half-edge list.
    pub half_edges: Vec<usize>,
}

/// A face — a bounded region on a surface.
/// Has one outer loop and zero or more inner loops (holes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Face {
    pub id: EntityId,
    /// The geometric surface this face lies on.
    pub surface: Surface,
    /// Index of the outer loop in the parent solid's loop list.
    pub outer_loop: usize,
    /// Indices of inner loops (holes) in the parent solid's loop list.
    pub inner_loops: Vec<usize>,
    /// Whether the face normal matches the surface normal.
    pub same_sense: bool,
}

/// A shell — a connected set of faces forming a closed or open surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shell {
    pub id: EntityId,
    /// Indices of faces in the parent solid's face list.
    pub faces: Vec<usize>,
    /// Whether this shell is closed (watertight).
    pub closed: bool,
}

/// A solid — the top-level B-rep entity.
/// Contains all topological entities in flat arrays for deterministic ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solid {
    pub id: EntityId,
    pub vertices: Vec<Vertex>,
    pub half_edges: Vec<HalfEdge>,
    pub edges: Vec<Edge>,
    pub loops: Vec<Loop>,
    pub faces: Vec<Face>,
    pub shells: Vec<Shell>,
}

impl Solid {
    /// Create an empty solid with the given ID.
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            vertices: Vec::new(),
            half_edges: Vec::new(),
            edges: Vec::new(),
            loops: Vec::new(),
            faces: Vec::new(),
            shells: Vec::new(),
        }
    }

    /// Add a vertex and return its index.
    pub fn add_vertex(&mut self, id: EntityId, point: Point) -> usize {
        let idx = self.vertices.len();
        self.vertices.push(Vertex { id, point });
        idx
    }

    /// Add an edge and return its index.
    pub fn add_edge(
        &mut self,
        id: EntityId,
        vertices: [usize; 2],
        curve: Curve,
        t_range: [f64; 2],
    ) -> usize {
        let idx = self.edges.len();
        self.edges.push(Edge {
            id,
            vertices,
            curve,
            t_range,
        });
        idx
    }

    /// Add a half-edge and return its index.
    pub fn add_half_edge(
        &mut self,
        id: EntityId,
        start_vertex: usize,
        edge: usize,
        forward: bool,
    ) -> usize {
        let idx = self.half_edges.len();
        self.half_edges.push(HalfEdge {
            id,
            start_vertex,
            edge,
            forward,
        });
        idx
    }

    /// Add a loop and return its index.
    pub fn add_loop(&mut self, id: EntityId, half_edges: Vec<usize>) -> usize {
        let idx = self.loops.len();
        self.loops.push(Loop { id, half_edges });
        idx
    }

    /// Add a face and return its index.
    pub fn add_face(
        &mut self,
        id: EntityId,
        surface: Surface,
        outer_loop: usize,
        inner_loops: Vec<usize>,
        same_sense: bool,
    ) -> usize {
        let idx = self.faces.len();
        self.faces.push(Face {
            id,
            surface,
            outer_loop,
            inner_loops,
            same_sense,
        });
        idx
    }

    /// Add a shell and return its index.
    pub fn add_shell(&mut self, id: EntityId, faces: Vec<usize>, closed: bool) -> usize {
        let idx = self.shells.len();
        self.shells.push(Shell { id, faces, closed });
        idx
    }
}

/// Counter for generating deterministic entity IDs.
/// Each feature gets its own counter starting from a feature-specific offset.
#[derive(Debug, Clone)]
pub struct IdGenerator {
    next_id: EntityId,
}

impl IdGenerator {
    pub fn new(start: EntityId) -> Self {
        Self { next_id: start }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_generator_is_deterministic() {
        let mut gen1 = IdGenerator::new(100);
        let mut gen2 = IdGenerator::new(100);
        for _ in 0..10 {
            assert_eq!(gen1.next(), gen2.next());
        }
    }

    #[test]
    fn test_solid_add_vertex() {
        let mut solid = Solid::new(0);
        let idx = solid.add_vertex(1, Point::new(1.0, 2.0, 3.0));
        assert_eq!(idx, 0);
        assert_eq!(solid.vertices[0].id, 1);

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-cli/src/main.rs" in /home/bacon/mycad
 succeeded in 0ms:
use clap::{Parser, Subcommand};
use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::tessellation::{tessellate_solid_with, to_ascii_stl, TessellationOptions};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "mycad", version, about = "MyCad CAD kernel CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a .mycad file to STL
    Export {
        /// Input .mycad file
        input: PathBuf,
        /// Output STL file
        #[arg(short, long)]
        output: PathBuf,
        /// Number of angular segments for curved surfaces (default: 32)
        #[arg(long)]
        segments: Option<usize>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Export {
            input,
            output,
            segments,
        } => {
            if let Err(msg) = run_export(&input, &output, segments) {
                eprintln!("error: {msg}");
                process::exit(1);
            }
        }
    }
}

fn run_export(
    input: &std::path::Path,
    output: &std::path::Path,
    segments: Option<usize>,
) -> Result<(), String> {
    let doc = Document::from_path(input)
        .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
    let mut gen = IdGenerator::new(0);
    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
        .map_err(|e| format!("failed to build solid: {e}"))?;
    let opts = match segments {
        Some(n) => TessellationOptions::new(n, 1),
        None => TessellationOptions::default(),
    };
    let mesh =
        tessellate_solid_with(&solid, &opts).map_err(|e| format!("failed to tessellate: {e}"))?;
    let stl = to_ascii_stl(&mesh, "model");
    std::fs::write(output, stl)
        .map_err(|e| format!("failed to write {}: {e}", output.display()))?;
    Ok(())
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-build/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
use mycad_format::Feature;
use mycad_kernel::brep::topology::{IdGenerator, Solid};
use mycad_kernel::error::KernelError;
use mycad_kernel::primitives::{make_cuboid, make_cylinder};

pub fn build_solid_from_features(
    features: &[Feature],
    gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    if features.is_empty() {
        return Err(KernelError::EmptyFeatureList);
    }

    if features.len() > 1 {
        return Err(KernelError::MultipleFeatures {
            count: features.len(),
        });
    }

    match &features[0] {
        Feature::CreateBox {
            width,
            height,
            depth,
            ..
        } => Ok(make_cuboid(*width, *height, *depth, gen)),
        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
        Feature::CreateSphere { .. } => Err(KernelError::UnsupportedFeature {
            kind: "create_sphere",
        }),
        Feature::Extrude { .. } => Err(KernelError::UnsupportedFeature { kind: "extrude" }),
        Feature::Cut { .. } => Err(KernelError::UnsupportedFeature { kind: "cut" }),
        Feature::Fuse { .. } => Err(KernelError::UnsupportedFeature { kind: "fuse" }),
        Feature::Intersect { .. } => Err(KernelError::UnsupportedFeature { kind: "intersect" }),
    }
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-format/src/document.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::component::Component;
use crate::error::FormatError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The top-level document representing a MyCad design file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Document {
    /// Kernel version that created this document.
    pub version: String,
    /// The root component (assembly or single part).
    pub root_component: Component,
}

impl Document {
    /// Create a new document with a single empty root component.
    pub fn new(name: &str) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            root_component: Component::new(name),
        }
    }

    /// Serialize to YAML string.
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Deserialize from YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Load a Document from a `.mycad` file path.
    pub fn from_path(path: &Path) -> Result<Self, FormatError> {
        let ext = path.extension().and_then(|s| s.to_str());
        if ext != Some("mycad") {
            return Err(FormatError::InvalidExtension(ext.map(str::to_string)));
        }
        let content = std::fs::read_to_string(path)?;
        let doc = Self::from_yaml(&content)?;
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::FormatError;
    use crate::feature::Feature;
    use std::path::Path;

    #[test]
    fn test_roundtrip_yaml() {
        let mut doc = Document::new("Test Assembly");
        doc.root_component.features.push(Feature::Extrude {
            id: "box_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 10.0,
        });

        let yaml = doc.to_yaml().unwrap();
        let doc2 = Document::from_yaml(&yaml).unwrap();

        assert_eq!(doc.version, doc2.version);
        assert_eq!(doc.root_component.name, doc2.root_component.name);
        assert_eq!(
            doc.root_component.features.len(),
            doc2.root_component.features.len()
        );
    }

    #[test]
    fn test_yaml_is_deterministic() {
        let mut doc = Document::new("Test");
        doc.root_component.features.push(Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        });

        let yaml1 = doc.to_yaml().unwrap();
        let yaml2 = doc.to_yaml().unwrap();
        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
    }

    #[test]
    fn test_from_path_loads_simple_box() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("simple_box.mycad");
        let doc = Document::from_path(&path).unwrap();
        assert_eq!(doc.version, "0.1.0");
        assert_eq!(doc.root_component.name, "Simple Box");
    }

    #[test]
    fn test_from_path_missing_file_returns_io_error() {
        let path = Path::new("does_not_exist.mycad");
        let err = Document::from_path(path).unwrap_err();
        assert!(matches!(err, FormatError::Io(_)));
    }

    #[test]
    fn test_from_path_invalid_extension() {
        let path = Path::new("foo.txt");
        let err = Document::from_path(path).unwrap_err();
        assert!(matches!(err, FormatError::InvalidExtension(Some(ref e)) if e == "txt"));
    }

    #[test]
    fn test_assembly_roundtrip() {
        use crate::component::ComponentRef;

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("assembly.mycad");
        let doc = Document::from_path(&path).unwrap();

        // Bolt child has a stdlib reference
        let bolt = doc
            .root_component
            .children
            .iter()
            .find(|c| c.name == "Bolt")
            .expect("Bolt component not found");
        assert!(
            matches!(&bolt.reference, Some(ComponentRef::StdLib(p)) if p == "fasteners/jis_b1176/M5x20")
        );

        // Roundtrip: serialize then deserialize yields the same YAML
        let yaml = doc.to_yaml().unwrap();
        let doc2 = Document::from_yaml(&yaml).unwrap();
        assert_eq!(doc.to_yaml().unwrap(), doc2.to_yaml().unwrap());
    }
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-format/src/error.rs" in /home/bacon/mycad
 succeeded in 0ms:
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("invalid file extension: expected .mycad, got {0:?}")]
    InvalidExtension(Option<String>),

    #[error("invalid reference {value:?}: {reason}")]
    InvalidReference { value: String, reason: &'static str },
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod stl;
pub use stl::to_ascii_stl;

use crate::brep::topology::Solid;
use crate::geometry::curve::Curve;
use crate::geometry::surface::{Surface, TessellationStrategy};
use crate::geometry::Point;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use thiserror::Error;

/// Seam jump detection threshold: |Δu| > π indicates a parameter-space seam crossing.
/// Minimum cross-product norm for a non-degenerate triangle.
const AREA_EPS: f64 = 1e-14;

/// A triangle mesh for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangleMesh {
    /// Vertex positions (x, y, z).
    pub positions: Vec<[f64; 3]>,
    /// Normal vectors per vertex.
    pub normals: Vec<[f64; 3]>,
    /// Triangle indices (every 3 indices form a triangle).
    pub indices: Vec<u32>,
}

impl TriangleMesh {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Number of triangles in the mesh.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

impl Default for TriangleMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Controls the resolution of curved-surface tessellation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TessellationOptions {
    pub angular_segments: usize,
    pub axial_segments: usize,
}

impl Default for TessellationOptions {
    fn default() -> Self {
        Self {
            angular_segments: 32,
            axial_segments: 1,
        }
    }
}

impl TessellationOptions {
    pub fn new(angular_segments: usize, axial_segments: usize) -> Self {
        Self {
            angular_segments: angular_segments.max(3),
            axial_segments: axial_segments.max(1),
        }
    }
}

/// Errors from tessellation.
#[derive(Debug, Error)]
pub enum TessellationError {
    #[error("unsupported surface type: {kind}")]
    UnsupportedSurface { kind: &'static str },
    #[error("trimmed or punctured faces are not yet supported")]
    TrimmedFaceUnsupported,
    #[error("non-manifold loop detected")]
    NonManifoldLoop,
}

/// Tessellate a B-rep solid into a triangle mesh using default options.
pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
    tessellate_solid_with(solid, &TessellationOptions::default())
}

/// Tessellate a B-rep solid into a triangle mesh with the given options.
pub fn tessellate_solid_with(
    solid: &Solid,
    opts: &TessellationOptions,
) -> Result<TriangleMesh, TessellationError> {
    let mut mesh = TriangleMesh::new();

    for face in &solid.faces {
        let strategy = face.surface.tessellation_strategy();
        match strategy {
            TessellationStrategy::BoundaryFan => {
                tessellate_face_fan(solid, face, opts, &mut mesh)?;
            }
            TessellationStrategy::UvGridFullPatch => {
                tessellate_face_uv_grid(solid, face, opts, &mut mesh)?;
            }
            TessellationStrategy::Unsupported => {
                return Err(TessellationError::UnsupportedSurface {
                    kind: face.surface.kind_name(),
                });
            }
        }
    }

    Ok(mesh)
}

/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
fn tessellate_face_fan(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
) -> Result<(), TessellationError> {
    let outer_loop = &solid.loops[face.outer_loop];

    let loop_points = collect_loop_points(solid, outer_loop, opts.angular_segments.max(3))?;

    if loop_points.len() < 3 {
        return Ok(());
    }

    let base_idx = mesh.positions.len() as u32;

    for p in &loop_points {
        let normal = face.surface.normal_at_point(p);
        let n = if face.same_sense {
            [normal.x, normal.y, normal.z]
        } else {
            [-normal.x, -normal.y, -normal.z]
        };
        mesh.positions.push([p.x, p.y, p.z]);
        mesh.normals.push(n);
    }

    for i in 1..(loop_points.len() as u32 - 1) {
        mesh.indices.push(base_idx);
        mesh.indices.push(base_idx + i);
        mesh.indices.push(base_idx + i + 1);
    }

    Ok(())
}

/// Collect the ordered boundary points of a loop by sampling each HE's curve.
fn collect_loop_points(
    solid: &Solid,
    lp: &crate::brep::topology::Loop,
    segments: usize,
) -> Result<Vec<Point>, TessellationError> {
    let mut points = Vec::new();

    for &he_idx in &lp.half_edges {
        let he = &solid.half_edges[he_idx];
        let edge = &solid.edges[he.edge];

        let (t_start, t_end) = if he.forward {
            (edge.t_range[0], edge.t_range[1])
        } else {
            (edge.t_range[1], edge.t_range[0])
        };

        let seg_points = edge.curve.sample_segment(t_start, t_end, segments);
        for p in seg_points {
            points.push(p);
        }
    }

    Ok(points)
}

/// Triangulate a face using UV grid sampling (full untrimmed periodic/rectangular patch).
fn tessellate_face_uv_grid(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
) -> Result<(), TessellationError> {
    if !face.inner_loops.is_empty() {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    let outer_loop = &solid.loops[face.outer_loop];

    // Guard: total Circle-edge span must be a positive integer multiple of 2π.
    // Summing spans (rather than testing any single edge) accepts edge-split full cylinders
    // (e.g. two π-arcs per cap) while still rejecting trimmed faces. 1e-9 tolerance is tight
    // enough that floating-point noise never masks a genuine sub-degree trim.
    let total_circle_span: f64 = outer_loop
        .half_edges
        .iter()
        .filter_map(|&he_idx| {
            let he = &solid.half_edges[he_idx];
            let edge = &solid.edges[he.edge];
            if matches!(edge.curve, Curve::Circle { .. }) {
                Some((edge.t_range[1] - edge.t_range[0]).abs())
            } else {
                None
            }
        })
        .sum();
    let full_rev_count = (total_circle_span / (2.0 * PI)).round() as i64;
    if full_rev_count <= 0 || (total_circle_span - full_rev_count as f64 * 2.0 * PI).abs() >= 1e-9 {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    // Determine v range from loop corner vertices (start_vertex of each HE).
    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
    // we do not sample the circle arcs here — the grid covers [0,2π] directly.
    let corner_uvs: Vec<(f64, f64)> = outer_loop
        .half_edges
        .iter()

exec
/bin/bash -lc "sed -n '220,520p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
        .iter()
        .map(|&he_idx| {
            let he = &solid.half_edges[he_idx];
            face.surface.uv_of(&solid.vertices[he.start_vertex].point)
        })
        .collect();

    let v_min = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
    let v_max = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
    let u_min = 0.0_f64;

    // Clamp at point of use — callers may bypass TessellationOptions::new() via public fields
    // or deserialization, so we defensively enforce minimum viable values here.
    let n_u = opts.angular_segments.max(3);
    let n_v = opts.axial_segments.max(1);

    let base_idx = mesh.positions.len() as u32;

    // Sample UV grid: (n_u+1) × (n_v+1) vertices
    // u goes from u_min to u_min + 2π, v goes from v_min to v_max
    let du = 2.0 * PI / n_u as f64;
    let dv = (v_max - v_min) / n_v as f64;

    for iv in 0..=n_v {
        for iu in 0..=n_u {
            let u = u_min + du * iu as f64;
            let v = v_min + dv * iv as f64;
            let p = face.surface.evaluate(u, v);
            let normal = face.surface.normal_at(u, v);
            let n = if face.same_sense {
                [normal.x, normal.y, normal.z]
            } else {
                [-normal.x, -normal.y, -normal.z]
            };
            mesh.positions.push([p.x, p.y, p.z]);
            mesh.normals.push(n);
        }
    }

    // Generate triangles for each cell
    for iv in 0..n_v {
        for iu in 0..n_u {
            let i00 = base_idx + (iv * (n_u + 1) + iu) as u32;
            let i10 = base_idx + (iv * (n_u + 1) + iu + 1) as u32;
            let i01 = base_idx + ((iv + 1) * (n_u + 1) + iu) as u32;
            let i11 = base_idx + ((iv + 1) * (n_u + 1) + iu + 1) as u32;

            // Two triangles per cell, orientation so ∂u×∂v points outward
            push_triangle(mesh, i00, i10, i01);
            push_triangle(mesh, i10, i11, i01);
        }
    }

    Ok(())
}

/// Push a triangle, but skip degenerate (zero-area) ones.
fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
    let p0: [f64; 3] = mesh.positions[i0 as usize];
    let p1: [f64; 3] = mesh.positions[i1 as usize];
    let p2: [f64; 3] = mesh.positions[i2 as usize];
    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let cross_norm = (u[1] * v[2] - u[2] * v[1]).powi(2)
        + (u[2] * v[0] - u[0] * v[2]).powi(2)
        + (u[0] * v[1] - u[1] * v[0]).powi(2).sqrt();
    if cross_norm < AREA_EPS * AREA_EPS {
        return;
    }
    mesh.indices.push(i0);
    mesh.indices.push(i1);
    mesh.indices.push(i2);
}

/// Surface kind name for error messages.
trait SurfaceKind {
    fn kind_name(&self) -> &'static str;
}

impl SurfaceKind for Surface {
    fn kind_name(&self) -> &'static str {
        match self {
            Surface::Plane { .. } => "plane",
            Surface::Cylinder { .. } => "cylinder",
            Surface::Sphere { .. } => "sphere",
            Surface::Cone { .. } => "cone",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::geometry::Vec3;
    use crate::primitives::make_cuboid;
    use crate::primitives::make_cylinder;

    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
    #[test]
    fn test_tessellate_cylinder_triangle_count() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        // Lateral: 32 angular segments × 1 axial × 2 triangles = 64
        // Each cap: 32-gon fan = 30 triangles (32 - 2)
        // Total: 64 + 30 + 30 = 124
        let expected = 32 * 2 + (32 - 2) * 2;
        assert_eq!(
            mesh.triangle_count(),
            expected,
            "expected {expected} triangles, got {}",
            mesh.triangle_count()
        );
    }

    /// T07: Determinism — same input → same output.
    #[test]
    fn test_tessellation_cylinder_deterministic() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);

        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();

        let m1 = tessellate_solid(&s1).unwrap();
        let m2 = tessellate_solid(&s2).unwrap();

        assert_eq!(m1.positions, m2.positions);
        assert_eq!(m1.normals, m2.normals);
        assert_eq!(m1.indices, m2.indices);
    }

    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
    #[test]
    fn test_tessellate_cuboid_unchanged() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
        let mesh = tessellate_solid(&solid).unwrap();

        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.positions.len(), 24);
        assert_eq!(mesh.normals.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
    }

    /// T08 extended: exact positions and indices snapshot.
    #[test]
    fn test_cuboid_positions_indices_snapshot() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);

        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);

        let m1 = tessellate_solid(&s1).unwrap();
        let m2 = tessellate_solid(&s2).unwrap();

        // Byte-identical across two runs
        assert_eq!(m1.positions, m2.positions);
        assert_eq!(m1.indices, m2.indices);
    }

    /// T09: Outward normals — all facet normals point away from centroid.
    #[test]
    fn test_cylinder_outward_normals() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        // Centroid of cylinder: (0, 0, 10)
        let centroid = [0.0f64, 0.0, 10.0];

        for tri in 0..mesh.triangle_count() {
            let i0 = mesh.indices[tri * 3] as usize;
            let i1 = mesh.indices[tri * 3 + 1] as usize;
            let i2 = mesh.indices[tri * 3 + 2] as usize;

            let p0 = mesh.positions[i0];
            let p1 = mesh.positions[i1];
            let p2 = mesh.positions[i2];

            // Facet center
            let center = [
                (p0[0] + p1[0] + p2[0]) / 3.0,
                (p0[1] + p1[1] + p2[1]) / 3.0,
                (p0[2] + p1[2] + p2[2]) / 3.0,
            ];

            // Cross product for facet normal
            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let nx = u[1] * v[2] - u[2] * v[1];
            let ny = u[2] * v[0] - u[0] * v[2];
            let nz = u[0] * v[1] - u[1] * v[0];

            // Vector from centroid to facet center
            let dx = center[0] - centroid[0];
            let dy = center[1] - centroid[1];
            let dz = center[2] - centroid[2];

            // Dot product should be positive (normal points outward)
            let dot = nx * dx + ny * dy + nz * dz;
            assert!(
                dot > 0.0,
                "facet {tri}: normal should point outward (dot={dot})"
            );
        }
    }

    /// T12: Resolution externalization + clamping.
    #[test]
    fn test_tessellation_options_clamp() {
        let opts = TessellationOptions::new(2, 0);
        assert_eq!(opts.angular_segments, 3);
        assert_eq!(opts.axial_segments, 1);
    }

    #[test]
    fn test_different_resolution_different_count() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();

        let mesh_default = tessellate_solid(&solid).unwrap();
        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();

        assert_ne!(mesh_default.triangle_count(), mesh_low.triangle_count());
        assert_ne!(mesh_default.triangle_count(), mesh_hi.triangle_count());

        // 8 angular, 1 axial: lateral=8*1*2=16, caps=(8-2)*2=12 → 28
        assert_eq!(mesh_low.triangle_count(), 8 * 2 + (8 - 2) * 2);
        // 64 angular, 2 axial: lateral=64*2*2=256, caps=(64-2)*2=124 → 380
        assert_eq!(mesh_hi.triangle_count(), 64 * 2 * 2 + (64 - 2) * 2);
    }

    /// T15: Unsupported surface type → error.
    #[test]
    fn test_unsupported_surface_error() {
        use crate::brep::topology::Solid as S;
        use crate::geometry::curve::Curve;

        let mut s = S::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0));
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
        );
        let he0 = s.add_half_edge(4, v0, e0, true);
        let lp = s.add_loop(5, vec![he0]);
        s.add_face(
            6,
            Surface::Sphere {
                center: Point::origin(),
                radius: 1.0,
            },
            lp,
            vec![],
            true,
        );
        s.add_shell(7, vec![0], true);

        let result = tessellate_solid(&s);
        assert!(matches!(
            result,
            Err(TessellationError::UnsupportedSurface { kind: "sphere" })
        ));
    }

    // Legacy tests preserved for backward compatibility
    #[test]
    fn test_tessellate_cuboid() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
        let mesh = tessellate_solid(&solid).unwrap();

        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.positions.len(), 24);
        assert_eq!(mesh.normals.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
    }

    #[test]
    fn test_tessellation_deterministic() {
        let mut id_gen1 = IdGenerator::new(0);
        let mut id_gen2 = IdGenerator::new(0);

        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);

        let mesh1 = tessellate_solid(&solid1).unwrap();
        let mesh2 = tessellate_solid(&solid2).unwrap();

        assert_eq!(mesh1.positions, mesh2.positions);

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::brep::topology::{IdGenerator, Solid};
use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::Point;
use crate::geometry::Vec3;

/// Create a cuboid (box) B-rep solid centered at the origin.
///
/// Parameters:
/// - `dx`: width (X dimension)
/// - `dy`: height (Y dimension)
/// - `dz`: depth (Z dimension)
/// - `id_gen`: deterministic ID generator
///
/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
    let mut solid = Solid::new(id_gen.next());

    let hx = dx / 2.0;
    let hy = dy / 2.0;
    let hz = dz / 2.0;

    // 8 vertices of the cuboid
    //
    //     6-------7
    //    /|      /|
    //   4-------5 |
    //   | 2-----|-3
    //   |/      |/
    //   0-------1
    //
    // v0 = (-hx, -hy, -hz), v1 = (hx, -hy, -hz), ...
    let v_positions = [
        Point::new(-hx, -hy, -hz), // 0: front-bottom-left
        Point::new(hx, -hy, -hz),  // 1: front-bottom-right
        Point::new(-hx, hy, -hz),  // 2: front-top-left
        Point::new(hx, hy, -hz),   // 3: front-top-right
        Point::new(-hx, -hy, hz),  // 4: back-bottom-left
        Point::new(hx, -hy, hz),   // 5: back-bottom-right
        Point::new(-hx, hy, hz),   // 6: back-top-left
        Point::new(hx, hy, hz),    // 7: back-top-right
    ];

    let v: Vec<usize> = v_positions
        .iter()
        .map(|p| solid.add_vertex(id_gen.next(), *p))
        .collect();

    // 12 edges of the cuboid
    // Each edge is defined by [start_vertex_index, end_vertex_index]
    let edge_defs: [(usize, usize); 12] = [
        (0, 1), // e0:  bottom-front
        (1, 3), // e1:  right-front
        (3, 2), // e2:  top-front
        (2, 0), // e3:  left-front
        (4, 5), // e4:  bottom-back
        (5, 7), // e5:  right-back
        (7, 6), // e6:  top-back
        (6, 4), // e7:  left-back
        (0, 4), // e8:  bottom-left
        (1, 5), // e9:  bottom-right
        (3, 7), // e10: top-right
        (2, 6), // e11: top-left
    ];

    let e: Vec<usize> = edge_defs
        .iter()
        .map(|&(v0, v1)| {
            let p0 = solid.vertices[v[v0]].point;
            let p1 = solid.vertices[v[v1]].point;
            let direction = p1 - p0;
            solid.add_edge(
                id_gen.next(),
                [v[v0], v[v1]],
                Curve::Line {
                    origin: p0,
                    direction,
                },
                [0.0, 1.0],
            )
        })
        .collect();

    // 6 faces of the cuboid
    // Each face is defined by its 4 edges and their orientation (forward/backward),
    // plus the surface definition.
    struct FaceDef {
        // (edge_index, forward, start_vertex_index)
        half_edges: [(usize, bool, usize); 4],
        surface: Surface,
        same_sense: bool,
    }

    let face_defs = [
        // Front face (Z = -hz): vertices 0,1,3,2 — normal -Z
        FaceDef {
            half_edges: [
                (0, true, 0), // e0: 0->1
                (1, true, 1), // e1: 1->3
                (2, true, 3), // e2: 3->2
                (3, true, 2), // e3: 2->0
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, 0.0, -hz),
                normal: -Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            same_sense: true,
        },
        // Back face (Z = +hz): vertices 5,4,6,7 — normal +Z
        FaceDef {
            half_edges: [
                (4, false, 5), // e4: 5->4
                (7, false, 4), // e7: 4->6
                (6, false, 6), // e6: 6->7
                (5, false, 7), // e5: 7->5
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, 0.0, hz),
                normal: Vec3::z(),
                u_axis: -Vec3::x(),
                v_axis: Vec3::y(),
            },
            same_sense: true,
        },
        // Bottom face (Y = -hy): vertices 0,4,5,1 — normal -Y
        FaceDef {
            half_edges: [
                (8, true, 0),  // e8: 0->4
                (4, true, 4),  // e4: 4->5
                (9, false, 5), // e9: 5->1
                (0, false, 1), // e0: 1->0
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, -hy, 0.0),
                normal: -Vec3::y(),
                u_axis: Vec3::x(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
        // Top face (Y = +hy): vertices 2,3,7,6 — normal +Y
        FaceDef {
            half_edges: [
                (2, false, 2),  // e2: 2->3
                (10, true, 3),  // e10: 3->7
                (6, true, 7),   // e6: 7->6
                (11, false, 6), // e11: 6->2
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, hy, 0.0),
                normal: Vec3::y(),
                u_axis: Vec3::x(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
        // Left face (X = -hx): vertices 0,2,6,4 — normal -X
        FaceDef {
            half_edges: [
                (3, false, 0), // e3: 0->2
                (11, true, 2), // e11: 2->6
                (7, true, 6),  // e7: 6->4
                (8, false, 4), // e8: 4->0
            ],
            surface: Surface::Plane {
                origin: Point::new(-hx, 0.0, 0.0),
                normal: -Vec3::x(),
                u_axis: Vec3::y(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
        // Right face (X = +hx): vertices 1,5,7,3 — normal +X
        FaceDef {
            half_edges: [
                (9, true, 1),   // e9: 1->5
                (5, true, 5),   // e5: 5->7
                (10, false, 7), // e10: 7->3
                (1, false, 3),  // e1: 3->1
            ],
            surface: Surface::Plane {
                origin: Point::new(hx, 0.0, 0.0),
                normal: Vec3::x(),
                u_axis: Vec3::y(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
    ];

    let mut face_indices = Vec::new();

    for face_def in &face_defs {
        let he_indices: Vec<usize> = face_def
            .half_edges
            .iter()
            .map(|&(edge_idx, forward, start_v)| {
                solid.add_half_edge(id_gen.next(), v[start_v], e[edge_idx], forward)
            })
            .collect();

        let loop_idx = solid.add_loop(id_gen.next(), he_indices);
        let face_idx = solid.add_face(
            id_gen.next(),
            face_def.surface.clone(),
            loop_idx,
            vec![],
            face_def.same_sense,
        );
        face_indices.push(face_idx);
    }

    solid.add_shell(id_gen.next(), face_indices, true);

    solid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;

    #[test]
    fn test_cuboid_topology() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);

        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
        assert!(solid.shells[0].closed, "cuboid shell should be closed");
        // 6 faces * 4 half-edges = 24 half-edges
        assert_eq!(
            solid.half_edges.len(),
            24,
            "cuboid should have 24 half-edges"
        );
        // 6 loops (one per face)
        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
    }

    #[test]
    fn test_cuboid_vertex_positions() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);

        // Check that all vertices are at the expected positions
        let expected = [
            Point::new(-1.0, -2.0, -3.0),
            Point::new(1.0, -2.0, -3.0),
            Point::new(-1.0, 2.0, -3.0),
            Point::new(1.0, 2.0, -3.0),
            Point::new(-1.0, -2.0, 3.0),
            Point::new(1.0, -2.0, 3.0),
            Point::new(-1.0, 2.0, 3.0),
            Point::new(1.0, 2.0, 3.0),
        ];

exec
/bin/bash -lc "sed -n '1,320p' crates/mycad-kernel/src/primitives/cylinder.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::{Point, Vec3};

/// Create a cylinder B-rep solid.
///
/// Axis = +Z, bottom center at origin, bottom z=0 / top z=height.
/// Seam vertex at (0, radius, z) — angle u=0 in the orthonormal_basis(+Z) = (+Y, -X) frame.
pub fn make_cylinder(
    radius: f64,
    height: f64,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "radius" });
    }
    if !height.is_finite() || height <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "height" });
    }

    let mut solid = Solid::new(id_gen.next());

    // 2 vertices: seam at bottom and top
    let v_bot = solid.add_vertex(id_gen.next(), Point::new(0.0, radius, 0.0));
    let v_top = solid.add_vertex(id_gen.next(), Point::new(0.0, radius, height));

    // 3 edges
    let e_bot = solid.add_edge(
        id_gen.next(),
        [v_bot, v_bot],
        Curve::Circle {
            center: Point::origin(),
            normal: Vec3::z(),
            radius,
        },
        [0.0, 2.0 * std::f64::consts::PI],
    );

    let e_top = solid.add_edge(
        id_gen.next(),
        [v_top, v_top],
        Curve::Circle {
            center: Point::new(0.0, 0.0, height),
            normal: Vec3::z(),
            radius,
        },
        [0.0, 2.0 * std::f64::consts::PI],
    );

    let e_seam = solid.add_edge(
        id_gen.next(),
        [v_bot, v_top],
        Curve::Line {
            origin: Point::new(0.0, radius, 0.0),
            direction: Vec3::new(0.0, 0.0, height),
        },
        [0.0, 1.0],
    );

    // 3 faces: bottom cap, top cap, lateral
    let mut face_indices = Vec::with_capacity(3);

    // Bottom cap: Surface::Plane with normal -Z, loop = e_bot reversed (CW from above = outward -Z)
    let he_bot_rev = solid.add_half_edge(id_gen.next(), v_bot, e_bot, false);
    let loop_bot = solid.add_loop(id_gen.next(), vec![he_bot_rev]);
    let f_bot = solid.add_face(
        id_gen.next(),
        Surface::Plane {
            origin: Point::origin(),
            normal: -Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        },
        loop_bot,
        vec![],
        true,
    );
    face_indices.push(f_bot);

    // Top cap: Surface::Plane with normal +Z, loop = e_top forward (CCW from above = outward +Z)
    let he_top_fwd = solid.add_half_edge(id_gen.next(), v_top, e_top, true);
    let loop_top = solid.add_loop(id_gen.next(), vec![he_top_fwd]);
    let f_top = solid.add_face(
        id_gen.next(),
        Surface::Plane {
            origin: Point::new(0.0, 0.0, height),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        },
        loop_top,
        vec![],
        true,
    );
    face_indices.push(f_top);

    // Lateral face: Surface::Cylinder, loop = 4 HE
    //   seam up (fwd, start v_bot) → top circle (rev, start v_top) →
    //   seam down (rev, start v_top) → bot circle (fwd, start v_bot)
    let he_seam_up = solid.add_half_edge(id_gen.next(), v_bot, e_seam, true);
    let he_top_rev = solid.add_half_edge(id_gen.next(), v_top, e_top, false);
    let he_seam_dn = solid.add_half_edge(id_gen.next(), v_top, e_seam, false);
    let he_bot_fwd = solid.add_half_edge(id_gen.next(), v_bot, e_bot, true);
    let loop_lat = solid.add_loop(
        id_gen.next(),
        vec![he_seam_up, he_top_rev, he_seam_dn, he_bot_fwd],
    );
    let f_lat = solid.add_face(
        id_gen.next(),
        Surface::Cylinder {
            origin: Point::origin(),
            axis: Vec3::z(),
            radius,
        },
        loop_lat,
        vec![],
        true,
    );
    face_indices.push(f_lat);

    solid.add_shell(id_gen.next(), face_indices, true);

    Ok(solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::surface::TessellationStrategy;

    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
    #[test]
    fn test_cylinder_deterministic() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);
        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();

        assert_eq!(s1.vertices.len(), s2.vertices.len());
        assert_eq!(s1.edges.len(), s2.edges.len());
        assert_eq!(s1.faces.len(), s2.faces.len());
        assert_eq!(s1.half_edges.len(), s2.half_edges.len());
        assert_eq!(s1.loops.len(), s2.loops.len());
        assert_eq!(s1.shells.len(), s2.shells.len());

        for (v1, v2) in s1.vertices.iter().zip(s2.vertices.iter()) {
            assert_eq!(v1.id, v2.id);
            assert_eq!(v1.point, v2.point);
        }
        for (e1, e2) in s1.edges.iter().zip(s2.edges.iter()) {
            assert_eq!(e1.id, e2.id);
            assert_eq!(e1.vertices, e2.vertices);
        }
        for (f1, f2) in s1.faces.iter().zip(s2.faces.iter()) {
            assert_eq!(f1.id, f2.id);
        }
    }

    /// T02: Topology counts.
    #[test]
    fn test_cylinder_topology() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();

        assert_eq!(s.vertices.len(), 2, "2 seam vertices");
        assert_eq!(
            s.edges.len(),
            3,
            "3 edges (bot circle, top circle, seam line)"
        );
        assert_eq!(s.faces.len(), 3, "3 faces (bottom cap, top cap, lateral)");
        assert_eq!(s.shells.len(), 1);
        assert!(s.shells[0].closed);
        assert_eq!(s.half_edges.len(), 6, "6 half-edges (3 edges × 2 HE each)");
        assert_eq!(s.loops.len(), 3, "3 loops (one per face)");
    }

    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
    #[test]
    fn test_cylinder_euler() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();

        let v = s.vertices.len();
        let e = s.edges.len();
        let f = s.faces.len();
        let shells = s.shells.len();
        // genus=0 for simple cylinder
        let euler = (v as i64) - (e as i64) + (f as i64);
        assert_eq!(
            euler,
            2 * (shells as i64),
            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
        );
    }

    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
    ///       plus loop closure (consecutive HE endpoint ≈ next HE start point).
    #[test]
    fn test_cylinder_manifold_and_loop_closure() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let eps = 1e-10;

        // Each edge must have exactly 2 HEs with opposite forward flags
        let mut edge_he_count: std::collections::HashMap<usize, Vec<bool>> =
            std::collections::HashMap::new();
        for he in &s.half_edges {
            edge_he_count.entry(he.edge).or_default().push(he.forward);
        }
        for (edge_idx, forwards) in &edge_he_count {
            assert_eq!(forwards.len(), 2, "edge {edge_idx} must have exactly 2 HEs");
            assert_ne!(
                forwards[0], forwards[1],
                "edge {edge_idx} HEs must have opposite orientation"
            );
        }

        // Loop closure: each loop's consecutive HEs must connect geometrically
        for (loop_idx, lp) in s.loops.iter().enumerate() {
            assert!(
                !lp.half_edges.is_empty(),
                "loop {loop_idx} must not be empty"
            );
            for i in 0..lp.half_edges.len() {
                let he_cur = &s.half_edges[lp.half_edges[i]];
                let he_next = &s.half_edges[lp.half_edges[(i + 1) % lp.half_edges.len()]];

                let cur_edge = &s.edges[he_cur.edge];
                let end_v = if he_cur.forward {
                    cur_edge.vertices[1]
                } else {
                    cur_edge.vertices[0]
                };

                let next_start = he_next.start_vertex;

                assert!(
                    (s.vertices[end_v].point - s.vertices[next_start].point).norm() < eps,
                    "loop {loop_idx}: HE {} end vertex {} ({:?}) != HE {} start vertex {} ({:?})",
                    i,
                    end_v,
                    s.vertices[end_v].point,
                    (i + 1) % lp.half_edges.len(),
                    next_start,
                    s.vertices[next_start].point,
                );
            }
        }
    }

    /// T05: Geometry — seam vertices and surface coefficients.
    #[test]
    fn test_cylinder_geometry() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let eps = 1e-10;

        // Seam vertices
        assert!((s.vertices[0].point - Point::new(0.0, 5.0, 0.0)).norm() < eps);
        assert!((s.vertices[1].point - Point::new(0.0, 5.0, 20.0)).norm() < eps);

        // Bottom cap surface: Plane with normal -Z
        match &s.faces[0].surface {
            Surface::Plane { normal, .. } => {
                assert!(
                    (*normal + Vec3::z()).norm() < eps,
                    "bottom cap normal should be -Z"
                );
            }
            _ => panic!("bottom cap should be a Plane"),
        }

        // Top cap surface: Plane with normal +Z
        match &s.faces[1].surface {
            Surface::Plane { normal, .. } => {
                assert!(
                    (*normal - Vec3::z()).norm() < eps,
                    "top cap normal should be +Z"
                );
            }
            _ => panic!("top cap should be a Plane"),
        }

        // Lateral surface: Cylinder
        match &s.faces[2].surface {
            Surface::Cylinder {
                radius: r, axis, ..
            } => {
                assert!((r - 5.0).abs() < eps);
                assert!((*axis - Vec3::z()).norm() < eps);
            }
            _ => panic!("lateral face should be Cylinder surface"),
        }
    }

    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
    #[test]
    fn test_cylinder_degenerate_inputs() {
        let mut gen = IdGenerator::new(0);

        // Zero radius
        let err = make_cylinder(0.0, 20.0, &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        // Negative radius
        let err = make_cylinder(-5.0, 20.0, &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        // NaN radius
        let err = make_cylinder(f64::NAN, 20.0, &mut gen).unwrap_err();
        assert!(matches!(

exec
/bin/bash -lc "sed -n '1,220p' Cargo.toml" in /home/bacon/mycad
 succeeded in 0ms:
[workspace]
members = [
    "crates/mycad-kernel",
    "crates/mycad-format",
    "crates/mycad-build",
    "crates/mycad-viewer",
    "crates/mycad-cli",
    "crates/xtask",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
rust-version = "1.75"

[workspace.dependencies]
# Math
nalgebra = { version = "0.33", features = ["serde-serialize"] }
# Serialization
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
# Schema generation
schemars = "0.8"
# Testing
approx = "0.5"
tempfile = "3"
# Error handling
thiserror = "2"
# CLI
clap = { version = "4", features = ["derive"] }
# Ordered collections (deterministic iteration)
indexmap = { version = "2", features = ["serde"] }
# Internal crates
mycad-format = { path = "crates/mycad-format" }
mycad-kernel = { path = "crates/mycad-kernel" }
mycad-build = { path = "crates/mycad-build" }

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod brep;
pub mod error;
pub mod geometry;
pub mod primitives;
pub mod tessellation;

pub use brep::topology;
pub use error::KernelError;

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/CLAUDE.md" in /home/bacon/mycad
 succeeded in 0ms:
# mycad-kernel — B-rep 幾何カーネル

## Module Dependency Graph

```
primitives → brep, geometry
tessellation → brep, geometry
brep → geometry
geometry → (nalgebra)
```

## Key Invariants

- すべてのEdgeは正確に2つのHalfEdgeを持つ
- 閉じたShellはwatertight（水密）
- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
- EntityIDは `IdGenerator` で決定的に生成される

## How to Add a New Primitive

1. `src/primitives/` に新ファイルを作成（`cuboid.rs` をテンプレートとして使用）
2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
3. `primitives/mod.rs` に `mod` と `pub use` を追加
4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）

## How to Add a New Surface Type

1. `geometry/surface.rs` の `Surface` enumに新しいバリアントを追加
2. `evaluate(u, v) -> Point` を実装
3. `normal_at(u, v) -> Vec3` を実装
4. 共有数学関数は `geometry/math.rs` に配置

## How to Add a New Curve Type

1. `geometry/curve.rs` の `Curve` enumに新しいバリアントを追加
2. `evaluate(t) -> Point` を実装
3. 共有数学関数は `geometry/math.rs` に配置

codex
計画内のテストケースが実データに対して成立するかを見ます。特に `assembly.mycad` を 422 ケースに使えるかは、設計の妥当性に直結します。
exec
/bin/bash -lc "sed -n '1,220p' examples/assembly.mycad" in /home/bacon/mycad
 succeeded in 0ms:
version: "0.1.0"
root_component:
  name: "Simple Assembly"
  children:
    - name: "Base Plate"
      features:
        - type: create_box
          id: plate_1
          width: 100.0
          height: 5.0
          depth: 100.0
    - name: "Pillar"
      transform:
        position: [0.0, 25.0, 0.0]
        rotation: [0.0, 0.0, 0.0]
      features:
        - type: create_box
          id: pillar_1
          width: 10.0
          height: 50.0
          depth: 10.0
    - name: "Bolt"
      transform:
        position: [20.0, 5.0, 20.0]
        rotation: [0.0, 0.0, 0.0]
      ref: "stdlib://fasteners/jis_b1176/M5x20"

exec
/bin/bash -lc "sed -n '1,220p' examples/simple_box.mycad" in /home/bacon/mycad
 succeeded in 0ms:
version: "0.1.0"
root_component:
  name: "Simple Box"
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 20.0
      depth: 30.0

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-format/src/feature.rs" in /home/bacon/mycad
 succeeded in 0ms:
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A reference to a topological entity (face, edge, vertex) on a feature's result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityRef {
    /// The feature that created this entity.
    pub feature_id: String,
    /// The role of this entity in the feature (e.g., "top_face", "side_edge_0").
    pub role: String,
}

/// A feature — one step in the operation history.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum Feature {
    /// Create a box primitive.
    #[serde(rename = "create_box")]
    CreateBox {
        id: String,
        width: f64,
        height: f64,
        depth: f64,
    },

    /// Create a cylinder primitive.
    #[serde(rename = "create_cylinder")]
    CreateCylinder {
        id: String,
        radius: f64,
        height: f64,
    },

    /// Create a sphere primitive.
    #[serde(rename = "create_sphere")]
    CreateSphere { id: String, radius: f64 },

    /// Extrude a sketch profile.
    #[serde(rename = "extrude")]
    Extrude {
        id: String,
        sketch: String,
        depth: f64,
    },

    /// Cut (boolean subtract) one body from another.
    #[serde(rename = "cut")]
    Cut {
        id: String,
        target: String,
        tool: String,
    },

    /// Fuse (boolean union) two bodies.
    #[serde(rename = "fuse")]
    Fuse {
        id: String,
        target: String,
        tool: String,
    },

    /// Intersect two bodies.
    #[serde(rename = "intersect")]
    Intersect {
        id: String,
        target: String,
        tool: String,
    },
}

impl Feature {
    /// Get the feature ID.
    pub fn id(&self) -> &str {
        match self {
            Feature::CreateBox { id, .. }
            | Feature::CreateCylinder { id, .. }
            | Feature::CreateSphere { id, .. }
            | Feature::Extrude { id, .. }
            | Feature::Cut { id, .. }
            | Feature::Fuse { id, .. }
            | Feature::Intersect { id, .. } => id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_serialization() {
        let feature = Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        };

        let yaml = serde_yaml::to_string(&feature).unwrap();
        assert!(yaml.contains("create_box"));
        assert!(yaml.contains("box_1"));

        let deserialized: Feature = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.id(), "box_1");
    }

    #[test]
    fn test_entity_ref_serialization() {
        let entity_ref = EntityRef {
            feature_id: "box_1".to_string(),
            role: "top_face".to_string(),
        };

        let yaml = serde_yaml::to_string(&entity_ref).unwrap();
        let deserialized: EntityRef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.feature_id, "box_1");
        assert_eq!(deserialized.role, "top_face");
    }
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-format/src/component.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::error::FormatError;
use crate::feature::Feature;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A transform in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Transform {
    /// Position offset [x, y, z].
    pub position: [f64; 3],
    /// Rotation as Euler angles in degrees [rx, ry, rz].
    pub rotation: [f64; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
        }
    }
}

/// Reference to an external component, serialized as a URI string.
///
/// - `"stdlib://path/to/part"` → `ComponentRef::StdLib("path/to/part")`
/// - Any other string → `ComponentRef::File(value)`
#[derive(Debug, Clone)]
pub enum ComponentRef {
    /// Standard library reference (e.g., `"stdlib://fasteners/jis_b1176/M5x20"`)
    StdLib(String),
    /// File reference (e.g., `"./motor_v2.mycad"`)
    File(String),
}

impl FromStr for ComponentRef {
    type Err = FormatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(FormatError::InvalidReference {
                value: s.to_string(),
                reason: "reference must not be empty",
            });
        }
        if let Some(path) = s.strip_prefix("stdlib://") {
            if path.is_empty() {
                return Err(FormatError::InvalidReference {
                    value: s.to_string(),
                    reason: "stdlib reference must have a non-empty path after 'stdlib://'",
                });
            }
            Ok(ComponentRef::StdLib(path.to_string()))
        } else {
            Ok(ComponentRef::File(s.to_string()))
        }
    }
}

impl fmt::Display for ComponentRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComponentRef::StdLib(path) => write!(f, "stdlib://{path}"),
            ComponentRef::File(path) => write!(f, "{path}"),
        }
    }
}

impl Serialize for ComponentRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ComponentRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for ComponentRef {
    fn schema_name() -> String {
        "ComponentRef".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        <String as schemars::JsonSchema>::json_schema(gen)
    }
}

/// A component in the design hierarchy.
/// Can contain features (inline part definition), children (sub-components),
/// or be a reference to an external file/library.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Component {
    /// Human-readable name.
    pub name: String,

    /// Transform relative to the parent component.
    #[serde(default, skip_serializing_if = "is_default_transform")]
    pub transform: Transform,

    /// External reference (if this component is not defined inline).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref")]
    pub reference: Option<ComponentRef>,

    /// Ordered list of features (the operation history).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<Feature>,

    /// Child components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Component>,
}

fn is_default_transform(t: &Transform) -> bool {
    t.position == [0.0, 0.0, 0.0] && t.rotation == [0.0; 3]
}

impl Component {
    /// Create a new empty component.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            transform: Transform::default(),
            reference: None,
            features: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Create a component referencing an external file or library.
    pub fn from_ref(name: &str, reference: &str) -> Result<Self, FormatError> {
        Ok(Self {
            name: name.to_string(),
            transform: Transform::default(),
            reference: Some(reference.parse()?),
            features: Vec::new(),
            children: Vec::new(),
        })
    }
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-cli/tests/export.rs" in /home/bacon/mycad
 succeeded in 0ms:
use std::path::PathBuf;
use std::process::Command;

#[test]
fn export_simple_box_produces_12_facets() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("simple_box.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let output = tmp.path();

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .status()
        .expect("run mycad export");
    assert!(status.success(), "export should succeed");

    let stl = std::fs::read_to_string(output).expect("read stl");
    let facet_count = stl.matches("facet normal").count();
    assert_eq!(
        facet_count, 12,
        "cuboid should produce 12 facets (6 faces * 2 triangles)"
    );
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-build/tests/feature_dispatcher.rs" in /home/bacon/mycad
 succeeded in 0ms:
use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use std::path::Path;

#[test]
fn simple_box_mycad_builds_solid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("simple_box.mycad");

    let doc = Document::from_path(&path).expect("load .mycad");
    let mut g = IdGenerator::new(0);
    let solid =
        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");

    // simple_box.mycad: width=10, height=20, depth=30 → cuboid topology
    assert_eq!(solid.vertices.len(), 8);
    assert_eq!(solid.edges.len(), 12);
    assert_eq!(solid.faces.len(), 6);
    assert_eq!(solid.shells.len(), 1);
}

codex
実装可否ではなく設計整合性の観点で、リポジトリ規約の明文化も確認します。新規 API クレートを `lib.rs` 一枚に寄せる方針が規約と噛み合うかを見ています。
exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-format/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod component;
pub mod document;
pub mod error;
pub mod feature;

pub use component::Component;
pub use document::Document;
pub use error::FormatError;
pub use feature::Feature;

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/brep/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod topology;

pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-build/Cargo.toml" in /home/bacon/mycad
 succeeded in 0ms:
[package]
name = "mycad-build"
description = "Feature dispatcher: converts .mycad Feature history into kernel geometry"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
mycad-format = { workspace = true }
mycad-kernel = { workspace = true }

[dev-dependencies]
mycad-format = { workspace = true }

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-viewer/Cargo.toml" in /home/bacon/mycad
 succeeded in 0ms:
[package]
name = "mycad-viewer"
description = "Minimal 3D viewer for MyCad"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
mycad-kernel = { path = "../mycad-kernel" }
mycad-format = { path = "../mycad-format" }

exec
/bin/bash -lc 'rg -n "1概念1ファイル|pub use|再エクスポート|mod.rs|one concept|one file|router|handler|ApiError" -S .' in /home/bacon/mycad
 succeeded in 0ms:
./CLAUDE.md:57:- **モジュール構造**: 1概念1ファイル。重要な型は `mod.rs` から `pub use` で再エクスポート。
./features/12-cylinder/design-review.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
./features/12-cylinder/design-review.md:94:  - `crates/mycad-kernel/src/tessellation/mod.rs` — 曲線折れ線化 + 曲面 UV グリッド対応 + `TessellationOptions`
./features/12-cylinder/design-review.md:98:  - テスト: `cylinder.rs` 内インライン / `tessellation/mod.rs` 内 / `mycad-build/tests/feature_dispatcher.rs` / `mycad-cli/tests/export.rs`
./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/12-cylinder/design-review.md:274:crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/design-review.md:279:crates/mycad-kernel/src/primitives/mod.rs
./features/12-cylinder/design-review.md:302:crates/mycad-kernel/src/brep/mod.rs
./features/12-cylinder/design-review.md:308:crates/mycad-kernel/src/geometry/mod.rs
./features/12-cylinder/design-review.md:427:CLAUDE.md:57:- **モジュール構造**: 1概念1ファイル。重要な型は `mod.rs` から `pub use` で再エクスポート。
./features/12-cylinder/design-review.md:447:features/12-cylinder/design-review.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review.md:452:features/12-cylinder/design-review.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/12-cylinder/design-review.md:479:.claude/skills/3ai/agents/codex-design-reviewer.md:37:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review.md:492:crates/mycad-format/src/lib.rs:6:pub use component::Component;
./features/12-cylinder/design-review.md:493:crates/mycad-format/src/lib.rs:7:pub use document::Document;
./features/12-cylinder/design-review.md:494:crates/mycad-format/src/lib.rs:8:pub use error::FormatError;
./features/12-cylinder/design-review.md:495:crates/mycad-format/src/lib.rs:9:pub use feature::Feature;
./features/12-cylinder/design-review.md:518:crates/mycad-kernel/src/lib.rs:7:pub use brep::topology;
./features/12-cylinder/design-review.md:519:crates/mycad-kernel/src/lib.rs:8:pub use error::KernelError;
./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review.md:524:crates/mycad-kernel/src/tessellation/mod.rs:2:pub use stl::to_ascii_stl;
./features/12-cylinder/design-review.md:525:crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
./features/12-cylinder/design-review.md:526:crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
./features/12-cylinder/design-review.md:527:crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review.md:529:crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/design-review.md:530:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review.md:532:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/design-review.md:534:crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review.md:535:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review.md:536:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/design-review.md:539:crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/design-review.md:540:crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/design-review.md:566:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/12-cylinder/design-review.md:594:crates/mycad-kernel/CLAUDE.md:24:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/12-cylinder/design-review.md:1170:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review.md:1173:pub use stl::to_ascii_stl;
./features/12-cylinder/design-review.md:2013:pub use brep::topology;
./features/12-cylinder/design-review.md:2014:pub use error::KernelError;
./features/12-cylinder/design-review.md:2017:/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/geometry/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review.md:2096:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/12-cylinder/design-review.md:2491:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./crates/mycad-format/src/lib.rs:6:pub use component::Component;
./crates/mycad-format/src/lib.rs:7:pub use document::Document;
./crates/mycad-format/src/lib.rs:8:pub use error::FormatError;
./crates/mycad-format/src/lib.rs:9:pub use feature::Feature;
./features/12-cylinder/final-review-2.md:18: crates/mycad-kernel/src/tessellation/mod.rs | 81 +++++++++++++++++++++++++++--
./features/12-cylinder/final-review-2.md:22:/bin/bash -lc 'git show --format=medium --unified=80 572a245 -- crates/mycad-kernel/src/tessellation/mod.rs' in /home/bacon/mycad
./features/12-cylinder/final-review-2.md:42:diff --git a/crates/mycad-kernel/src/tessellation/mod.rs b/crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/final-review-2.md:44:--- a/crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/final-review-2.md:45:+++ b/crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/final-review-2.md:48: pub use stl::to_ascii_stl;
./features/12-cylinder/final-review-2.md:498:crates/mycad-kernel/src/tessellation/mod.rs:97:        let strategy = face.surface.tessellation_strategy();
./features/12-cylinder/final-review-2.md:499:crates/mycad-kernel/src/tessellation/mod.rs:102:            TessellationStrategy::UvGridFullPatch => {
./features/12-cylinder/final-review-2.md:500:crates/mycad-kernel/src/tessellation/mod.rs:206:    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
./features/12-cylinder/final-review-2.md:501:crates/mycad-kernel/src/tessellation/mod.rs:293:            Surface::Cylinder { .. } => "cylinder",
./features/12-cylinder/final-review-2.md:502:crates/mycad-kernel/src/tessellation/mod.rs:555:            Surface::Cylinder {
./features/12-cylinder/final-review-2.md:1052:/bin/bash -lc "nl -ba crates/mycad-kernel/src/tessellation/mod.rs | sed -n '150,245p'" in /home/bacon/mycad
./features/12-cylinder/final-review-2.md:1379:/bin/bash -lc "rg -n \"triangle_count\\(|make_cylinder|tessellate_solid_with\\(&solid, &TessellationOptions|zero_segments|partial_revolution\" crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/primitives -g'*.rs'" in /home/bacon/mycad
./features/12-cylinder/final-review-2.md:1381:crates/mycad-kernel/src/tessellation/mod.rs:37:    pub fn triangle_count(&self) -> usize {
./features/12-cylinder/final-review-2.md:1382:crates/mycad-kernel/src/tessellation/mod.rs:306:    use crate::primitives::make_cylinder;
./features/12-cylinder/final-review-2.md:1383:crates/mycad-kernel/src/tessellation/mod.rs:310:    fn test_tessellate_cylinder_triangle_count() {
./features/12-cylinder/final-review-2.md:1384:crates/mycad-kernel/src/tessellation/mod.rs:312:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1385:crates/mycad-kernel/src/tessellation/mod.rs:320:            mesh.triangle_count(),
./features/12-cylinder/final-review-2.md:1386:crates/mycad-kernel/src/tessellation/mod.rs:323:            mesh.triangle_count()
./features/12-cylinder/final-review-2.md:1387:crates/mycad-kernel/src/tessellation/mod.rs:333:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/12-cylinder/final-review-2.md:1388:crates/mycad-kernel/src/tessellation/mod.rs:334:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/12-cylinder/final-review-2.md:1389:crates/mycad-kernel/src/tessellation/mod.rs:351:        assert_eq!(mesh.triangle_count(), 12);
./features/12-cylinder/final-review-2.md:1390:crates/mycad-kernel/src/tessellation/mod.rs:378:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1391:crates/mycad-kernel/src/tessellation/mod.rs:384:        for tri in 0..mesh.triangle_count() {
./features/12-cylinder/final-review-2.md:1392:crates/mycad-kernel/src/tessellation/mod.rs:432:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1393:crates/mycad-kernel/src/tessellation/mod.rs:435:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/12-cylinder/final-review-2.md:1394:crates/mycad-kernel/src/tessellation/mod.rs:436:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/12-cylinder/final-review-2.md:1395:crates/mycad-kernel/src/tessellation/mod.rs:438:        assert_ne!(mesh_default.triangle_count(), mesh_low.triangle_count());
./features/12-cylinder/final-review-2.md:1396:crates/mycad-kernel/src/tessellation/mod.rs:439:        assert_ne!(mesh_default.triangle_count(), mesh_hi.triangle_count());
./features/12-cylinder/final-review-2.md:1397:crates/mycad-kernel/src/tessellation/mod.rs:442:        assert_eq!(mesh_low.triangle_count(), 8 * 2 + (8 - 2) * 2);
./features/12-cylinder/final-review-2.md:1398:crates/mycad-kernel/src/tessellation/mod.rs:444:        assert_eq!(mesh_hi.triangle_count(), 64 * 2 * 2 + (64 - 2) * 2);
./features/12-cylinder/final-review-2.md:1399:crates/mycad-kernel/src/tessellation/mod.rs:493:        assert_eq!(mesh.triangle_count(), 12);
./features/12-cylinder/final-review-2.md:1400:crates/mycad-kernel/src/tessellation/mod.rs:517:    fn test_zero_segments_clamped_at_use() {
./features/12-cylinder/final-review-2.md:1401:crates/mycad-kernel/src/tessellation/mod.rs:519:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1402:crates/mycad-kernel/src/tessellation/mod.rs:526:        assert_eq!(mesh.triangle_count(), 8);
./features/12-cylinder/final-review-2.md:1403:crates/mycad-kernel/src/tessellation/mod.rs:537:    fn test_partial_revolution_cylinder_face_errors() {
./features/12-cylinder/final-review-2.md:1404:crates/mycad-kernel/src/primitives/mod.rs:5:pub use cylinder::make_cylinder;
./features/12-cylinder/final-review-2.md:1424:crates/mycad-kernel/src/tessellation/mod.rs:198:        matches!(edge.curve, Curve::Circle { .. })
./features/12-cylinder/final-review-2.md:1425:crates/mycad-kernel/src/tessellation/mod.rs:456:        let e0 = s.add_edge(
./features/12-cylinder/final-review-2.md:1426:crates/mycad-kernel/src/tessellation/mod.rs:541:        let e0 = s.add_edge(
./features/12-cylinder/final-review-2.md:1427:crates/mycad-kernel/src/tessellation/mod.rs:544:            Curve::Circle {
./features/12-cylinder/final-review-2.md:1445:- [P1] Tighten the full-revolution span check — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:199-199
./features/12-cylinder/final-review-2.md:1448:- [P2] Accept full cylinders whose circular bounds are edge-split — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:195-200
./features/12-cylinder/final-review-2.md:1454:- [P1] Tighten the full-revolution span check — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:199-199
./features/12-cylinder/final-review-2.md:1457:- [P2] Accept full cylinders whose circular bounds are edge-split — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:195-200
./.claude/skills/3ai/agents/codex-design-reviewer.md:37:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review-3.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review-3.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
./features/12-cylinder/design-review-3.md:94:  - `crates/mycad-kernel/src/tessellation/mod.rs` — 曲線折れ線化 + 曲面 UV グリッド対応 + `TessellationOptions`
./features/12-cylinder/design-review-3.md:98:  - テスト: `cylinder.rs` 内インライン / `tessellation/mod.rs` 内 / `mycad-build/tests/feature_dispatcher.rs` / `mycad-cli/tests/export.rs`
./features/12-cylinder/design-review-3.md:223:/bin/bash -lc "rg -n \"make_cuboid|tessellate_solid|IdGenerator|KernelError|HalfEdge|inner_loops|Surface::|Curve::|TriangleMesh|Feature::CreateCylinder|CreateCylinder|UnsupportedFeature|InvalidParameter|mod.rs|pub use\" crates/mycad-kernel crates/mycad-build crates/mycad-cli -g '"'!target'"'" in /home/bacon/mycad
./features/12-cylinder/design-review-3.md:229:crates/mycad-kernel/src/lib.rs:7:pub use brep::topology;
./features/12-cylinder/design-review-3.md:230:crates/mycad-kernel/src/lib.rs:8:pub use error::KernelError;
./features/12-cylinder/design-review-3.md:253:crates/mycad-kernel/src/tessellation/mod.rs:2:pub use stl::to_ascii_stl;
./features/12-cylinder/design-review-3.md:254:crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
./features/12-cylinder/design-review-3.md:255:crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
./features/12-cylinder/design-review-3.md:256:crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
./features/12-cylinder/design-review-3.md:257:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review-3.md:258:crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/design-review-3.md:259:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:260:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review-3.md:261:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:262:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:263:crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-3.md:264:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:265:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:266:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/design-review-3.md:267:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/design-review-3.md:268:crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/design-review-3.md:269:crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/design-review-3.md:275:crates/mycad-kernel/CLAUDE.md:24:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/12-cylinder/design-review-3.md:319:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-3.md:350:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/12-cylinder/design-review-3.md:360:crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/design-review-3.md:364:crates/mycad-kernel/src/brep/mod.rs
./features/12-cylinder/design-review-3.md:368:crates/mycad-kernel/src/geometry/mod.rs
./features/12-cylinder/design-review-3.md:372:crates/mycad-kernel/src/primitives/mod.rs
./features/12-cylinder/design-review-3.md:868:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review-3.md:871:pub use stl::to_ascii_stl;
./features/12-cylinder/design-review-3.md:1573:crates/mycad-kernel/src/geometry/mod.rs:6:use serde::{Deserialize, Serialize};
./features/12-cylinder/design-review-3.md:1574:crates/mycad-kernel/src/geometry/mod.rs:15:#[derive(Debug, Clone, Serialize, Deserialize)]
./features/12-cylinder/design-review-3.md:1575:crates/mycad-kernel/src/tessellation/mod.rs:6:use serde::{Deserialize, Serialize};
./features/12-cylinder/design-review-3.md:1576:crates/mycad-kernel/src/tessellation/mod.rs:9:#[derive(Debug, Clone, Serialize, Deserialize)]
./features/12-cylinder/design-review-3.md:1581:/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/CLAUDE.md && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-kernel/src/primitives/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review-3.md:1606:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/12-cylinder/design-review-3.md:1625:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-2.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review-2.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
./features/12-cylinder/design-review-2.md:94:  - `crates/mycad-kernel/src/tessellation/mod.rs` — 曲線折れ線化 + 曲面 UV グリッド対応 + `TessellationOptions`
./features/12-cylinder/design-review-2.md:98:  - テスト: `cylinder.rs` 内インライン / `tessellation/mod.rs` 内 / `mycad-build/tests/feature_dispatcher.rs` / `mycad-cli/tests/export.rs`
./features/12-cylinder/design-review-2.md:225:/home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/design-review-2.md:228:/home/bacon/mycad/crates/mycad-kernel/src/geometry/mod.rs
./features/12-cylinder/design-review-2.md:231:/home/bacon/mycad/crates/mycad-kernel/src/primitives/mod.rs
./features/12-cylinder/design-review-2.md:510:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-2.md:557:/bin/bash -lc "sed -n '1,320p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:560:pub use stl::to_ascii_stl;
./features/12-cylinder/design-review-2.md:1040:crates/mycad-kernel/src/tessellation/mod.rs:76:            let n = if face.same_sense {
./features/12-cylinder/design-review-2.md:1041:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:1042:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1043:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1044:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1421:pub use component::Component;
./features/12-cylinder/design-review-2.md:1422:pub use document::Document;
./features/12-cylinder/design-review-2.md:1423:pub use error::FormatError;
./features/12-cylinder/design-review-2.md:1424:pub use feature::Feature;
./features/12-cylinder/design-review-2.md:1430:crates/mycad-format/src/lib.rs:7:pub use document::Document;
./features/12-cylinder/design-review-2.md:1488:pub use brep::topology;
./features/12-cylinder/design-review-2.md:1489:pub use error::KernelError;
./features/12-cylinder/design-review-2.md:1845:/bin/bash -lc "rg -n \"ADR-001|ADR-004|Feature history|source of truth|index-based|mod.rs|pub use\" -g '*.md' -g '*.rs' ." in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1848:./crates/mycad-format/src/lib.rs:6:pub use component::Component;
./features/12-cylinder/design-review-2.md:1849:./crates/mycad-format/src/lib.rs:7:pub use document::Document;
./features/12-cylinder/design-review-2.md:1850:./crates/mycad-format/src/lib.rs:8:pub use error::FormatError;
./features/12-cylinder/design-review-2.md:1851:./crates/mycad-format/src/lib.rs:9:pub use feature::Feature;
./features/12-cylinder/design-review-2.md:1854:./crates/mycad-kernel/src/lib.rs:7:pub use brep::topology;
./features/12-cylinder/design-review-2.md:1855:./crates/mycad-kernel/src/lib.rs:8:pub use error::KernelError;
./features/12-cylinder/design-review-2.md:1857:./features/12-cylinder/design-review.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review-2.md:1859:./features/12-cylinder/design-review.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
./features/12-cylinder/design-review-2.md:1860:./features/12-cylinder/design-review.md:94:  - `crates/mycad-kernel/src/tessellation/mod.rs` — 曲線折れ線化 + 曲面 UV グリッド対応 + `TessellationOptions`
./features/12-cylinder/design-review-2.md:1861:./features/12-cylinder/design-review.md:98:  - テスト: `cylinder.rs` 内インライン / `tessellation/mod.rs` 内 / `mycad-build/tests/feature_dispatcher.rs` / `mycad-cli/tests/export.rs`
./features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1864:./features/12-cylinder/design-review.md:274:crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/design-review-2.md:1865:./features/12-cylinder/design-review.md:279:crates/mycad-kernel/src/primitives/mod.rs
./features/12-cylinder/design-review-2.md:1866:./features/12-cylinder/design-review.md:302:crates/mycad-kernel/src/brep/mod.rs
./features/12-cylinder/design-review-2.md:1867:./features/12-cylinder/design-review.md:308:crates/mycad-kernel/src/geometry/mod.rs
./features/12-cylinder/design-review-2.md:1868:./features/12-cylinder/design-review.md:427:CLAUDE.md:57:- **モジュール構造**: 1概念1ファイル。重要な型は `mod.rs` から `pub use` で再エクスポート。
./features/12-cylinder/design-review-2.md:1871:./features/12-cylinder/design-review.md:447:features/12-cylinder/design-review.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review-2.md:1873:./features/12-cylinder/design-review.md:452:features/12-cylinder/design-review.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
./features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1876:./features/12-cylinder/design-review.md:479:.claude/skills/3ai/agents/codex-design-reviewer.md:37:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review-2.md:1877:./features/12-cylinder/design-review.md:492:crates/mycad-format/src/lib.rs:6:pub use component::Component;
./features/12-cylinder/design-review-2.md:1878:./features/12-cylinder/design-review.md:493:crates/mycad-format/src/lib.rs:7:pub use document::Document;
./features/12-cylinder/design-review-2.md:1879:./features/12-cylinder/design-review.md:494:crates/mycad-format/src/lib.rs:8:pub use error::FormatError;
./features/12-cylinder/design-review-2.md:1880:./features/12-cylinder/design-review.md:495:crates/mycad-format/src/lib.rs:9:pub use feature::Feature;
./features/12-cylinder/design-review-2.md:1881:./features/12-cylinder/design-review.md:518:crates/mycad-kernel/src/lib.rs:7:pub use brep::topology;
./features/12-cylinder/design-review-2.md:1882:./features/12-cylinder/design-review.md:519:crates/mycad-kernel/src/lib.rs:8:pub use error::KernelError;
./features/12-cylinder/design-review-2.md:1883:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-2.md:1884:./features/12-cylinder/design-review.md:524:crates/mycad-kernel/src/tessellation/mod.rs:2:pub use stl::to_ascii_stl;
./features/12-cylinder/design-review-2.md:1885:./features/12-cylinder/design-review.md:525:crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
./features/12-cylinder/design-review-2.md:1886:./features/12-cylinder/design-review.md:526:crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
./features/12-cylinder/design-review-2.md:1887:./features/12-cylinder/design-review.md:527:crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
./features/12-cylinder/design-review-2.md:1888:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review-2.md:1889:./features/12-cylinder/design-review.md:529:crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/design-review-2.md:1890:./features/12-cylinder/design-review.md:530:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:1891:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review-2.md:1892:./features/12-cylinder/design-review.md:532:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1893:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/design-review-2.md:1894:./features/12-cylinder/design-review.md:534:crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-2.md:1895:./features/12-cylinder/design-review.md:535:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1896:./features/12-cylinder/design-review.md:536:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1897:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/design-review-2.md:1898:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/design-review-2.md:1899:./features/12-cylinder/design-review.md:539:crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/design-review-2.md:1900:./features/12-cylinder/design-review.md:540:crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/design-review-2.md:1901:./features/12-cylinder/design-review.md:566:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/12-cylinder/design-review-2.md:1902:./features/12-cylinder/design-review.md:594:crates/mycad-kernel/CLAUDE.md:24:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/12-cylinder/design-review-2.md:1903:./features/12-cylinder/design-review.md:1170:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1904:./features/12-cylinder/design-review.md:1173:pub use stl::to_ascii_stl;
./features/12-cylinder/design-review-2.md:1911:./features/12-cylinder/design-review.md:2013:pub use brep::topology;
./features/12-cylinder/design-review-2.md:1912:./features/12-cylinder/design-review.md:2014:pub use error::KernelError;
./features/12-cylinder/design-review-2.md:1913:./features/12-cylinder/design-review.md:2017:/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/geometry/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1914:./features/12-cylinder/design-review.md:2096:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/12-cylinder/design-review-2.md:1916:./features/12-cylinder/design-review.md:2491:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review-2.md:1919:./crates/mycad-kernel/CLAUDE.md:24:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/12-cylinder/design-review-2.md:1924:./crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/12-cylinder/design-review-2.md:1925:./crates/mycad-kernel/src/tessellation/mod.rs:2:pub use stl::to_ascii_stl;
./features/12-cylinder/design-review-2.md:1927:./.claude/skills/3ai/agents/codex-design-reviewer.md:37:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review-2.md:1928:./crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-2.md:1930:./features/12-cylinder/design-review-2.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review-2.md:1932:./features/12-cylinder/design-review-2.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
./features/12-cylinder/design-review-2.md:1933:./features/12-cylinder/design-review-2.md:94:  - `crates/mycad-kernel/src/tessellation/mod.rs` — 曲線折れ線化 + 曲面 UV グリッド対応 + `TessellationOptions`
./features/12-cylinder/design-review-2.md:1934:./features/12-cylinder/design-review-2.md:98:  - テスト: `cylinder.rs` 内インライン / `tessellation/mod.rs` 内 / `mycad-build/tests/feature_dispatcher.rs` / `mycad-cli/tests/export.rs`
./features/12-cylinder/design-review-2.md:1937:./features/12-cylinder/design-review-2.md:225:/home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/design-review-2.md:1938:./features/12-cylinder/design-review-2.md:228:/home/bacon/mycad/crates/mycad-kernel/src/geometry/mod.rs
./features/12-cylinder/design-review-2.md:1939:./features/12-cylinder/design-review-2.md:231:/home/bacon/mycad/crates/mycad-kernel/src/primitives/mod.rs
./features/12-cylinder/design-review-2.md:1940:./features/12-cylinder/design-review-2.md:510:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1941:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-2.md:1942:./features/12-cylinder/design-review-2.md:557:/bin/bash -lc "sed -n '1,320p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1943:./features/12-cylinder/design-review-2.md:560:pub use stl::to_ascii_stl;
./features/12-cylinder/design-review-2.md:1944:./features/12-cylinder/design-review-2.md:1040:crates/mycad-kernel/src/tessellation/mod.rs:76:            let n = if face.same_sense {
./features/12-cylinder/design-review-2.md:1945:./features/12-cylinder/design-review-2.md:1041:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:1946:./features/12-cylinder/design-review-2.md:1042:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1947:./features/12-cylinder/design-review-2.md:1043:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1948:./features/12-cylinder/design-review-2.md:1044:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1949:./features/12-cylinder/design-review-2.md:1421:pub use component::Component;
./features/12-cylinder/design-review-2.md:1950:./features/12-cylinder/design-review-2.md:1422:pub use document::Document;
./features/12-cylinder/design-review-2.md:1951:./features/12-cylinder/design-review-2.md:1423:pub use error::FormatError;
./features/12-cylinder/design-review-2.md:1952:./features/12-cylinder/design-review-2.md:1424:pub use feature::Feature;
./features/12-cylinder/design-review-2.md:1953:./features/12-cylinder/design-review-2.md:1430:crates/mycad-format/src/lib.rs:7:pub use document::Document;
./features/12-cylinder/design-review-2.md:1954:./features/12-cylinder/design-review-2.md:1488:pub use brep::topology;
./features/12-cylinder/design-review-2.md:1955:./features/12-cylinder/design-review-2.md:1489:pub use error::KernelError;
./features/12-cylinder/design-review-2.md:1956:./CLAUDE.md:57:- **モジュール構造**: 1概念1ファイル。重要な型は `mod.rs` から `pub use` で再エクスポート。
./features/12-cylinder/design-review-2.md:2151:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/12-cylinder/final-review.md:26: crates/mycad-kernel/src/primitives/mod.rs      |   2 +
./features/12-cylinder/final-review.md:27: crates/mycad-kernel/src/tessellation/mod.rs    | 450 ++++++++++++++++++++++---
./features/12-cylinder/final-review.md:937:diff --git a/crates/mycad-kernel/src/primitives/mod.rs b/crates/mycad-kernel/src/primitives/mod.rs
./features/12-cylinder/final-review.md:939:--- a/crates/mycad-kernel/src/primitives/mod.rs
./features/12-cylinder/final-review.md:940:+++ b/crates/mycad-kernel/src/primitives/mod.rs
./features/12-cylinder/final-review.md:945: pub use cuboid::make_cuboid;
./features/12-cylinder/final-review.md:946:+pub use cylinder::make_cylinder;
./features/12-cylinder/final-review.md:947:diff --git a/crates/mycad-kernel/src/tessellation/mod.rs b/crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/final-review.md:949:--- a/crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/final-review.md:950:+++ b/crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/final-review.md:953: pub use stl::to_ascii_stl;
./features/12-cylinder/final-review.md:2443:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/lib.rs && echo '---' && sed -n '1,200p' crates/mycad-kernel/src/primitives/mod.rs" in /home/bacon/mycad
./features/12-cylinder/final-review.md:2451:pub use brep::topology;
./features/12-cylinder/final-review.md:2452:pub use error::KernelError;
./features/12-cylinder/final-review.md:2457:pub use cuboid::make_cuboid;
./features/12-cylinder/final-review.md:2458:pub use cylinder::make_cylinder;
./features/12-cylinder/final-review.md:2473:pub use component::Component;
./features/12-cylinder/final-review.md:2474:pub use document::Document;
./features/12-cylinder/final-review.md:2475:pub use error::FormatError;
./features/12-cylinder/final-review.md:2476:pub use feature::Feature;
./features/12-cylinder/final-review.md:2610:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/tessellation/mod.rs && echo '---PART2---' && sed -n '260,520p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
./features/12-cylinder/final-review.md:2613:pub use stl::to_ascii_stl;
./features/12-cylinder/final-review.md:3239:./features/12-cylinder/design-review-3.md:257:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3240:./features/12-cylinder/design-review-3.md:263:crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/final-review.md:3241:./features/12-cylinder/design-review-3.md:268:crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/final-review.md:3242:./features/12-cylinder/design-review-3.md:269:crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/final-review.md:3304:./features/12-cylinder/design-review-2.md:1888:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3305:./features/12-cylinder/design-review-2.md:1894:./features/12-cylinder/design-review.md:534:crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/final-review.md:3306:./features/12-cylinder/design-review-2.md:1899:./features/12-cylinder/design-review.md:539:crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/final-review.md:3307:./features/12-cylinder/design-review-2.md:1900:./features/12-cylinder/design-review.md:540:crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/final-review.md:3315:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3316:./features/12-cylinder/design-review.md:534:crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/final-review.md:3317:./features/12-cylinder/design-review.md:539:crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/final-review.md:3318:./features/12-cylinder/design-review.md:540:crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/final-review.md:3336:./crates/mycad-kernel/src/tessellation/mod.rs:84:pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:3337:./crates/mycad-kernel/src/tessellation/mod.rs:297:        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:3338:./crates/mycad-kernel/src/tessellation/mod.rs:320:        let m1 = tessellate_solid(&s1).unwrap();
./features/12-cylinder/final-review.md:3339:./crates/mycad-kernel/src/tessellation/mod.rs:321:        let m2 = tessellate_solid(&s2).unwrap();
./features/12-cylinder/final-review.md:3340:./crates/mycad-kernel/src/tessellation/mod.rs:333:        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:3341:./crates/mycad-kernel/src/tessellation/mod.rs:350:        let m1 = tessellate_solid(&s1).unwrap();
./features/12-cylinder/final-review.md:3342:./crates/mycad-kernel/src/tessellation/mod.rs:351:        let m2 = tessellate_solid(&s2).unwrap();
./features/12-cylinder/final-review.md:3343:./crates/mycad-kernel/src/tessellation/mod.rs:363:        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:3344:./crates/mycad-kernel/src/tessellation/mod.rs:418:        let mesh_default = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:3345:./crates/mycad-kernel/src/tessellation/mod.rs:463:        let result = tessellate_solid(&s);
./features/12-cylinder/final-review.md:3346:./crates/mycad-kernel/src/tessellation/mod.rs:475:        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:3347:./crates/mycad-kernel/src/tessellation/mod.rs:491:        let mesh1 = tessellate_solid(&solid1).unwrap();
./features/12-cylinder/final-review.md:3348:./crates/mycad-kernel/src/tessellation/mod.rs:492:        let mesh2 = tessellate_solid(&solid2).unwrap();
./features/12-cylinder/final-review.md:3351:/bin/bash -lc 'git show 360f324''^:crates/mycad-kernel/src/tessellation/mod.rs | sed -n '"'1,220p'" in /home/bacon/mycad
./features/12-cylinder/final-review.md:3354:pub use stl::to_ascii_stl;
./features/12-cylinder/final-review.md:3657:crates/mycad-kernel/src/tessellation/mod.rs:277:            Surface::Cylinder { .. } => "cylinder",
./features/12-cylinder/final-review.md:3702:1574:crates/mycad-kernel/src/geometry/mod.rs:15:#[derive(Debug, Clone, Serialize, Deserialize)]
./features/12-cylinder/final-review.md:3703:1576:crates/mycad-kernel/src/tessellation/mod.rs:9:#[derive(Debug, Clone, Serialize, Deserialize)]
./features/12-cylinder/final-review.md:3873:/bin/bash -lc "nl -ba crates/mycad-kernel/src/tessellation/mod.rs | sed -n '1,260p'" in /home/bacon/mycad
./features/12-cylinder/final-review.md:3876:     2	pub use stl::to_ascii_stl;
./features/12-cylinder/final-review.md:4137:/bin/bash -lc "nl -ba crates/mycad-kernel/src/tessellation/mod.rs | sed -n '260,520p'" in /home/bacon/mycad
./features/12-cylinder/final-review.md:4420:- [P2] Restrict UV-grid tessellation to canonical full cylinders — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:205-205
./features/12-cylinder/final-review.md:4423:- [P2] Validate segment counts when consuming tessellation options — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:214-215
./features/12-cylinder/final-review.md:4429:- [P2] Restrict UV-grid tessellation to canonical full cylinders — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:205-205
./features/12-cylinder/final-review.md:4432:- [P2] Validate segment counts when consuming tessellation options — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:214-215
./crates/mycad-kernel/CLAUDE.md:24:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./crates/mycad-kernel/src/lib.rs:7:pub use brep::topology;
./crates/mycad-kernel/src/lib.rs:8:pub use error::KernelError;
./features/18-api-skeleton/design-review.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/18-api-skeleton/design-review.md:94:| `crates/mycad-api/src/lib.rs` | 新規。Router 構築関数 `pub fn app() -> Router`、ハンドラ、`ApiError` |
./features/18-api-skeleton/design-review.md:107:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
./features/18-api-skeleton/design-review.md:109:enum ApiError { NotFound, BadExtension(String), Format(String), Build(String), Tessellate(String) }
./features/18-api-skeleton/design-review.md:110:impl IntoResponse for ApiError { /* status + {"error": "..."} */ }
./features/18-api-skeleton/design-review.md:118:- `tessellate_solid_with(&Solid, &TessellationOptions) -> Result<TriangleMesh, TessellationError>`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
./features/18-api-skeleton/design-review.md:119:- `TriangleMesh`(同 mod.rs:16)は `Serialize` 済み → `Json(mesh)` でそのまま返却。
./features/18-api-skeleton/design-review.md:134:- `FormatError`/`KernelError`/`TessellationError` は `Serialize` 非対応 → `ApiError` enum に `.to_string()` で集約し、`IntoResponse` で status + `{"error": "..."}` を返す。
./features/18-api-skeleton/design-review.md:176:/bin/bash -lc 'rg --files /home/bacon/mycad/crates | rg "mycad-(cli|format|build|kernel)|Cargo.toml$|mod.rs$|document.rs$|main.rs$|tessellation"' in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:197:/home/bacon/mycad/crates/mycad-kernel/src/primitives/mod.rs
./features/18-api-skeleton/design-review.md:201:/home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs
./features/18-api-skeleton/design-review.md:205:/home/bacon/mycad/crates/mycad-kernel/src/brep/mod.rs
./features/18-api-skeleton/design-review.md:209:/home/bacon/mycad/crates/mycad-kernel/src/geometry/mod.rs
./features/18-api-skeleton/design-review.md:213:/bin/bash -lc 'rg -n "struct IdGenerator|pub struct IdGenerator|run_export|build_solid_from_features|tessellate_solid_with|struct TriangleMesh|enum FormatError|pub enum FormatError|JsonSchema|thiserror|workspace.dependencies|mod.rs|HalfEdge|Euler|Serialize|Deserialize" -S Cargo.toml crates' in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:242:crates/mycad-kernel/CLAUDE.md:24:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/18-api-skeleton/design-review.md:252:crates/mycad-kernel/src/tessellation/mod.rs:8:use serde::{Deserialize, Serialize};
./features/18-api-skeleton/design-review.md:253:crates/mycad-kernel/src/tessellation/mod.rs:10:use thiserror::Error;
./features/18-api-skeleton/design-review.md:254:crates/mycad-kernel/src/tessellation/mod.rs:17:#[derive(Debug, Clone, Serialize, Deserialize)]
./features/18-api-skeleton/design-review.md:255:crates/mycad-kernel/src/tessellation/mod.rs:18:pub struct TriangleMesh {
./features/18-api-skeleton/design-review.md:256:crates/mycad-kernel/src/tessellation/mod.rs:49:#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
./features/18-api-skeleton/design-review.md:257:crates/mycad-kernel/src/tessellation/mod.rs:86:    tessellate_solid_with(solid, &TessellationOptions::default())
./features/18-api-skeleton/design-review.md:258:crates/mycad-kernel/src/tessellation/mod.rs:90:pub fn tessellate_solid_with(
./features/18-api-skeleton/design-review.md:259:crates/mycad-kernel/src/tessellation/mod.rs:445:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/18-api-skeleton/design-review.md:260:crates/mycad-kernel/src/tessellation/mod.rs:446:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/18-api-skeleton/design-review.md:261:crates/mycad-kernel/src/tessellation/mod.rs:525:    /// (public fields bypass new() clamping, so tessellate_solid_with must guard at point of use).
./features/18-api-skeleton/design-review.md:262:crates/mycad-kernel/src/tessellation/mod.rs:534:        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
./features/18-api-skeleton/design-review.md:275:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/18-api-skeleton/design-review.md:284:crates/mycad-kernel/src/geometry/mod.rs:6:use serde::{Deserialize, Serialize};
./features/18-api-skeleton/design-review.md:285:crates/mycad-kernel/src/geometry/mod.rs:15:#[derive(Debug, Clone, Serialize, Deserialize)]
./features/18-api-skeleton/design-review.md:791:/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:794:pub use stl::to_ascii_stl;
./features/18-api-skeleton/design-review.md:1015:/bin/bash -lc "sed -n '220,520p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:1958:pub use brep::topology;
./features/18-api-skeleton/design-review.md:1959:pub use error::KernelError;
./features/18-api-skeleton/design-review.md:1987:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/18-api-skeleton/plan.md:20:| `crates/mycad-api/src/lib.rs` | 新規。Router 構築関数 `pub fn app() -> Router`、ハンドラ、`ApiError` |
./features/18-api-skeleton/plan.md:33:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
./features/18-api-skeleton/plan.md:35:enum ApiError { NotFound, BadExtension(String), Format(String), Build(String), Tessellate(String) }
./features/18-api-skeleton/plan.md:36:impl IntoResponse for ApiError { /* status + {"error": "..."} */ }
./features/18-api-skeleton/plan.md:44:- `tessellate_solid_with(&Solid, &TessellationOptions) -> Result<TriangleMesh, TessellationError>`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
./features/18-api-skeleton/plan.md:45:- `TriangleMesh`(同 mod.rs:16)は `Serialize` 済み → `Json(mesh)` でそのまま返却。
./features/18-api-skeleton/plan.md:60:- `FormatError`/`KernelError`/`TessellationError` は `Serialize` 非対応 → `ApiError` enum に `.to_string()` で集約し、`IntoResponse` で status + `{"error": "..."}` を返す。
./crates/mycad-kernel/src/tessellation/mod.rs:2:pub use stl::to_ascii_stl;
./crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
./crates/mycad-kernel/src/primitives/mod.rs:5:pub use cylinder::make_cylinder;

codex
設計原則の明文化をもう一段確認します。`feature history = source of truth` と index-based topology の記述がどこまで固定ルールかを押さえます。
exec
/bin/bash -lc "sed -n '1,180p' CLAUDE.md" in /home/bacon/mycad
 succeeded in 0ms:
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
├── mycad-build/      Featureディスパッチャー (format → kernel の橋渡し)
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

exec
/bin/bash -lc 'rg -n "Feature history|source of truth|index-based|Index-based|flat array|インデックス|pointer|ポインタ" -S CLAUDE.md crates features .claude' in /home/bacon/mycad
 succeeded in 0ms:
CLAUDE.md:62:- **Index-based topology**: `Solid` 内のトポロジーエンティティはフラット配列に格納し、インデックスで参照（ポインタ不使用）。
CLAUDE.md:63:- **Feature history = source of truth**: `mycad-format` の Feature 列が設計の真実の源。B-rep は Feature から再生成される。
.claude/skills/3ai/agents/glm-implementer.md:24:- Index-based topology: ポインタを使わず `Solid` 内のフラット配列へのインデックスで参照
.claude/skills/3ai/agents/glm-implementer.md:25:- HalfEdge: `twin`/`next`/`prev` インデックスが循環的に正しく設定されているか確認
.claude/skills/3ai/agents/codex-final-reviewer.md:14:- HalfEdge の twin/next/prev インデックスが循環的に正しく設定されているか
.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
features/12-cylinder/design-review.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
features/12-cylinder/design-review.md:48:- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
features/12-cylinder/design-review.md:49:- Feature history = source of truth の原則と矛盾しないか
features/12-cylinder/design-review.md:411:.claude/skills/3ai/agents/glm-implementer.md:25:- HalfEdge: `twin`/`next`/`prev` インデックスが循環的に正しく設定されているか確認
features/12-cylinder/design-review.md:430:.claude/skills/3ai/agents/codex-final-reviewer.md:14:- HalfEdge の twin/next/prev インデックスが循環的に正しく設定されているか
features/12-cylinder/design-review.md:433:.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
features/12-cylinder/design-review.md:442:features/12-cylinder/design-review.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
features/12-cylinder/design-review.md:474:.claude/skills/3ai/agents/codex-design-reviewer.md:15:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
features/12-cylinder/design-review.md:590:crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
features/12-cylinder/design-review.md:679:/// Contains all topological entities in flat arrays for deterministic ordering.
features/12-cylinder/design-review.md:1953:- トポロジーエンティティはフラット配列に格納（index-based）
features/12-cylinder/design-review.md:1981:- ADR-001 のとおり **Feature history が真実の源で、B-rep は Feature から再生成される**。`.mycad` に永続化されるのは Feature 列であり B-rep ではないため、`Surface` / `Curve` / `Edge` の内部表現は後で改修してもファイル移行が不要。後付けコストが構造的に低い。
features/12-cylinder/design-review.md:2088:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
features/12-cylinder/design-review.md:2406:### Index-based Topology
features/12-cylinder/design-review.md:2408:`Solid` 内のトポロジーエンティティ（Vertex, Edge, Face 等）はフラット配列に格納し、ポインタではなくインデックスで相互参照する。
features/12-cylinder/design-review.md:2469:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
features/12-cylinder/design-review.md:2489:- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
features/12-cylinder/design-review.md:2490:- Feature history = source of truth の原則と矛盾しないか
features/12-cylinder/design-review.md:2531:    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
features/12-cylinder/design-review.md:2561:    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
.claude/skills/3ai/agents/codex-design-reviewer.md:15:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
.claude/skills/3ai/agents/codex-design-reviewer.md:35:- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
.claude/skills/3ai/agents/codex-design-reviewer.md:36:- Feature history = source of truth の原則と矛盾しないか
features/12-cylinder/final-review.md:1723:/// Contains all topological entities in flat arrays for deterministic ordering.
features/12-cylinder/design-review-2.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
features/12-cylinder/design-review-2.md:48:- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
features/12-cylinder/design-review-2.md:49:- Feature history = source of truth の原則と矛盾しないか
features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/12-cylinder/design-review-2.md:1204:/// Contains all topological entities in flat arrays for deterministic ordering.
features/12-cylinder/design-review-2.md:1845:/bin/bash -lc "rg -n \"ADR-001|ADR-004|Feature history|source of truth|index-based|mod.rs|pub use\" -g '*.md' -g '*.rs' ." in /home/bacon/mycad
features/12-cylinder/design-review-2.md:1853:./docs/decisions/001-brep-representation.md:28:- トポロジーエンティティはフラット配列に格納（index-based）
features/12-cylinder/design-review-2.md:1856:./features/12-cylinder/design-review.md:49:- Feature history = source of truth の原則と矛盾しないか
features/12-cylinder/design-review-2.md:1906:./features/12-cylinder/design-review.md:1953:- トポロジーエンティティはフラット配列に格納（index-based）
features/12-cylinder/design-review-2.md:1909:./features/12-cylinder/design-review.md:1981:- ADR-001 のとおり **Feature history が真実の源で、B-rep は Feature から再生成される**。`.mycad` に永続化されるのは Feature 列であり B-rep ではないため、`Surface` / `Curve` / `Edge` の内部表現は後で改修してもファイル移行が不要。後付けコストが構造的に低い。
features/12-cylinder/design-review-2.md:1915:./features/12-cylinder/design-review.md:2490:- Feature history = source of truth の原則と矛盾しないか
features/12-cylinder/design-review-2.md:1922:./docs/decisions/004-freeform-geometry-commitment.md:22:- ADR-001 のとおり **Feature history が真実の源で、B-rep は Feature から再生成される**。`.mycad` に永続化されるのは Feature 列であり B-rep ではないため、`Surface` / `Curve` / `Edge` の内部表現は後で改修してもファイル移行が不要。後付けコストが構造的に低い。
features/12-cylinder/design-review-2.md:1926:./.claude/skills/3ai/agents/codex-design-reviewer.md:36:- Feature history = source of truth の原則と矛盾しないか
features/12-cylinder/design-review-2.md:1929:./features/12-cylinder/design-review-2.md:49:- Feature history = source of truth の原則と矛盾しないか
features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/12-cylinder/design-review-2.md:1957:./CLAUDE.md:63:- **Feature history = source of truth**: `mycad-format` の Feature 列が設計の真実の源。B-rep は Feature から再生成される。
features/12-cylinder/design-review-2.md:2047:- トポロジーエンティティはフラット配列に格納（index-based）
features/12-cylinder/design-review-2.md:2092:- ADR-001 のとおり **Feature history が真実の源で、B-rep は Feature から再生成される**。`.mycad` に永続化されるのは Feature 列であり B-rep ではないため、`Surface` / `Curve` / `Edge` の内部表現は後で改修してもファイル移行が不要。後付けコストが構造的に低い。
features/12-cylinder/design-review-2.md:2143:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
features/12-cylinder/final-review-2.md:1231:/// Contains all topological entities in flat arrays for deterministic ordering.
crates/mycad-build/Cargo.toml:3:description = "Feature dispatcher: converts .mycad Feature history into kernel geometry"
features/12-cylinder/design-review-3.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
features/12-cylinder/design-review-3.md:48:- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
features/12-cylinder/design-review-3.md:49:- Feature history = source of truth の原則と矛盾しないか
features/12-cylinder/design-review-3.md:149:  2. `BoundaryFan`（平面） → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。巻き方向は §1 で構築済み（底キャップ rev / 上キャップ fwd）なので fan の幾何法線が外向きになる。**スワップなし → cuboid のインデックス列もバイト不変**（T08 で固定）。
features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/12-cylinder/design-review-3.md:186:- R02(high) **検証して反映**: cuboid.rs / stl.rs を読み確認 → 「実行時の向き整合スワップは cuboid 出力不変」という前回主張は **誤り**（現行 cuboid のインデックス巻きが右手系外向き規約と一致する保証はなく、スワップ導入で index 列が変わり T08 が回帰しうる）。→ **テッセレータからスワップを撤去**し、外向きは §1 の**構築時巻き方向**（底キャップ=e_bot rev / 上キャップ=e_top fwd / 側面グリッド=∂u×∂v 外向き）で保証。平面 fan 経路は現行コードとバイト同一 → cuboid 完全不変（T08 で固定）。
features/12-cylinder/design-review-3.md:271:crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
features/12-cylinder/design-review-3.md:456:/// Contains all topological entities in flat arrays for deterministic ordering.
features/12-cylinder/design-review-3.md:1598:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
features/12-cylinder/design-review-3.md:2008:description = "Feature dispatcher: converts .mycad Feature history into kernel geometry"
crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
features/18-api-skeleton/design-review.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
features/18-api-skeleton/design-review.md:48:- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
features/18-api-skeleton/design-review.md:49:- Feature history = source of truth の原則と矛盾しないか
features/18-api-skeleton/design-review.md:240:crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
features/18-api-skeleton/design-review.md:369:/// Contains all topological entities in flat arrays for deterministic ordering.
features/18-api-skeleton/design-review.md:1979:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
features/18-api-skeleton/design-review.md:2408:description = "Feature dispatcher: converts .mycad Feature history into kernel geometry"
features/18-api-skeleton/design-review.md:2598:./features/12-cylinder/design-review-2.md:1845:/bin/bash -lc "rg -n \"ADR-001|ADR-004|Feature history|source of truth|index-based|mod.rs|pub use\" -g '*.md' -g '*.rs' ." in /home/bacon/mycad
features/18-api-skeleton/design-review.md:2792:設計原則の明文化をもう一段確認します。`feature history = source of truth` と index-based topology の記述がどこまで固定ルールかを押さえます。
crates/mycad-kernel/src/brep/topology.rs:78:/// Contains all topological entities in flat arrays for deterministic ordering.

exec
/bin/bash -lc "sed -n '1,220p' docs/decisions/001-brep-representation.md" in /home/bacon/mycad
 succeeded in 0ms:
# ADR-001: B-rep (Boundary Representation) の採用

## Status

Accepted

## Context

3D CAD カーネルの形状表現方式を選択する必要がある。主な候補:

1. **CSG (Constructive Solid Geometry)**: ブーリアン演算のツリーとして形状を表現
2. **B-rep (Boundary Representation)**: 面・辺・頂点で境界を表現
3. **Voxel / Implicit**: 暗黙的な距離関数で表現

## Decision

B-rep を採用する。

## Rationale

- **エッジ・フェイスへの直接アクセス**: フィレット、面取り、面選択等の操作に必要
- **業界標準**: STEP, IGES 等の標準フォーマットが B-rep ベース
- **テッセレーションが直接的**: 面を直接メッシュ化できる
- **CSG との併用可能**: ブーリアン演算の結果を B-rep として保持

## Implementation Details

- トポロジーエンティティはフラット配列に格納（index-based）
- `Solid` が最上位エンティティ、内部に Shell → Face → Loop → HalfEdge → Vertex の階層
- 各エンティティは `EntityId` を持ち、Feature からの参照に使用

exec
/bin/bash -lc "sed -n '1,220p' docs/decisions/004-freeform-geometry-commitment.md" in /home/bacon/mycad
 succeeded in 0ms:
# ADR-004: 自由曲面・自由曲線を確定要件として扱う

## Status

Accepted

## Context

現在の幾何カーネルは解析曲面・解析曲線のみを持つ:

- `Surface` (`crates/mycad-kernel/src/geometry/surface.rs`): `Plane` / `Cylinder` / `Sphere` / `Cone`
- `Curve` (`crates/mycad-kernel/src/geometry/curve.rs`): `Line` / `Circle`

「解析曲面だけで進められないか」を検討したが、ロードマップ上の操作を踏むと自由曲面・自由曲線は構造的に避けられないと結論した:

- **Boolean 演算 (Phase 4)**: 曲面同士の交線は一般に円・直線にならない。例として円筒 ∩ 球の交わりは 4 次の空間曲線であり、`Curve::Line` / `Curve::Circle` では表せない。→ 自由**曲線** (交線・スプライン) が必須。
- **フィレット・面取り (ADR-001 が動機として挙げた操作)**: 一般のフィレット面や掃引面は解析式に乗らない。→ 自由**曲面** (NURBS 等) が必須。

一方、次の理由から「今すぐ実装する必要はない」:

- `Surface` / `Curve` は Rust の enum であり、variant 追加は加算的。`evaluate` / `normal_at` / tessellation などの `match` はコンパイラが網羅性を強制するため、追加漏れは検出される。
- ADR-001 のとおり **Feature history が真実の源で、B-rep は Feature から再生成される**。`.mycad` に永続化されるのは Feature 列であり B-rep ではないため、`Surface` / `Curve` / `Edge` の内部表現は後で改修してもファイル移行が不要。後付けコストが構造的に低い。

## Decision

自由曲面・自由曲線を「いつか検討する」ものではなく **確定要件** として扱う。ただし NURBS 等の実装は今は行わず (現在 Phase 1)、Boolean が要求する範囲から漸進的に導入する。これに伴い、以降の設計で次を守る:

1. **`Surface` / `Curve` enum を唯一の幾何拡張点**とする。`Nurbs` / スプライン variant の追加が加算的であり続けるよう、アルゴリズムは variant 集合を仮定しない。
2. **アルゴリズムは曲面・曲線の型に非依存**であること。平面・直線前提をアルゴリズムへ埋め込まない (例: 面法線は面ごとに 1 回でなく、点ごとに `Surface::normal_at_point` で評価する)。
3. **数値モデル (トレラント方式 vs 厳密方式) は本 ADR では保留**し、曲面 Boolean を実装する Phase 4 着手時に決定する。自由曲面の交線は数値的近似になるため、業界カーネル (Parasolid / ACIS) はエンティティ毎に公差を持つトレラントモデルを採る。これを後から全エンティティ・全比較へ導入するのは大改修であり、Phase 4 で意思決定する論点として明示しておく。なお `CLAUDE.md` の **決定性** 原則とは両立可能 (アルゴリズム固定で再現できる) だが、数値ロバスト性の確保が決定性維持の難所になる点に留意する。

## Rationale

- **要件確定だけ先に行う理由**: 後付けが致命的に痛い横断的決定 (上記 2・3) を「事故的に」決めてしまわないため。実装そのもの (NURBS 評価・Boolean アルゴリズム) は enum + Feature 再生成により安全に後回しでき、先回り実装はむしろ過剰設計になる。
- **専用 Phase を立てない理由**: 自由曲面を独立フェーズにすると、どの操作がそれを必要とするかと切り離されて陳腐化する (ADR-002 の「空想 Issue は作らない」と同趣旨)。Boolean / フィレットが要求した時点で必要分を入れる。

## Implementation Details

- 本 ADR は ADR-001 (B-rep 採用) を拡張する位置づけ。番号 003 は ADR-002 で Viewer スタック用に予約済みのため 004 を使用。
- **未決の横断的論点 (Phase 4 で再検討)**:
  - **pcurve (パラメータ空間曲線)**: トリムされた周期・自由曲面を tessellation / Boolean で安定に扱うには、half-edge ごとに面のパラメータ空間上の 2D 曲線を持たせる構造が事実上必要になりうる (OpenCASCADE 等が保持)。現在の `Edge` は 3D `curve + t_range` のみで pcurve を持たない。
  - **数値モデル (トレラント vs 厳密)**: 上記 Decision 3。
- 本 ADR 採択に伴う即時対応: `tessellate_solid` が面法線を `normal_at(0.0, 0.0)` で 1 回だけ評価していた平面前提を解消し、`Surface::normal_at_point` による頂点毎評価へ変更した (平面では従来と同一の結果)。

exec
/bin/bash -lc "sed -n '1,200p' crates/mycad-kernel/src/primitives/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
mod cuboid;
mod cylinder;

pub use cuboid::make_cuboid;
pub use cylinder::make_cylinder;

exec
/bin/bash -lc "sed -n '2480,2625p' features/12-cylinder/design-review.md" in /home/bacon/mycad
 succeeded in 0ms:
- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）

### 5. テスト計画の充足性
- 決定性反復テスト（T0x 系）があるか
- Euler-Poincaré 検証テストがあるか
- 退化入力エッジケースが十分にあるか
- golden YAML ラウンドトリップテストがあるか

### 6. アーキテクチャ整合性
- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
- Feature history = source of truth の原則と矛盾しないか
- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか

---

## 出力フォーマット（厳守）

```yaml
issues:
  - id: R01
    severity: critical  # critical | high | medium | low
    section: "設計方針 > 決定性"
    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
    suggestion: "IdGenerator::next() に置き換えること"
  - id: R02
    severity: medium
    ...

verdict: pass  # pass | fail
# fail = Critical または High が1件以上ある
```

`issues` が空の場合は `issues: []` と書く。
**このフォーマット以外の出力は禁止。前置きや後置きの文章は書かない。**

codex
issues:
  - id: R01
    severity: critical
    section: "設計方針 > 2. テッセレータ（ADR-004 準拠の加算拡張）"
    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
  - id: R02
    severity: high
    section: "設計方針 > 1. トポロジー / 2. テッセレータ / 3. ディスパッチ / CLI"
    finding: "退化入力を『正値前提』としてスコープ外にしており、`radius <= 0`/`height <= 0` に加えて公開 API/CLI から入る `angular_segments < 3` や `axial_segments < 1` も無検証のままになっている。ゼロ長 seam、面積ゼロ cap、空または縮退メッシュを生成しうる。"
    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
  - id: R03
    severity: high
    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
  - id: R04
    severity: medium
    section: "設計方針 > 2. テッセレータ"
    finding: "数値境界の方針が未記載で、`uv_of` の seam アンラップ、幾何法線と `normal_at_point` の向き比較、ゼロ面積三角形の除外条件が実装者任せになっている。決定性はあっても数値ロバスト性の再現条件が固定されない。"
    suggestion: "`geometry`/`tessellation` で共有する epsilon 定数と比較規約を設計に明記し、実装とテストで同じ閾値を使うこと。"
  - id: R05
    severity: medium
    section: "テスト計画"
    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
    suggestion: "`Document::from_path` -> `to_yaml` -> `from_yaml` -> `to_yaml` の equality を `examples/cylinder.mycad` に対して追加し、`create_cylinder` の schema/serialize 回帰を固定すること。"

verdict: fail
tokens used
122,623
issues:
  - id: R01
    severity: critical
    section: "設計方針 > 2. テッセレータ（ADR-004 準拠の加算拡張）"
    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
  - id: R02
    severity: high
    section: "設計方針 > 1. トポロジー / 2. テッセレータ / 3. ディスパッチ / CLI"
    finding: "退化入力を『正値前提』としてスコープ外にしており、`radius <= 0`/`height <= 0` に加えて公開 API/CLI から入る `angular_segments < 3` や `axial_segments < 1` も無検証のままになっている。ゼロ長 seam、面積ゼロ cap、空または縮退メッシュを生成しうる。"
    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
  - id: R03
    severity: high
    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
  - id: R04
    severity: medium
    section: "設計方針 > 2. テッセレータ"
    finding: "数値境界の方針が未記載で、`uv_of` の seam アンラップ、幾何法線と `normal_at_point` の向き比較、ゼロ面積三角形の除外条件が実装者任せになっている。決定性はあっても数値ロバスト性の再現条件が固定されない。"
    suggestion: "`geometry`/`tessellation` で共有する epsilon 定数と比較規約を設計に明記し、実装とテストで同じ閾値を使うこと。"
  - id: R05
    severity: medium
    section: "テスト計画"
    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
    suggestion: "`Document::from_path` -> `to_yaml` -> `from_yaml` -> `to_yaml` の equality を `examples/cylinder.mycad` に対して追加し、`create_cylinder` の schema/serialize 回帰を固定すること。"

verdict: fail

exec
/bin/bash -lc 'rg -n "''^issues:|''^verdict:|id: R" -n features/18-api-skeleton/design-review.md features/18-api-skeleton/plan.md' in /home/bacon/mycad
 succeeded in 0ms:
features/18-api-skeleton/design-review.md:57:issues:
features/18-api-skeleton/design-review.md:58:  - id: R01
features/18-api-skeleton/design-review.md:63:  - id: R02
features/18-api-skeleton/design-review.md:67:verdict: pass  # pass | fail

exec
/bin/bash -lc "sed -n '1,120p' features/18-api-skeleton/design-review.md" in /home/bacon/mycad
 succeeded in 0ms:
Reading additional input from stdin...
OpenAI Codex v0.121.0 (research preview)
--------
workdir: /home/bacon/mycad
model: gpt-5.4
provider: openai
approval: never
sandbox: read-only
reasoning effort: xhigh
reasoning summaries: none
session id: 019e5dac-0790-7bb1-a206-adf6c3cfcd2b
--------
user
# Codex 設計レビュアー（MyCad CAD カーネル専用）

あなたは Rust 製 B-rep CAD カーネル「MyCad」の設計ドキュメントをレビューする専門家です。
stdin に渡された設計・テスト計画ドキュメントを読み、以下の観点で問題を指摘してください。

## レビュー観点

### 1. 決定性（MyCad の核心）
- `IdGenerator` を使った決定的 ID 生成になっているか
- 同一入力で必ず同一出力（同一 ID・同一座標）が保証されているか
- 非決定的要素（HashMap のイテレーション順、thread_rng など）が混入していないか

### 2. B-rep トポロジー妥当性
- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
- Loop/Shell/Solid の入れ子構造が一貫しているか

### 3. 退化幾何の扱い
- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
- 数値的境界（浮動小数点の epsilon 比較）の方針が明記されているか

### 4. API・型設計
- `Debug, Clone, Serialize, Deserialize` が付与されているか（JsonSchema が必要な型は追記）
- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
- ライブラリエラーには `thiserror` を使用しているか
- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）

### 5. テスト計画の充足性
- 決定性反復テスト（T0x 系）があるか
- Euler-Poincaré 検証テストがあるか
- 退化入力エッジケースが十分にあるか
- golden YAML ラウンドトリップテストがあるか

### 6. アーキテクチャ整合性
- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
- Feature history = source of truth の原則と矛盾しないか
- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか

---

## 出力フォーマット（厳守）

```yaml
issues:
  - id: R01
    severity: critical  # critical | high | medium | low
    section: "設計方針 > 決定性"
    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
    suggestion: "IdGenerator::next() に置き換えること"
  - id: R02
    severity: medium
    ...

verdict: pass  # pass | fail
# fail = Critical または High が1件以上ある
```

`issues` が空の場合は `issues: []` と書く。
**このフォーマット以外の出力は禁止。前置きや後置きの文章は書かない。**

<stdin>
# Issue #18: API 層スケルトン (mycad-api / axum)

## Context

Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。

既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。

## 実装対象

- 新規クレート `crates/mycad-api/`(lib + bin 構成)
- Issue: #18 / Milestone: Phase 2

### 影響ファイル

| ファイル | 変更 |
|----------|------|
| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
| `crates/mycad-api/Cargo.toml` | 新規。既存規約(`version.workspace = true` 等)に従う。`[[bin]]` で binary 名 `mycad-api` |
| `crates/mycad-api/src/lib.rs` | 新規。Router 構築関数 `pub fn app() -> Router`、ハンドラ、`ApiError` |
| `crates/mycad-api/src/main.rs` | 新規。`#[tokio::main]`、`127.0.0.1:3000` バインド |
| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` による統合テスト |

### シグネチャ

```rust
// lib.rs
pub fn app() -> axum::Router;

#[derive(serde::Deserialize)]
struct MeshQuery { file: String }

async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;

enum ApiError { NotFound, BadExtension(String), Format(String), Build(String), Tessellate(String) }
impl IntoResponse for ApiError { /* status + {"error": "..."} */ }
```

## 設計方針

### 再利用(新規ロジックを書かない)
- `Document::from_path(&Path) -> Result<Document, FormatError>`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
- `build_solid_from_features(&[Feature], &mut IdGenerator) -> Result<Solid, KernelError>`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ対応。
- `tessellate_solid_with(&Solid, &TessellationOptions) -> Result<TriangleMesh, TessellationError>`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
- `TriangleMesh`(同 mod.rs:16)は `Serialize` 済み → `Json(mesh)` でそのまま返却。
- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き、末尾を `Ok(Json(mesh))` に置換した形。

exec
/bin/bash -lc "sed -n '57,90p' features/18-api-skeleton/design-review.md" in /home/bacon/mycad
 succeeded in 0ms:
issues:
  - id: R01
    severity: critical  # critical | high | medium | low
    section: "設計方針 > 決定性"
    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
    suggestion: "IdGenerator::next() に置き換えること"
  - id: R02
    severity: medium
    ...

verdict: pass  # pass | fail
# fail = Critical または High が1件以上ある
```

`issues` が空の場合は `issues: []` と書く。
**このフォーマット以外の出力は禁止。前置きや後置きの文章は書かない。**

<stdin>
# Issue #18: API 層スケルトン (mycad-api / axum)

## Context

Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。

既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。

## 実装対象

- 新規クレート `crates/mycad-api/`(lib + bin 構成)
- Issue: #18 / Milestone: Phase 2

### 影響ファイル

| ファイル | 変更 |

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/error.rs" in /home/bacon/mycad
 succeeded in 0ms:
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("empty feature list")]
    EmptyFeatureList,

    #[error("multiple features not yet supported (got {count})")]
    MultipleFeatures { count: usize },

    #[error("unsupported feature variant: {kind}")]
    UnsupportedFeature { kind: &'static str },

    #[error("invalid parameter: {kind}")]
    InvalidParameter { kind: &'static str },
}

ERROR: Reconnecting... 2/5
codex
想定エッジケースの既存フィクスチャ有無だけ確認します。計画がそのまま流用できる素材を持っているか見ています。
exec
/bin/bash -lc 'find examples -maxdepth 2 -type f | sort' in /home/bacon/mycad
 succeeded in 0ms:
examples/assembly.mycad
examples/cylinder.mycad
examples/simple_box.mycad

exec
/bin/bash -lc 'rg -n "create_sphere|create_cylinder|width: 0|height: 0|depth: 0|multiple features|unsupported" examples crates/mycad-format/tests crates/mycad-build/tests crates/mycad-cli/tests crates/mycad-kernel/tests -S' in /home/bacon/mycad
 exited 2 in 0ms:
rg: crates/mycad-format/tests: No such file or directory (os error 2)
examples/cylinder.mycad:5:    - type: create_cylinder

exec
/bin/bash -lc "sed -n '1,120p' examples/cylinder.mycad" in /home/bacon/mycad
 succeeded in 0ms:
version: "0.1.0"
root_component:
  name: "Simple Cylinder"
  features:
    - type: create_cylinder
      id: cyl_1
      radius: 5.0
      height: 20.0

codex
issues:
  - id: R01
    severity: high
    section: "設計方針 > パス安全性 / 決定性"
    finding: "`file` クエリの相対パス解決規則が API 契約として固定されていない。計画自身が tests では `env!(\"CARGO_MANIFEST_DIR\")` 起点の絶対パス化を要求する一方、E2E 例は `examples/simple_box.mycad` の相対パスを使っており、同じ HTTP 入力でもサーバ起動 CWD によって結果が変わる。"
    suggestion: "絶対パスのみ受け付けるか、`app(state)` に明示的な base directory を持たせて相対パス解決を固定し、その規則を T02 と E2E で検証すること。"
  - id: R02
    severity: high
    section: "設計方針 > 再利用(新規ロジックを書かない) / 退化幾何"
    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
    suggestion: "Box/Cylinder を含む退化入力の検証ポイントと error mapping を設計に追加し、少なくとも `0`/負値寸法 fixture を 422 で落とすテストを入れること。比較閾値は共有定数として明記すること。"
  - id: R03
    severity: medium
    section: "設計方針 > 再利用(新規ロジックを書かない) / アーキテクチャ整合性"
    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
    suggestion: "v0 を『root_component に単一 part feature を持つ文書のみ対応』と明記して assembly 非対応を別エラーに分けるか、Document/Component 階層を解釈する document-level build API を設計すること。T06 は専用 fixture に差し替えること。"
  - id: R04
    severity: medium
    section: "実装対象 > 影響ファイル / API・型設計"
    finding: "`crates/mycad-api/src/lib.rs` に Router・handler・`ApiError` を同居させる計画は、リポジトリの『1概念1ファイル、重要な型は再エクスポート』規約と合っていない。"
    suggestion: "`router.rs` / `handler.rs` / `error.rs` などへ分割し、`lib.rs` は `pub use` 中心にすること。"
  - id: R05
    severity: medium
    section: "テスト計画"
    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
    suggestion: "API crate 側で fixture から `Solid` を作り `V - E + F = 2(S - H)` を確認する回帰テストを追加し、`examples/cylinder.mycad` について `200`/決定性/`from_path -> to_yaml -> from_yaml -> to_yaml` 完全一致を追加すること。"
verdict: fail
tokens used
98,294
issues:
  - id: R01
    severity: high
    section: "設計方針 > パス安全性 / 決定性"
    finding: "`file` クエリの相対パス解決規則が API 契約として固定されていない。計画自身が tests では `env!(\"CARGO_MANIFEST_DIR\")` 起点の絶対パス化を要求する一方、E2E 例は `examples/simple_box.mycad` の相対パスを使っており、同じ HTTP 入力でもサーバ起動 CWD によって結果が変わる。"
    suggestion: "絶対パスのみ受け付けるか、`app(state)` に明示的な base directory を持たせて相対パス解決を固定し、その規則を T02 と E2E で検証すること。"
  - id: R02
    severity: high
    section: "設計方針 > 再利用(新規ロジックを書かない) / 退化幾何"
    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
    suggestion: "Box/Cylinder を含む退化入力の検証ポイントと error mapping を設計に追加し、少なくとも `0`/負値寸法 fixture を 422 で落とすテストを入れること。比較閾値は共有定数として明記すること。"
  - id: R03
    severity: medium
    section: "設計方針 > 再利用(新規ロジックを書かない) / アーキテクチャ整合性"
    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
    suggestion: "v0 を『root_component に単一 part feature を持つ文書のみ対応』と明記して assembly 非対応を別エラーに分けるか、Document/Component 階層を解釈する document-level build API を設計すること。T06 は専用 fixture に差し替えること。"
  - id: R04
    severity: medium
    section: "実装対象 > 影響ファイル / API・型設計"
    finding: "`crates/mycad-api/src/lib.rs` に Router・handler・`ApiError` を同居させる計画は、リポジトリの『1概念1ファイル、重要な型は再エクスポート』規約と合っていない。"
    suggestion: "`router.rs` / `handler.rs` / `error.rs` などへ分割し、`lib.rs` は `pub use` 中心にすること。"
  - id: R05
    severity: medium
    section: "テスト計画"
    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
    suggestion: "API crate 側で fixture から `Solid` を作り `V - E + F = 2(S - H)` を確認する回帰テストを追加し、`examples/cylinder.mycad` について `200`/決定性/`from_path -> to_yaml -> from_yaml -> to_yaml` 完全一致を追加すること。"
verdict: fail
