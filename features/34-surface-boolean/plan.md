# Issue #34 — 曲面 Boolean: 円柱/球を含む Cut/Fuse/Intersect (Phase 4 完了)

## Context

Phase 4 のクリティカルパス `#27 ∥ #32 → #33 → #31 → #34 → #35` の本丸。#31 で確立した 2 つの基盤 (`Curve2D`/`Pcurve` 型、ADR-004 のトレラント方式 + per-entity `Tolerance` 型、`IntersectionSegment` per-edge provenance、`derive_edge_name`) の上で、`#33` の平面 Boolean アルゴリズムを **円柱・球** を含むケースに一般化する。本 Issue 完了で Phase 4 の Acceptance「Cut/Fuse/Intersect が `.mycad` から動作する」が満たされ、#35 (ビューア目視確認) で Phase 4 を close できる。

調査により判明した重要事実:

- Boolean 実装の「平面前提」は 5 つのレイヤに染み込んでいる:
  - `partition.rs` の `PlaneData`/`intersect_planes`/`pslg_subdivide`/`clip_line_to_polygon_2d*`
  - `classify.rs` の `point_in_polyhedron` が **曲面 face を silent skip** (`classify.rs:181-184`) → in/out 判定破綻
  - `assemble.rs` の `signed_volume` が **曲面 face を silent skip** (`assemble.rs:476-479`) → 符号判定破綻
  - `assemble.rs` の `reverse_face_orientation` が `Surface::Plane` 限定 (`assemble.rs:393-395`)
  - `assemble.rs` の intersection edge `Curve::Line` 決め打ち (`assemble.rs:180`)
  - `mod.rs:51` の `NonPlanarBooleanInput` 入口 gate
- `Surface` enum (`geometry/surface.rs:25-47`) は Plane/Cylinder/Sphere/Cone の 4 variant、`evaluate`/`normal_at`/`uv_of`/`normal_at_point`/`tessellation_strategy` が揃っている。**偏微分 (`du`/`dv`) は無い**
- Sphere の極軸は **+Z** (`surface.rs:69-75`): `v.sin()` が z 成分 → 緯度 v=const の等高線は xy 平面平行円。Cylinder の極軸は外部から与える `axis` パラメータ
- `Curve` enum は `Line`/`Circle` のみ。`Curve2D` も `Line2D`/`Circle2D` のみ (#31 で導入)
- `make_cylinder` (`primitives/cylinder.rs:11`) は 1 個の side 面 + seam edge (4-HE outer loop)、**pcurve 未付与**
- `make_sphere` (`primitives/sphere.rs:12`) は 1 個の自己隣接周期面 + 1 seam edge (half-circle)、**pcurve 未付与**
- テッセレーション側は trim 未対応: `tessellate_face_uv_grid` の全周ゲート (`mod.rs:381-399`) と `tessellate_face_sphere` の canonical チェック (`mod.rs:1166-1291` の reject test) が trim 入力を弾く
- `collect_loop_points` (`mod.rs:319-362`) は pcurve があれば UV→3D 経路を持つ (#31 で実装済み) — trim path 用の基礎は既にある
- 公開関数名は `boolean_planar` (`booleans/mod.rs:14`)。`mycad-build/src/lib.rs:151,167,184` の 3 箇所のみで呼ばれる

#31 で公開された型を**そのまま流用**できるもの:
- `Curve2D::Line2D` / `Circle2D` (MVP の交線 pcurve はこの 2 variant で足りる)
- `Pcurve` (HalfEdge.pcurve スロット)
- `IntersectionSegment { p_start, p_end, partner }` (per-edge provenance)
- `FaceFragment.boundary_partners: Vec<Option<EntityRef>>` (per-edge partner)
- `derive_edge_name(parents, op, selector) -> Result<EntityRef, KernelError>` (intersection edge naming, total invariant)
- `assign_intersection_edge_selectors` (deterministic selector)
- `validate_manifold -> Result<(), KernelError>` (pcurve 整合性チェック付き)

本 Issue は **#33 の構造を保ったまま** Surface 抽象を引き上げ、4 種類の曲面ペア (Plane×Plane / Plane×Cylinder / Plane×Sphere / Cylinder×Sphere) を**軸整列条件下でのみ** 動かす。それ以外は新 `KernelError::UnsupportedSurfaceIntersection` で reject。

## Non-Goals

- **Sampled 曲線 / NURBS 曲線**: `Curve::Sampled` / `Curve2D::Sampled2D` の variant 追加は行わない。MVP の交線は全て `Curve::Line` か `Curve::Circle` で表現可能
- **Plane × Cylinder の軸非整列**: cylinder.axis が plane.normal と平行/反平行でない（`|axis·normal|.abs() < 1.0 - ANGLE_TOLERANCE`）ケースは `UnsupportedSurfaceIntersection` で reject。楕円交線は MVP 外
- **Plane × Cylinder の軸 ∥ plane (`axis·normal ≈ 0`)**: 直線 2 本の交線になるが、MVP では reject。実装複雑度を下げるため
- **Plane × Sphere の plane normal が ±Z 以外**: Sphere の極軸が +Z 固定のため、緯度等高線になる plane normal `±Z` のみ accept。それ以外は `UnsupportedSurfaceIntersection` で reject。box の側面 (normal=±X/±Y) と sphere の交差は MVP 外
- **Cylinder × Sphere の同軸でないケース**: sphere.center が cylinder.axis 直線上に無い、または cylinder.axis と sphere の極軸 (+Z) が平行でないケースは reject。Steinmetz 様交線は MVP 外
- **Cylinder × Cylinder の任意配置**: 同軸でも非同軸でも全 reject
- **Cone を絡む全 Boolean**: 既存 `Unsupported` のまま
- **per-entity tolerance 移行**: `Vertex.tolerance` / `Edge.tolerance` / `Face.tolerance` field 追加と既存 `LENGTH_TOLERANCE` 参照箇所の差し替えは別 Issue。本 Issue は `Tolerance::DEFAULT` 一本で動かす
- **coplanar / co-curved 検出の曲面拡張**: Plane×Plane の `coplanar_pairs` パスのみ既存維持。co-cylindrical / co-spherical / co-cylinder-sphere 検出は実装しない (Non-Goal)
- **Cylinder の周方向 seam 跨ぎ交線の精密分割**: 交線が seam edge を跨ぐ場合の pcurve t_range wrap は最小対応 (UV unwrap で u を `u + 2π` 補正) のみ、複数 edge への分割はしない
- **inner_loop の inner_loop (穴の中の穴)**: 1 階層のみ
- **`tessellate_face_uv_grid` の Cone trim 対応**: Cone は `Unsupported` のまま
- **ビューア目視確認** (`#35` の責務)
- **既存テスト `assert_solids_equal` 関数の根本書き換え** (`mycad-build/tests/feature_dispatcher.rs:8-60`): name 比較版は #31 で `assert_solids_equal_with_names` として追加済み。本 Issue では追加しない
- **Surface 偏微分 `du/dv` の追加**: MVP の交線は解析的に得られるため不要。後続 Issue で必要になった時点で

## 実装対象

- **Issue**: #34
- **影響クレート/ファイル**:
  - `crates/mycad-kernel/src/geometry/surface_intersect.rs` *(新規)* — 4 種類の曲面ペア交線関数 + dispatcher
  - `crates/mycad-kernel/src/geometry/mod.rs` — submodule 宣言 + 再エクスポート
  - `crates/mycad-kernel/src/error.rs` — `UnsupportedSurfaceIntersection { reason }` variant 追加
  - `crates/mycad-kernel/src/booleans/mod.rs` — 公開関数 `boolean_planar` → `boolean` rename、`validate_boolean_input` の gate 緩和
  - `crates/mycad-kernel/src/booleans/partition.rs` — `FaceFragment.plane: PlaneData` → `surface: Surface` 置換、`intersect_planes` 廃止 → `intersect_surfaces` 呼び出し、`pslg_subdivide` への曲線サンプル投入、UV unwrap helper
  - `crates/mycad-kernel/src/booleans/classify.rs` — `ray_intersect_surface` 拡張、曲面 face の point-in-face 判定 (UV polygon 経由)
  - `crates/mycad-kernel/src/booleans/assemble.rs` — intersection edge `Curve` 再構築 (partner surface pair → Line/Circle dispatch)、face `Surface` 転送 (fragment 由来そのまま)、`reverse_face_orientation` の Cylinder/Sphere 対応、`signed_volume` の曲面 face テッセレーション経由対応、`attach_pcurves_for_trimmed_faces` 補助関数
  - `crates/mycad-kernel/src/tessellation/mod.rs` — `tessellate_face_uv_grid` の trim path 分岐、`tessellate_face_sphere` の trim path 分岐、cylinder seam 跨ぎ補正 helper
  - `crates/mycad-build/src/lib.rs` — `boolean_planar` 呼び出し 3 箇所 → `boolean`
  - `crates/mycad-build/tests/feature_dispatcher.rs` — `t21_nonplanar_input_cylinder` の期待値反転、Acceptance test A1-A4 追加
  - `docs/decisions/004-freeform-geometry-commitment.md` — Decision 4 (曲面 Boolean MVP 範囲) を追記

- **変更する型・関数のシグネチャ**:

  ```rust
  // crates/mycad-kernel/src/geometry/surface_intersect.rs (新規)

  /// 1 本の 3D 交線と、両側 surface 上での pcurve 表現を束ねた構造。
  /// MVP の交線は全て Curve::Line か Curve::Circle で表現される。
  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct IntersectionLoop {
      pub curve_3d: Curve,           // Line or Circle
      pub t_range: [f64; 2],         // curve_3d のパラメータ範囲
      pub pcurve_on_a: Curve2D,      // surface_a の UV 上の表現 (Line2D or Circle2D)
      pub pcurve_on_a_t_range: [f64; 2],
      pub pcurve_on_b: Curve2D,
      pub pcurve_on_b_t_range: [f64; 2],
  }

  /// 2 つの surface の交線群を求める dispatcher。MVP は Plane×Plane / Plane×Cylinder /
  /// Plane×Sphere / Cylinder×Sphere の軸整列ケースのみ accept。
  /// その他のペアは `Err(UnsupportedSurfaceIntersection { reason })` を返す。
  pub fn intersect_surfaces(
      a: &Surface,
      b: &Surface,
      tol: Tolerance,
  ) -> Result<Vec<IntersectionLoop>, KernelError>;

  // 各ペア専用関数 (内部):
  fn intersect_plane_plane(a: &Surface, b: &Surface, tol: Tolerance) -> Result<Vec<IntersectionLoop>, KernelError>;
  fn intersect_plane_cylinder(plane: &Surface, cylinder: &Surface, tol: Tolerance) -> Result<Vec<IntersectionLoop>, KernelError>;
  fn intersect_plane_sphere(plane: &Surface, sphere: &Surface, tol: Tolerance) -> Result<Vec<IntersectionLoop>, KernelError>;
  fn intersect_cylinder_sphere(cylinder: &Surface, sphere: &Surface, tol: Tolerance) -> Result<Vec<IntersectionLoop>, KernelError>;
  ```

  ```rust
  // crates/mycad-kernel/src/error.rs (追加 variant)
  pub enum KernelError {
      // ...existing...
      UnsupportedSurfaceIntersection { reason: &'static str },
  }
  // 既存 NonPlanarBooleanInput は維持。Cone 等は引き続きこれで reject。
  ```

  ```rust
  // crates/mycad-kernel/src/booleans/partition.rs (変更)
  // PlaneData は廃止し、Surface を直接保持する。

  pub struct FaceFragment {
      pub source_face_index: usize,
      pub polygon_3d: Vec<Point>,
      pub surface: Surface,                       // ← plane: PlaneData から置換
      pub parent_name: EntityRef,
      pub traversal_index: u32,
      pub is_tool_side: bool,
      pub boundary_partners: Vec<Option<EntityRef>>,
  }

  // 各 face の 2D パラメータ空間へ落とす helper:
  // - Plane: (u, v) = (d·u_axis / |u_axis|², d·v_axis / |v_axis|²)
  // - Cylinder: (u, v) = (θ, axis 方向距離)  ※seam 跨ぎ補正は別途
  // - Sphere: (u, v) = (longitude, latitude)
  fn project_to_face_uv(surface: &Surface, p: &Point) -> (f64, f64);

  // Cylinder/Sphere の周期 u を outer_loop 内で連続化する helper:
  // 「直前点の u と今回点の u の差が π より大きければ ±2π 補正」して unwrap
  fn unwrap_periodic_uv(surface: &Surface, raw_uvs: &[(f64, f64)]) -> Vec<(f64, f64)>;
  ```

  ```rust
  // crates/mycad-kernel/src/booleans/mod.rs (変更)

  // rename: boolean_planar → boolean
  pub fn boolean(target: &Solid, tool: &Solid, op: BooleanOp) -> Result<Solid, KernelError>;

  // validate_boolean_input 緩和:
  // - `Plane` / `Cylinder` / `Sphere` のいずれかは accept
  // - Cone は引き続き NonPlanarBooleanInput で reject (Cone tessellation Unsupported のため)
  // - inner_loops 非空は **Cylinder/Sphere face では accept**、Plane face では引き続き reject
  ```

  ```rust
  // crates/mycad-kernel/src/booleans/classify.rs (変更)

  /// 光線が surface と交わる t 値 (origin + t*dir で衝突点)。
  /// Plane: 0 or 1 個, Cylinder: 0/1/2 個, Sphere: 0/1/2 個, Cone: 未対応 (panic 相当 unreachable)。
  fn ray_intersect_surface(origin: &Point, dir: &Vec3, surface: &Surface) -> Vec<f64>;

  /// 曲面 face の outer_loop / inner_loops に対する point-in-face 判定。
  /// 当該点を `surface.uv_of()` で UV へ落とし、unwrap 済み UV polygon の inside test。
  /// self-adjacent periodic face (outer_loop が seam edge 1 本のみ for sphere) は無条件 inside。
  fn point_in_curved_face(point: &Point, face: &Face, surface: &Surface, solid: &Solid) -> bool;
  ```

  ```rust
  // crates/mycad-kernel/src/booleans/assemble.rs (変更)

  // intersection edge の 3D Curve を partner surface pair から再構築:
  // - (Plane, Plane) → Curve::Line {origin, direction}
  // - (Plane, Cylinder) → Curve::Circle {center=Plane への projection, normal=plane.normal, radius=cylinder.radius}
  // - (Plane, Sphere) → Curve::Circle {center=sphere.center 上 plane.normal 方向, normal=±plane.normal, radius=sqrt(R²-d²)}
  // - (Cylinder, Sphere) → Curve::Circle {center=cylinder.axis 上, normal=cylinder.axis, radius=計算}
  fn reconstruct_intersection_curve(
      surface_a: &Surface,
      surface_b: &Surface,
      sample_points_3d: &[Point],  // PSLG 後の boundary edge 端点 2 つ (Line の場合) / 3 点 (Circle の場合)
  ) -> Result<Curve, KernelError>;

  // reverse_face_orientation の統一仕様 (R03+R01 採用: same_sense 単独エンコード):
  // 既存 Face.same_sense (brep/topology.rs:74「Whether the face normal matches the surface normal」)
  // を唯一の orientation エンコードとする。Surface variant の内部 field は一切ミューテートしない。
  // 全 Surface 共通の動作:
  //   1. outer_loop の HE 順を反転
  //   2. face.same_sense をトグル (true ↔ false)
  // これだけ。Plane / Cylinder / Sphere いずれも同じ手順。
  // - 既存実装が行っていた `Surface::Plane.normal/u_axis/v_axis` の符号反転は撤去 (round 3 R01)。
  //   これは「Plane だけ Surface flip + same_sense toggle で effective normal が二重反転して元に戻る」
  //   矛盾を解消するため。
  // - Cylinder の axis 反転も行わない (round 2 R03 で既決定: u=0 reference 破壊を避ける)。
  // - Sphere は元から Surface 不変。
  fn reverse_face_orientation(solid: &mut Solid, face_idx: usize, id_gen: &mut IdGenerator);

  // signed_volume の曲面対応:
  // 曲面 face は粗いテッセレーション (既存 tessellation::tessellate_face_* を tol=既定で呼ぶ) で
  // 三角形分割し、各三角形の signed tetrahedral volume を集計。
  fn signed_volume(solid: &Solid) -> f64;

  // 補助: trim された曲面 face の outer_loop / inner_loop に対し、各 HE に pcurve を後付けで生成。
  // - Cylinder face の HE: 円弧 edge (intersection由来) → Circle2D pcurve (v=const, u: 範囲)、
  //                       seam 直線 edge → Line2D pcurve
  // - Sphere face の HE: 円弧 edge → Line2D pcurve (v=const, u: 0..2π)、seam → Line2D pcurve
  fn attach_pcurves_for_trimmed_faces(solid: &mut Solid);
  ```

  ```rust
  // crates/mycad-kernel/src/tessellation/mod.rs (変更)

  // tessellate_face_uv_grid の冒頭で trim 判定 → 分岐:
  // - 全周 & inner_loop なし → 既存 UvGridFullPatch 経路
  // - それ以外 → tessellate_face_trimmed_uv (新規) へ
  fn tessellate_face_trimmed_uv(
      solid: &Solid,
      face_idx: usize,
      opts: &TessellationOptions,
      mesh: &mut TriangleMesh,
  ) -> Result<(), KernelError>;
  // 実装方針: collect_loop_points (pcurve あれば UV→3D) から outer/inner ループの UV 列を取得 →
  // cylinder の周方向 seam 跨ぎを unwrap_periodic_uv で補正 →
  // earcut_polygon で 2D 三角分割 → 各頂点を surface.evaluate(u,v) で 3D 復元、法線を normal_at(u,v) で計算。

  // tessellate_face_sphere も trim 分岐 (cylinder × sphere の lens 形状で必要):
  fn tessellate_face_sphere_trimmed(
      solid: &Solid,
      face_idx: usize,
      opts: &TessellationOptions,
      mesh: &mut TriangleMesh,
  ) -> Result<(), KernelError>;
  // 実装方針: outer/inner loop の UV を Sphere の (longitude, latitude) で取得 →
  // seam 跨ぎを unwrap →
  // UV polygon earcut → 3D 復元。
  ```

## 設計方針

### 1. 公開 API rename と入口 gate 緩和

- `boolean_planar(target, tool, op)` → `boolean(target, tool, op)` に rename。中で曲面ペア dispatch
- 呼び出し site 3 箇所 (`mycad-build/src/lib.rs:151,167,184`) を追従
- 既存テスト `t21_nonplanar_input_cylinder` (`feature_dispatcher.rs:1281`) は **「軸整列の Plane×Cylinder は accept、Cone を含む入力は引き続き reject、Cylinder × Cylinder は `UnsupportedSurfaceIntersection`」** を期待する形に書き換え
- `validate_boolean_input`:
  - `Plane` / `Cylinder` / `Sphere` のいずれの face も accept (Cone は引き続き `NonPlanarBooleanInput`)
  - **R01 採用: Plane face の inner_loops 受入** — 全 surface 種別で **1 階層の inner_loops は accept** とする。理由: A1 の Cut 結果は box 上面 plane face に 1 個の inner_loop (cylinder の blind hole 円形開口) を持ち、その出力を次の Boolean に再投入できない (`Feature history = source of truth` 違反) のを防ぐため。reject するのは: (a) 2 階層以上の nested inner_loops、(b) inner_loop が outer_loop と接触/交差している退化形状、(c) inner_loop が空 (頂点なし)、のみ
  - `is_convex_polygon_3d` の凸チェックは **撤廃** — 穴付き Plane face の outer_loop は凸でも穴があれば「凸 face」とは言えないため意味を失う。代わりに「outer_loop が単純多角形 (自己交差なし) かつ全頂点が同一平面上にある」のみを Plane face の妥当性条件とする (`is_simple_planar_polygon_3d` 相当の helper を新設)
  - 曲面 face では polygon が 3D で凸でない可能性があるため、外形検証は UV 上での「simple polygon (自己交差なし)」チェックに置き換える

### 2. 曲面交線モジュール `geometry/surface_intersect.rs`

- ディスパッチ表（`(a, b)` 順序は呼び出し側が制御、対称ペアは `(b, a)` で自動 swap）:

  | ペア | 受入条件 | 出力 (交線あり) | tangent / 接触ケース (R03) | 不適合時 (R03) |
  |------|----------|----------------|---------------------------|----------------|
  | Plane × Plane | 常時 | 1 個の `Curve::Line` | 同一平面 (coplanar) は本表の対象外、既存 coplanar_pairs パスで処理 | distinct でなく非 coplanar → 空 Vec |
  | Plane × Cylinder | `|axis·normal|.abs() >= 1.0 - ANGLE_TOLERANCE` (軸 ⊥ plane) かつ plane と cylinder.axis が交わる | 1 個の `Curve::Circle` (normal=plane.normal, radius=cylinder.radius, center=cylinder.axis と plane の交点) | N/A (軸 ⊥ かつ交わる場合 tangent は radius==0 のみ、退化として Err) | 軸非整列 (∥ 含む全中間角度) → `UnsupportedSurfaceIntersection { reason: "non-perpendicular plane × cylinder" }` |
  | Plane × Sphere | `plane.normal` が `(0,0,±1)` (±Z 軸) かつ `|d| < radius - LENGTH_TOLERANCE` (distance d = (sphere.center - plane.origin)·plane.normal) | 1 個の `Curve::Circle` (radius=√(R²-d²), center=sphere.center - d·n, normal=plane.normal) | `|d| > radius - LENGTH_TOLERANCE` (含む `|d|>=radius` tangent / 離れている) → **空 Vec** | plane.normal が ±Z でない → `UnsupportedSurfaceIntersection { reason: "non-Z-aligned plane × sphere" }` |
  | Cylinder × Sphere | sphere.center が cylinder.axis 上 (`(sphere.center - cylinder.origin).cross(axis).norm() < LENGTH_TOLERANCE`) かつ cylinder.axis が ±Z 軸 | 球の中心軸上 cap までの軸方向距離 h_max を計算し、`-h_max < h < h_max - LENGTH_TOLERANCE` 内の h で 2 個の `Curve::Circle` (上下)、`< -h_max + LENGTH_TOLERANCE` または `> h_max - LENGTH_TOLERANCE` (cylinder が球を貫通せず) → 空 Vec | tangent (`|h² + cyl_r² - sph_r²| < LENGTH_TOLERANCE`) → **空 Vec** | 同軸条件不成立 → `UnsupportedSurfaceIntersection { reason: "non-coaxial cylinder × sphere" }` |

  **R03 採用: tangent と unsupported の統一仕様** — 「accept しても交線が物理的に消える tangent / 離れている」ケースは全て **`Ok(空 Vec)`** で扱う。「surface 種別の組合せ or 姿勢が MVP の数式アルゴリズムで処理できない」ケースは全て **`Err(UnsupportedSurfaceIntersection { reason })`** で reject。Plane × Cylinder の `|axis·normal|.abs() < 1.0 - ANGLE_TOLERANCE` (軸非整列、∥ 含む) は全て後者。Cylinder × Sphere の tangent は前者。

- 各ペアごとに **pcurve 表現** も同時に返す:
  - Plane × Plane: 両側 `Curve2D::Line2D` (各 plane の uv_axis で direction を投影)
  - Plane × Cylinder: Plane 側 `Circle2D` (center=plane への projection、radius=cylinder.radius)、Cylinder 側 `Line2D` (v=軸方向 const、direction=(1,0) で u: 0..2π)
  - Plane × Sphere: Plane 側 `Circle2D`、Sphere 側 `Line2D` (v=latitude const、direction=(1,0) で u: 0..2π)
  - Cylinder × Sphere: Cylinder 側 `Line2D` (v=const)、Sphere 側 `Line2D` (v=const)

### 3. `PlaneData` の置換と `FaceFragment` の `Surface` 化

- `FaceFragment.plane: PlaneData` → `surface: Surface` に置き換え
- 2D PSLG への投影 helper `project_to_face_uv(surface, p)` を新設:
  - Plane: 既存 `PlaneData::project_2d` 相当 (u_axis/v_axis 内積)
  - Cylinder: `surface.uv_of(p)` → (θ, axis 方向距離) を返す
  - Sphere: `surface.uv_of(p)` → (longitude, latitude)
- 3D 復元 helper: `surface.evaluate(u, v)` を呼ぶだけ
- `partition_faces` の `target_polygons_2d` / `tool_polygons_2d` 構築箇所 (`partition.rs:97-115`) を `project_to_face_uv` 経由に変更
- Cylinder face の outer_loop の u (=θ) は **seam 跨ぎ**で 2π wrap する可能性があるため、`unwrap_periodic_uv` で連続化してから PSLG に投入

### 4. `intersect_planes` の廃止と `pslg_subdivide` への曲線サンプル投入

- `partition.rs:495-527` の `intersect_planes` は削除。代わりに `geometry::surface_intersect::intersect_surfaces(&target.surface, &tool.surface, Tolerance::DEFAULT)` を呼ぶ
- 戻り値の `Vec<IntersectionLoop>` から PSLG 入力を作る:
  - `Curve::Line` 交線 → 1 本の直線セグメントとして `IntersectionSegment` を 1 個 push
  - `Curve::Circle` 交線 → 円弧を `opts.angular_segments` で N 等分し、各サブセグメントを `IntersectionSegment` として push (partner は同じ tool face 名を共有)
- `pslg_subdivide` は既存ロジック (直線セグメント前提) のまま使い回す。曲線は事前にサンプル化済みの直線セグメント列として渡る
- assemble 時に「**この PSLG sub-face boundary edge 列のうち、partner が同じで連続する区間は元々 1 本の曲線**」と識別し、`reconstruct_intersection_curve` で `Curve::Line` か `Curve::Circle` を解析的に組み立てる

**R01 採用: 解析的トポロジー確定契約**

最終 B-rep のトポロジー (V/E/F カウント、`IdGenerator` を回す対象、`derive_edge_name` の selector) は **`angular_segments` に依存しないこと**。これを担保する具体プロトコル:

1. **PSLG 内のサンプル点はローカル一時データ**:
   - `pslg_subdivide` 内で生成される中間サンプル頂点 (円弧を `angular_segments` で N 等分した点) はあくまで「2D 内で交差を計算するための足場」であり、`IdGenerator` を消費しない
   - PSLG 出力 (`Vec<(Vec<(f64,f64)>, Vec<Option<EntityRef>>)>`) はこのローカル ID 空間のままで返る
2. **assemble 段階で連続区間を 1 本の解析 edge に縮退**:
   - assemble は PSLG sub-face の boundary edge 列を走査し、**「partner provenance が同じ EntityRef で連続する区間」を 1 つのグループとして識別**
   - グループの両端点 (始端と終端) のみを Solid の最終 Vertex として `IdGenerator` で確定
   - グループ内の中間サンプル頂点は **捨てる** (Solid の Vertex 配列に入らない)
   - グループ全体を 1 本の `Edge` として確定し、`Edge.curve` は `reconstruct_intersection_curve` で **解析曲線 (`Curve::Line` または `Curve::Circle`)** を組み立てる
3. **Full circle 交線の periodic face 上での seam vertex 規約 (R02 採用)**:

   Plane × Cylinder で box の上面 (z=+5) と cylinder lateral が全周交差するケース (A1 blind hole の上面 inner_loop)、および Cylinder × Sphere で sphere を cylinder が貫通するケース (A2、上下 2 個の交線)、で Cylinder/Sphere lateral face 上に **full-circle 交線** が現れる。これは閉じた自己ループ edge だが、B-rep の HalfEdge ループは「両端に vertex を持つ edge の列」として表現する必要があるため、seam で必ず 1 個の vertex を打って split する。

   - **新規 seam vertex を必ず挿入する** (既存 seam edge 端点の流用は不可):
     - Cylinder lateral の既存 seam edge は (R, 0, z_min) ↔ (R, 0, z_max) の縦直線。一方、交線は z=h の水平円であり、両者は (R, 0, h) で交わる。既存端点 (z_min/z_max) は z=h と一致しないため流用できない
     - Sphere の既存 seam edge は (R cos0, R sin0, R sinv) の meridian (v: -π/2 → π/2)。交線は v=v_h の水平円で、(R, 0, R sin(v_h)) で交わる。同様に既存端点を流用できない
   - **seam vertex 挿入の決定論的規約**:
     - Cylinder: 交線円の平面と seam line (u=0, v 軸方向) の交点を `surface.evaluate(0.0, h)` で計算し、新規 `Vertex` を `IdGenerator` で確定
     - Sphere: 交線円の平面と seam meridian の交点を `surface.evaluate(0.0, v_h)` で計算し、新規 `Vertex` を `IdGenerator` で確定
     - これらの seam vertex の name は `EntityRef::Derived { op: "<op>_isect_seam_vertex", parents: [periodic_face_name, intersection_loop_index], selector: 0 }` で命名
   - **既存 seam edge の split**:
     - 既存 seam edge `E_seam = (V_start, V_end)` を、新規 seam vertex `V_seam` で 2 本に split: `E_seam_lower = (V_start, V_seam)`, `E_seam_upper = (V_seam, V_end)`
     - 元の seam edge を共有していた自己隣接周期面 (cylinder/sphere lateral) の outer_loop HE 列も、対応する 2 HE 対に split
     - split された 2 本の edge name は `derive_edge_name` で `selector: "lower"` / `"upper"` を付けて確定
   - **Full-circle intersection edge の HE 接続**:
     - 交線 edge `E_isect = (V_seam, V_seam)` は両端が同じ vertex の **自己ループ Circle edge** として確定
     - その HE は periodic face の outer_loop に 1 個挿入される (full-circle なら outer_loop は 1 HE のみで構成され、self-adjacent periodic face と同型になる)
     - 反対側 face (例えば Plane face) では、この交線 edge は inner_loop として現れる (full-circle なので 1 HE のみで閉じる inner_loop)
   - **同一規約を cylinder と sphere の両方に適用**: 上記手順は cylinder lateral / sphere いずれの periodic face でも同形。`split_periodic_seam_for_intersection(solid, periodic_face_idx, intersection_h_or_v) -> (new_vertex_idx, [lower_edge_idx, upper_edge_idx])` を共通 helper として実装
4. **`assign_intersection_edge_selectors` (#31) の入力は最終 edge 集合のみ**:
   - 解析的に確定した edge の `normalize_edge_key` で sort → ordinal 付与
   - 中間サンプル頂点が消えているため、`angular_segments` を変えても selector 順は不変
5. **検証**: T22b (新規) — A1 / A2 を `opts.angular_segments = 8` と `opts.angular_segments = 64` の 2 通りで build し、`assert_solids_equal_with_names` で **V/E/F カウントと全 entity name が完全一致** することを確認

これにより「サンプル密度を変えても B-rep のトポロジーと派生名が決定的に同一」が保証される。

### 5. `classify.rs` の ray-surface 拡張

- `ray_intersect_surface(origin, dir, surface)`:
  - Plane: 既存 `(plane.origin - origin)·n / dir·n` を 1 個返す
  - Cylinder: 2 次方程式 `|d_perp + t * dir_perp|² = R²` を解く (dir, d を axis 直交成分に射影)
  - Sphere: `|origin + t*dir - center|² = R²` を解く
  - Cone: `unreachable!()` (Cone は `validate_boolean_input` で reject 済み)
- `point_in_polyhedron` (`classify.rs:147-207`) の改修:
  - 各 face について `ray_intersect_surface` で 0/1/2 個の t を取得
  - 各 t について衝突点 `p_hit = origin + t*dir` を `point_in_curved_face(p_hit, face, &face.surface, solid)` でチェック
  - inside 判定された衝突をカウント。majority-vote (3 軸 ray キャスト) は現状維持
- `point_in_curved_face`:
  - 当該点を `surface.uv_of()` で UV へ落とす (これを `(u_raw, v_raw)` とする)
  - face の outer_loop / inner_loops の 3D 点列を同様に UV へ落とし、`unwrap_periodic_uv` で seam 跨ぎを連続化 → 各 loop の u 値は `[u_min_loop, u_max_loop]` の範囲を持つ
  - **R02 採用: 判定点 UV の枝補正** — 判定点の `u_raw` を loop の u 範囲に合わせて `±2π` シフトする:
    - `align_u_to_loop_range(u_raw, u_min_loop, u_max_loop) -> f64` helper を新設
    - 仕様: `u_aligned = u_raw + 2π·k` で `u_min_loop - π <= u_aligned <= u_max_loop + π` となる整数 k を選ぶ (loop の中央寄りに引き付ける)
    - これにより、判定点の hit 座標が seam 直近 (`u_raw ≈ 0`) で loop が `u ≈ 2π..4π` に unwrap されていても、`u_aligned ≈ 2π` に補正されて正しく inside test できる
    - v は周期的でないため補正不要 (Cylinder の v、Sphere の v ともに)
  - UV 多角形に対する 2D odd-even rule (`point_in_polygon_2d`) で `(u_aligned, v_raw)` を判定
  - 例外: outer_loop が seam edge 1 本のみで full surface を覆う self-adjacent periodic face (canonical sphere primitive 等) は無条件 `true`
  - **R02 検証**: T13c (新規) — Cylinder の seam edge (`u=0` 付近) のすぐ外側にある hit 点 (`u_raw=0.001`) を、loop が `u: 2π..4π` に unwrap された trim face に対して inside 判定。`align_u_to_loop_range` で `u_aligned ≈ 2π.001` に補正されることを確認 (補正前は誤って outside と判定される)
- `polygons_have_2d_overlap` (`classify.rs:264-307`) は Plane×Plane の coplanar 検出にしか使わないため **そのまま維持** (曲面 coplanar は Non-Goal)

### 6. `assemble.rs` の edge curve / face surface dispatch

- intersection edge 構築 (`assemble.rs:173-187`):
  - PSLG sub-face boundary edge の `boundary_partners[i]` が `Some` の edge を抽出
  - 同じ partner provenance を持つ連続区間ごとに「partner pair → 元の Surface pair」を逆引き (FaceFragment.surface と partner_face.surface から)
  - `reconstruct_intersection_curve(surface_a, surface_b, sample_points_3d)` で 3D Curve を再構築
    - Plane × Plane → `Curve::Line { origin=sample_points_3d[0], direction=normalize(p1-p0) }`
    - Plane × Cylinder → `Curve::Circle { center=cylinder.axis と plane の交点, normal=plane.normal, radius=cylinder.radius }`
    - Plane × Sphere → `Curve::Circle { center=sphere.center - d*n, normal=plane.normal, radius=√(R²-d²) }`
    - Cylinder × Sphere → `Curve::Circle { center=cylinder.axis 上 (軸方向 h offset), normal=cylinder.axis, radius=√(R_sphere²-h²) }`
- intersection edge の `derive_edge_name` 呼び出しは #31 で実装済み (`canonicalize_provenance` + `assign_intersection_edge_selectors`) → そのまま使い回す
- face の Surface 構築 (`assemble.rs:250-269`):
  - `FaceFragment.surface` を**そのまま転送**。`Surface::Plane { ... }` で再構築する既存コードを撤廃
- `reverse_face_orientation` (R03 round 2 + R01 round 3 採用: `Face.same_sense` 単独エンコードに統一):
  - **全 Surface 共通の動作**: HE 順反転 + `face.same_sense = !face.same_sense` のみ
  - **Surface variant の内部 field は一切ミューテートしない** — Plane.normal/u_axis/v_axis、Cylinder.axis、Sphere の center/radius、すべて不変
  - 既存実装 (`assemble.rs:393-395`) の `Surface::Plane.normal` 反転は **撤去**。これは round 3 R01 で指摘された「same_sense トグル + Surface flip の二重反転で effective normal が元に戻る」矛盾の解消
  - Cone → `unreachable!` (reject 済み)
- **R03 採用: 全 normal 算出経路で `same_sense` を尊重 (唯一のソース)**:
  - **face-effective outward normal の正式定義**: `if face.same_sense { surface.normal_at(u,v) } else { -surface.normal_at(u,v) }`
  - tessellation 側: `tessellate_face_*` 群と `tessellate_face_trimmed_uv` / `tessellate_face_sphere_trimmed` の全経路で、頂点法線算出時に `face.same_sense` を読んで条件付き符号反転を行う (既存 `mod.rs:170-174,303,435,591,606,615,629,645,660,670-672` の同様パターンを新規 trim 経路にも同じ形で適用)
  - `classify.rs::point_in_curved_face` は inside test 自体には normal を使わないため影響なし。`ray_intersect_surface` の hit カウントは effective normal の向きに依存しないため修正不要
  - `signed_volume` (`assemble.rs:468-521`): **撤去対象を伴う修正** — 既存実装は Plane face の `Surface::Plane.normal` を `same_sense` チェック無しに直読みしている (`assemble.rs:477`)。Plane.normal を flip しなくする本修正と整合させるため、signed_volume も `if face.same_sense { surface.normal_at } else { -surface.normal_at }` を経由する形に変更。さらに曲面 face はテッセレーション経由で各三角形の signed tetrahedral volume を集計 (元の plan 通り)、三角形頂点順は `face.same_sense=false` のときは反転して出力する
  - `validate_manifold` (#31 で導入) の pcurve consistency check は端点+中点座標一致を見るだけで normal 方向は問わないため影響なし
- **R03 検証**: T28 を「A3 の内側 sphere face は `same_sense=false`、`Surface::Sphere` の center/radius/極軸は外側 reference sphere と完全同一 (Surface 不変が新仕様)、tessellation の triangle normals は **face-effective outward** (sphere 中心から外側、void 側へ向く) を assert」に書き換え。同じ test 内で A1 の内側 cylinder lateral face も「`same_sense=false`、`Surface::Cylinder.axis` 不変、tessellation triangle normal は cylinder 中心軸から放射方向に外向き (=void 側)」を assert
- **R01 検証 (round 3)**: T28d (新規) — A3 の void shell sphere face と外側参照 sphere の `face.surface` 同士を `assert_eq!` で完全一致確認 (Surface flip が無いことの直接 assert)。A1 の void shell cylinder lateral face も同様に外側参照 cylinder と Surface 一致 assert
- `signed_volume` (`assemble.rs:468-521`):
  - 曲面 face を「`Surface::Plane` 以外を skip」していたロジックを撤廃
  - 全 face をテッセレーション (`tessellation::tessellate_solid_with_opts` の内部呼び出しを参照、または同じ三角分割ロジックを `for_volume_only=true` flag で呼ぶ) し、各三角形の signed tetrahedral volume `(p1·(p2×p3))/6` を集計
- `attach_pcurves_for_trimmed_faces(solid)`:
  - assemble 最終段で呼ぶ
  - 各 face を走査し、Cylinder/Sphere face の outer_loop / inner_loops の HE について:
    - intersection 由来の HE (edge.curve が `Curve::Circle` で partner が surface 直交 plane など) → 当該 surface の UV 上の表現に変換し `Pcurve` を組み立てる
    - seam edge → Line2D pcurve
  - これにより tessellation の `collect_loop_points` (`mod.rs:319-362`) が pcurve 経路で UV→3D サンプルを正確に行える

### 7. テッセレーション trim 対応

- `tessellate_face_uv_grid` (`mod.rs:365-460`) の冒頭で **trim 判定** を追加:
  - `inner_loops` が空かつ outer_loop の Circle edge `t_range` 合計が 2π の整数倍 → 既存 UvGridFullPatch 経路
  - それ以外 → `tessellate_face_trimmed_uv` 新規関数へ分岐
- `tessellate_face_trimmed_uv`:
  - `collect_loop_points` (pcurve 経由) で outer_loop と inner_loops の UV 列を取得
  - cylinder の場合 `unwrap_periodic_uv` で seam 跨ぎ補正
  - earcut で 2D 三角分割 (`earcut` crate は既に依存にある: `mod.rs:286`)
  - 各 UV 頂点を `surface.evaluate(u,v)` で 3D 復元、法線を `surface.normal_at(u,v)` で計算
- `tessellate_face_sphere` (`mod.rs:467-668`) も同様に trim 判定 + `tessellate_face_sphere_trimmed` 新規分岐
- 既存「全周必須」テスト 3 件 (`test_partial_revolution_cylinder_face_errors` mod.rs:979, `test_t06_full_revolution_rejects_at_angle_tolerance` mod.rs:1537, `test_non_canonical_sphere_face` mod.rs:1166) は **STEP F で同時に書き換え**: trim でも `Ok` を期待する形に

### 8. 決定性要件

- `intersect_surfaces` の Vec 順序は dispatcher で固定 (Plane → Cylinder → Sphere の順、対称ペアは入力順をそのまま使う)
- `reconstruct_intersection_curve` の `sample_points_3d` は PSLG 内部の boundary 列順 (Vec iteration 順) で渡す
- `attach_pcurves_for_trimmed_faces` は `solid.faces` iteration 順、各 face 内は HE Vec 順
- `signed_volume` のテッセレーション再呼び出しは決定的 (既存 `tessellate_solid` は決定的)
- 既存 `assign_intersection_edge_selectors` (#31) はそのまま流用 — `normalize_edge_key` 昇順 sort で deterministic ordinal
- ray キャストの方向選択は ±X/±Y/±Z 固定順 (majority-vote)
- HashMap/HashSet は iteration 禁止、sort 経由のみ

**R04 採用: 複数 IntersectionLoop と Circle の正準順序規約**

`intersect_surfaces` が複数 `IntersectionLoop` を返すペアで、loop 順・各 Circle の `normal` 向き・`t_range` 方向を実装非依存に固定する。これにより同一入力でも edge selector・curve パラメータ・entity name が決定的に同一になる。

1. **複数 loop の sort key**:
   - `Cylinder × Sphere` で 2 個の Circle を返す場合: **circle.center を cylinder.axis 方向に射影した値 (h = (center - cylinder.origin)·axis) 昇順で sort**。h が等しい場合は radius 昇順、それも等しい場合は実装順 (起こり得ないが安全弁)
   - 他のペアは現状 1 個以下しか返さないため sort 不要
   - `Vec<IntersectionLoop>` の長さは MVP 範囲では Plane×Plane=0 or 1, Plane×Cyl/Sph=0 or 1, Cyl×Sph=0/1/2 のいずれか
2. **Circle::normal の正準方向**:
   - `Plane × Cylinder` の Circle: `normal = plane.normal` (plane 側 surface の normal をそのまま採用、`unit_z` か否かは plane.normal が決める)
   - `Plane × Sphere` の Circle: `normal = plane.normal`
   - `Cylinder × Sphere` の Circle: `normal = cylinder.axis` (`axis` を正規化したもの。`-axis` は採用しない)
3. **Circle::t_range の正準方向**:
   - 全 Circle 交線で `t_range = [0.0, 2π]` 固定 (full circle のみ MVP で扱うため)。`reverse` した `[2π, 0.0]` は使わない
   - 部分円弧 (trim された circle、box の側面で cylinder が部分的にしか貫通しない etc.) は MVP では発生しない (Plane × Cylinder の軸 ⊥ 受入条件で plane.normal が cylinder.axis と平行 ⇒ 平面と cylinder lateral の交線は必ず full circle)
4. **Pcurve の t_range も同様に正準化**:
   - Plane 側 `Curve2D::Circle2D` → `t_range = [0.0, 2π]`、`center` は plane uv 上の (u,v) で射影、`radius` は cylinder/sphere の半径から計算
   - Cylinder/Sphere 側 `Curve2D::Line2D` → `t_range = [0.0, 2π]`、`origin = (0.0, v_const)`、`direction = (1.0, 0.0)` (u 軸正方向で 0→2π を 1:1 マップ)
5. **検証**: T34 (新規) — A2 (cyl ∩ sph) を 2 回 build し、返ってきた 2 個の Circle の `center.z` が昇順 (下側が `IntersectionLoop[0]`、上側が `IntersectionLoop[1]`)、両 Circle の `normal = (0,0,1)` (axis=+Z 固定)、両 `t_range = [0, 2π]` を直接 assert

### 9. 退化幾何の扱い (R03 で §2 と統一)

**Ok(空 Vec) で返すケース** (tangent / 離れ / 平行非交差):
- `intersect_plane_sphere` で `|d| > radius - LENGTH_TOLERANCE` (含む `|d| >= radius` の tangent / 離れ) → 空 Vec
- `intersect_cylinder_sphere` で tangent (`|h² + cyl_r² - sph_r²| < LENGTH_TOLERANCE`) または cylinder が球を貫通せず → 空 Vec
- `ray_intersect_surface` の Cylinder 判別式 `< LENGTH_TOLERANCE²` → 0 個 (tangent は count しない)
- `ray_intersect_surface` の Sphere 判別式 同様 → 0 個

**Err(UnsupportedSurfaceIntersection { reason }) で reject するケース** (姿勢が MVP の数式アルゴリズムで処理できない):
- `intersect_plane_cylinder` で `|axis·normal|.abs() < 1.0 - ANGLE_TOLERANCE` (軸 ⊥ plane でない、∥ 含む全中間角度) → reason: `"non-perpendicular plane × cylinder"`
- `intersect_plane_sphere` で plane.normal が ±Z 軸でない (`|normal.z|.abs() < 1.0 - ANGLE_TOLERANCE`) → reason: `"non-Z-aligned plane × sphere"`
- `intersect_cylinder_sphere` で sphere.center が cylinder.axis 上にない、または cylinder.axis が ±Z 軸でない → reason: `"non-coaxial cylinder × sphere"`
- それ以外のサポート外曲面ペア (Cylinder × Cylinder 任意配置、Cone を含む全て) → 各 dispatcher で reason 付き

**既存 NonPlanarBooleanInput で reject するケース**:
- `validate_boolean_input` で Cone を含む入力 (gate 緩和外、Cone tessellation が Unsupported のため Boolean には載せない)

**KernelError の使い分け**: 「surface 種別 (Cone) が tessellation 含めサポート外」→ `NonPlanarBooleanInput`、「surface 種別はサポート対象だが姿勢が MVP 範囲外」→ `UnsupportedSurfaceIntersection`。エラーメッセージで区別可能

### 10. derive 規約

| 型 | derive |
|---|---|
| `IntersectionLoop` | `Debug, Clone, PartialEq, Serialize, Deserialize` (CLAUDE.md「公開型には `Debug, Clone, Serialize, Deserialize` を付与」規約に従う。round 3 R04 採用) |
| `KernelError::UnsupportedSurfaceIntersection` | `thiserror` の既存パターンに従う |

### 11. workspace.dependencies

新規依存は **無し**。既存 crates (`nalgebra`, `serde`, `thiserror`, `earcut` 等) で実装可能。

## テスト計画 (ID 付き)

| ID  | 種別     | 内容                                                                                                  | 期待結果                                                              |
|-----|----------|-------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------|
| **A1** | 統合 (Acceptance) | box (10×10×10 中心原点、z=-5..+5) から `Feature::CreateCylinder { radius=2, height=12 }` (現行 primitive 仕様で bottom が z=0 固定、top が z=12) で Cut。`Feature` レベルで Translate が無いため**実体は blind hole** (上面のみ貫通) | 結果が manifold、体積 ≈ 10³ - π·4·5 = 1000 - 20π ≈ 937.17。**box 上面 (z=+5) のみに inner_loop (円) が現れる** (下面 z=-5 は不変)、内側に新規面 2 個 (cylinder lateral trimmed v∈[0,5] + cylinder bottom disk at z=0 が穴の底)、**単一 shell・genus 0** |
| **A2** | 統合 (Acceptance) | center=(0,0,0) radius=5 sphere ∩ axis=+Z origin=(0,0,-10) radius=3 height=20 cylinder の Intersect | 結果が manifold、上下に 2 個の交線 Circle (radius=√(25-h²) for h=cyl height end)、sphere face と cylinder lateral face が trim される |
| **A3** | 統合 (Acceptance) | box (10×10×10) と中心 (5,5,5) radius=2 sphere の Cut (sphere が完全に box 内) | 結果が manifold、box face は変化なし、sphere が反転して inner shell として保持される (Cut の void shell ケース) |
| **A4a** | エッジ (reject) | box と axis=(1,1,0) (斜め) の cylinder の Cut | `Err(UnsupportedSurfaceIntersection { reason: "non-perpendicular plane × cylinder" })` (R03 round 3 採用: T04/T04b と統一) |
| **A4b** | エッジ (reject) | sphere center=(1,0,0) radius=2 と cylinder axis=+Z origin=(0,0,0) radius=1 (同軸でない) の Intersect | `Err(UnsupportedSurfaceIntersection { reason: "non-coaxial cylinder × sphere" })` |
| **A4c** | エッジ (reject) | 任意配置の cylinder × cylinder Cut | `Err(UnsupportedSurfaceIntersection { reason })` |
| **A4d** | エッジ (reject) | Cone を含む入力 | `Err(NonPlanarBooleanInput)` (既存 gate、変更なし) |
| T01 | 正常系 | `intersect_surfaces(Plane{+Z, z=0}, Cylinder{+Z, R=2})` (軸 ⊥ plane) | 1 個の IntersectionLoop、curve_3d は Circle (center=(0,0,0), normal=+Z, radius=2)、pcurve_on_a は Circle2D、pcurve_on_b は Line2D (v=0, u: 0→2π) |
| T02 | 正常系 | `intersect_surfaces(Plane{+Z, z=3}, Sphere{center=(0,0,0), R=5})` (plane normal が +Z) | 1 個の IntersectionLoop、curve_3d は Circle (radius=4)、pcurve_on_sphere は Line2D (v=arcsin(3/5)=0.6435..., u: 0→2π) |
| T03 | 正常系 | `intersect_surfaces(Cylinder{+Z, R=3, origin=(0,0,-10)}, Sphere{center=(0,0,0), R=5})` (同軸) | 2 個の IntersectionLoop、curve_3d は両方 Circle (radius=3, h=±4)、pcurve_on_both は Line2D (v=const) |
| T04 | エッジ | `intersect_surfaces(Plane{+X, x=0}, Cylinder{+Z, R=2})` (axis ∥ plane、R03 統一仕様) | `Err(UnsupportedSurfaceIntersection { reason: "non-perpendicular plane × cylinder" })` |
| T04b | エッジ | `intersect_surfaces(Plane{normal=(1,1,0)/√2}, Cylinder{+Z, R=2})` (中間角度) | `Err(UnsupportedSurfaceIntersection { reason: "non-perpendicular plane × cylinder" })` |
| T05 | エッジ | `intersect_surfaces(Plane{+X, x=0}, Sphere{center=(0,0,0), R=5})` (plane normal が +X、Z以外) | `Err(UnsupportedSurfaceIntersection { reason: "non-Z-aligned plane × sphere" })` |
| T06 | エッジ | `intersect_surfaces(Cylinder{+Z, R=2}, Sphere{center=(1,0,0), R=2})` (sphere.center が axis 外) | `Err(UnsupportedSurfaceIntersection { reason: "non-coaxial cylinder × sphere" })` |
| T07 | 退化 | `intersect_surfaces(Plane{+Z, z=10}, Sphere{center=(0,0,0), R=5})` (distance > R、交わらず) | Ok(空 Vec) |
| T08 | 退化 | `intersect_surfaces(Plane{+Z, z=5}, Sphere{center=(0,0,0), R=5})` (tangent) | Ok(空 Vec) — MVP では tangent は無視 |
| T09 | 正常系 | `ray_intersect_surface(origin=(0,0,5), dir=-Z, Sphere{center=(0,0,0), R=2})` | `vec![3.0, 7.0]` (Z=2 と Z=-2 を貫通) |
| T10 | 正常系 | `ray_intersect_surface(origin=(5,0,0), dir=-X, Cylinder{axis=+Z, R=2, origin=(0,0,0)})` | `vec![3.0, 7.0]` (X=2 と X=-2 を貫通) |
| T11 | エッジ | `ray_intersect_surface` で tangent (判別式 < LENGTH_TOLERANCE²) | 空 Vec (count しない) |
| T12 | 統合 (`point_in_curved_face`) | full sphere primitive の self-adjacent periodic face に対し任意点 | 常に true |
| T13 | 統合 (`point_in_curved_face`) | trim された cylinder face (outer_loop = 半円弧 + 2 line edges) で、UV polygon の内側にある点 | true |
| T13c | 統合 R02 (`point_in_curved_face`) | Cylinder の seam (`u=0` 付近) 直近 hit 点 (`u_raw=0.001`) を loop が `u: 2π..4π` に unwrap された trim face に対して inside 判定 | true (補正後 `u_aligned ≈ 2π.001`、補正前は誤って false になる回帰テスト) |
| T14 | 統合 (`point_in_curved_face`) | 同上で UV polygon の外側にある点 | false |
| T15 | 正常系 (rename) | `boolean(box, box, Cut)` 既存 t05 と同等の入出力 | 既存 expected と完全一致 |
| T16 | 既存反転 | `t21_nonplanar_input_cylinder` を「軸整列 Plane × Cylinder で Ok、Cone で Err」と書き換え | Plane×Cylinder で Ok、Cone face を含む solid で `Err(NonPlanarBooleanInput)` |
| T17 | 統合 | A1 の Cut 結果から `tessellate_solid` → mesh の triangle 数が finite で > 0、決定的 | 2 回 build → 完全一致 |
| T18 | 統合 | A2 の Intersect 結果から `tessellate_solid` → mesh 検証 | 同上 |
| T19 | 統合 | A1 の結果に対し `validate_manifold()` | `Ok(())` |
| T20 | 統合 | A2 の結果に対し `validate_manifold()` | `Ok(())` |
| T21 | 統合 (pcurve 整合性) | A1 の cylinder 内側 face の intersection HE は pcurve を持ち、`validate_manifold` の pcurve consistency check (端点+中点) を通過 | `Ok(())` |
| T22 | 決定性 | A1 / A2 / A3 を 2 回 build し `assert_solids_equal_with_names` で完全一致 | 全 entity name 同一 |
| T22b | 決定性 R01 | A1 / A2 を `opts.angular_segments = 8` と `= 64` の 2 通りで build し `assert_solids_equal_with_names` | V/E/F カウントと全 entity name が完全一致 (サンプル密度に依存しない) |
| T23 | golden | A1 の YAML golden を `examples/boolean_box_cylinder_cut.mycad` に追加 → build → YAML round-trip → byte-identical | byte-identical |
| T24 | trim tessellation 反転 | `test_partial_revolution_cylinder_face_errors` を Ok 期待に反転 | Ok |
| T25 | trim tessellation 反転 | `test_non_canonical_sphere_face` を Ok 期待に反転 | Ok |
| T26 | trim tessellation 反転 | `test_t06_full_revolution_rejects_at_angle_tolerance` を Ok 期待に反転 | Ok |
| T27 | 統合 (signed_volume) | A1 の Cut 結果の `signed_volume()` | `≈ 10³ - π·4·5 = 1000 - 20π ≈ 937.17` (相対誤差 < 1%、テッセレーション精度に依存) |
| T28 | 統合 R03 (reverse_face_orientation) | A3 の Cut で生成された内側 sphere face は `face.same_sense == false`、`Surface::Sphere` の center/radius は外側 reference と同一、tessellation の各 triangle normal が **球中心から外側 (=void shell の内側)** を向く。さらに A1 の内側 cylinder lateral face は `same_sense == false`、`Surface::Cylinder.axis` は不変、tessellation triangle normal が cylinder 中心軸から外側を向く | normal の向きを直接 assert |
| **T34** | 決定性 R04 (Cyl×Sph 正準順序) | A2 (cyl ∩ sph Intersect) から `intersect_surfaces` の返り値を直接受け取り: `loops[0].curve_3d.center.z < loops[1].curve_3d.center.z`、両 Circle の `normal == (0,0,1)`、両 `t_range == [0.0, 2π]` を assert | 等式成立 |
| T29 | doc | `docs/decisions/004-freeform-geometry-commitment.md` に「曲面 Boolean MVP 範囲 (Plane×Plane / Plane×Cylinder 軸 ⊥ / Plane×Sphere normal ±Z / Cylinder×Sphere 同軸)」節が存在 | grep で節タイトル hit |
| T30 | 統合 (entity name) | A1 の intersection edge name が `EntityRef::Derived{op:"cut_isect_edge", ..}` で 2 親 (box.face[+Z], cylinder.lateral) | name 一致 |
| **T31** | 統合 R04 (Euler-Poincaré) | A1 (box + cyl Cut blind hole, 単一 shell, genus 0, box 上面に inner_loop 1 個) で `V - E + F - L_inner = 2(S - G) = 2(1 - 0) = 2` を assert (B-rep inner-loop 込みの Euler-Poincaré 形式) | 等式成立 |
| **T32** | 統合 R04 (Euler-Poincaré + nesting) | A3 (box + 内包 sphere Cut, outer + inner の 2 shell、inner shell は self-adjacent periodic sphere face で構成) で total `V - E + F - L_inner = 2(S - G) = 2(2 - 0) = 4`、inner shell sphere face の `same_sense=false`、`Solid.shells` 内に outer (6 box faces) / inner (1 sphere face) が両方存在 | 等式成立 + inner shell nesting 検証 |
| **T33** | 統合 R04 (Euler-Poincaré) | A2 (cyl ∩ sphere Intersect、単一 shell、貫通穴なし、inner_loop なし) で `V - E + F - L_inner = 2` を assert | 等式成立 |

**テストの配置**:
- T01-T11: `crates/mycad-kernel/src/geometry/surface_intersect.rs` 内の `#[cfg(test)] mod tests`
- T09-T14: `crates/mycad-kernel/src/booleans/classify.rs` 内の `#[cfg(test)] mod tests`
- T15-T16, A4d: `crates/mycad-build/tests/feature_dispatcher.rs` (公開 API 経由)
- T17-T28, T30, T34, A1, A2, A3, A4a-c: `crates/mycad-build/tests/feature_dispatcher.rs`
- T24-T26: `crates/mycad-kernel/src/tessellation/mod.rs` (既存テストを反転)
- T29: 新規 `crates/mycad-kernel/tests/adr_004_doc.rs` (#31 で導入済みの仕組みを流用)
- T23: `crates/mycad-build/tests/golden/` (golden ファイル)

## 幾何的不変条件チェックリスト (Boolean/Partition/Assemble 系)

- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか → **既存 PSLG の挙動を維持**、`surface.normal_at(u,v)` を `normal_at_point` 経由で問い合わせるため、polygon 頂点順は normal 整合性に影響しない
- [x] 各プリミティブの face ごとの outer_loop 2D 向き (CW/CCW) が文書化されているか → **既存 cuboid/cylinder/sphere の向きは ADR-005 §7 で既出**、本 Issue は primitive を変更しない
- [x] flip_normals / same_sense の意味論が明確か → **`reverse_face_orientation` を Surface 別に実装** (§6 設計方針)、Plane: normal 反転、Cylinder: axis 反転、Sphere: そのまま (face HE 順で表現)
- [x] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか → **PSLG ロジックは変更せず**、円弧を直線サンプル化して投入するためアルゴリズム的に等価
- [x] **(追加)** trim された曲面 face の UV polygon 向きが earcut の前提 (CCW outer / CW inner) に整合しているか → `unwrap_periodic_uv` 後の UV 列を Newell's normal で検査し、必要に応じて反転する helper `ensure_ccw_uv_polygon` を新設
- [x] **(追加)** Cylinder の seam 跨ぎで UV が 2π wrap した場合、PSLG への投入時に切れ目が入っていないか → `unwrap_periodic_uv` で連続化済み、PSLG の input polygon は 1 本の連続多角形

## 実装順序

各 STEP 完了後に `cargo xtask ci` グリーンを期待。STEP A-C は 1 GLM ループ、D-F は 2 ループ目、G-I は 3 ループ目想定。

1. **STEP A** — `crates/mycad-kernel/src/error.rs` に `UnsupportedSurfaceIntersection { reason }` 追加。`crates/mycad-kernel/src/booleans/mod.rs` で `boolean_planar` → `boolean` rename、`crates/mycad-build/src/lib.rs:151,167,184` 追従。`validate_boolean_input` の gate を Plane/Cylinder/Sphere accept に緩和 (ただし内部実装はまだ Plane のみ動作するため、Cylinder/Sphere 入力に対しては最初の Plane vs 曲面 dispatch で `UnsupportedSurfaceIntersection` が返るのを期待)。`t21_nonplanar_input_cylinder` の期待値を「Cone は引き続き `NonPlanarBooleanInput`、Cylinder は新エラーで Err」に書き換え (本 STEP 段階では実装が無いため Err)。既存 Plane×Plane test 群が全部緑であることを確認 (`mycad-build/src/lib.rs` の呼び出し追従のみで動く)

2. **STEP B** — `crates/mycad-kernel/src/geometry/surface_intersect.rs` 新設。`IntersectionLoop` 構造 + `intersect_surfaces` dispatcher を作る。Plane×Plane のみ実装 (既存 `intersect_planes` (`partition.rs:495-527`) を移植)。他のペアは `Err(UnsupportedSurfaceIntersection)` を返す。`geometry/mod.rs` に submodule 宣言。T01 (の Plane×Plane 部分) と T04-T06 (の Err パス) が緑

3. **STEP C** — `crates/mycad-kernel/src/booleans/partition.rs` を `Surface` 直保持に refactor:
   - `FaceFragment.plane: PlaneData` → `surface: Surface`
   - `PlaneData::project_2d` 呼び出しを `project_to_face_uv(surface, p)` に置換 (Plane 以外は本 STEP では使われない)
   - `partition_faces` 内の `intersect_planes` 呼び出しを `intersect_surfaces` の返り値処理に置換 (Plane×Plane のみ動作)
   - `pslg_subdivide` は変更しない (依然直線セグメントベース、円弧サンプル化は STEP D で導入)
   - 既存 Plane×Plane Boolean test 群 t02-t20 が全部緑

4. **STEP D** — Plane × Cylinder (軸 ⊥) 実装:
   - `surface_intersect.rs` に `intersect_plane_cylinder` 実装 (1 個の Circle + Cylinder 側 Line2D pcurve + Plane 側 Circle2D pcurve)
   - `classify.rs::ray_intersect_surface` に Cylinder 経路追加 (2 次方程式)
   - `classify.rs::point_in_curved_face` 実装 (UV polygon inside test)
   - `classify.rs::point_in_polyhedron` を `ray_intersect_surface` 一般化に書き換え
   - `assemble.rs::reconstruct_intersection_curve` 実装 (Plane×Cylinder → Circle)
   - `assemble.rs::reverse_face_orientation` の Cylinder 対応 (axis 反転)
   - `assemble.rs::signed_volume` の曲面 face 対応 (テッセレーション経由で集計)
   - `partition.rs` の Cylinder face PSLG 投入 (`project_to_face_uv` で UV 列を取得し `unwrap_periodic_uv` で seam 補正、円弧 IntersectionSegment を `opts.angular_segments` でサンプル化)
   - **A1 (box + cylinder Cut) と A4a (斜め cylinder reject) が緑**
   - 平面 Boolean test 群が全部緑

5. **STEP E** — Plane × Sphere (plane normal ±Z) 実装:
   - `surface_intersect.rs` に `intersect_plane_sphere` 実装 (1 個の Circle + Sphere 側 Line2D pcurve)
   - `classify.rs::ray_intersect_surface` に Sphere 経路追加
   - `assemble.rs::reconstruct_intersection_curve` に Plane×Sphere 経路追加
   - `assemble.rs::reverse_face_orientation` の Sphere 対応 (そのまま)
   - `partition.rs` の Sphere face PSLG 投入 (Sphere UV 上で同様の seam unwrap)
   - **A3 (box + 内包 sphere Cut)** が緑 (sphere が box 内なので交線なし、Plane×Sphere の交線計算経路は実行されないが reverse_face / signed_volume / classify の sphere 経路は動く)
   - T05 (normal が ±Z 以外で Err) 緑

6. **STEP F** — Tessellation trim 対応:
   - `tessellation/mod.rs::tessellate_face_uv_grid` の冒頭で trim 判定 + `tessellate_face_trimmed_uv` 新規分岐
   - `tessellation/mod.rs::tessellate_face_sphere` の trim 分岐 + `tessellate_face_sphere_trimmed` 新規分岐
   - `unwrap_periodic_uv` helper を tessellation 側にも置く (partition.rs と共有)
   - `assemble.rs::attach_pcurves_for_trimmed_faces` 実装 (intersection HE に pcurve を後付け)
   - **T24/T25/T26 の既存 reject test を Ok 期待に反転**
   - A1/A2/A3 の mesh 出力が finite + 三角形数 > 0 を確認 (T17/T18 緑)

7. **STEP G** — Cylinder × Sphere (同軸) 実装:
   - `surface_intersect.rs` に `intersect_cylinder_sphere` 実装 (0/1/2 個の Circle)
   - `assemble.rs::reconstruct_intersection_curve` に Cylinder×Sphere 経路追加
   - `partition.rs` の Cylinder×Sphere face PSLG 投入経路
   - **A2 (同軸 cylinder ∩ sphere)** 緑
   - T03 / T06 緑

8. **STEP H** — Edge naming / determinism / golden:
   - intersection edge name (`derive_edge_name` 経由) が曲面ケースでも正しく付与されることを確認
   - T22 (決定性) / T23 (golden) / T30 (entity name) 緑
   - 100-run 決定性チェック (`cargo test -- --test-threads=1` で 5 回回す手動チェックを Plan に含めるかは省略可)

9. **STEP I** — `docs/decisions/004-freeform-geometry-commitment.md` に「曲面 Boolean MVP 範囲」節を追記 (T29)。CLAUDE.md (`crates/mycad-kernel/CLAUDE.md`) の "How to Add a New Surface Type" に「Boolean サポートが必要なら surface_intersect.rs のディスパッチ表を更新する」を追記。`cargo xtask ci` 最終グリーン化

## 引き継ぎ (本 Issue → #35 / 後続)

- **#35** (Phase 4 close — ビューア目視確認): A1 (box + 円柱 Cut) / A2 (同軸 cylinder ∩ sphere) / A3 (box + 球 Cut) の結果を `mycad view` で開いて視覚確認、Phase 4 完了マーク
- per-entity tolerance 移行は **別 Issue** (Phase 5 の foundation として切り出す候補)
- 自由曲面 Boolean (任意軸 cylinder × cylinder、Sphere × Sphere、Cone × *、Sampled 曲線) は **別 Phase の別 Issue** (フィレット・面取り Phase の前提)
- `tessellate_face_uv_grid` の Cone trim は **別 Issue** (Cone をサポートする Phase で)
- `Surface::du/dv` 偏微分は **必要になった時点で別 Issue** (任意軸 Plane × Cylinder の楕円交線 marching が必要なときなど)

## Verification

実装完了後に以下を順次実行して全緑を確認:

```bash
cargo test --workspace                                # 全テスト
cargo clippy --workspace -- -D warnings               # lint
cargo fmt --all -- --check                            # フォーマット
cargo xtask ci                                        # 全部
```

Acceptance の目視確認 (#35 に委譲):
```bash
mycad export examples/boolean_box_cylinder_cut.mycad -o /tmp/A1.stl
mycad view examples/boolean_box_cylinder_cut.mycad  # ブラウザで blind hole (上面の円形開口) を確認
```
