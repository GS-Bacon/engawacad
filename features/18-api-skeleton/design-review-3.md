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
session id: 019e5dc1-9272-7a70-b9a3-fa9d29c4365a
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

> Codex 設計レビュー round 1 + round 2 反映済み。round1: R01/R03/R04 受諾、R02 kernel 検証追加(ユーザー決定)、R05 部分受諾。round2: R01(構造化エラー)受諾、R02(assembly 判別)受諾、R03(公差)は #17 委譲で反論、R04(fixture roundtrip)反論。

## 実装対象

- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
- Issue: #18 / Milestone: Phase 2

### v0 スコープ明確化(R03)
- 対応するのは **`root_component` に単一 part feature を持つ文書のみ**(Box / Cylinder)。
- assembly(`children`/`reference` を持ち root features が空)や複数フィーチャは v0 非対応。明確なエラー + 422 系で返す(クラッシュさせない)。

### 影響ファイル

| ファイル | 変更 |
|----------|------|
| `crates/mycad-kernel/src/primitives/cuboid.rs` | `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0.0` なら `InvalidParameter { kind }`。**(R03)** 閾値は `make_cylinder`(cylinder.rs:16-22)と同じ exact `<= 0.0` で対称化。near-zero 正値の公差判定は導入しない(下記 R03 注記)。インライン test を `.unwrap()` 化 |
| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
| `crates/mycad-api/Cargo.toml` | 新規。`version.workspace = true` 等の既存規約。`[[bin]]` name=`mycad-api` |
| `crates/mycad-api/src/lib.rs` | 新規。**(R04)** `pub use` 中心の薄い再エクスポートのみ |
| `crates/mycad-api/src/router.rs` | 新規。`pub fn app() -> Router`(`/api/v0` を `nest`) |
| `crates/mycad-api/src/handler.rs` | 新規。`get_mesh` ハンドラ + パス解決ロジック |
| `crates/mycad-api/src/error.rs` | 新規。`ApiError` + `IntoResponse` |
| `crates/mycad-api/src/main.rs` | 新規。`#[tokio::main]`、`127.0.0.1:3000` バインド |
| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` 統合テスト + fixtures |
| `crates/mycad-api/tests/fixtures/*.mycad` | 新規。退化/未対応ケース用 fixture(R02/R03 テスト用) |

### シグネチャ

```rust
// router.rs
pub fn app() -> axum::Router;

// handler.rs
#[derive(serde::Deserialize)]
struct MeshQuery { file: String }
async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;

// error.rs — (R01) status は型から確定。文字列解析しない
enum ApiError {
    BadRequest(String),     // 400
    NotFound(String),       // 404
    Unprocessable(String),  // 422
    Internal(String),       // 500
}
impl From<FormatError>      for ApiError { /* variant 別に 400/404/500 */ }
impl From<KernelError>      for ApiError { /* 全 variant → 422 */ }
impl From<TessellationError> for ApiError { /* 全 variant → 422 */ }
impl IntoResponse for ApiError { /* status は variant から、body = {"error": msg} */ }
```

## 設計方針

### 再利用(新規ロジックを書かない)
- `Document::from_path`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
- `build_solid_from_features`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ。
- `tessellate_solid_with`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
- `TriangleMesh`(mod.rs:16, Serialize 済み)→ `Json(mesh)`。
- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き末尾を `Ok(Json(mesh))` に置換。

### パス解決の契約(R01)
- `file` クエリは **絶対パスならそのまま、相対パスならサーバプロセスの CWD(`std::env::current_dir`)基準**で解決する、と明文化。
- bin は起動時に effective base(CWD)をログ出力。
- テストは `env!("CARGO_MANIFEST_DIR")` 起点で絶対パスを組み立て(`crates/mycad-api` から `../../examples/...`)、CWD 非依存にする。
- E2E 例(`examples/...` 相対)はリポジトリルートを CWD として起動する前提と注記。

### エンドポイント / バージョニング
- `GET /api/v0/mesh?file=<path>`。`Router::nest("/api/v0", ...)`。クエリは `file` のみ(v0 スコープ)。解像度は default 固定。

### パス安全性(多層防御 — 任意位置の .mycad 読取とセキュリティを両立)
1. **CWD ジェイルなし** → 任意パスの `.mycad` を読める(要望)。
2. **`.mycad` 拡張子限定**(`Document::from_path` が既に強制)→ 機密ファイル読取を遮断。
3. **`canonicalize` + 実ファイル存在確認** → 無ければ 404、ディレクトリ等は 400。
4. **`127.0.0.1` のみバインド** → ネットワークから到達不可。
5. **Host ヘッダ検証ミドルウェア** → loopback 以外は 403(DNS リバインディング対策)。

### エラーハンドリング(R01: 型から status を確定。文字列解析禁止)
- `ApiError` は 4 variant(`BadRequest`/`NotFound`/`Unprocessable`/`Internal`)。各下層エラーから `From` で変換し、`IntoResponse` は **variant から status を決め**、body は `{"error": msg}` に整形するだけ。
- `From<FormatError>`: `InvalidExtension`/`Yaml` → 400(`BadRequest`)、`Io(ErrorKind::NotFound)` → 404、その他 `Io`/`InvalidReference` → 500(`Internal`)。
- `From<KernelError>`: `InvalidParameter`(退化寸法)・`UnsupportedFeature`・`MultipleFeatures`・`EmptyFeatureList` → 422(`Unprocessable`)。
- `From<TessellationError>`: `UnsupportedSurface`・`TrimmedFaceUnsupported`・`NonManifoldLoop` → 422。
- handler が assembly/reference 文書を検出した場合(下記 R02)も 422(`Unprocessable`)。

### v0 文書判別(R02: handler で build 前に検査)
- `build_solid_from_features` を呼ぶ前に `root_component` を検査する:
  - `reference.is_some()` または `!children.is_empty()` → assembly/reference 文書 → `ApiError::Unprocessable("assembly/reference は v0 非対応")`。
  - `features.is_empty()`(かつ assembly でない)→ 「part なのに feature が空」として 422。
- これにより `EmptyFeatureList` の意味を「不正な空 part」に限定し、assembly を別メッセージで区別する。

### 依存・規約
- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。

## テスト計画

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
| T09 | 422 assembly 非対応(R03) | fixture: root features 空 + children あり | 422(EmptyFeatureList、クラッシュしない) |
| K01 | kernel 単体 | `make_cuboid(0.0, 1.0, 1.0, ..)` / 負値 / NaN | `Err(InvalidParameter { kind: "..." })` |
| K02 | kernel 単体(R03 境界可視化) | `make_cuboid(1e-9, 1.0, 1.0, ..)`(near-zero 正値) | 現状は `Ok`(exact `<= 0.0` 方針)であることを assert し境界を明文化 |

> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
>
> **R03 判断**(round 2): 退化寸法の閾値は `make_cylinder` と同じ exact `<= 0.0` で対称化する。**グローバル公差(near-zero の扱い)は ROADMAP の Issue #17「単位系・グローバル公差の早期固定(ADR-004 範囲)」が所有する未決事項**であり、#18 で独自 EPS を導入すると #17 の設計判断を先取りしてしまう。よって #18 は exact-zero 方針に留め、near-zero 正値が現状 `Ok` となる境界を K02 で明文化する(将来 #17 で公差確定後に統一)。
>
> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。

## 検証(end-to-end)

```bash
cargo xtask ci                      # fmt → clippy(-D warnings) → test --workspace → build
cargo run -p mycad-api --bin mycad-api &   # 127.0.0.1:3000(repo ルートを CWD に)
curl -s 'http://127.0.0.1:3000/api/v0/mesh?file=examples/simple_box.mycad' | head -c 300   # TriangleMesh JSON
curl -s 'http://127.0.0.1:3000/api/v0/mesh?file=examples/cylinder.mycad'    | head -c 300   # 同上(曲面)
curl -s -o /dev/null -w '%{http_code}\n' 'http://127.0.0.1:3000/api/v0/mesh?file=examples/nope.mycad'  # 404
```

## 3ai フロー上の注記

- **STEP 3(Codex 設計レビュー)はプランモードを抜けずに実行する**(Bash dispatch は plan mode でも通る。deny は `crates/**` の Edit/Write のみ)。本改訂後に再レビュー(round 2/3)し、Critical/High 解消後に STEP 4 `ExitPlanMode` で最終承認。
- 承認後: STEP 5(ブランチ `cad/18-api-skeleton`)→ STEP 6(GLM 実装、`crates/**` は Claude 編集禁止)→ STEP 7(Codex 最終レビュー)→ STEP 8(squash マージ, `Closes #18`)。
</stdin>
codex
設計ドキュメントの整合性を確認するため、関連する既存型とエラー定義だけ読みます。API 契約と既存クレートの意味づけが噛み合っているかを先に見ます。
exec
/bin/bash -lc "rg -n \"enum FormatError|struct TriangleMesh|enum KernelError|enum TessellationError|fn from_path|build_solid_from_features|make_cuboid|make_cylinder|EmptyFeatureList|InvalidReference\" crates -g '*.rs'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-format/src/component.rs:42:            return Err(FormatError::InvalidReference {
crates/mycad-format/src/component.rs:49:                return Err(FormatError::InvalidReference {
crates/mycad-format/src/error.rs:4:pub enum FormatError {
crates/mycad-format/src/error.rs:15:    InvalidReference { value: String, reason: &'static str },
crates/mycad-format/src/document.rs:36:    pub fn from_path(path: &Path) -> Result<Self, FormatError> {
crates/mycad-cli/src/main.rs:2:use mycad_build::build_solid_from_features;
crates/mycad-cli/src/main.rs:55:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
crates/mycad-build/src/lib.rs:6:pub fn build_solid_from_features(
crates/mycad-build/src/lib.rs:11:        return Err(KernelError::EmptyFeatureList);
crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
crates/mycad-build/src/lib.rs:27:        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
crates/mycad-build/tests/feature_dispatcher.rs:1:use mycad_build::build_solid_from_features;
crates/mycad-build/tests/feature_dispatcher.rs:17:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
crates/mycad-kernel/src/error.rs:4:pub enum KernelError {
crates/mycad-kernel/src/error.rs:6:    EmptyFeatureList,
crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
crates/mycad-kernel/src/primitives/mod.rs:5:pub use cylinder::make_cylinder;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
crates/mycad-kernel/src/tessellation/mod.rs:18:pub struct TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:75:pub enum TessellationError {
crates/mycad-kernel/src/tessellation/mod.rs:315:    use crate::primitives::make_cuboid;
crates/mycad-kernel/src/tessellation/mod.rs:316:    use crate::primitives::make_cylinder;
crates/mycad-kernel/src/tessellation/mod.rs:322:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:343:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:344:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:358:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
crates/mycad-kernel/src/tessellation/mod.rs:373:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
crates/mycad-kernel/src/tessellation/mod.rs:374:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
crates/mycad-kernel/src/tessellation/mod.rs:388:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:442:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:500:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
crates/mycad-kernel/src/tessellation/mod.rs:514:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
crates/mycad-kernel/src/tessellation/mod.rs:515:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
crates/mycad-kernel/src/tessellation/mod.rs:529:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:11:pub fn make_cylinder(
crates/mycad-kernel/src/primitives/cylinder.rs:138:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:139:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:165:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:184:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:204:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:258:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:305:        let err = make_cylinder(0.0, 20.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:312:        let err = make_cylinder(-5.0, 20.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:319:        let err = make_cylinder(f64::NAN, 20.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:326:        let err = make_cylinder(f64::INFINITY, 20.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:333:        let err = make_cylinder(5.0, 0.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:340:        let err = make_cylinder(5.0, -10.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:347:        let err = make_cylinder(5.0, f64::NAN, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:354:        let err = make_cylinder(5.0, f64::INFINITY, &mut gen).unwrap_err();
crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);

exec
/bin/bash -lc 'git status --short' in /home/bacon/mycad
 succeeded in 0ms:
 M .claude/skills/3ai/SKILL.md
 M .gitignore
?? .claude/settings.json
?? .claude/skills/3ai/agents/
?? .claude/skills/3ai/scripts/guard-crates.sh
?? features/

codex
既存の `FormatError` と `TriangleMesh`、`make_cuboid`/`make_cylinder` の現状を確認します。レビューの論点になりそうなのは、エラー分類の意味と既存の決定性テストの有無です。
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

exec
/bin/bash -lc "sed -n '1,140p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
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

exec
/bin/bash -lc "sed -n '1,120p' crates/mycad-kernel/src/primitives/cylinder.rs" in /home/bacon/mycad
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

exec
/bin/bash -lc "sed -n '1,120p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
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

exec
/bin/bash -lc "sed -n '1,160p' crates/mycad-build/src/lib.rs" in /home/bacon/mycad
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
/bin/bash -lc "sed -n '1,200p' crates/mycad-format/src/component.rs" in /home/bacon/mycad
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
/bin/bash -lc "sed -n '160,380p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
 succeeded in 0ms:
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

        for (v, exp) in solid.vertices.iter().zip(expected.iter()) {
            assert!(
                (v.point - exp).norm() < 1e-12,
                "vertex {:?} doesn't match expected {:?}",
                v.point,
                exp
            );
        }
    }

    #[test]
    fn test_cuboid_deterministic() {
        // Building the same cuboid twice with the same ID generator should
        // produce identical results.
        let mut id_gen1 = IdGenerator::new(0);
        let mut id_gen2 = IdGenerator::new(0);

        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);

        assert_eq!(solid1.vertices.len(), solid2.vertices.len());
        for (v1, v2) in solid1.vertices.iter().zip(solid2.vertices.iter()) {
            assert_eq!(v1.id, v2.id);
            assert_eq!(v1.point, v2.point);
        }

        assert_eq!(solid1.edges.len(), solid2.edges.len());
        for (e1, e2) in solid1.edges.iter().zip(solid2.edges.iter()) {
            assert_eq!(e1.id, e2.id);
            assert_eq!(e1.vertices, e2.vertices);
        }

        assert_eq!(solid1.faces.len(), solid2.faces.len());
        for (f1, f2) in solid1.faces.iter().zip(solid2.faces.iter()) {
            assert_eq!(f1.id, f2.id);
        }
    }
}

codex
既存の cuboid/cylinder テストにどこまで決定性・Euler 検証があるかを見ます。今回の計画がその既存担保を正しく引用しているかを確認します。
exec
/bin/bash -lc "rg -n \"Euler|V - E \\+ F|2\\(S - H\\)|test_.*euler|deterministic|yaml_is_deterministic\" crates/mycad-kernel crates/mycad-format -g '*.rs'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-format/src/component.rs:12:    /// Rotation as Euler angles in degrees [rx, ry, rz].
crates/mycad-kernel/tests/cuboid_roundtrip.rs:26:/// Integration test: deterministic pipeline produces identical results.
crates/mycad-kernel/tests/cuboid_roundtrip.rs:28:fn cuboid_pipeline_is_deterministic() {
crates/mycad-format/src/document.rs:75:    fn test_yaml_is_deterministic() {
crates/mycad-format/src/document.rs:86:        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
crates/mycad-kernel/src/primitives/cylinder.rs:135:    fn test_cylinder_deterministic() {
crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
crates/mycad-kernel/src/primitives/cylinder.rs:182:    fn test_cylinder_euler() {
crates/mycad-kernel/src/primitives/cylinder.rs:195:            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
crates/mycad-kernel/src/tessellation/mod.rs:339:    fn test_tessellation_cylinder_deterministic() {
crates/mycad-kernel/src/tessellation/mod.rs:510:    fn test_tessellation_deterministic() {
crates/mycad-kernel/src/brep/topology.rs:7:/// Generated deterministically by creation order within a feature.
crates/mycad-kernel/src/brep/topology.rs:78:/// Contains all topological entities in flat arrays for deterministic ordering.
crates/mycad-kernel/src/brep/topology.rs:182:/// Counter for generating deterministic entity IDs.
crates/mycad-kernel/src/brep/topology.rs:207:    fn test_id_generator_is_deterministic() {
crates/mycad-kernel/src/tessellation/stl.rs:72:    fn ascii_stl_deterministic() {
crates/mycad-kernel/src/primitives/cuboid.rs:13:/// - `id_gen`: deterministic ID generator
crates/mycad-kernel/src/primitives/cuboid.rs:273:    fn test_cuboid_deterministic() {

exec
/bin/bash -lc "sed -n '120,240p' crates/mycad-kernel/src/primitives/cylinder.rs" in /home/bacon/mycad
 succeeded in 0ms:
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

exec
/bin/bash -lc "sed -n '330,560p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
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
        assert_eq!(mesh1.normals, mesh2.normals);
        assert_eq!(mesh1.indices, mesh2.indices);
    }
    /// Directly-constructed TessellationOptions with 0 segments should not produce NaN vertices
    /// (public fields bypass new() clamping, so tessellate_solid_with must guard at point of use).
    #[test]
    fn test_zero_segments_clamped_at_use() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let opts = TessellationOptions {
            angular_segments: 0,
            axial_segments: 0,
        };
        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
        // Clamped to angular=3, axial=1: lateral=3*2=6, caps=(3-2)*2=2 → 8
        assert_eq!(mesh.triangle_count(), 8);
        assert!(
            mesh.positions
                .iter()
                .all(|p| p.iter().all(|x| x.is_finite())),
            "mesh must not contain NaN/inf vertices"
        );
    }

    /// Non-full-revolution cylinder face (arc < 2π) must return TrimmedFaceUnsupported.
    #[test]
    fn test_partial_revolution_cylinder_face_errors() {
        use crate::brep::topology::Solid as S;
        let mut s = S::new(0);
        let v0 = s.add_vertex(1, Point::new(5.0, 0.0, 0.0));
        let e0 = s.add_edge(
            2,
            [v0, v0],
            Curve::Circle {
                center: Point::origin(),
                normal: Vec3::z(),
                radius: 5.0,
            },
            [0.0, PI], // half revolution
        );

exec
/bin/bash -lc "sed -n '1,280p' crates/mycad-kernel/src/brep/topology.rs" in /home/bacon/mycad
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
        assert_eq!(solid.vertices[0].point, Point::new(1.0, 2.0, 3.0));
    }
}

exec
/bin/bash -lc "sed -n '1,140p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
 succeeded in 0ms:
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::primitives::make_cuboid;
use mycad_kernel::tessellation::tessellate_solid;

/// Integration test: create a cuboid, tessellate it, and verify the pipeline.
#[test]
fn cuboid_to_mesh_pipeline() {
    let mut id_gen = IdGenerator::new(0);
    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);

    // Verify topology
    assert_eq!(solid.vertices.len(), 8);
    assert_eq!(solid.edges.len(), 12);
    assert_eq!(solid.faces.len(), 6);

    // Tessellate
    let mesh = tessellate_solid(&solid).unwrap();

    // Verify mesh
    assert_eq!(mesh.triangle_count(), 12); // 6 faces * 2 triangles
    assert_eq!(mesh.positions.len(), 24); // 6 faces * 4 vertices
    assert_eq!(mesh.normals.len(), 24);
    assert_eq!(mesh.indices.len(), 36); // 12 triangles * 3 indices
}

/// Integration test: deterministic pipeline produces identical results.
#[test]
fn cuboid_pipeline_is_deterministic() {
    let run = || {
        let mut id_gen = IdGenerator::new(42);
        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
        tessellate_solid(&solid).unwrap()
    };

    let mesh1 = run();
    let mesh2 = run();

    assert_eq!(mesh1.positions, mesh2.positions);
    assert_eq!(mesh1.normals, mesh2.normals);
    assert_eq!(mesh1.indices, mesh2.indices);
}

codex
issues:
  - id: R01
    severity: high
    section: "設計方針 > パス解決の契約(R01)"
    finding: "相対 `file` を `std::env::current_dir()` 基準で解決するため、同じ `GET /api/v0/mesh?file=examples/simple_box.mycad` でもサーバの起動 CWD によって参照先と結果が変わる。これは MyCad の『同一入力→同一出力』に対する隠れた環境依存になる。"
    suggestion: "v0 では絶対パスのみ受け付けるか、起動時に固定 `base_dir` を設定値として保持して相対パスは必ずその固定値基準で解決すること。相対パスを残すなら専用の成功/失敗テストも追加すること。"

  - id: R02
    severity: high
    section: "設計方針 > 再利用(新規ロジックを書かない)"
    finding: "`tessellate_solid_with(..., &TessellationOptions::default())` を API 契約にしており、kernel 側の `Default` 値変更がそのまま `/api/v0` の三角形数・頂点順序を変えうる。URL で `v0` を切っていてもメッシュ仕様が静かにドリフトする。"
    suggestion: "API v0 専用の明示的な `TessellationOptions` 定数を定義して固定値で呼び出し、`Default` への依存を外すこと。"

  - id: R03
    severity: medium
    section: "設計方針 > エラーハンドリング(R01)"
    finding: "`FormatError::InvalidReference` を 500 に分類しているが、これは `ref` 文字列が空/不正なユーザー入力で発生する既知の文書不正であり、内部障害ではない。クライアント起因の失敗を 500 にすると原因切り分けと再試行判定を誤る。"
    suggestion: "`InvalidReference` は 400 もしくは 422 に寄せ、500 は I/O 異常や想定外の内部故障に限定すること。"

  - id: R04
    severity: medium
    section: "テスト計画 > T09"
    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
    suggestion: "T09 は assembly/reference 専用メッセージを明示的に assert し、`features.is_empty()` の空 part は別テストに分離すること。"

  - id: R05
    severity: low
    section: "テスト計画 > K01"
    finding: "`make_cuboid` の新仕様は `non-finite` を拒否すると書いているが、K01 の列挙は `0.0`/負値/`NaN` までで、`INFINITY`/`-INFINITY` の分岐が未検証である。"
    suggestion: "`dx/dy/dz` ごとに `f64::INFINITY` と `f64::NEG_INFINITY` を追加し、新しい入力検証の全分岐を網羅すること。"

verdict: fail
tokens used
90,712
issues:
  - id: R01
    severity: high
    section: "設計方針 > パス解決の契約(R01)"
    finding: "相対 `file` を `std::env::current_dir()` 基準で解決するため、同じ `GET /api/v0/mesh?file=examples/simple_box.mycad` でもサーバの起動 CWD によって参照先と結果が変わる。これは MyCad の『同一入力→同一出力』に対する隠れた環境依存になる。"
    suggestion: "v0 では絶対パスのみ受け付けるか、起動時に固定 `base_dir` を設定値として保持して相対パスは必ずその固定値基準で解決すること。相対パスを残すなら専用の成功/失敗テストも追加すること。"

  - id: R02
    severity: high
    section: "設計方針 > 再利用(新規ロジックを書かない)"
    finding: "`tessellate_solid_with(..., &TessellationOptions::default())` を API 契約にしており、kernel 側の `Default` 値変更がそのまま `/api/v0` の三角形数・頂点順序を変えうる。URL で `v0` を切っていてもメッシュ仕様が静かにドリフトする。"
    suggestion: "API v0 専用の明示的な `TessellationOptions` 定数を定義して固定値で呼び出し、`Default` への依存を外すこと。"

  - id: R03
    severity: medium
    section: "設計方針 > エラーハンドリング(R01)"
    finding: "`FormatError::InvalidReference` を 500 に分類しているが、これは `ref` 文字列が空/不正なユーザー入力で発生する既知の文書不正であり、内部障害ではない。クライアント起因の失敗を 500 にすると原因切り分けと再試行判定を誤る。"
    suggestion: "`InvalidReference` は 400 もしくは 422 に寄せ、500 は I/O 異常や想定外の内部故障に限定すること。"

  - id: R04
    severity: medium
    section: "テスト計画 > T09"
    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
    suggestion: "T09 は assembly/reference 専用メッセージを明示的に assert し、`features.is_empty()` の空 part は別テストに分離すること。"

  - id: R05
    severity: low
    section: "テスト計画 > K01"
    finding: "`make_cuboid` の新仕様は `non-finite` を拒否すると書いているが、K01 の列挙は `0.0`/負値/`NaN` までで、`INFINITY`/`-INFINITY` の分岐が未検証である。"
    suggestion: "`dx/dy/dz` ごとに `f64::INFINITY` と `f64::NEG_INFINITY` を追加し、新しい入力検証の全分岐を網羅すること。"

verdict: fail
