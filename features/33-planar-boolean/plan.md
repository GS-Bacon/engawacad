# Issue #33 — 平面 Boolean: Cut/Fuse/Intersect を多面体ソリッドで動作

## Context

Phase 4「Boolean 演算」の入口。`.mycad` から `Cut/Fuse/Intersect` を多面体(平面のみで構成された Solid)同士で end-to-end に動かす。#27 が定義した `EntityRef::Derived` を実地で消費し、#32 が用意した `BuiltBodies` のマルチボディ基盤に Boolean 結果ボディを書き込む初の Issue。曲面交差・pcurve は #31/#34 のスコープで本 Issue では扱わない(平面同士の交線=直線で完結)。

実装方針はユーザー確認済み:
- アルゴリズム: **面パーティショニング方式**(B-rep 直接編集) — BSP は採用しない
- スコープ: **接触ケース → 空洞シェル発生ケース** の順で同一 PR 内で進める
- base name: **最小付与のみ**(cuboid + extrude の face/edge/vertex に `EntityRef::Named` を付与。cylinder/sphere は今 Boolean に絡まないので後追い別 Issue)

---

## 実装対象

### 影響クレート / ファイル

| パス | 変更内容 |
|---|---|
| `crates/mycad-kernel/Cargo.toml` | `mycad-format = { workspace = true }` 依存追加 + `earcutr` 依存追加(workspace.dependencies に登録) |
| `crates/mycad-kernel/src/error.rs` | `KernelError` に variant 追加(下記) |
| `crates/mycad-kernel/src/brep/topology.rs` | `Vertex` / `Edge` / `Face` に `pub name: Option<EntityRef>` フィールド追加、`add_*` API を name 受け取り可能に拡張 |
| `crates/mycad-kernel/src/booleans/mod.rs`(新規) | 公開 API: `boolean_planar(target, tool, op, id_gen) -> Result<Solid, KernelError>` |
| `crates/mycad-kernel/src/booleans/types.rs`(新規) | `BooleanOp { Cut, Fuse, Intersect }`、`FragmentLabel`、`HalfSpaceClass` |
| `crates/mycad-kernel/src/booleans/partition.rs`(新規) | Phase A — face 対 face の平面交差・2D 多角形クリッピングで sub-polygon に分割 |
| `crates/mycad-kernel/src/booleans/classify.rs`(新規) | Phase B — fragment を `Inside/Outside/SharedSame/SharedOpposite` に分類、point-in-polyhedron テスト |
| `crates/mycad-kernel/src/booleans/assemble.rs`(新規) | Phase C — fragment 採集 → 共有 vertex/edge マージ → HalfEdge/Loop/Face/Shell 再構成、空洞シェル分離 |
| `crates/mycad-kernel/src/booleans/derive_name.rs`(新規) | 派生 face/edge/vertex への `EntityRef::Derived` 合成、canonical sort |
| `crates/mycad-kernel/src/primitives/cuboid.rs` | 6 面 + 12 辺 + 8 頂点に `EntityRef::Named` を付与 |
| `crates/mycad-kernel/src/primitives/extrusion.rs` | cap_start/cap_end/side_{i} の役割名で付与 |
| `crates/mycad-kernel/src/tessellation/mod.rs` | 穴付き Face(inner_loops 非空) の三角化を `earcutr` 経由で対応 |
| `crates/mycad-kernel/src/lib.rs` | `pub mod booleans` の追加と再エクスポート |
| `crates/mycad-build/src/lib.rs` | `Cut/Fuse/Intersect` の `UnsupportedFeature` を実呼び出しに置換。target/tool の consume(隠す)動作と結果 register を実装 |
| `crates/mycad-api/src/handler.rs` | bodies 列挙箇所を `live()` ベースに切替(consumed 済みを viewer に出さない、Codex R06) |
| `crates/mycad-cli/src/main.rs` 等 export 経路 | 同じく `live()` ベースに(まだ `all()` を使っている箇所があれば対象) |
| `examples/boolean_box_cut.mycad`(新規) | Cut の最小サンプル |
| `examples/boolean_box_fuse.mycad`(新規) | Fuse の最小サンプル |
| `examples/boolean_box_intersect.mycad`(新規) | Intersect の最小サンプル |
| `examples/boolean_box_void.mycad`(新規) | 空洞シェル発生ケース |
| `crates/mycad-build/tests/feature_dispatcher.rs` | T01〜T20 の Boolean 統合テストを追加 |

### 公開シグネチャ

```rust
// crates/mycad-kernel/src/booleans/mod.rs
pub use self::types::BooleanOp;

pub fn boolean_planar(
    target: &Solid,
    tool: &Solid,
    op: BooleanOp,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError>;

// crates/mycad-kernel/src/booleans/types.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BooleanOp { Cut, Fuse, Intersect }
```

### `KernelError` 追加 variant

```rust
#[error("boolean result is empty (no volume remains)")]
EmptyBooleanResult,
#[error("non-planar surface in boolean input: {kind}")]
NonPlanarBooleanInput { kind: &'static str },
#[error("boolean input solid is not closed manifold")]
OpenBooleanInput,
#[error("degenerate boolean intersection (only surface contact, zero volume)")]
DegenerateBooleanContact,
#[error("boolean fuse result is disjoint (would produce multiple solids)")]
DisjointFuseResult,
#[error("boolean {op} result produced multiple positive-volume shells (would require multi-body)")]
MultipleOuterShellsResult { op: &'static str },
#[error("boolean input missing entity name (required for derived name composition)")]
MissingEntityName,
```

### `topology.rs` の field 追加

```rust
pub struct Vertex { pub id: EntityId, pub point: Point, pub name: Option<EntityRef> }
pub struct Edge   { pub id: EntityId, pub vertices: [usize; 2], pub curve: Curve,
                    pub t_range: [f64; 2], pub name: Option<EntityRef> }
pub struct Face   { pub id: EntityId, pub surface: Surface, pub outer_loop: usize,
                    pub inner_loops: Vec<usize>, pub same_sense: bool,
                    pub name: Option<EntityRef> }
```

`add_vertex` / `add_edge` / `add_face` のシグネチャに `name: Option<EntityRef>` を末尾追加。既存呼び出し箇所(cuboid/cylinder/sphere/extrusion/tessellation/topology tests)はすべて引数を更新。

---

## 設計方針

### 決定性要件 (Codex R02 反映)

1. `IdGenerator` は build dispatcher が保持し、Boolean 呼び出しに `&mut` で渡す。Boolean 内で新規 ID を発行する順序は以下に固定する:
   - 派生 vertex(3 面交点・1 面+元交差点)
   - 派生 edge(交線セグメント)
   - 派生 HalfEdge
   - 派生 Loop
   - 派生 Face
   - Shell(外殻・空洞シェル — 外殻が必ず先頭)
2. 各 face × face 交差処理ループの順序は **target.faces の index 昇順 → tool.faces の index 昇順** に固定。
3. 同一 face 内で複数 sub-polygon に分割される場合の **canonical order は 1 本化**:
   - **主順序**: `traversal_index`(Phase A の DCEL face graph BFS 順、outer_loop の最初の半辺を含む sub-face を 0 とし、半辺隣接で BFS 番号付け)
   - **tie-break**: BFS で隣接が無い disjoint sub-face は「sub-face polygon の **重心の (x, y, z) lexicographic 昇順**」(`f64::total_cmp`)で続けて番号付け
   - この `traversal_index` を **fragment 出力順 / `id_gen.next()` 発番順 / selector seed のすべてで共有**する(=`traversal_index` が face 内の唯一の canonical order)
4. 共有 vertex マージは `point_near` 比較で行うが、衝突時の代表 vertex 選択は `EntityId` の小さい方(=target 由来優先)を採用 — これも canonical。

### B-rep トポロジー妥当性

- 全 Boolean 結果に対し `validate_manifold()` を必ず呼び出し pass 必須。
- Euler-Poincaré 等式 `V − E + F = 2 (S − H)` を新規ヘルパー `Solid::euler_poincare()` で算出し、acceptance テストで assert。
  - `S` = `shells.len()`、`H` = (将来用)Face 内 inner_loops の総数。**この Issue では H は 0 になることを期待**(穴付き face があっても Solid 全体の genus には寄与しない — 各 Shell は閉曲面で genus 0 のため)。空洞シェルは `S` を増やすのみ。
  - 実装メモ: Phase 4 終了までに正式な Euler 式定式化は再検討余地あり。本 Issue では「外殻 1 + 空洞シェル数 → S」「inner_loops は H に寄与せず Face 内 trim として処理」のシンプルな会計で進め、コメントに ADR 参照を残す。

### 退化幾何の扱い (Codex R05 反映)

#### 明示閾値の固定

- **長さ閾値** `LEN_EPS = LENGTH_TOLERANCE`(1e-9)
- **面積閾値** `AREA_EPS = LENGTH_TOLERANCE * LENGTH_TOLERANCE`(1e-18)
- **角度閾値** `ANGLE_EPS = ANGLE_TOLERANCE`(1e-9)

各 Phase で以下のサニタイズを必須化:

- **Phase A 終了直後**: 全 segment bag に対し `|p1 - p0| < LEN_EPS` の segment を破棄。
- **PSLG 構築直後**: 同一座標 node を `point_near` で同値類化し、重複 node を統合(R05 重複頂点除去)。
- **PSLG subdivision 直後**: 各 sub-face polygon について
  - 連続する重複頂点(`point_near` で同値)を除去
  - 連続する 3 頂点が co-linear(`turn_cross` の絶対値 < `AREA_EPS`)なら中央頂点を削除
  - 残り頂点数 < 3 ならその sub-face は **破棄**
  - signed area の絶対値 < `AREA_EPS` ならその sub-face は **破棄**
- **Phase C 終了直後**: face 単位で再度の signed area チェック、`face.outer_loop.half_edges.len() < 3` の face を破棄。
- **空洞シェル候補が全 fragment 破棄で空になる場合**: Boolean 種別に応じて `EmptyBooleanResult` または `DegenerateBooleanContact` を返す。

#### ケース対応表

| ケース | 振る舞い |
|---|---|
| 入力 Solid に非平面 Surface が混在 | `NonPlanarBooleanInput { kind: "<variant>" }` |
| 入力 Solid が `validate_manifold` で fail | `OpenBooleanInput` |
| 共有体積ゼロ(接触のみ)で `Intersect` | `DegenerateBooleanContact` |
| target ⊆ tool で `Cut` → 結果空 | `EmptyBooleanResult` |
| `Fuse` で 2 つが disjoint(連結成分が 2 つ) | `DisjointFuseResult`(新 variant)で reject。マルチボディ結果は将来検討 |
| `Fuse` 共有面のみで体積接続なし(点/線接触) | `DegenerateBooleanContact` |
| 入力 face/edge/vertex に `name == None` がある | `MissingEntityName` |
| sliver fragment(長さ/面積閾値以下)が結果に残る | サニタイズで破棄。残量ゼロなら上記いずれかのエラー |

### derive 規約

- `BooleanOp` は `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize` を付与(YAML 内では使わないが将来の API 露出に備える)。`JsonSchema` は不要(Feature enum 経由のため `Feature` 側で導出)。
- 新 KernelError variant は既存に倣い `#[error("...")]` のみ。

### エラーハンドリング

- 全公開 API は `Result<_, KernelError>`。`thiserror` の既存パターンに従う。
- Boolean 内部関数(`partition`/`classify`/`assemble`)は `Result<_, BooleanInternalError>` を経由し、`mod.rs` の境界で `KernelError` に `From` 変換する(内部詳細のリーク防止)。

### workspace.dependencies 規約

`Cargo.toml`(workspace) に以下を追加:

```toml
earcutr = "0.4"
```

`mycad-kernel/Cargo.toml` に以下を追加:

```toml
[dependencies]
mycad-format = { workspace = true }
earcutr = { workspace = true }
```

**依存方向**: `mycad-format` は `mycad-kernel` に依存していないことを確認済み(format は YAML スキーマ層、kernel に対する依存は無い)。`kernel → format` の一方向依存を新設しても循環は発生しない。`EntityRef` を「永続化される identity 型」と位置づける ADR-005 解釈で、kernel が format の `EntityRef` を直接消費する。

**代替案の検討**:
- (a) `EntityRef` を kernel に移動し format から再エクスポート — 影響大(#27 を巻き戻す)、却下
- (b) `mycad-types` 新クレートに切り出し — 過剰、却下
- (c) kernel が format に依存 ← 採用

### アルゴリズム詳細(面パーティショニング、3 段階)

#### Phase A: Partition (Codex R01 反映)

入力: `target: &Solid, tool: &Solid`
出力: `(target_fragments: Vec<FaceFragment>, tool_fragments: Vec<FaceFragment>)`

各 face は分割後 1 つ以上の `FaceFragment { source_face_index, polygon_2d: Vec<Point2D>, plane: Plane, parent_name: EntityRef, traversal_index: u32 }` を持つ。`traversal_index` は派生名 selector に使う face 内の決定的 index(R04 対応)。

**設計の核**: face を**1 本ずつ切断する**のではなく、その face と相手 Solid の**全 face との交差線分をすべて集約**し、face 内の **planar straight-line graph (PSLG)** として構築してから、その graph 上で planar subdivision を計算して sub-polygon に分割する。これにより:
- 交差線分の端点を多角形境界まで「人為的に延長」する操作を完全に廃止 — R01 で指摘された誤分割を回避。
- 相手 face の輪郭が target face 内で閉ループを成すケース(穴付き face を生む典型 — T18)も自然に扱える(閉ループはそのまま inner_loop を生む)。

手順(face_a を対象に説明、target/tool 同じ手順を双方向で実行):

1. **共有平面/平行平面の事前判別** (Codex R03 反映):
   - target 全 face × tool 全 face のペアで `Plane` の normal 一致(`angle_near`)と距離(`length_near`)を判定 — これは coplanar 候補集合の絞り込み
   - coplanar と判定されたペアについて **2D polygon overlap (Sutherland-Hodgman)** を実行し、**重なり面積 > AREA_EPS** を満たすペアのみ「真の共有面 candidate」としてマーク
   - 真の共有面 candidate は Phase B での `SharedSame/SharedOpposite` 判定にだけ使う(face 分割の入力には**乗せない**)
   - **同一平面だが overlap しない / overlap が sliver 以下 のペアは coplanar として扱わず**、通常の分割対象として Phase A2 へ流す(=平面交差はないので segment は生まれず、結果として分割されないだけ — これは正しい挙動)
2. **交差線分の収集**: face_a と各「相手 face_u(共有面 candidate を除く)」について:
   - 平面交差直線 `L_{au} = P_a ∩ P_u` を算出(平行なら no-op)
   - `L_{au}` を face_a の outer_loop polygon と face_u の outer_loop polygon の**両方**で 2D 区間クリップし、両方の中に入る区間 `[p0, p1]` を求める(両 polygon を 3D→2D に投影してから 1D 区間交差)
   - 区間長 < `LENGTH_TOLERANCE` なら破棄(R05 対応)
   - face_a の 2D 投影座標系で `Segment2D { start, end, partner_face: usize }` として `face_a` の **segment bag** に追加
3. **PSLG 構築**:
   - face_a の outer_loop polygon の頂点と segment bag の端点をすべて **point set** にまとめ、`point_near` で同値類化(R05 の重複頂点除去)
   - 各 segment と outer_loop edge の交差を計算して新たな graph node を追加
   - segment 同士の交差(face_a 内で複数 partner からの線分が交わる)も graph node に追加 — 重要: ここで segment 同士の交差点が「3 face 交点」となり後段で新規 vertex の生成元になる
4. **Planar subdivision**: 構築した PSLG を「半辺データ構造」として歩き、左周りの最小ループ(= sub-face)を列挙する(古典的な doubly connected edge list / DCEL の face traversal アルゴリズム)。outer_loop と一致する最外周ループは除外し、内部 sub-face を `FaceFragment` として確定。
5. **traversal_index 付与**: face_a の **outer_loop の最初の半辺** を含む sub-face を index 0、その**反時計回り隣接**を 1、… と DCEL face graph を BFS で番号付け(R04 の selector の安定キー)。隣接が無い disjoint な inner sub-face(穴の外側に閉じた fragment)は BFS 後に「outer_loop からの距離 + 重心 lex 順」を補助キーとして続けて番号付け。

**幾何ユーティリティ(新規実装)**:
- `signed_distance_point_plane(point: &Point, plane: &Plane) -> f64`
- `intersect_two_planes(a: &Plane, b: &Plane) -> Option<(Point, Vec3)>`(交線の origin + direction)
- `project_polygon_to_uv(polygon_3d: &[Point], plane: &Plane) -> Vec<Point2D>`
- `clip_line_to_polygon_2d(line_o: Point2D, line_d: Vec2D, polygon: &[Point2D]) -> Option<(Point2D, Point2D)>`
- `pslg_build_and_subdivide(outer_loop_2d: &[Point2D], segments: &[Segment2D]) -> Vec<SubFace>`(PSLG → sub-faces)
- `segment_segment_intersect_2d(a0: Point2D, a1: Point2D, b0: Point2D, b1: Point2D) -> Option<Point2D>`

#### Phase B: Classify

各 `FaceFragment` に対し `FragmentLabel` を判定:

```rust
enum FragmentLabel {
    InsideOther,           // 相手 Solid の体積内側
    OutsideOther,          // 相手 Solid の体積外側
    SharedSameDirection,   // 相手の face と同一平面・同向き
    SharedOppositeDirection,// 相手の face と同一平面・逆向き
}
```

判定法 (Codex R04 反映):
- 共有平面の事前判定は Phase A で済んでいる → 真の共有面 candidate にだけ SharedSame/SharedOpposite を優先割当
- それ以外は **fragment の確実な内部点**を取り、相手 Solid に対し **ray casting による point-in-polyhedron テスト** を実施:
  - 内部点の取り方: fragment polygon を ear-clipping で簡易三角化(`earcutr` 経由)し、**最初の三角形の重心**を fragment 内部点として採用(凹 polygon でも triangle 重心は確実に fragment 内部にある)。三角化結果は決定的(同一入力は同一三角形列)。
  - ray は内部点から `+X` 方向に発射(NaN 防止のため重心が相手の face 上にある場合は別軸へ揺らす)
  - 相手 Solid の全 face と ray-face 交差を取って奇偶判定
  - 退化(face 端を貫く)時は ray 方向をわずかに揺らす。揺らし系列は決定性のため固定: `(+X, +Y, +Z, +X+Y, +X+Z, +Y+Z, +X+Y+Z)` の順で初回成功するまで試行

#### Phase C: Assemble

Boolean 種別に応じて fragment を採集:

| Op | target 側採集 | tool 側採集 | tool 側法線 |
|---|---|---|---|
| Fuse | OutsideOther + SharedSameDirection | OutsideOther | そのまま |
| Intersect | InsideOther + SharedSameDirection | InsideOther | そのまま |
| Cut (target − tool) | OutsideOther + SharedOppositeDirection | InsideOther | **反転** |

採集後の組立工程:

1. **共有 vertex マージ**: 全 fragment の polygon 頂点を `LENGTH_TOLERANCE` で同値類化、代表 vertex を選定。
2. **共有 edge 同定**: 隣接する 2 fragment が同じ vertex pair を共有する辺は同一 edge にマージ。
3. **HalfEdge 生成**: 各 face の outer loop に沿って HalfEdge を順次生成。Boolean で法線反転した face は loop 順を逆転。
4. **Loop 構築**: HalfEdge index 列を Loop に。
5. **Face 構築**: outer + inner loops。inner_loops は「同一平面で外側 loop に内包される穴」を Phase A の cut で生まれた場合に検出して付与。
6. **Shell 構築**: face-edge-face グラフで連結成分を計算 → 各連結成分が 1 Shell。
7. **連結成分の正体積判定と複数 outer shell の reject** (Codex R05 反映):
   - 各 Shell の符号付き体積 `signed_volume(shell)` を計算(三角化された face の符号付き四面体和、外向き法線で正・内向きで負)
   - **正体積 Shell** = 外殻候補、**負体積 Shell** = 空洞候補
   - 正体積 Shell が **2 個以上**の場合は **明示エラー**で reject:
     - `Cut` で正体積 Shell が 2 個以上 → `MultipleOuterShellsResult { op: "cut" }`
     - `Intersect` で正体積 Shell が 2 個以上 → `MultipleOuterShellsResult { op: "intersect" }`
     - `Fuse` で正体積 Shell が 2 個以上 → `DisjointFuseResult`(既存 variant)
   - 正体積 Shell が **0 個**(全部負)→ `EmptyBooleanResult`
   - 正体積 Shell が **ちょうど 1 個** → 外殻として確定
   - 負体積 Shell は AABB 包含で外殻に内包されることを assert(包含されない異常ケースは `KernelError::Internal("orphan inner shell")` に集約)
   - 結果型を多 body に拡張する路線は **本 Issue ではスコープ外**(将来 #34 以降で再検討)
8. **空洞シェルの内向き再構築** (Codex R02 反映):
   - 空洞 Shell の各 Face について、loop を「頂点列の逆順」から **HalfEdge を新規に作り直す**(in-place 編集ではない):
     - 古い loop の `half_edges` 列から「終点 vertex の列」を抽出
     - 逆順に並べ、新規 HalfEdge を `id_gen.next()` で発行して各 vertex に対し再構築
     - 新規 HalfEdge の `start_vertex` は逆順 vertex 列で正しく配置(R02 で指摘された start_vertex 抜けを根本的に解消)
     - 新規 HalfEdge の `edge` は元 edge を再利用、`forward = !old.forward` で対応する逆方向半辺を指す
     - 古い loop / HalfEdge 群は `Solid` の配列内で物理削除せず、新規追加して新 index を Loop が指すよう更新(配列インデックス安定性のため)
     - 古い HalfEdge / Loop は `validate_manifold` の整合性のため使われなくなるが、配列には残る → これは shrink フェーズ(後述)で除去
   - 各 Face の `same_sense: bool` も**反転**
   - inner_loops を含む face が空洞シェルに混入するケースは本 Issue では発生しない前提だが、発生したら同じ方式で内側 loop も再構築
9. **shrink フェーズ**: 全 Boolean 工程の最後に、`Solid` の配列から「どの Loop からも参照されていない HalfEdge」「どの Face からも参照されていない Loop」を圧縮(=未参照要素を削除し、参照側の index を再マップする)。圧縮は決定的に行う(`id` 昇順を保持する圧縮アルゴリズム)。圧縮しないと validate_manifold が壊れる(orphan HalfEdge は 2 個ペア条件を満たさない)。
10. `shells: Vec<Shell>` に **外殻を先頭** にして空洞シェルを `EntityId` 昇順で続ける(決定性)。

### 派生名(EntityRef::Derived)合成ルール

#### selector の設計

`EntityRef::Derived { kind, op, from, selector }` の各フィールドの決定方針:

- `op` の値(文字列):
  - face: `"cut"` / `"fuse"` / `"intersect"`
  - edge: `"cut_isect_line"` / `"fuse_isect_line"` / `"intersect_isect_line"`
  - vertex: `"cut_isect_vertex"` / `"fuse_isect_vertex"` / `"intersect_isect_vertex"`
- `from`:
  - face fragment: 元 face 1 つの name を slot0(target 由来は target 側に、tool 由来は tool 側に)。Boolean 種別が **non-commutative (Cut)** の場合は target/tool で slot を分ける(target=slot0、tool=slot1)。**Fuse/Intersect は commutative** なので、対応する元 face name を `canonical_name()` で sort してから `from` に格納する。
  - edge: 交線を生む 2 つの parent face の name。Cut のみ slot 順(target_face, tool_face)を保持、Fuse/Intersect は canonical_name sort。
  - vertex: 交点を生む 3 face(または既存 vertex 一致なら親 vertex 1 つ)の name を canonical_name sort(Cut でも 3 face なら symmetric extension とみなす)。
- `selector`(英数+ハイフンのみの安定キー、**離散値のみで構成し座標を埋め込まない** — Codex R04):
  - face fragment: 親 face 内の `traversal_index`(Phase A で算出した DCEL face graph BFS 順)を使い `format!("frag{:04}", traversal_index)`。
  - edge: 交線セグメントは「2 partner face の `traversal_index` ペア + face_a の cut graph 上での **edge slot index**」を組み合わせて `format!("e{:04}_p{:04}_{:04}", edge_slot, partner_index_low, partner_index_high)`(partner_index は `target_face_index < tool_face_index` を canonical 順で前置)。
  - vertex: 「3 face 交点なら 3 face index を canonical sort して `format!("v{:04}_{:04}_{:04}", a, b, c)`」「2 face 交点 + 既存 vertex 一致なら親 vertex の base name の `canonical_name()` を使う(`from` に親 vertex を入れて selector は `"merged"`)」。

**設計根拠**: PSLG/DCEL から得られる**組合せ的な序列**(`traversal_index`, face index, edge slot)は浮動小数の丸めに完全に依存しないため、`Fuse(A,B)` と `Fuse(B,A)` で同一の selector を生む(face index ペアを canonical sort することで両者の `(target, tool)` ペアが同じ集合に正規化される)。`Cut(A,B)` では target/tool を slot 順で固定するので別の selector になる。

#### Multi-parent canonical sort

Fuse/Intersect 結果 face が 2 つ以上の元 face をマージして生まれるケース(共有面で SharedSameDirection が選ばれた場合)は、`from` の slot 順を `canonical_name()` 昇順 sort で固定する。これにより `Fuse(A,B)` と `Fuse(B,A)` の同一派生 face は **同じ canonical_name** を持つ(acceptance T13)。

非可換 Cut では target=slot0、tool=slot1 を強制し、`Cut(A,B)` と `Cut(B,A)` で **異なる canonical_name** を持つ(acceptance T14)。

### tessellation 拡張 (Codex R03 反映)

- 既存 fan tess は **外側ループが凸である**ことを暗黙に前提にしている。Boolean 由来の face は `inner_loops` の有無に関わらず**凹形状**になりうる(例: T06 の L 字 face — 穴なしだが凹)。fan のままだと self-overlapping triangle が生まれ STL/export が破綻する。
- 対策: `Face` 構造に `pub robust_tess_required: bool` を追加(あるいは別経路として「凸性を保証できない face」を tessellator が判定して earcutr ルートへ振り分ける)。
  - **採用案**: `Face` 自体にはフラグを持たせず、tessellator 側で `outer_loop` の 2D 投影に対し凸性判定(連続 3 頂点の cross product 符号が一定か)を行い、**凸ならば fan / 凹または inner_loops 非空ならば earcutr** へ自動振り分け。これにより primitive 由来 face は従来通り fan、Boolean 由来 face は earcutr に乗る。
  - 凸性判定の決定性: cross product 符号を `LENGTH_TOLERANCE * LENGTH_TOLERANCE` 閾値で判定し、丸めゾーンに入ったら **保守的に earcutr 側**に流す。
- `earcutr::earcut` は flat な vertex 配列 + hole indices 形式。外側 + 各 inner loop を `Vec<f64>` (xy 連続) に詰め、hole indices に inner loop の先頭オフセットを渡す。
- 決定性検証: T18 で 100 回ループ assert、加えて T06 の L 字 face を 100 回回して同一 hash も assert。

### build dispatcher 改修

`crates/mycad-build/src/lib.rs` の Cut/Fuse/Intersect 各分岐(L123-161)を以下に置き換え:

```rust
Feature::Cut { id, target, tool } => {
    let t = built.get(target).ok_or_else(|| BodyNotFound { id: target.clone() })?;
    let u = built.get(tool).ok_or_else(|| BodyNotFound { id: tool.clone() })?;
    let solid = boolean_planar(&t.solid, &u.solid, BooleanOp::Cut, gen)?;
    // consume target/tool: register 後に live ボディから外す
    built.consume(target);
    built.consume(tool);
    built.register(id.clone(), solid);
}
```

`BuiltBodies::consume(feature_id: &str)` を新設。「物理削除はせず `index` から外して `all()` には残すが、`get` で引けなくなる」セマンティクスにする(STL export は live ボディのみ集約 → 結果ボディだけが残る)。

`BuiltBodies` の差分:
```rust
pub struct BuiltBodies {
    bodies: Vec<Body>,
    index: HashMap<String, usize>,
    consumed: HashSet<String>,  // 新規
}
impl BuiltBodies {
    pub fn consume(&mut self, feature_id: &str) { self.consumed.insert(feature_id.into()); self.index.remove(feature_id); }
    pub fn live(&self) -> impl Iterator<Item = &Body> { self.bodies.iter().filter(|b| !self.consumed.contains(&b.feature_id)) }
}
```
既存の `all()` `len()` `is_empty()` セマンティクスは保持(全 body を返す)。export 系は `live()` に切り替え。

### primitive の base name 付与 (Codex R01 反映)

`EntityRef` の role / feature_id charset は `[A-Za-z0-9_-]` のみ。`+` `.` 等は禁止。

- `cuboid.rs`(charset を厳守):
  - vertex role: `v_{xs}{ys}{zs}` 形式で `xs/ys/zs ∈ {p, n}` (p=plus、n=negative)。例: `v_pnn`, `v_ppp`
  - edge role: 接続する 2 vertex role を canonical sort して `_` 連結。例: `e_v_pnn__v_pnp`
  - face role: 法線軸 + 符号で `f_x_pos` / `f_x_neg` / `f_y_pos` / `f_y_neg` / `f_z_pos` / `f_z_neg`
  - `feature_id` は cuboid feature の `id` フィールド(`Document` 段階で identifier validation 済 → 安全)
- `extrusion.rs`(charset 厳守):
  - face role:
    - cap: `f_cap_start`, `f_cap_end`
    - side: `f_side_{i:04}`(`i` は元 profile edge の canonical index)
  - edge role:
    - profile edge cap_start 側: `e_profile_start_{i:04}`
    - profile edge cap_end 側: `e_profile_end_{i:04}`
    - side edge(押し出し方向): `e_side_{i:04}`(`i` は元 profile vertex の canonical index)
  - vertex role: `v_profile_{i:04}_{end}`(`end ∈ {start, end}`)

base name 値は `EntityRef::try_named` 経由で生成し、format crate 側の validation に通すこと(charset 違反は CI で即発覚)。

---

## テスト計画(ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T01 | 決定性 | box × box の Cut/Fuse/Intersect を 2 回 build し、Solid 配列・EntityId・座標・name が完全一致 | `assert_eq!` |
| T02 | 正常系・Fuse 接触 | 2 つの単位 box が一面共有 → 単一 Solid に連結、shell.len() == 1 | OK + Euler-Poincaré |
| T03 | 正常系・Fuse 重なり | 2 つの box が体積重なり → 単一 Solid | OK |
| T04 | 正常系・Intersect 重なり | 2 つの box の重なり → 重なり領域の小 box | OK |
| T05 | 退化・Intersect 非交差 | disjoint → `EmptyBooleanResult` | エラー |
| T06 | 正常系・Cut 部分 | target − tool の部分削除 → L 字 Solid | OK |
| T07 | 退化・Cut 完全除去 | target ⊆ tool → 結果空 | `EmptyBooleanResult` |
| T08 | 正常系・Cut 空洞 | tool が target に完全内包 → 空洞シェル発生 Solid (shells.len() == 2)、内側 shell の face 法線が内向き | OK + Euler-Poincaré (S=2, H=0, V−E+F=4) |
| T09 | 退化・Intersect 接触のみ | 共有体積ゼロ(面接触のみ) → `DegenerateBooleanContact` | エラー |
| T10 | 妥当性・Euler-Poincaré | T02〜T08 の全結果で V−E+F = 2(S−H) を assert | OK |
| T11 | 妥当性・validate_manifold | T02〜T08 の全結果で pass | OK |
| T12 | 派生名・決定性 | 同じ Boolean を 2 回回し、全 face/edge/vertex の `name` が一致 | `assert_eq!` |
| T13 | 派生名・commutative | `Fuse(A,B)` と `Fuse(B,A)` の派生 face name の `canonical_name()` が一致 | `assert_eq!` |
| T14 | 派生名・non-commutative | `Cut(A,B)` と `Cut(B,A)` の派生 face name の `canonical_name()` が**異なる** | `assert_ne!` |
| T15 | 統合・Extrude × Extrude | extrude プロファイルから作った 2 prism の Boolean が動作 | OK |
| T16 | golden YAML | `examples/boolean_box_cut.mycad` → Document → rebuild → 再出力が byte-identical | `assert_eq!` |
| T17 | export STL | T16 の結果 Solid を STL に export、ファイルが生成され `merge_meshes` で確認 | ファイル存在 + triangle 数 > 0 |
| T18 | tess・穴付き face | Boolean で穴付き face が出るケース(例: box の側面に tool が貫通 → 側面に矩形穴)を tess し、mesh が生成。100 回回して同一出力 | 同一 hash |
| T19 | 派生名・selector 非衝突 | 1 つの親 face から 3 つ以上の sub-polygon が出るケースで全 selector が unique | `HashSet::len == fragments.len` |
| T20 | build 統合 | `boolean_box_cut.mycad` の Document を `build_bodies_from_features` で処理 → live() が結果 1 body のみ | `assert_eq!(live.count(), 1)` |
| T21 | エラー・非平面入力 | cylinder × box の Boolean を試みる → `NonPlanarBooleanInput` | エラー |
| T22 | エラー・name 欠如 | 名前なし Solid を Boolean に渡す → `MissingEntityName` | エラー |
| T23 | エラー・複数 outer shell | Cut で結果が disjoint な 2 正体積成分(例: 2 つの離れた box を貫通 Cut)→ `MultipleOuterShellsResult { op: "cut" }` | エラー |
| T24 | エラー・Disjoint Fuse | 2 つの disjoint な box を Fuse → `DisjointFuseResult` | エラー |
| T25 | tess 振り分け | 凸 face は fan、凹 face は earcutr に流れることを `Face::id` ベースで追跡可能なフラグ or テスト用 hook で確認 | OK |
| T26 | API live() | Cut 後の Document を mycad-api 経由で /bodies に問い合わせ → 結果ボディ 1 つのみ返却(consumed 済み target/tool は出てこない) | OK |

実装フェーズの内訳:
- **Sub-phase A (接触ケース)**: T01〜T07, T09〜T17, T19〜T22
- **Sub-phase B (空洞シェル + 穴付き face)**: T08, T18

GLM 実装は単一の dispatch で max-turns 80 を確保し、両 sub-phase を内部で順次実装する。

---

## 入力前提のスコープ明示 (Codex R01 反映)

本 Issue の `boolean_planar()` 公開 API の**入力 Solid の前提**:

- **primitive 由来の Solid のみ**を入力として受け付ける(cuboid / extrude が生成した Solid)
- **`inner_loops` を持つ face を含む Solid は入力にできない**(本 Issue で扱う `FaceFragment` は `outer_loop` ベースであり、annulus 片(穴あき面)を入力 face として処理する経路を持たない)
- **凹 outer_loop を持つ face を含む Solid も入力にできない**(本 Issue の入力前提として primitive 出力に限定すれば、cuboid は凸、extrude も convex profile 限定なので凹は生まれない)
- **Boolean 結果 Solid を再 Boolean の入力にすることはスコープ外**(=「`Cut → 結果に対して再 Cut`」のような連鎖はサポートしない)

検証フロー: `boolean_planar()` 入口で以下を**この順序で**実行(Codex R03 反映):

1. `validate_manifold()` を target と tool に対し実行 → 失敗なら `OpenBooleanInput`
2. 全 face の `Surface` が `Surface::Plane` であることを確認 → 失敗なら `NonPlanarBooleanInput { kind }`
3. 全 face が `inner_loops.is_empty()` であることを確認 → 失敗なら `UnsupportedBooleanInput { reason: "input face has inner loops" }`(新 variant)
4. 全 face の `outer_loop` polygon が凸であることを確認(`turn_cross` 符号が一定) → 失敗なら `UnsupportedBooleanInput { reason: "input face has non-convex outer loop" }`
5. 全 face/edge/vertex に `name.is_some()` を確認 → 失敗なら `MissingEntityName`

このフローにより:
- `NonPlanarBooleanInput` のテスト(T21)は cylinder 入力で 2 段目で先に発火 → 確定
- `MissingEntityName` のテスト(T22)は **平面 unnamed solid を別途用意**(test 内で primitive を生成後に name を None で上書きする helper を用意)→ 5 段目で発火 → 確定
- ハンドラ間の優先順位が一意に決まり、Codex R03 の「契約満たせない」問題を解消

`KernelError` に以下を追加:
```rust
#[error("unsupported boolean input: {reason}")]
UnsupportedBooleanInput { reason: &'static str },
```

将来 Boolean 結果を再 Boolean の入力にしたい場合は、入力前提を拡張する別 Issue(穴付き face 対応の partition / classify)を立てる。

---

## オープン課題

1. **空洞シェル判定の包含テスト**: AABB だけでは不十分なケース(凹包形状)があるが、本 Issue の凸前提では bbox 包含で安全に判定可能。コメントに前提を明記する。凸前提が崩れる将来は ray-casting ベースの包含判定に置き換える前提。
2. **`MissingEntityName` の扱い**: primitive 側 base name 付与を必須化すると既存テストの一部(name 無し Solid を直接生成しているケース)が壊れる可能性。**過渡的に `Option<EntityRef>` のままにし**、Boolean に渡された時点で `None` を `MissingEntityName` エラーにする方針(primitive 経由ならば常に Some が保証される)。
3. **`BuiltBodies::consume` のセマンティクス**: 「物理削除しない・index から外す」設計で良いか。STL export と viewer は `live()` 経由でこの境界を尊重する。
4. **`mycad-kernel` → `mycad-format` 依存追加**: 依存方向としての妥当性確認(本 plan の検討では「妥当」と判断、代替案 (a)(b) は却下)。
5. **Disjoint Fuse の扱い**: 連結性が壊れる結果は単一 Solid に収まらない。本 Issue では `DisjointFuseResult` で reject。マルチボディ結果を返す路線は #34 以降で別途検討。

### Codex 設計レビュー(loop 1)反映済み事項

- R01 (critical): Phase A を「人工延長」型から **PSLG/DCEL ベースの planar subdivision** に変更。
- R02 (high): 空洞 shell は `same_sense` のみならず **loop 半辺順序 + HalfEdge.forward も反転**。
- R03 (high): tessellator は `inner_loops` の有無ではなく **outer_loop の凸性判定** で fan/earcutr を振り分け、保守的に earcutr へ流す。
- R04 (high): selector を **離散値のみ**(`traversal_index`, face index, edge slot)で構成、座標を埋め込まない。
- R05 (medium): 長さ・面積閾値を明示し、各 Phase 終了時のサニタイズ手順(重複頂点除去・co-linear 中点削除・最小辺/最小面積破棄)を確定。

### Codex 設計レビュー(loop 2)反映済み事項

- R01 (critical): cuboid/extrusion の base name role は `EntityRef` charset(`[A-Za-z0-9_-]`)に厳守。`+` `.` 禁止。`v_pnn`, `f_x_pos`, `e_side_{i:04}` 等の ASCII セーフ形式に確定。
- R02 (critical): 空洞シェル反転で `HalfEdge.start_vertex` が更新漏れになる問題に対処。**in-place mutation ではなく、新規 HalfEdge を `id_gen.next()` で再構築**して loop が新 index を指す形に。古い HalfEdge は shrink フェーズで圧縮。
- R03 (high): coplanar 判定の後段に **2D polygon overlap (Sutherland-Hodgman)** を加え、真に重なる場合のみ SharedSame/SharedOpposite を割り当て。
- R04 (high): point-in-polyhedron テストの代表点を **fragment 三角化後の最初の triangle 重心**(凹 polygon でも内部保証)に変更。
- R05 (high): Cut/Intersect でも **正体積 Shell が複数**生じうるため、各 Boolean 種別ごとに reject 条件と新 `KernelError::MultipleOuterShellsResult { op }` variant を追加。多 body 結果は本 Issue ではスコープ外。
- R06 (medium): export/viewer の bodies 列挙経路を `live()` に切替。対象に `mycad-api/src/handler.rs` を追加。

### Codex 設計レビュー(loop 3)反映済み事項

- R01 (high): 入力 Solid を **primitive 由来(凸 outer_loop / inner_loops 空 / 平面のみ)に限定**するスコープ前提を明示。Boolean 結果 Solid の再入力(穴付き face 入力)は本 Issue ではサポートしない旨を契約として確定。
- R02 (medium): face 内 fragment の canonical order を **`traversal_index` の 1 本**に統一(主=BFS、tie-break=重心 lex)し、fragment 出力順 / ID 発番順 / selector seed のすべてで共有。
- R03 (medium): boolean 入口の検証順を **`validate_manifold` → 非平面検出 → inner_loops 検出 → 凸性検出 → name presence** の 5 段で固定し、`KernelError::UnsupportedBooleanInput { reason }` を新設。

---

## 最終レビュー指摘(最優先で修正 — STEP 7 Codex final review: blocking=3)

### F01 (critical): assemble.rs — Edge 共有・逆向き HalfEdge が未実装

**症状**: 空でない Boolean 結果が常に `validate_manifold()` で `edge must have exactly 2 half-edges` になり、正常系 Boolean は一切動かない。

**根本原因**: `assemble.rs` が各ポリゴン辺ごとに毎回新しい Edge を生成して HalfEdge を 1 本しかぶら下げていない。B-rep の invariant「各 Edge には逆向き HalfEdge が 2 本必要」が満たされていない。

**修正方針**:
- 「無向辺キー」= min(v_a, v_b) をキー、max(v_a, v_b) を値とする HashMap で Edge の共有を管理
- fragment を処理するとき、同じ vertex pair を共有する辺は同一 Edge を指す 2 本の HalfEdge(互いに forward=true/false)を生成する
- 具体的な処理順序:
  1. 全 selected fragment を走査し、全辺の vertex pair を収集
  2. 各 vertex pair を正規化(min, max)してユニーク Edge を作成
  3. 各 fragment の loop を張るとき、各辺について「この Edge の forward/reverse のどちら向きか」を判定して HalfEdge を生成
  4. 結果: 各 Edge に forward HalfEdge 1 本 + reverse HalfEdge 1 本の合計 2 本
- `reverse_face_orientation()` も同じ規約(in-place mutation ではなく再構築)で一致させる

### F02 (high): build/lib.rs — all()/len() が消費済みボディを返す

**修正**: `BuiltBodies::all()` / `len()` / `is_empty()` を `live()` 相当(consumed を除く)に変更するか、**CLI/API の呼び出し箇所を `live()` に切り替える**。

すでに `live()` は実装済みのはず。以下の呼び出し元を `all()` → `live()` に更新:
- `crates/mycad-cli/src/main.rs` の STL export 箇所(merge_meshes の入力)
- `crates/mycad-api/src/handler.rs` の /bodies エンドポイント
- `crates/mycad-build/tests/feature_dispatcher.rs` の t20 等で確認

### F03 (high): partition.rs — clip_line_to_polygon_2d が ray ではなく線分で機能する

**症状**: `t_min` が 0 に丸められているため、平面交差直線を polygon の内側で clip できない。有効な交差区間が切り落とされたり延長される。

**修正**: 無限直線を clip する場合は `t_min = -∞`(または `f64::NEG_INFINITY`)で開始し、polygon の各辺との交差で `t_min`/`t_max` を絞る方式(line-polygon clip = Cyrus-Beck アルゴリズム)に変更:
```rust
let mut t_min = f64::NEG_INFINITY;
let mut t_max = f64::INFINITY;
// 各 polygon 辺との交差で t_min/t_max を絞る
// 最終的に t_min < t_max なら区間 [t_min, t_max] が有効
```

### F04 (medium): assemble.rs — find_connected_shells の HashMap で決定性なし

**修正**: `HashMap` を `IndexMap` に変更するか、結果の shell リストを face index 昇順でソートして返す:
```rust
// 現状
let shells: HashMap<usize, Vec<usize>> = ...
// 修正後: IndexMap (追加順が保持される)
use indexmap::IndexMap;
let mut shells: IndexMap<usize, Vec<usize>> = IndexMap::new();
// 最終返却時に各 shell の face 列を昇順 sort、shell リストも先頭 face index で sort
```

### F00 (追加 clippy): assemble.rs L25-39 の match 式を `matches!()` に変換

`cargo xtask ci` が以下で失敗している:
```
error: match expression looks like `matches!` macro
  --> crates/mycad-kernel/src/booleans/assemble.rs:25:29
```

**修正**: `crates/mycad-kernel/src/booleans/assemble.rs` L25-39 の以下:
```rust
let should_select = match (op, is_tool, cf.label) {
    (BooleanOp::Cut, false, FragmentLabel::OutsideOther) => true,
    ...
    _ => false,
};
```
を以下に書き換える:
```rust
let should_select = matches!(
    (op, is_tool, cf.label),
    (BooleanOp::Cut, false, FragmentLabel::OutsideOther)
        | (BooleanOp::Cut, false, FragmentLabel::SharedOppositeDirection)
        | (BooleanOp::Cut, true, FragmentLabel::InsideOther)
        | (BooleanOp::Fuse, false, FragmentLabel::OutsideOther)
        | (BooleanOp::Fuse, false, FragmentLabel::SharedSameDirection)
        | (BooleanOp::Fuse, true, FragmentLabel::OutsideOther)
        | (BooleanOp::Intersect, false, FragmentLabel::InsideOther)
        | (BooleanOp::Intersect, false, FragmentLabel::SharedSameDirection)
        | (BooleanOp::Intersect, true, FragmentLabel::InsideOther)
);
```

この修正後に `cargo fmt --all` を実行し、その後 `cargo xtask ci` が green になることを確認する。

### 正常系テスト(T02〜T08)の追加も必須

F01 修正後に正常系 Boolean が動くようになったら、`crates/mycad-build/tests/feature_dispatcher.rs` に以下を追加:
- **T02**: 2 box が接触(面共有) → Fuse → `validate_manifold` + Euler V-E+F=2 + shell.len()==1
- **T03**: 2 box が重なり → Fuse → `validate_manifold` + Euler
- **T04**: 2 box が重なり → Intersect → `validate_manifold` + Euler
- **T06**: box から小 box を切る(部分 Cut) → `validate_manifold` + Euler + 正しい face 数
- **T08**: box 内に小 box を完全内包 → Cut → `shells.len()==2`(外殻+空洞)

---

## 既知バグ(最優先で修正) — `t05_intersect_disjoint_boxes` FAILED

### 症状
```
test t05_intersect_disjoint_boxes ... FAILED
expected EmptyBooleanResult, got Err(BooleanInternal("manifold validation failed: edge must have exactly 2 half-edges"))
```

### 根本原因

`crates/mycad-kernel/src/booleans/classify.rs` の `classify_fragment_against_solid` 関数(L63〜) における **coplanar 判定が 2D polygon overlap を確認せずに `SharedSameDirection` を早期 return** している。

現在の判定ロジック(L69-90):
```rust
// 平面の法線一致 + 距離 0 → 即 SharedSameDirection/SharedOppositeDirection
if dist < len_eps {
    let dot = frag.plane.normal.dot(&other_plane.normal);
    if dot > 0.0 {
        return Ok(FragmentLabel::SharedSameDirection);
    } else {
        return Ok(FragmentLabel::SharedOppositeDirection);
    }
}
```

問題: **disjoint な 2 つの box が同じ平面(e.g. z=0)に底面を持つとき**、法線一致 + 距離 0 なので coplanar 扱いになるが、実際の polygon 同士は x 方向に離れていて **2D 上で overlap しない**。それにもかかわらず `SharedSameDirection` が返るため、Intersect で誤って face fragment が selected に含まれ、不完全な Solid が組み立てられて `validate_manifold` でエラーになる。

### 修正箇所 (`classify.rs` L69-90)

coplanar と判定した後に **2D polygon overlap check** を追加する。overlap がない場合は coplanar としては扱わず、point-in-polyhedron テストへ続ける。

```rust
// coplanar 候補を確定する前に 2D overlap を確認する
if dist < len_eps {
    // 2D polygon overlap check: fragment の polygon と other face の polygon が
    // 相互に点を含むか、または AABB が交差するかを確認
    let frag_3d = &frag.polygon_3d;
    let other_loop_verts = get_other_loop_verts(other, fi);  // helper function
    
    let has_overlap = polygons_have_2d_overlap(frag_3d, &other_loop_verts, &other_plane.normal, len_eps);
    
    if has_overlap {
        let dot = frag.plane.normal.dot(&other_plane.normal);
        if dot > 0.0 {
            return Ok(FragmentLabel::SharedSameDirection);
        } else {
            return Ok(FragmentLabel::SharedOppositeDirection);
        }
    }
    // overlap なし → coplanar として扱わず fall through して point-in-polyhedron へ
}
```

**`polygons_have_2d_overlap` の実装指針**:
- 両 polygon を dominant axis に沿って 2D 投影する
- **AABB 交差 check で十分**(凸入力前提): fragment の AABB と other face の AABB が重なるか  
  `max_a.x >= min_b.x && min_a.x <= max_b.x && max_a.y >= min_b.y && min_a.y <= max_b.y`
- AABB が交差しない → overlap なし(disjoint ケースを正しく弾く)
- AABB が交差する → overlap あり(正常な coplanar face を正しく通す)
- 厳密な polygon intersection は凸入力限定では AABB 交差で十分であり、sliver への耐性も LENGTH_TOLERANCE で担保される

また `get_other_loop_verts` は `get_loop_vertex_points(other, other.faces[fi].outer_loop)` で実装する(既存の L177 の関数と同じ実装)。

### 修正後の確認

- `cargo xtask ci` green
- 特に `t05_intersect_disjoint_boxes` が `EmptyBooleanResult` で pass すること

---

## 検証手順 (end-to-end)

1. `cargo xtask ci` が green になる
2. 以下を手動実行:
   ```bash
   cargo run -p mycad-cli -- export examples/boolean_box_cut.mycad -o /tmp/box_cut.stl
   cargo run -p mycad-cli -- export examples/boolean_box_fuse.mycad -o /tmp/box_fuse.stl
   cargo run -p mycad-cli -- export examples/boolean_box_intersect.mycad -o /tmp/box_isect.stl
   cargo run -p mycad-cli -- export examples/boolean_box_void.mycad -o /tmp/box_void.stl
   ```
   各 STL が MeshLab/Blender で開けることを目視確認(これは #35 の人間検証 gate に正式化されているが、#33 内でも sanity check として行う)。
3. `cargo run -p mycad-cli -- view examples/boolean_box_void.mycad` でビューア起動し、空洞シェルが描画されること(視覚的には外殻と同色の box が見えるだけだが、`view` のメッシュ枚数で確認可能)。

