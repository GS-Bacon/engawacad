# Issue #18: API 層スケルトン (mycad-api / axum)

## Context

Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。

既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。

> Codex 設計レビュー round 1-3 反映済み(3回上限)。round3 残存指摘は全て受諾し最終修正済み: R01=絶対パスのみ受理(決定的)、R02=v0 専用テッセレーション定数で Default 依存排除、R03=InvalidReference→422、R04=T09/T10 で assembly と空 part を分離、R05=K01 に INFINITY 分岐追加。round3 の verdict は fail のままだが指摘内容は本改訂で解消済み(ユーザー判断で再レビューせず確定)。

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
- `tessellate_solid_with`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。**(R02)** API v0 専用の固定定数 `const V0_TESSELLATION: TessellationOptions = TessellationOptions { angular_segments: 32, axial_segments: 1 }` を api crate に定義して渡す。kernel の `Default` には依存しない(Default 変更でメッシュ仕様が静かにドリフトするのを防ぐ)。
- `TriangleMesh`(mod.rs:16, Serialize 済み)→ `Json(mesh)`。
- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き末尾を `Ok(Json(mesh))` に置換。

### パス解決の契約(R01: 絶対パスのみ — 決定的)
- `file` クエリは **絶対パスのみ受理**する。相対パスは `400 BadRequest`(理由: CWD 依存だと同一 HTTP 入力でもサーバ起動 CWD で結果が変わり「同一入力→同一出力」に反する)。
- これにより「任意の位置の `.mycad` を読める」要望は満たしつつ、CWD 非依存で決定的になる。
- テストは `env!("CARGO_MANIFEST_DIR")` 起点で絶対パスを組み立てる(`crates/mycad-api` から `../../examples/...` を `canonicalize`)。
- 相対パス拒否の専用テスト(T11)を追加。

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
- `From<FormatError>`: `InvalidExtension`/`Yaml` → 400(`BadRequest`)、`Io(ErrorKind::NotFound)` → 404、`InvalidReference` → 422(`Unprocessable`、ユーザー入力起因の文書不正)(R03)、その他 `Io` → 500(`Internal`)。
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
| T09 | 422 assembly 非対応(R04) | fixture: root features 空 + children あり | 422、body に assembly/reference 専用メッセージ(空 part とは別経路) |
| T10 | 422 空 part(R04) | fixture: 単一 part だが features が空(children/ref なし) | 422(EmptyFeatureList、assembly とは別メッセージ) |
| T11 | 400 相対パス(R01) | `file=examples/simple_box.mycad`(相対) | 400(相対パス拒否) |
| K01 | kernel 単体 | `make_cuboid` の `0.0` / 負値 / `NaN` / `INFINITY` / `-INFINITY`(dx/dy/dz 各々)(R05) | 全分岐で `Err(InvalidParameter { kind: "..." })` |
| K02 | kernel 単体(R03 境界可視化) | `make_cuboid(1e-9, 1.0, 1.0, ..)`(near-zero 正値) | 現状は `Ok`(exact `<= 0.0` 方針)であることを assert し境界を明文化 |

> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
>
> **R03 判断**(round 2): 退化寸法の閾値は `make_cylinder` と同じ exact `<= 0.0` で対称化する。**グローバル公差(near-zero の扱い)は ROADMAP の Issue #17「単位系・グローバル公差の早期固定(ADR-004 範囲)」が所有する未決事項**であり、#18 で独自 EPS を導入すると #17 の設計判断を先取りしてしまう。よって #18 は exact-zero 方針に留め、near-zero 正値が現状 `Ok` となる境界を K02 で明文化する(将来 #17 で公差確定後に統一)。
>
> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。

## 検証(end-to-end)

```bash
cargo xtask ci                      # fmt → clippy(-D warnings) → test --workspace → build
cargo run -p mycad-api --bin mycad-api &   # 127.0.0.1:3000
ROOT=$(pwd)   # 絶対パスのみ受理(R01)
curl -s "http://127.0.0.1:3000/api/v0/mesh?file=$ROOT/examples/simple_box.mycad" | head -c 300   # TriangleMesh JSON
curl -s "http://127.0.0.1:3000/api/v0/mesh?file=$ROOT/examples/cylinder.mycad"    | head -c 300   # 同上(曲面)
curl -s -o /dev/null -w '%{http_code}\n' "http://127.0.0.1:3000/api/v0/mesh?file=$ROOT/examples/nope.mycad"  # 404
curl -s -o /dev/null -w '%{http_code}\n' 'http://127.0.0.1:3000/api/v0/mesh?file=examples/simple_box.mycad'  # 400(相対)
```

## 3ai フロー上の注記

- **STEP 3(Codex 設計レビュー)はプランモードを抜けずに実行する**(Bash dispatch は plan mode でも通る。deny は `crates/**` の Edit/Write のみ)。本改訂後に再レビュー(round 2/3)し、Critical/High 解消後に STEP 4 `ExitPlanMode` で最終承認。
- 承認後: STEP 5(ブランチ `cad/18-api-skeleton`)→ STEP 6(GLM 実装、`crates/**` は Claude 編集禁止)→ STEP 7(Codex 最終レビュー)→ STEP 8(squash マージ, `Closes #18`)。
