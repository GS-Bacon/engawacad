# #32 build: Component 内マルチボディモデル — feature_id→Solid 複数共存

## Context（なぜやるか）

現 `build_solid_from_features`（`crates/mycad-build/src/lib.rs:8`）は `solid: Option<Solid>` で
**単一ボディしか持てず**、create 系 Feature が2つ以上あると `MultipleFeatures` エラーになる。
Boolean の `Cut/Fuse/Intersect { target, tool }` は2つのボディを feature_id で参照して
共存させる必要があるため、Boolean アルゴリズム（#33 平面 / #34 曲面）の**前提となる土台**として
マルチボディ build モデルを実装する。

本 Issue が作るのは「土台」: ① feature_id→Solid の複数ボディ管理、② Boolean が target/tool を
id 参照で解決できる経路（存在しなければエラー）。Boolean の幾何演算本体は #33/#34 が担当する。

加えてユーザー要望により **Web UI（ビューア）が複数立体を「ボディ毎に別メッシュ」で
色分け表示**できるようにする（API 応答を per-body 配列化）。STL export は単一ファイル合成（merge）のまま。

**スコープ外**: Component 階層越し・複数 occurrence 参照（Phase 5、ADR-005 が明示延期）。
本 Issue は単一 component の `features` リスト内に閉じる。Boolean 幾何アルゴリズム本体。
per-body 配置（transform）は無いため複数ボディは各プリミティブの正準位置（原点中心等）に
重なり得る。これは #32 の cosmetic 制約として許容し、ビューアは色分けで区別する（空間配置は将来）。

## 実装対象

- 影響クレート/ファイル:
  - `crates/mycad-build/src/lib.rs` — build モデルをマルチボディへ変更（中心）
  - `crates/mycad-kernel/src/error.rs` — `BodyNotFound` 追加・`MultipleFeatures` 削除
  - `crates/mycad-kernel/src/tessellation/mod.rs` — `merge_meshes` 追加（cli export 用）
  - `crates/mycad-cli/src/main.rs:71` — export: 全ボディ tessellate→`merge_meshes`→STL
  - `crates/mycad-api/src/handler.rs:34` — get_mesh: per-body `Vec<BodyMesh>` を返す（merge しない）
  - `crates/mycad-api/src/transport/` — `BodyMesh { feature_id, mesh }` 追加（`TS` derive）
  - `crates/xtask/src/main.rs` — `gen_ts()` roots と drift テスト一覧に `BodyMesh` 追加＋golden
  - `web/src/api.ts` — `/api/v0/mesh` を `BodyMesh[]` で受ける
  - `web/src/viewer.ts` — 各ボディを別 Three.js Mesh として色分け描画・全体 bbox にカメラフィット
  - `web/src/main.ts` — 配列を `initViewer` へ渡す
  - `web/src/generated/BodyMesh.ts` — gen-ts 生成物（コミット対象）
  - `crates/mycad-build/tests/feature_dispatcher.rs` — テスト更新・追加
  - `crates/mycad-api/tests/mesh_api.rs` — 応答が配列になった点を反映・更新
  - `examples/two_bodies.mycad` — **XY 位置をずらした2つの押し出し角柱**で空間的に**重ならない2体**。
    確定構成: `create_sketch id=sketch_a`（矩形プロファイル 原点付近）→ `create_sketch id=sketch_b`
    （矩形プロファイル X=+50 付近）→ `extrude id=body_a sketch=sketch_a` → `extrude id=body_b sketch=sketch_b`。
    ボディは extrude が生成するため **feature_id 順 = `["body_a","body_b"]`**。transform 不在の #32 でも
    真に「並べて」表示でき、merge した STL も交差せず明確。cli export / api テスト / ブラウザ確認に共用。

### 変更する型・関数のシグネチャ

```rust
// mycad-build/src/lib.rs（新規・公開型）
// 注: BuiltBodies/Body はメモリ上の build 成果物で永続化も wire 送信もしない
// （wire 型は mycad-api 側の Vec<BodyMesh>）。よって Serialize/Deserialize は付けない
// → mycad-build に serde 依存を増やさない＆ HashMap 直列化の非決定性問題(R02)を回避。
#[derive(Debug, Clone)]
pub struct Body {
    pub feature_id: String,
    pub solid: Solid,
}

#[derive(Debug, Clone, Default)]
pub struct BuiltBodies {
    bodies: Vec<Body>,                 // feature 生成順（出力順の真実）
    index: std::collections::HashMap<String, usize>, // id→bodies の位置（ルックアップ専用）
}

impl BuiltBodies {
    pub fn get(&self, feature_id: &str) -> Option<&Body>;
    pub fn all(&self) -> &[Body];      // 生成順スライス
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

// build_solid_from_features を置換（CLAUDE.md「互換 shim 禁止」）
pub fn build_bodies_from_features(
    features: &[Feature],
    gen: &mut IdGenerator,
) -> Result<BuiltBodies, KernelError>;

// kernel/tessellation/mod.rs（新規・公開関数。cli export の merge 用）
pub fn merge_meshes(meshes: &[TriangleMesh]) -> TriangleMesh;

// kernel/error.rs
// 追加: BodyNotFound { id: String }  ("referenced body not found: {id}")
// 削除: MultipleFeatures { count }   ← 単一ボディ制約が消えるため obsolete

// mycad-api/src/transport/（新規・公開型。TS 生成対象）
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BodyMesh {
    pub feature_id: String,
    pub mesh: TriangleMesh,
}

// mycad-api/src/handler.rs — 応答型を変更
// get_mesh -> Result<Json<Vec<BodyMesh>>, ApiError>
//   各ボディを tessellate し [{feature_id, mesh}, ...] を bodies.all() 順で返す（merge しない）
```

## 設計方針

### マルチボディ管理（決定性が肝 — ADR-005 §9）
- `bodies: Vec<Body>` を feature 生成順で保持。**出力・反復は必ずこの Vec 順**を使う。
- `index: HashMap<String, usize>` は **id 引きルックアップ専用**。HashMap の反復順は一切出力に使わない。
- 単一 `IdGenerator` を全 create feature に通すため、body1 が id 0..n、body2 が n.. … と
  決定的に連番付与される（既存の gen フローと同一）。同一入力→同一 EntityId・座標が保証される。

### `consumed`/`live` フラグは入れない（YAGNI）
Boolean が結果ボディを生んで入力を畳む処理は #33/#34 のスコープ。#32 では Boolean が
結果ボディを生まない（後述の通り `UnsupportedFeature` で停止）ため、`consumed` フラグは
**常に未使用の dead code**になる。CLAUDE.md「投機的設計・中途半端な実装の禁止」に従い入れない。
よって #32 では「live ボディ = 全ボディ」。consume の概念は #33 が結果ボディ生成と同時に導入する。

### Boolean feature の扱い（土台の境界）
`Cut/Fuse/Intersect` に到達したら:
1. `index` で `target`・`tool` を引く。**いずれか無ければ `BodyNotFound { id }`** を返す。
   - 処理は生成順なので、後方で定義される body はこの時点で `index` に未登録 →
     前方参照は自動的に `BodyNotFound` になる（前方参照禁止を満たす）。
2. 参照が両方とも解決できても**幾何演算は未実装**なので `UnsupportedFeature { kind }` を返す。
   → 「参照解決は実装・幾何は #33/#34」という土台の境界を明示。
   acceptance test「存在しない id → KernelError」はステップ1で、未実装は順序的にステップ2で満たす。

### 複数ボディ出力（cli=merge / Web=per-body の2系統）

**cli export (STL)** — 1ファイルなので全ボディを合成:
```rust
// API と同じ単一 component 前提を fail-closed で先にチェック（R02）:
// root.reference.is_some() || !root.children.is_empty() → assembly/reference は dedicated error で拒否。
// （子ボディを黙って無視した部分 STL を出力しないため）
let root = &doc.root_component;
if root.reference.is_some() || !root.children.is_empty() {
    return Err("assembly/reference documents are not supported".into());
}
let bodies = build_bodies_from_features(&root.features, &mut gen)?;
let meshes = bodies.all().iter()
    .map(|b| tessellate_solid_with(&b.solid, &opts))
    .collect::<Result<Vec<_>, _>>()?;
let mesh = merge_meshes(&meshes);  // → to_ascii_stl
```
`merge_meshes` は positions/normals を連結し、各メッシュの indices に**それまでの頂点数を
オフセット加算**して連結（決定的）。kernel は build 層を知らない（メッシュのみ扱う）ため
`&[TriangleMesh]` を取り、body の反復順は cli 側で `bodies.all()` 順に固定して決定性を保つ。

**api get_mesh (Web)** — ボディ毎に別メッシュ（色分け描画のため merge しない）:
```rust
let bodies = build_bodies_from_features(&root.features, &mut gen)?;
let out: Vec<BodyMesh> = bodies.all().iter()
    .map(|b| Ok(BodyMesh { feature_id: b.feature_id.clone(),
                           mesh: tessellate_solid_with(&b.solid, &V0_TESSELLATION)? }))
    .collect::<Result<_, TessellationError>>()?;
Ok(Json(out))   // [{feature_id, mesh}, ...] を bodies.all() 順で（決定的）
```
既存のアセンブリ拒否（`root.children`/`reference`）・空 features エラーは維持。

### Web フロントエンド（ボディ毎に色分け描画）
- `api.ts`: `fetchMesh()` → `fetchBodies(): Promise<BodyMesh[]>` に変更（配列を返す）。
- `viewer.ts`: `initViewer(container, bodies: BodyMesh[])`。各 body について
  `meshToGeometry(body.mesh)` で geometry を作り、**固定パレット**（例: 6色の配列）を
  index でサイクルして `MeshPhongMaterial` の色に割当て、各 `Mesh` を scene に追加。
  カメラフィットは全 Mesh を含む `Group`/`Box3` 合成 bbox に対して行う（現行ロジックを bbox 合成へ拡張）。
  `meshToGeometry`/`validateMesh`（`mesh.ts`）は per-mesh のまま再利用（変更不要）。
- `main.ts`: `fetchBodies()` の配列を `initViewer(app, bodies)` へ。空配列時はエラー表示。
- 色割当ては body 順（=feature 順）で決定的。重なり配置でも色で区別できる。

### TS 型生成（ts-rs）への登録
`crates/xtask/src/main.rs`:
- `gen_ts()` の `roots` に `("BodyMesh", Box::new(BodyMesh::export_all))` を追加
  （`use mycad_api::transport::BodyMesh;`）。
- drift テストの生成ファイル一覧に `"BodyMesh.ts"` を追加し、決定性比較対象にする。
- `BodyMesh` の golden（Feature/TriangleMesh と同様の exact 文字列）テストを追加。
- `cargo xtask gen-ts` を実行し `web/src/generated/BodyMesh.ts` をコミット（drift チェックを通す）。

### ゼロボディの扱い
sketch のみで create solid が無い Document は body 0 個 → 既存挙動を維持し `EmptyFeatureList` を返す
（旧 `solid.ok_or(EmptyFeatureList)` と同じ）。`features.is_empty()` の先頭チェックも維持。

### 既存ロジックの維持
- `seen_ids` による全 feature の重複 id 検出（`DuplicateFeatureId`）を維持。
- `validate_feature_id` / `validate_sketch_segment_ids` / `validate_profile_closed` をそのまま維持。
- sketch 登録・extrude 解決ロジックは維持（extrude はボディを生成し index 登録）。
- 削除: `count_solid_features`（`MultipleFeatures` と共に obsolete）。grep で他参照が無いことを確認して削除。

### B-rep 妥当性・退化幾何
本 Issue は新規幾何を生成せず、既存プリミティブ（make_cuboid 等、検証済み）を複数並べるだけ。
各ボディの V-E+F=2 は各プリミティブが既に保証。退化幾何の新規発生経路は無い。

### derive 規約 / エラー / 依存
- `Body`/`BuiltBodies` は `Debug, Clone`（+ `BuiltBodies` に `Default`）のみ。永続化・wire 送信
  されないため `Serialize/Deserialize/JsonSchema` は付けない（mycad-build に serde 依存を増やさない）。
  wire 型の `BodyMesh`（mycad-api、serde 既存）には従来どおり `Debug, Clone, Serialize, Deserialize, TS`。
- `KernelError` は `thiserror`、既存スタイルで `BodyNotFound { id: String }` 追加。
- 新規 workspace 依存は追加しない（mycad-build の Cargo.toml も変更不要）。

## テスト計画（ID 付き）

mycad-build/tests/feature_dispatcher.rs（既存success系は新APIへ更新）＋ kernel tessellation inline。

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 2体 Document を2回 build → body数・feature_id順・各 body の `solid.id` **と全 topological entity の ID・参照・座標**（vertices/edges/half_edges/loops/faces/shells を field-by-field）が完全一致。`Solid` 比較ヘルパーで明示比較し mesh 比較には頼らない | assert_eq! |
| T02 | 正常系(複数create) | box+cylinder を build → `len()==2`、`get("box1")`/`get("cyl1")` が Some、各 solid の topology 数検証、`all()` の順序確認 | Some + counts |
| T03 | 正常系(単一・回帰) | 既存 simple_box/sphere/extruded_rect/cylinder を新 API で build、従来 topology 数（V/E/F/shell）維持 | counts |
| T04 | 参照エラー | `Cut{target:"missing", tool:"box1"}`（box1 は先行 create）→ `BodyNotFound{id:"missing"}` | matches! |
| T05 | 参照解決後未実装 | `Cut{target:"box1", tool:"box2"}`（両方先行 create）→ `UnsupportedFeature{kind:"cut"}`。Fuse/Intersect も同様 | matches! |
| T06 | 前方参照禁止 | `Cut` が後方で定義される body を参照 → `BodyNotFound` | matches! |
| T07 | 重複id(回帰) | 重複 feature id → `DuplicateFeatureId` | err string contains |
| T08 | merge正常 | `merge_meshes(&[m1,m2])`: positions/normals = 連結、m2 由来 index に m1.positions.len() オフセット加算 | assert_eq! |
| T09 | merge決定性 | 同一2ボディを2回 build→各tessellate→merge が positions/normals/indices 完全一致 | assert_eq! |
| T10 | merge空/単一 | `merge_meshes(&[])` は空メッシュ、`&[m1]` は m1 と同一 | assert_eq! |
| T11 | ゼロボディ | sketch のみ Document → `EmptyFeatureList` | matches! |
| T12 | api 配列(複数) | `examples/two_bodies.mycad` を GET /api/v0/mesh → `Vec<BodyMesh>` len==2、**feature_id 配列が `["body_a","body_b"]` の exact order**（=feature 生成順）と一致、各 mesh 非空 | exact order assert |
| T13 | api 配列(単一) | 既存 simple_box 等 → len==1、`bodies[0].feature_id` 一致、`bodies[0].mesh` の三角形数が従来値 | len + id + counts |
| T14 | api 決定性 | 同一リクエスト2回で応答 body 文字列が完全一致 | assert_eq! |
| T15 | api ゴールデン回帰 | `BodyMesh.ts` golden と gen-ts 出力一致・drift なし | xtask test |
| T16 | cli アセンブリ拒否 | `assembly.mycad`（children あり）を export → fail-closed エラー（部分 STL を出さない）。新規 cli テスト | is_err + msg |

- 削除するテスト: `multiple_solid_features`（複数 create が許容されたため不成立。T02 が置換）。
- Solid に PartialEq は無い（既存方針維持）。決定性比較はテスト内ヘルパー
  `assert_solids_equal(&a, &b)` を用意し、`id` と vertices/edges/half_edges/loops/faces/shells の
  各 ID・参照インデックス・座標を field-by-field で比較する（`cuboid.rs:288` の粒度）。
  テッセレート mesh 比較は**補助**に留め、entity ID 退行検出を mesh 比較に依存しない（R01 対応）。
- mesh_api.rs の既存テスト（t01/t02/t07/t17 等）は応答が `TriangleMesh` → `Vec<BodyMesh>` に
  変わるため、ローカル `BodyMesh` デシリアライズ struct を定義し `bodies[0].mesh` 経由に更新する。
- web: `mesh.ts` の単体テストは不変（per-mesh）。`viewer.ts` は WebGL 依存で jsdom 単体テスト困難なため、
  配列描画はブラウザ手動確認に委ねる（下記検証手順）。`api.ts` の配列型は型チェック（tsc）で担保。

## 検証手順（end-to-end）

```bash
cargo xtask ci                         # web build + fmt + clippy -D warnings + 全テスト + TS drift（GLM が内部で green まで）
cargo test -p mycad-build              # マルチボディ build / 参照 / 決定性
cargo test -p mycad-kernel tessellation::tests::  # merge_meshes 系
cargo test -p mycad-api                # 応答配列化 + 複数/単一/決定性
cargo xtask gen-ts                     # BodyMesh.ts 生成（drift チェックを通す）

# cli: 2 ボディ STL は merge で1メッシュ、単一ボディ export は従来どおり
cargo run -p mycad-cli -- export examples/two_bodies.mycad -o /tmp/two.stl
cargo run -p mycad-cli -- export examples/simple_box.mycad -o /tmp/box.stl

# Web UI 手動確認（ボディ毎の色分け表示）:
cargo xtask web                        # vite build で web/dist 更新
cargo run -p mycad-cli -- view examples/two_bodies.mycad   # ブラウザで2体が別色で描画されること
```

**ブラウザ確認の観点**: `two_bodies.mycad`（XY をずらした押し出し角柱2本）が**2つの異なる色**の
立体として**重ならず横並びに**表示され、OrbitControls で回転でき、カメラが両者を含む範囲に
フィットすること。単一ボディの既存 example（simple_box 等）が従来どおり1体表示で壊れないこと。
呼び出し元（cli `run_export` / api `get_mesh`）の既存統合テストが単一ボディ Document で通る回帰も確認。
