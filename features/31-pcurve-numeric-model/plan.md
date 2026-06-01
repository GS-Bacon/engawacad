# Issue #31 — 曲面 Boolean 用 pcurve + 数値モデル(ADR-004)の決定と実装

## Context

Phase 4 (Boolean) のクリティカルパスは `#27 ∥ #32` → `#33` → `#31` → `#34` → `#35`。
本 Issue #31 は **foundation** であり、曲面 Boolean (#34) が必要とする 2 つの基盤を確立する:

1. **数値モデル**: ADR-004 が「Phase 4 で決定」と保留した論点を確定する
2. **pcurve (曲面パラメータ空間上の 2D 曲線)**: 曲面交線を安定に扱う構造

調査により判明した重要事実:
- pcurve / Curve2D は今コードベースに**全く存在しない** (`grep` 0 hit)
- `HalfEdge` は pcurve slot なし、face back-reference なし (`crates/mycad-kernel/src/brep/topology.rs:23-32`)
- 平面 Boolean (#33) の intersection edge は `name: None` で構築されている (`crates/mycad-kernel/src/booleans/assemble.rs:127`) — Derived 名前付けの隙間が残っている
- `FaceFragment.parent_name` は単一親のみ (`partition.rs:14`) — 2 親 (target_face + tool_face) provenance を運べない
- `LENGTH_TOLERANCE` 系グローバル定数は booleans/tessellation/primitives/build から多数参照されている — 一括差し替えはリスク

本 Issue は **型導入 + 新規パスのみ**で完結し、既存 call site の per-entity 公差移行は段階的 (#34 以降)。

## Non-Goals

- **曲面 ∩ 曲面の交線計算アルゴリズム本体** — #34 が担当 (本 Issue は型と plumbing のみ)
- **既存 LENGTH_TOLERANCE/ANGLE_TOLERANCE 参照箇所の全面 per-entity 移行** — 段階移行、本 Issue は型新設と新規コード経路のみ
- **`tessellate_face_uv_grid` の trim-loop 対応 (UvGridFullPatch 全周ゲート緩和)** — #34
- **`Surface` への `du/dv/tangent_at_uv` 偏微分メソッド導入** — #34 で必要になった時点で
- **`Curve` enum への自由曲線 (NURBS 等) 追加** — pcurve 2D に限定。3D 自由曲線は #34
- **`Curve2D` への NURBS/Sampled variant 追加** — 最小 (`Line2D` + `Circle2D`) で発足、後続で additive 追加
- **DegenerateFace の zero-area 検出強化** — 別 Issue 案件 (前回 plan の R01 棄却に対応)
- **`Document`/`Component` mutation API 改修 (`register_validated` 等)** — 別 Issue 案件 (前回 plan の R02 棄却)
- **`face_scale` モジュール導入** — 必要になった時点で配置決定 (前回 plan の R03 棄却)
- **Sphere 極・Cone apex における tangent 退化対応** — `tangent_at_uv` を入れないため発生せず (前回 plan の R04 解消)
- **EntityRef enum の variant 追加 / JsonSchema 変更** — 完全 additive (新規 op 文字列のみ)。`t11_json_schema_golden` / TS golden は不変
- **`assert_solids_equal` (`crates/mycad-build/tests/feature_dispatcher.rs:8-60`) の根本書き換え** — name 比較版は本 Issue で**追加**するが、既存関数のシグネチャは変えない

## 実装対象

- **Issue**: #31
- **影響クレート/ファイル**:
  - `crates/mycad-kernel/src/geometry/tolerance.rs` *(新規)* — `Tolerance` newtype + per-entity 公差比較 helper
  - `crates/mycad-kernel/src/geometry/pcurve.rs` *(新規)* — `Curve2D` enum + `Pcurve` struct
  - `crates/mycad-kernel/src/geometry/mod.rs` — submodule 宣言 + 再エクスポート
  - `crates/mycad-kernel/src/error.rs` — 新 variant 追加 (additive)
  - `crates/mycad-kernel/src/brep/topology.rs` — `HalfEdge.pcurve: Option<Pcurve>` 追加 + `add_half_edge_with_pcurve` helper + `validate_manifold` の戻り型を `Result<(), KernelError>` に変更 + pcurve 整合性チェック (Round 3 R03)
  - `crates/mycad-kernel/src/tessellation/mod.rs` — `collect_loop_points` が pcurve あれば UV→3D で sample、直線 pcurve は既存 `Curve::Line.sample_segment` と同じ 1 点契約 (Round 3 R04)
  - `crates/mycad-kernel/src/booleans/partition.rs` — `IntersectionSegment` 型導入 + `pslg_subdivide` 戻り型変更 + `FaceFragment.boundary_partners` (per-edge provenance、Round 3 R01)
  - `crates/mycad-kernel/src/booleans/assemble.rs` — `derive_edge_name` + `canonicalize_provenance` helper、per-edge partner 経由で intersection edge name 付与 (Round 3 R01)
  - `crates/mycad-kernel/src/lib.rs` — 再エクスポート追加
  - `crates/mycad-build/tests/feature_dispatcher.rs` — `assert_solids_equal_with_names` (新規) + 既存テストはそのまま
  - `docs/decisions/004-freeform-geometry-commitment.md` — Decision 3 改訂、トレラント方式確定

- **変更する型/関数のシグネチャ**:

  ```rust
  // crates/mycad-kernel/src/geometry/tolerance.rs (新規)
  // R01 対応: 内部 f64 は private。Deserialize は validating constructor 経由でのみ通る。
  #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
  pub struct Tolerance(f64); // private field — public field 公開なし
  // Deserialize は custom impl: tuple-struct 形式で読んで Tolerance::new() に通す

  impl Tolerance {
      pub const DEFAULT: Tolerance = Tolerance(LENGTH_TOLERANCE);
      pub fn new(value: f64) -> Result<Tolerance, KernelError>; // value <= 0 / NaN / Inf -> InvalidTolerance
      pub fn value(self) -> f64;
      pub fn max(self, other: Tolerance) -> Tolerance;
  }

  pub fn length_near_within(a: f64, b: f64, tol: Tolerance) -> bool;
  pub fn point_near_within(a: &Point, b: &Point, tol: Tolerance) -> bool;
  ```

  ```rust
  // crates/mycad-kernel/src/geometry/pcurve.rs (新規)
  // R01 対応: variant payload も含めて field は private。アクセスは accessor 経由。
  // Serialize は派生、Deserialize は custom impl (shadow enum → try_* 経由で validate)。
  // EntityRef (crates/mycad-format/src/feature.rs:153-242) と同じパターン。
  #[derive(Debug, Clone, PartialEq, Serialize)]
  #[serde(tag = "curve2d_type", rename_all = "snake_case")]
  pub enum Curve2D {
      Line2D   { origin: (f64, f64), direction: (f64, f64) },
      Circle2D { center: (f64, f64), radius: f64 },
  }
  // 注: variant payload field は構造定義上は付くが、enum 外から直接 mutation できないように
  //     全 mutation は try_line/try_circle 経由とし、read は accessor (line_data(), circle_data()) 経由。
  //     pattern match での read は許容するが、ユーザはこれを使うべきではない (validate 不能のため)。

  impl Curve2D {
      // R02 対応: パラメータ空間 (無次元) なので、reject は non-finite と「真の zero」のみ。
      //          LENGTH_TOLERANCE (3D mm) との閾値比較は使わない。
      pub fn try_line(origin: (f64, f64), direction: (f64, f64)) -> Result<Self, KernelError>;
      // reject: !origin.{0,1}.is_finite() || !direction.{0,1}.is_finite() || (direction == (0.0, 0.0))
      pub fn try_circle(center: (f64, f64), radius: f64) -> Result<Self, KernelError>;
      // reject: !center.{0,1}.is_finite() || !radius.is_finite() || radius <= 0.0 (真の zero/負)
      pub fn evaluate(&self, t: f64) -> (f64, f64);
      // sample_segment は t_start → t_end の方向に N 点 (終点含まず)。
      // t_start > t_end (降順) も許可: CW 円弧や逆向き直線が表現可能。
      pub fn sample_segment(&self, t_start: f64, t_end: f64, segments: usize) -> Vec<(f64, f64)>;
  }

  #[derive(Debug, Clone, PartialEq, Serialize)]
  pub struct Pcurve {
      curve_2d: Curve2D, // private
      t_range: [f64; 2], // private — [t_at_start_vertex, t_at_end_vertex] HE traversal 順
  }

  impl Pcurve {
      // 降順 (t_range[0] > t_range[1]) を許可。両端 finite 必須。t_range[0] == t_range[1] (ゼロ長) は reject。
      pub fn try_new(curve_2d: Curve2D, t_range: [f64; 2]) -> Result<Self, KernelError>;
      pub fn curve_2d(&self) -> &Curve2D;
      pub fn t_range(&self) -> [f64; 2];
      pub fn evaluate(&self, t: f64) -> (f64, f64); // clamp は min/max(t_range)
      // sample は t_range[0] → t_range[1] の HE traversal 方向に進む。
      // (Circle2D + 降順 t_range で CW 円弧、 + 昇順 t_range で CCW 円弧)
      pub fn sample(&self, segments: usize) -> Vec<(f64, f64)>;
  }
  // Pcurve / Curve2D 共に custom Deserialize: shadow struct/enum で受けて try_* で validate。
  // 不正な YAML/JSON は serde::de::Error::custom 経由で reject。
  ```

  ```rust
  // crates/mycad-kernel/src/brep/topology.rs (変更)
  pub struct HalfEdge {
      pub id: EntityId,
      pub start_vertex: usize,
      pub edge: usize,
      pub forward: bool,
      #[serde(skip_serializing_if = "Option::is_none", default)]
      pub pcurve: Option<Pcurve>, // ← 新規
  }

  impl Solid {
      // 既存 add_half_edge は内部で add_half_edge_with_pcurve(.., None) を呼ぶ delegate に変更 (signature は不変)
      pub fn add_half_edge(&mut self, id: EntityId, start_vertex: usize, edge: usize, forward: bool) -> usize;
      pub fn add_half_edge_with_pcurve(&mut self, id: EntityId, start_vertex: usize, edge: usize, forward: bool, pcurve: Option<Pcurve>) -> usize;
  }
  ```

  ```rust
  // crates/mycad-kernel/src/booleans/partition.rs (変更、Round 3 R01)
  // intersection 由来の segment は partner provenance を per-segment で保持する。
  pub struct IntersectionSegment {
      pub p_start: (f64, f64),
      pub p_end:   (f64, f64),
      pub partner: EntityRef, // 当該 segment を生成した相手側 face 名 (intersection の他方の親)
  }

  // pslg_subdivide は各 sub-fragment の boundary edge ごとの partner を返す。
  // 出力 polygon の edge i は vertex[i] → vertex[(i+1) % n] を指し、
  // partners[i] = 当該 edge が internal segment 由来なら Some(その partner), outer loop 由来なら None。
  // segments 同士の交点で edge が分割されても、両分割 edge は同じ partner を継承する。
  fn pslg_subdivide(
      outer_loop: &[(f64, f64)],
      segments: &[IntersectionSegment],
      plane: &PlaneData,
  ) -> Vec<(Vec<(f64, f64)>, Vec<Option<EntityRef>>)>;

  // 出力 boundary_partners は polygon_3d の edge 構造と並列。
  pub struct FaceFragment {
      pub source_face_index: usize,
      pub polygon_3d: Vec<Point>,
      pub plane: PlaneData,
      pub parent_name: EntityRef,
      pub traversal_index: u32,
      pub is_tool_side: bool,
      pub boundary_partners: Vec<Option<EntityRef>>, // Round 3 R01: per-edge provenance
  }
  ```

  ```rust
  // crates/mycad-kernel/src/booleans/assemble.rs (新規 helper)
  // R03 対応: intersection edge 命名は total invariant (必須)。Option ではなく Result を返す。
  // 親 provenance 欠落は KernelError::MissingIntersectionProvenance で明示エラー。
  fn derive_edge_name(parents: &[EntityRef], op: BooleanOp, selector: &str) -> Result<EntityRef, KernelError>;
  fn canonicalize_provenance(parents: &[EntityRef], op: BooleanOp) -> Vec<EntityRef>;
  // op の文字列: "cut_isect_edge" | "fuse_isect_edge" | "intersect_isect_edge"

  // selector の一意化: 結果 Solid 内で `(op, canonical_parents)` group ごとに、
  // edge の normalize_edge_key (= [min_vi, max_vi]) 昇順 sort で deterministic ordinal を振る。
  // → 同じ親 face pair から複数 intersection edge が出ても衝突しない。
  fn assign_intersection_edge_selectors(
      // (op, canonical_parents, edge_idx, edge_key) のリストを受け、
      // group 内 sort で "e{:04}" 形式の selector を返す
      candidates: &[(BooleanOp, Vec<EntityRef>, usize, [usize; 2])],
  ) -> HashMap<usize, String>; // edge_idx → selector
  ```

  ```rust
  // crates/mycad-kernel/src/error.rs に追加 (additive variant)
  pub enum KernelError {
      // ...existing...
      InvalidTolerance               { value: f64 },
      InvalidPcurveTrange            { t_start: f64, t_end: f64 },
      DegeneratePcurve               { reason: &'static str },
      PcurveSurfaceMismatch          { he_idx: usize, deviation: f64, tolerance: f64 },
      MissingIntersectionProvenance  { op: String }, // R03: intersection edge の親欠落
  }
  ```

## 設計方針

### 1. 数値モデル: トレラント (per-entity) 方式採用

ADR-004 Decision 3 を改訂して以下を確定:

- **採用方式**: Vertex/Edge/Face が `Tolerance` を保持する Parasolid/ACIS 流のトレラントモデル
- **本 Issue で実装する範囲**: `Tolerance` 型と `length_near_within` / `point_near_within` 等の per-entity 公差 helper のみ。既存の `Vertex`/`Edge`/`Face` への field 埋め込みと既存 call site の差し替えは段階移行 (#34 以降)
- **互換性**: `LENGTH_TOLERANCE` / `ANGLE_TOLERANCE` / `RELATIVE_TOLERANCE` グローバル定数は **既定値** として残し、`Tolerance::DEFAULT` から参照
- **段階移行の根拠**: 既存 call site 数が 5 ファイル多数 (booleans/{mod,partition,classify,assemble}, tessellation, primitives, build) — 一括差し替えは決定性回帰リスク。型導入と新規パスを foundation として固めた上で #34 が実装と同時に必要箇所を移行する

ADR-004 改訂後の Decision 3 (要約):
> Phase 4 は Parasolid/ACIS 流の per-entity トレラント方式を採用する。`Tolerance` newtype を `geometry::tolerance` に導入し、`Vertex.tolerance` / `Edge.tolerance` / `Face.tolerance` field を将来追加する (本 Issue では型のみ、field 追加は #34)。比較は両端の Tolerance の max を用いる。グローバル定数は `Tolerance::DEFAULT` の値として残し、既存 call site を段階移行する。

### 2. 交線 pcurve: HalfEdge 側に `Option<Pcurve>`

- `HalfEdge` に `pcurve: Option<Pcurve>` を追加 (OpenCASCADE 慣行)
- 同一 `Edge` の両側 HalfEdge は **独立した pcurve** を持てる (異なる surface のパラメータ空間に対応)
- pcurve なし HE は従来通り `Edge.curve` (3D) から直接サンプル — 既存挙動を完全に温存
- `Curve2D` は最小 2 variant (`Line2D`, `Circle2D`) で発足、`Sampled2D` / `NURBS2D` は #34 / 後続 issue で additive 追加

**HE の向きと pcurve の関係 (R01 対応)**:
- `Pcurve.t_range = [t_at_start_vertex, t_at_end_vertex]` で**当該 HE の traversal 方向そのもの**を表す
- **降順 t_range (`t_range[0] > t_range[1]`) を許可** — CW 円弧や逆向き直線のトリムを表現可能にする
- `Circle2D` 自体には方向 field を持たせない (t_range の順序で十分; trim 円弧の sweep は `(t_range[0], t_range[1])` の差分が表す)
- `HalfEdge.forward` フラグは pcurve に対しては独立: pcurve は HE 視点から見て常に `t_range[0]` 起点で進む (forward は edge.curve 側の解釈のみに使う)
- `try_new` / `evaluate` / `sample` は降順を尊重 (反転や絶対値化をしない)

**サンプリング契約**:
- `Curve2D::sample_segment(t_start, t_end, segments)` は **終点を含まない** N 点 (既存 `Curve::sample_segment` の慣行に合わせる)
- 降順 (`t_start > t_end`) でも同様に N 点を生成 (t は `t_start` から `t_end` へ単調変化)
- `Pcurve::evaluate(t)` は `t_range` の閉区間外で clamp する (clamp は `min(t_range)` と `max(t_range)` を用いる)

### 3. pcurve 整合性チェック (`validate_manifold` 拡張)

**戻り型変更 (Round 3 R03 対応)**: `validate_manifold` の現行 signature `pub fn validate_manifold(&self) -> Result<(), &'static str>` を **`Result<(), KernelError>`** へ変更する。pcurve 整合性専用の structured エラー (`PcurveSurfaceMismatch{he_idx, deviation, tolerance}`, `InvalidPcurveTrange{..}`, `DegeneratePcurve{..}`) を返せるようにするため。
- 既存の `Err("...")` 文字列リテラルは `KernelError::ManifoldViolation { reason: &'static str }` 等の包む variant を新設して移行 (additive、既存の caller は文字列リテラルを生成しないので壊れない)
- `mycad-build` / テスト側の `validate_manifold().unwrap()` などの単純 caller はそのまま動作
- structured エラーが必要な caller (本 Issue で追加される T13b/T13c) は match で variant を読む

**pcurve の権威性 (Round 1 R02 対応)**: HE に `pcurve` がある場合、**tessellation はその pcurve を権威としてサンプル**する。
したがって `pcurve` と `edge.curve` が**同一の 3D 曲線**を異なるパラメトリゼーションで表現していなければ、
B-rep の `edge.curve` ベース解析と tessellation 境界が食い違う。これを `validate_manifold` が検出する。

pcurve を持つ各 HE について以下を検査:

1. `t_range` の両端が finite (降順は許可、`t_start == t_end` のみ `InvalidPcurveTrange` で reject)
2. `Curve2D` の degenerate チェック (`Line2D` の direction がゼロベクトルでない、`Circle2D` の radius が正)
3. **3D 整合性 (端点 + 中点)**: 以下の 3 サンプル点でそれぞれ「pcurve → surface→3D 持ち上げ」と「edge.curve → 3D 評価」が一致 (`Tolerance::DEFAULT` 内、本 Issue では per-entity tolerance field 未導入のため定数):
   - **HE 始点** (fraction `f = 0.0`):
     - pcurve 側: `surface.evaluate(pcurve.evaluate(t_range_p[0]))`
     - edge 側: `edge.curve.evaluate(t_e_at_he_start)` ← `forward` なら `edge.t_range[0]`、reverse なら `edge.t_range[1]`
   - **HE 中点** (fraction `f = 0.5`):
     - pcurve 側: `surface.evaluate(pcurve.evaluate(lerp(t_range_p[0], t_range_p[1], 0.5)))`
     - edge 側: `edge.curve.evaluate(lerp(t_e_at_he_start, t_e_at_he_end, 0.5))`
   - **HE 終点** (fraction `f = 1.0`): 同様に
   - **中点チェックが R02 を塞ぐ**: 端点 2 点だけ一致して中身が違う曲線 (例: edge は直線、pcurve は円弧) は中点で乖離するため検出される

整合性チェックは **pcurve を持つ HE にのみ** 適用し、pcurve なし HE は従来の vertex 連結性チェックのみ。既存テスト群が壊れない。

**face back-reference の解決**: `HalfEdge` 自体は face を知らないが、`validate_manifold` は `solid.faces[].outer_loop` / `inner_loops` → `solid.loops[].half_edges` のスキャン中に face コンテキストを保持できる。新規 helper:
```rust
fn iter_face_halfedges(solid: &Solid) -> impl Iterator<Item = (usize /*face_idx*/, usize /*he_idx*/, bool /*is_outer*/)>;
```
を `topology.rs` 内 private 関数として導入。決定性のため faces Vec 順 → loop 内 he Vec 順で iterate。

### 4. 交線エンティティの安定名 (Derived 名)

現状: 平面 Boolean の intersection edge は `name: None`、これは ADR-005 §4 が要求する安定名の欠落。本 Issue で塞ぐ。

**契約 (R03 対応)**: intersection edge の命名は **total** (全 intersection edge が必ず名前を持つ) かつ **unique** (結果 Solid 内で canonical_name が衝突しない) な invariant。

- **op 文字列** (charset `[A-Za-z0-9_-]` 準拠): `cut_isect_edge` / `fuse_isect_edge` / `intersect_isect_edge`
- **provenance**: `from = [target_face_name, tool_face_name]` (2 親必須)
- **canonical 順序** (ADR-005 §5 決定性制約):
  - 非可換 op (`Cut`): 順序保持 `[target, tool]`
  - 可換 op (`Fuse`, `Intersect`): `EntityRef::canonical_name()` 字句順で安定 sort
- **selector の一意性ロジック (R03 対応)**:
  - 結果 Solid 全体の intersection edge を `(op, canonicalized_parents)` ごとに group 化
  - 各 group 内で edge の `normalize_edge_key([min_vi, max_vi])` 昇順 sort
  - sort 順に `e0000`, `e0001`, ... を deterministic に割当
  - これにより同じ親 face pair から複数 fragment が生じても衝突しない

**provenance plumbing (Round 2 R03 + Round 3 R01 対応)**:

**重要 (Round 3 R01)**: provenance は **fragment レベルではなく per-edge レベル**で保持する。理由は `partition_faces` (`partition.rs:168-227`) が複数 partner face 由来の segment を 1 本の `segments` Vec に平坦化してから `pslg_subdivide` に渡すため、結果 fragment の boundary edge は異なる partner 由来の segment を混在させ得る。fragment 単位の `partner_parent_name: Option<EntityRef>` では混在 fragment の per-edge 復元ができない。

実装方針:
- `Segment2D = ((f64,f64),(f64,f64))` を `IntersectionSegment { p_start, p_end, partner: EntityRef }` へ変更
- `partition.rs:168-227` で 2 面交差を計算するループの `segments.push(((s_start_final, s_end_final)))` を、相手 tool face の名前 (`tool.faces[ui].name`) を伴う `IntersectionSegment` push に変更。tool 面側ループ (lines 266-328) も対称に
- `pslg_subdivide` の戻り型を `Vec<Vec<(f64,f64)>>` から **`Vec<(Vec<(f64,f64)>, Vec<Option<EntityRef>>)>`** に変更。出力 polygon の edge i (vertex[i]→vertex[(i+1)%n]) の partner を 2 番目 Vec が同 index で保持
  - edge 由来判定: pslg_subdivide が build する `edges: Vec<(usize, usize)>` の各 edge を「outer loop 由来」「intersection segment 由来」のどちらかでタグ付けし、 segments 由来の場合は partner を `Some(...)` で記録。outer loop 由来は `None`
  - segment 同士の交点で edge が分割されても、両分割 edge は同じ partner を継承
- `FaceFragment` は `parent_name` (元 face の名前) と **`boundary_partners: Vec<Option<EntityRef>>`** (per-edge partner) を併せ持つ
- `assemble.rs:101-132` の edge 構築ループで以下を行う:
  1. 各 edge について両側 fragment の `boundary_partners[edge_local_idx]` を読む
  2. **両側に partner provenance がある (Some)** → intersection edge と判定。`canonicalize_provenance([parent_name_target, partner])` か `([parent_name_target, partner_left, ...])` 形式の親リストで `derive_edge_name(...)` を呼ぶ
     - target 側 fragment と tool 側 fragment が向かい合う edge では partner = 相手 face 名 → 2 親が一致するはず (target 側 fragment.parent_name = T、partner = U、tool 側 fragment.parent_name = U、partner = T)。canonical sort で同じ親リストに正規化される
  3. partner が None (両側または片側) → outer loop 由来等、name = None のまま (従来通り。coplanar 由来 edge も含む)
  4. `derive_edge_name` が `Err(MissingIntersectionProvenance)` を返したら `boolean_planar` 全体を fail させる
- coplanar overlap (`partition.rs:374-420`) は対応する CoplanarPair から partner を `Some` で記録するか、明示的に「coplanar 由来 edge は intersection ではない」契約のもと None で push する (本 Issue では後者: coplanar 由来 edge は従来通り name = None のまま、scope 外)

**エラーモード**: 「片側に partner provenance があるのに反対側に無い」は内部不整合 → `KernelError::MissingIntersectionProvenance` で明示 fail。`name: None` には絶対に落ちない (intersection 判定された edge については)。

### 5. 曲面交線テッセレーション

`collect_loop_points` (`tessellation/mod.rs:315-339`) を分岐:

```rust
let points = if let Some(pcurve) = &he.pcurve {
    let face = &solid.faces[face_idx];
    // Round 3 R04 対応: pcurve の Curve2D variant ごとに既存 3D curve と同じ点数契約を維持
    let uv_samples = match pcurve.curve_2d() {
        Curve2D::Line2D { .. } => {
            // 既存 Curve::Line.sample_segment は 1 点 (t_start のみ) を返す契約
            // (crates/mycad-kernel/src/geometry/curve.rs:42-44 参照)。
            // 直線 pcurve も同じ 1 点契約に揃えることで T14「既存挙動同等」を維持
            vec![pcurve.evaluate(pcurve.t_range()[0])]
        }
        Curve2D::Circle2D { .. } => {
            // 円弧は既存 Curve::Circle と同様 N 点 (終点含まず)
            pcurve.sample(segments)
        }
    };
    uv_samples.into_iter()
        .map(|(u, v)| face.surface.evaluate(u, v))
        .collect()
} else {
    edge.curve.sample_segment(t_start, t_end, segments)
};
```

`face_idx` は呼び出し元 (`tessellate_face_*` 系) から渡す。
**注意**: `tessellate_face_uv_grid` の全周ゲート (`mod.rs:358-376`) は本 Issue では緩和しない (Non-Goal)。平面 face の `BoundaryFan` 経路と `tessellate_face_earcut` 経路で pcurve サンプリングが動くことを優先する。

**Round 3 R04 補足**: `Pcurve::sample(segments)` 自体は `Circle2D` で N 点 / `Line2D` で N 点を返す素直な実装にし、「直線は 1 点」のレイアウト判断は `collect_loop_points` 側で行う。これにより `Pcurve::sample` の単体テスト (T06) は variant ごとの素直なサンプリングを検証でき、tessellation 統合の点数制約は T14 が検証する。

### 6. 決定性要件

- `Tolerance` の `PartialOrd` 比較は NaN 入力を `Tolerance::new` で弾く (構築時 reject)
- `Curve2D` / `Pcurve` の Serialize は struct field 宣言順
- `derive_edge_name` の selector は parent fragment 内のフラット index (HashMap iteration 禁止)
- `validate_manifold` の pcurve 整合性チェックは Vec 順序のみで iteration (ADR-005 §9 準拠)
- `canonicalize_provenance` の 可換 op sort は `EntityRef::canonical_name()` 文字列で安定 sort
- 新規 helper は HashMap/HashSet を iteration せず (`grep` で確認、ある場合は sort 経由)

### 7. 退化幾何の扱い

新規 `KernelError` variant で reject:
- `Tolerance::new(value)`: `value <= 0` / NaN / Inf で `InvalidTolerance { value }`
- `Pcurve::try_new`: NaN / Inf、または `t_start == t_end` (ゼロ長 trim) で `InvalidPcurveTrange { t_start, t_end }`。**降順 (`t_start > t_end`) は許可** (Round 1 R01 対応: CW 円弧トリム表現のため)
- **`Curve2D::try_line`** (Round 2 R02 対応 — パラメータ空間は無次元、3D LENGTH_TOLERANCE 閾値を使わない):
  - 各座標 / direction 成分が NaN/Inf → `DegeneratePcurve { reason: "non-finite component" }`
  - direction が**真の zero** (`direction == (0.0, 0.0)`) → `DegeneratePcurve { reason: "zero direction" }`
- **`Curve2D::try_circle`** (Round 2 R02 対応):
  - center 成分 / radius が NaN/Inf → `DegeneratePcurve { reason: "non-finite component" }`
  - `radius <= 0.0` (真の zero / 負) → `DegeneratePcurve { reason: "non-positive radius" }`
- `validate_manifold`: pcurve 持ち上げが edge.curve サンプル (端点 + 中点) と乖離 `> Tolerance::DEFAULT` で `PcurveSurfaceMismatch { he_idx, deviation, tolerance }` (Round 1 R02 対応: 中点を含めた 3 点で乖離を検出)
- **`derive_edge_name`** (Round 2 R03 対応): 親 provenance 欠落 (`parents.is_empty()` 等) で `MissingIntersectionProvenance { op: op_str }`

**重要 (Round 2 R01 対応)**: `Tolerance` / `Curve2D` / `Pcurve` は全て field を private 化し、`Deserialize` を custom impl で validate する。
- pattern: shadow struct/enum を中間に置く (EntityRef の Custom Deserialize 実装 `crates/mycad-format/src/feature.rs:198-242` を参照)
- shadow → `try_*` 経由 → 不正は `serde::de::Error::custom` で reject
- これにより YAML/JSON 経由でも不変条件を bypass できない

既存の DegenerateFace (1 点退化 sentinel) は本 Issue では拡張しない (前回 plan R01 に対応する scope 防衛)。

### 8. derive 規約

| 型 | derive | Deserialize |
|---|---|---|
| `Tolerance` | `Debug, Clone, Copy, PartialEq, PartialOrd, Serialize` | **custom** (Round 2 R01) |
| `Curve2D` | `Debug, Clone, PartialEq, Serialize` | **custom** (Round 2 R01) |
| `Pcurve` | `Debug, Clone, PartialEq, Serialize` | **custom** (Round 2 R01) |

**Custom Deserialize の方針 (Round 2 R01 対応)**:
- shadow struct/enum を中間に置いて raw field を読み、`try_*` constructor 経由で validate
- 不正は `serde::de::Error::custom("...")` で `Err` 化
- 参考実装: `crates/mycad-format/src/feature.rs:198-242` の `EntityRef` Deserialize
- `Tolerance` / `Pcurve` の field は private 化、accessor method (`curve_2d()`, `t_range()`, `value()` 等) 経由で read

**Round 3 R02 棄却 (公開 enum variant payload の opaque 化は scope 外)**:
- `Curve2D` の `Line2D { origin, direction }` / `Circle2D { center, radius }` の struct-like variant は **pub のまま残す** (`features/31-pcurve-numeric-model/rejection.md` R02 参照)
- 棄却理由要旨: pub enum + struct-like variant の直接構築問題は workspace 全 enum (`Curve`, `Surface`, `EntityRef`, `Feature` 等) に共通する Rust 構文上の制約で、#31 単独で opaque pattern を導入すると一貫性が崩れる。実質的な不正侵入経路 (YAML/JSON 境界・公開 constructor) は Round 2 R01 で塞ぎ済み
- 代替措置: rustdoc で「variant の直接構築禁止、必ず `try_line` / `try_circle` を使う」を明記。`crates/mycad-kernel/CLAUDE.md` に「`Curve2D` literal 構築禁止」を実装フェーズ (STEP B) で追加

**JsonSchema / TS は付与しない**:
- 全て kernel-internal 型 (Solid に乗るが Document には乗らない)
- TS 側で使われない (TriangleMesh だけが viewer の消費対象)
- `EntityRef` の JsonSchema/TS golden は変更なし (additive な op 文字列のみ追加)

### 9. エラーハンドリング

`thiserror` 使用、既存パターンに従う。`KernelError` の新 variant は追加 (existing variants は不変)。`KernelError` の JSON schema は存在しないため、列挙追加で外部破壊なし。

### 10. workspace.dependencies

新規依存は **無し**。既存 crates (`nalgebra`, `serde`, `thiserror`, etc.) のみで実装可能。

## テスト計画 (ID 付き)

| ID  | 種別     | 内容                                                                                                  | 期待結果                                                              |
|-----|----------|-------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------|
| T01 | 正常系   | `Tolerance::new(1e-9)`                                                                                | `Ok(Tolerance(1e-9))`                                                 |
| T01b| 境界 YAML| `serde_yaml::from_str::<Tolerance>("-1e-9")` / `"NaN"` / `"Infinity"` (R01)                            | `Err(serde error)` — custom Deserialize で reject                     |
| T02 | エッジ   | `Tolerance::new(-1e-9)` / `Tolerance::new(f64::NAN)` / `Tolerance::new(f64::INFINITY)`                | `Err(InvalidTolerance{..})` 各々                                      |
| T03 | 正常系   | `Tolerance::max(Tolerance(0.1), Tolerance(0.5))`                                                      | `Tolerance(0.5)`                                                      |
| T04 | 正常系   | `length_near_within(1.0, 1.0 + 5e-10, Tolerance::DEFAULT)` / `point_near_within(p, q, tol)`           | true / 境界値で適切                                                   |
| T05 | 正常系   | `Curve2D::try_line((0.0,0.0), (1.0,0.0)).evaluate(0.5)`                                               | `(0.5, 0.0)`                                                          |
| T06 | 正常系   | `Curve2D::try_circle((0.0,0.0), 1.0).sample_segment(0.0, PI, 4)`                                      | 4 点全てが単位円上 (半径 1±ε)、終点 (π) 含まず                        |
| T07 | エッジ   | `Curve2D::try_line(.., (0.0,0.0))` (真の zero) / `Curve2D::try_circle(.., 0.0)` / `try_circle(.., -1.0)` | `Err(DegeneratePcurve{..})` (R02: LENGTH_TOLERANCE 閾値は使わない)    |
| T07b| 正常系 R02| `Curve2D::try_line(.., (1e-15, 0.0))` (極小だが非ゼロ)                                                | `Ok(..)` — UV はパラメータ空間なので極小値も valid                    |
| T07c| 境界 YAML| invalid Curve2D YAML (`{curve2d_type: line2d, origin: [0,0], direction: [0,0]}`) (R01)                | `Err(serde error)` — Deserialize で reject                            |
| T08 | エッジ   | `Pcurve::try_new(curve, [1.0, 1.0])` (ゼロ長) / `Pcurve::try_new(curve, [NaN, 1.0])`                  | `Err(InvalidPcurveTrange{..})`                                        |
| T08b| 正常系   | `Pcurve::try_new(Circle2D{..}, [PI, 0.0])` (降順 t_range, CW 円弧, Round 1 R01 対応)                  | `Ok(..)`、`sample(4)` が CW 順の 4 点を返す (角度が単調減少)          |
| T08c| 正常系   | `Curve2D::Line2D.sample_segment(1.0, 0.0, 3)` (降順)                                                  | 3 点が t=1.0 から t=0.0 へ単調進行 (反転や絶対値化なし)               |
| T08d| 境界 YAML| invalid Pcurve YAML (`{curve_2d: ..., t_range: [1.0, 1.0]}` ゼロ長) (Round 2 R01)                     | `Err(serde error)` — custom Deserialize で reject                     |
| T09 | 正常系   | `HalfEdge { pcurve: None }` の YAML roundtrip                                                         | pcurve field が serialize 出力に現れない (skip_serializing_if)        |
| T10 | 正常系   | `HalfEdge { pcurve: Some(Pcurve{..}) }` の YAML roundtrip                                             | 完全一致                                                              |
| T11 | 正常系   | `solid.add_half_edge_with_pcurve(.., Some(pcurve))` + `solid.add_half_edge(..)` の混在                | 両 API で HE が正しく追加                                             |
| T12 | 正常系   | `validate_manifold` で pcurve 端点 3D 持ち上げが edge 端点と一致 (Tolerance::DEFAULT 以内)            | `Ok`                                                                  |
| T13 | エッジ   | `validate_manifold` で pcurve 持ち上げが edge 端点から 1e-3 ずれている                                | `Err(PcurveSurfaceMismatch{ deviation, tolerance, .. })`              |
| T13b| エッジ   | `validate_manifold` で**端点一致だが中点乖離** (edge は直線、pcurve は同端点を持つ円弧, R02 対応)     | `Err(PcurveSurfaceMismatch{..})` (中点チェックで検出)                 |
| T13c| 正常系   | `validate_manifold` で**降順 t_range の pcurve** が reverse HE の edge.curve と整合 (R01 対応)        | `Ok` (R01/R02 の組み合わせ動作確認)                                   |
| T14 | 統合     | 平面 face + 直線 pcurve あり HE で tessellate_solid 実行 (Round 3 R04)                                | pcurve 経由でも edge.curve (Curve::Line) と同じ 1 点契約。loop 点数が既存と完全一致 |
| T14b| 統合     | 平面 face + 円弧 pcurve あり HE で tessellate_solid 実行                                              | Circle2D は N 点サンプリング (終点含まず)。点数 = `angular_segments`            |
| T15 | 正常系   | `derive_edge_name([f_target, f_tool], BooleanOp::Cut, "e0001")` の canonical_name (R03: Result 返し)  | `Ok(D(E;cut_isect_edge;e0001;[N(f_target;F:..),N(f_tool;F:..)]))`     |
| T15b| エッジ   | `derive_edge_name(&[], BooleanOp::Cut, "e0001")` (R03: 親 provenance 欠落)                            | `Err(MissingIntersectionProvenance{..})`                              |
| T16 | 決定性   | `derive_edge_name([B, A], Fuse, "e0001")` と `derive_edge_name([A, B], Fuse, "e0001")` の canonical_name | 完全一致 (commutative sort 効果)                                      |
| T17 | 正常系   | `derive_edge_name([B, A], Cut, "e0001")` と `derive_edge_name([A, B], Cut, "e0001")` の canonical_name | 異なる (非可換 op で順序保持)                                         |
| T17b| 決定性 R03| `assign_intersection_edge_selectors` で同じ `(op, canonical_parents)` group に 3 edge → e0000/e0001/e0002 | 全て unique、`normalize_edge_key` 昇順で deterministic                |
| T18 | 統合     | 既存 `boolean_planar` Cut 例で intersection edge が `Some(EntityRef::Derived{op:"cut_isect_edge",..})` | edge.name が `Some(..)` で正しい `op` / 2 親、unnamed に落ちない (R03) |
| T18b| 統合 R03 | 同じ `(target_face, tool_face)` から複数 intersection edge が出る Boolean 例 (T-tee 形状の Cut)        | 全 intersection edge が unique な selector、衝突なし                  |
| T18c| 統合 R3R01| 1 target face が複数の異なる tool face と交差する Boolean 例 (1 face を 2 tool 面が貫通)              | 各 intersection edge が **正しい partner** で名前付け (per-edge provenance) |
| T13d| エッジ R3R03| `validate_manifold` の戻り型が `Result<(), KernelError>` であり、エラー variant が match で読める     | `match validate_manifold() { Err(KernelError::PcurveSurfaceMismatch{..}) => .. }` |
| T19 | 決定性   | 平面 Boolean を 2 回 build → `assert_solids_equal_with_names` で名前を含めた完全一致                  | 全 edge/vertex/face name が同一                                       |
| T20 | golden   | T18 の boolean_box_cut.mycad → build 結果の intersection edge name を YAML golden として固定          | byte-identical                                                        |
| T21 | doc      | ADR-004 改訂版が「トレラント方式採用」「段階移行プラン」節を含む                                       | grep で節タイトルが見つかる (doc test)                                |
| T22 | 統合     | pcurve あり HE を含む solid を build → YAML roundtrip → 再 build → tessellate                          | tessellation 結果が完全一致                                           |

**テストの配置**:
- T01-T08d (T01b/T07b/T07c/T08b/T08c/T08d 含む): `crates/mycad-kernel/src/geometry/tolerance.rs` / `pcurve.rs` 内の `#[cfg(test)] mod tests`
- T09-T13d (T13b/T13c/T13d 含む): `crates/mycad-kernel/src/brep/topology.rs` 内の `#[cfg(test)] mod tests`
- T14, T14b, T22: `crates/mycad-kernel/src/tessellation/mod.rs` 内の `#[cfg(test)] mod tests`
- T15-T17b (T15b/T17b 含む): `crates/mycad-kernel/src/booleans/assemble.rs` 内の `#[cfg(test)] mod tests`
- T18-T20 (T18b/T18c 含む): `crates/mycad-build/tests/feature_dispatcher.rs` の integration test (公開 API 経由)
- T21: `docs/decisions/004-freeform-geometry-commitment.md` を読む integration test (場所は `crates/mycad-kernel/tests/adr_004_doc.rs` を新設)

## 幾何的不変条件チェックリスト (Boolean/Partition/Assemble 系)

- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか → **既存挙動を変更しない** (partition の polygon 順序ロジックに本 Issue は触れない、FaceFragment への field 追加のみ)
- [x] 各プリミティブの face ごとの outer_loop 2D 向き (CW/CCW) が文書化されているか → **N/A** (新規 primitive 追加なし、既存 cuboid/cylinder/sphere/extrusion の向きは ADR-005 §7 で既出)
- [x] flip_normals / same_sense の意味論が明確か → **N/A** (本 Issue は normal を変えない、pcurve は normal 反転を引き起こさない)
- [x] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか → **N/A** (PSLG 変更なし、partition.rs:914-966 はそのまま)
- [x] **(追加)** pcurve 2D 上の巻き方向と 3D edge 端点順 (forward フラグ) の整合性 → **T12 / T13 で 3D 端点一致を検証**

## 実装順序

1. **STEP A** — `crates/mycad-kernel/src/geometry/tolerance.rs` 新設 + `Tolerance` 型 + helper + T01-T04
2. **STEP B** — `crates/mycad-kernel/src/geometry/pcurve.rs` 新設 + `Curve2D` + `Pcurve` 型 + T05-T08
3. **STEP C** — `crates/mycad-kernel/src/error.rs` に新 variant 追加
4. **STEP D** — `crates/mycad-kernel/src/brep/topology.rs` で `HalfEdge.pcurve` + `add_half_edge_with_pcurve` helper + T09-T11
5. **STEP E** — 同ファイル `validate_manifold` 拡張 (face コンテキスト保持 iter + pcurve 整合性チェック) + T12-T13
6. **STEP F** — `crates/mycad-kernel/src/tessellation/mod.rs` の `collect_loop_points` に pcurve 経路 + T14, T22
7. **STEP G** — `crates/mycad-kernel/src/booleans/assemble.rs` に `derive_edge_name` + `canonicalize_provenance` helper + T15-T17 (unit test)
8. **STEP H** — `crates/mycad-kernel/src/booleans/partition.rs` の `FaceFragment.partner_parent_name` 追加 + `assemble.rs` の intersection edge name 付与 + `crates/mycad-build/tests/feature_dispatcher.rs` に `assert_solids_equal_with_names` 追加 + T18-T20
9. **STEP I** — `docs/decisions/004-freeform-geometry-commitment.md` 改訂 (Decision 3 確定形 + 段階移行プラン) + T21

各 STEP 完了後に `cargo xtask ci` グリーンを期待。

## 引き継ぎ (本 Issue → #34)

- `Surface` への `du/dv/tangent_at_uv` 偏微分メソッド導入は **#34** が必要に応じて追加
- `Curve2D` への `Sampled2D` / `NURBS2D` variant 追加は **#34** で additive
- `tessellate_face_uv_grid` の trim-loop 対応 (UV grid → earcut への切り替え) は **#34**
- `Vertex`/`Edge`/`Face` への `tolerance` field 埋め込みと、既存 `LENGTH_TOLERANCE` 参照箇所の per-entity 移行は **#34** が実装と同時に進める
- 曲面 ∩ 曲面の交線計算アルゴリズム本体は **#34**
