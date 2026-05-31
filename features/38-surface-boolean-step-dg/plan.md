# Issue #38 — 曲面 Boolean STEP D–G (縮小スコープ): stub 完成 + A3 成立

## Context

Phase 4 (Boolean) のクリティカルパス上、commit ddf8ec1 (#34 部分実装) で残った stub のうち **「A3 (内包球 void shell) を成立させるための最小集合」** を埋める。A1 (box - cylinder Cut blind hole) は **partition の plane-only 撤廃 + intersection provenance チェーン + multi-loop face (annulus) サポート + cyclic seam-crossing arc merge** が必要で独立した複雑度を持つため、別 Issue #39 へ切り出す (Codex R02/R03/R04 の構造的指摘を受けたスコープ判断、ユーザー承認済み)。

A3 は **box と sphere が intersection しない** (sphere 完全内包) 入力で、partition の交線処理ループは回らない。必要なのは:
- partition.rs 入口で sphere face を含む入力が panic しないこと (R01: 現状 `PlaneData::from_surface(&Surface::Sphere).unwrap()` で panic、`Option<PlaneData>` 化が必須)
- classify / signed_volume / reverse_face_orientation の sphere 経路 (ddf8ec1 で landed 済み)
- sphere face を含む Solid が `validate_boolean_input` を通る (convex 撤廃 + analytic Circle loop accept)
- sphere primitive のエンティティ名付け (Acceptance test で `assert_solids_equal_with_names` を使うため)

ddf8ec1 で既に landed (本 Issue では触らない):
- `intersect_surfaces` dispatcher と Plane×{Cylinder/Sphere}・Cylinder×Sphere の交線生成
- `assemble.rs::reverse_face_orientation` の `same_sense` 単独エンコード
- `assemble.rs::signed_volume` の曲面 face 対応 (effective_normal 経由)
- `classify.rs::ray_intersect_surface` の Cylinder/Sphere 対応 と曲面 face UV 投影
- `validate_boolean_input` の Plane/Cylinder/Sphere accept、Cone のみ reject
- `booleans::boolean` への rename + mycad-build 3 箇所追従

本 Issue で落とすべき残り (縮小後):
1. **partition.rs 入口の plane-only 前提を解消** (R01) — `target_planes` / `tool_planes` を `Vec<Option<PlaneData>>` に変え、`PlaneData::from_surface` の `None` を許容 (sphere/cylinder face は plane 情報なし)。coplanar 判定は両方 `Some` のときだけ実施
2. **convex チェック撤廃 + analytic Circle loop accept** (R01 補強) — `mod.rs:84-88` の `is_convex_polygon_3d` ブロックを削除。Planar face で outer_loop が 1HE `Curve::Circle` の場合は頂点数チェック対象から除外、それ以外は `n >= 3` を要求
3. **make_sphere エンティティ名付け** — `primitives/sphere.rs` の `add_vertex` / `add_edge` / `add_face` に `EntityRef::Named` (ADR-005 §7 準拠の role 名) を付与
4. **cyl×sph partition gate** (#39 のスコープ侵食防止) — `validate_boolean_input` 入口で cylinder + sphere 両方を含む入力 (target に cylinder lateral、tool に sphere face、または逆) を `UnsupportedSurfaceIntersection { reason: "cyl×sph partition deferred to #39" }` で reject。A3 (box + sphere) と A1 (box + cylinder、別 Issue) は通す
5. **A3 Acceptance test** — `feature_dispatcher.rs` に box (10×10×10 中心原点) Cut sphere (center=(0,0,0)、radius=3、box 完全内包 (max coord 3 ≤ box half-extent 5)) → outer + inner 2-shell の void shell トポロジーを検証

設計判断 (壁打ち + Codex Round 1/2 で確定):
- A1 (box-cylinder blind hole) は **Non-Goal**、別 Issue #39 (PSLG 投入 + multi-loop face + arc 再構成 + provenance + trim tessellation + pcurve 後付け + cyclic seam-crossing merge)
- A2 (cyl ∩ sph Intersect) は **Non-Goal**、別 Issue (cylinder と sphere 両面 trim + sphere seam UV unwrap)
- convex 撤廃の代替は「polygon は `n >= 3`、analytic Circle outer_loop は除外」(Codex R01 採用)
- partition.rs の `PlaneData::from_surface(..).unwrap()` panic 回避は #38 で **必須** (R01 採用)
- arc 再構成 / `ArcProvenance` / PSLG sanitize / `BOOLEAN_ARC_SAMPLES` / `attach_pcurves_for_trimmed_faces` / trim tessellation は **すべて #39 へ移送**

## Non-Goals

- **A1 (box - cylinder Cut blind hole) と A1 関連の T17/T19/T21/T22(A1)/T27/T31**: 別 Issue #39。partition の Circle 交線 PSLG 投入、`reconstruct_intersection_curve`、`ArcProvenance`、`BOOLEAN_ARC_SAMPLES`、PSLG sanitize、cyclic seam-crossing arc merge、`attach_pcurves_for_trimmed_faces`、`tessellate_face_uv_grid` の trim 経路、multi-loop face (annulus) サポート、PSLG nesting、`assemble.add_face` の inner loops 受け入れ拡張、`pslg_subdivide` の multi-loop 出力拡張、すべて #39 へ
- **A2 (Cylinder ∩ Sphere Intersect) と T18/T20/T34**: 別 Issue (#40 候補)。cylinder と sphere 両面の trim + sphere 側 trim tessellation を担う
- **Self-intersection (simple polygon) チェック**: convex 撤廃の代替として「polygon は n >= 3、analytic Circle loop は除外」のみ実施。将来 Issue
- **make_cylinder のエンティティ名付け**: 本 Issue は sphere のみ。cylinder は別 Issue
- **`point_in_curved_face` の named helper 抽出**: 既に `count_ray_face_intersections` (`classify.rs:182-190`) に inline で実装済み。リファクタは別 Issue
- **`unwrap_periodic_uv` helper**: A3 では UV unwrap が不要 (sphere intersection なし)。partition.rs / tessellation 共有化は #39 のスコープ
- **STEP H (golden T23) と STEP I (ADR/CLAUDE.md 文書 T29)**: A1 が落ち着いてから別 Issue

## 実装対象

- **Issue**: #38 (縮小スコープ)
- **影響クレート/ファイル**:
  - `crates/mycad-kernel/src/booleans/partition.rs` (R01: `Vec<Option<PlaneData>>` 化、coplanar 判定の `Some` ガード追加)
  - `crates/mycad-kernel/src/booleans/mod.rs` (convex 撤廃、analytic Circle loop accept、cyl×sph partition gate)
  - `crates/mycad-kernel/src/primitives/sphere.rs` (`EntityRef::Named` 付与)
  - `crates/mycad-build/tests/feature_dispatcher.rs` (A3 Acceptance + T32 (A3 Euler) 追加)
- **変更する型・関数のシグネチャ**:
  - `partition_faces` 戻り値・引数は不変。内部の `target_planes: Vec<PlaneData>` / `tool_planes: Vec<PlaneData>` を `Vec<Option<PlaneData>>` に変更。`PlaneData::from_surface` が `Surface::Plane` 以外で `None` を返す前提 (現状 `Option` を返すかは要確認 — `.unwrap()` を外す段階で確認、`None` を返さないなら関数自体を `pub fn from_surface(s: &Surface) -> Option<PlaneData>` に変更)
  - `is_convex_polygon_3d` を削除。代わりに `validate_planar_face_outer_loop_basic(face: &Face, solid: &Solid) -> Result<(), KernelError>` を新設: (a) outer_loop が 1HE `Curve::Circle` なら `circle.radius > LENGTH_TOLERANCE` を確認して `Ok(())`、(b) それ以外は頂点数 ≥ 3、隣接頂点距離 > `LENGTH_TOLERANCE` (ゼロ長辺禁止)、3 点以上の場合は signed area の絶対値 > `LENGTH_TOLERANCE.powi(2)` (面積ゼロ多角形禁止) を確認 (R03 反映)
  - `cyl×sph` gate は `validate_boolean_input` ではなく **`partition_faces` 内部の face pair ループ内で skip** に変更 (R04 反映): target face が Cylinder かつ tool face が Sphere (またはその逆) の pair に対しては `intersect_surfaces` を呼ばずに continue + warn ログ。混在入力全体は通す。これにより A3 (box+sphere) と将来の cylinder+box は通り、cyl×sph face pair のみ #39 へ deferred
  - `make_sphere`: 戻り値・引数シグネチャ不変、`add_vertex`/`add_edge`/`add_face` に `Some(EntityRef::try_named("sphere", kind, role)?)` を渡す。**role 名は ADR-005 §7 厳密準拠** (`"south_pole"` / `"north_pole"` / `"seam"` / `"surface"`)

## 設計方針

### 決定性要件

- `make_sphere` の name 文字列はハードコード (feature_id=`"sphere"`、role=`"south_pole"` / `"north_pole"` / `"seam"` / `"surface"`) で決定的
- `partition_faces` の `Vec<Option<PlaneData>>` への変換は `target.faces.iter().map(|f| PlaneData::from_surface(&f.surface))` で順序保証 (input face 順)
- すべての新規 entity (sphere primitive) は `IdGenerator::next()` で発番。決定性は T10 (sphere 2 回呼び比較) で検証

### B-rep トポロジー妥当性

- A3 結果: 2 shell (outer = 6 box faces、inner = 1 self-adjacent periodic sphere face)、genus 0、inner_loop なし。Euler-Poincaré (B-rep 形式): `V - E + F - L_inner = 2(S - G) = 2(2 - 0) = 4` を **T32** で検証
- A3 の inner shell sphere face は `same_sense == false` (Cut で sphere の内向き法線が void shell の外向きになる)
- `validate_manifold()` を A3 で実行し `Ok(())` を確認

### 退化幾何の扱い

- A3 では box と sphere が交差しないため partition の交線処理ループは回らない (`intersect_surfaces` が空 Vec を返す)。退化対策は最小 (sphere center=box center で完全対称なケースが安全側)
- partition の `Option<PlaneData>` 化により、sphere face を含む coplanar 判定は両方 `Some` のときだけ実施。`None` 側 (sphere face) は coplanar 候補から除外され「常に non-coplanar」扱い

### derive 規約

- `EntityRef::Named` の name 文字列は ASCII `[A-Za-z0-9_-]` のみ (既存制約、`feature.rs:133`)
- `PlaneData::from_surface` 戻り値が `Option<PlaneData>` であることを確認 (現状不明、実装段階で確認し、`Option` でなければ変更)

### エラーハンドリング

- 既存 `KernelError::UnsupportedSurfaceIntersection { reason: String }` を再利用
- 新規 reject 理由: `"cyl×sph partition deferred to #39"` (cyl×sph partition gate)
- `validate_planar_face_outer_loop_basic` の失敗は既存 `KernelError::InvalidBooleanInput { reason: String }` (なければ実装段階で適切な variant を選択)

### workspace.dependencies

新規依存は **無し**。

## テスト計画 (ID 付き)

| ID  | 種別     | 内容                                                                                                  | 期待結果                                                                 |
|-----|----------|-------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------|
| T07 | 単体 (mod) | `validate_boolean_input` に頂点 3 個の非 convex (V 字) outer_loop polygon を持つ planar face を入力 | 旧 `is_convex_polygon_3d` で reject されていた入力が **`Ok(())`** に変わる |
| T08 | 単体 (mod) | `validate_boolean_input` に頂点 2 個 (collinear) の planar face outer_loop を入力 | `Err(InvalidBooleanInput)` (n >= 3 ガード) |
| T09 | 単体 (sphere) | `make_sphere(center, radius, id_gen)` の戻り値 `Solid` から各 entity の `EntityRef::Named { feature_id, kind, role }` を取得 | `feature_id == "sphere"`、role が `"south_pole"` (V) / `"north_pole"` (V) / `"seam"` (E) / `"surface"` (F) — ADR-005 §7 準拠 |
| T10 | 単体 (sphere) | `make_sphere` を 2 回呼び出し、ID 以外の entity 構造と name を比較 | 全 name 一致 (決定性) |
| T13 | 単体 (mod R01) | `validate_boolean_input` に Planar face で outer_loop が 1 HE `Curve::Circle` (cylinder cap_top 相当、`radius=2`) の Solid を入力 | `Ok(())` (analytic Circle loop は頂点数チェック対象から除外、radius チェック通過) |
| T14 | 単体 (partition R01) | `partition_faces` に sphere face を含む Solid (例: A3 入力の box + sphere) を渡す | panic せず `Ok((target_fragments, tool_fragments))` を返す (sphere face は coplanar 判定から除外) |
| T15 | 単体 (partition R04) | `partition_faces` に cylinder face と sphere face を含む混在入力 (target=cylinder solid、tool=sphere solid) を渡す | panic せず `Ok(..)` を返し、partition 結果には cylinder lateral × sphere の交線処理は含まれない (skip + warn ログ)。box などの planar pair は通常通り処理される |
| T16 | 単体 (mod R03 退化) | `validate_boolean_input` に (a) 3 点 collinear polygon、(b) 面積ゼロ polygon、(c) ゼロ長辺を含む polygon、(d) `radius = LENGTH_TOLERANCE / 2` の analytic Circle outer_loop を入力 | (a)-(c) は `Err(InvalidBooleanInput)`、(d) は `Err(InvalidBooleanInput { reason: "analytic circle radius below LENGTH_TOLERANCE" })` |
| **A3** | 統合 (Acceptance) | box (中心原点、10×10×10) を Cut with sphere (center=(0,0,0)、radius=3、box 完全内包 (max coord 3 ≤ box half-extent 5)) | `Ok(Solid)`、`validate_manifold == Ok(())`、`shells.len() == 2` (outer 6 face + inner 1 sphere face)、inner shell の sphere face `same_sense == false` |
| T22(A3) | 決定性 | A3 を 2 回 build し `assert_solids_equal_with_names` で完全一致 | 全 entity name 同一 |
| T32 | Euler (A3, R04 part) | A3 結果で `V - E + F - L_inner = 2(S - G) = 2(2 - 0) = 4`、`shells.len() == 2`、inner shell の sphere face `same_sense == false` | 等式成立 + nesting 検証 |
| T_sphere_existing_unchanged | 既存リグレッション | `t03_sphere_regression` 既存テストが pass し続ける (sphere naming 追加で既存 expected が壊れていないか) | 既存と同じ triangle count / mesh hash |

### テスト配置

- T07/T08/T13/T16: `crates/mycad-kernel/src/booleans/mod.rs` 内 `#[cfg(test)] mod tests`
- T09/T10: `crates/mycad-kernel/src/primitives/sphere.rs` 内 `#[cfg(test)] mod tests`
- T14/T15: `crates/mycad-kernel/src/booleans/partition.rs` 内 `#[cfg(test)] mod tests`
- A3, T22(A3), T32: `crates/mycad-build/tests/feature_dispatcher.rs`
- T_sphere_existing_unchanged: 既存テストを動かすだけ

### 既存テスト影響評価

- `crates/mycad-build/tests/feature_dispatcher.rs::t03_sphere_regression` (line 168): sphere の triangle count / mesh hash で固定されているため、**naming 追加では破綻しない** (mesh は entity name に依存しない)。golden YAML / entity ref を assert している経路があれば追従が必要 → 実装段階で確認、必要なら expected を更新
- 既存 Plane×Plane Boolean (`t02_*` 〜 `t20_*`) は `validate_boolean_input` で reject されないため (V 字 outer_loop 等は使われていない)、convex 撤廃で破綻しない見込み。実装後に必ず `cargo test --workspace` で確認
- `partition.rs` の `target_planes: Vec<Option<PlaneData>>` 化は Plane×Plane 既存経路で `Some` 路径のみを通るため、coplanar 判定の出力は不変。回帰なし

## 幾何的不変条件チェックリスト (Boolean/Partition/Assemble 系)

- [x] **partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか** → ddf8ec1 の `signed_volume` が `effective_normal = if same_sense { face_normal } else { -face_normal }` で normal の符号を分離。本 Issue は同方針を継承
- [x] **各プリミティブの face ごとの outer_loop 2D 向き (CW/CCW) が文書化されているか** → cuboid/cylinder/sphere の向きは ADR-005 §7 で既出。本 Issue は sphere の primitive 構造を変更しない (名前付与のみ)
- [x] **flip_normals / same_sense の意味論が明確か** → ddf8ec1 で `same_sense` 単独エンコードに統一済み。A3 の inner shell sphere face は `same_sense = false` で表現 (T32 で assert)
- [x] **pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか** → A3 では PSLG が走らないため対象外 (#39 で対応)
- [x] **(R01 反映) partition.rs の `PlaneData::from_surface` panic 回避** → `Vec<Option<PlaneData>>` 化、coplanar 判定は両方 `Some` のときだけ実施。T14 で sphere face 入力の panic-free を検証
- [x] **(R01 反映) `validate_boolean_input` の analytic Circle loop accept** → 1HE `Curve::Circle` outer_loop の planar face は頂点数チェック除外。T13 で検証

## 実装順序

各 STEP 完了後に `cargo xtask ci` の該当部分グリーンを期待。

1. **STEP 1 — sphere naming** (`primitives/sphere.rs` + T09/T10)
   - ADR-005 §7 準拠の role 名 (`"south_pole"` / `"north_pole"` / `"seam"` / `"surface"`) を `EntityRef::try_named("sphere", kind, role)` で付与
   - 既存 `t03_sphere_regression` が pass し続けることを確認 (entity name に依存しない mesh hash の比較なので破綻しないはず)
   - 完了条件: T09/T10 + 既存テスト全 green

2. **STEP 2 — convex 撤廃 + analytic Circle accept + 退化チェック (R01 part 1 + R03 Round 3)** (`booleans/mod.rs` + T07/T08/T13/T16)
   - `is_convex_polygon_3d` 削除
   - `validate_planar_face_outer_loop_basic` を新設、`validate_boolean_input` から呼ぶ:
     - outer_loop が 1HE `Curve::Circle` なら `circle.radius > LENGTH_TOLERANCE` を確認 → Ok
     - それ以外は (a) 頂点数 ≥ 3、(b) 隣接頂点距離 > `LENGTH_TOLERANCE` (ゼロ長辺禁止)、(c) signed area の絶対値 > `LENGTH_TOLERANCE.powi(2)` (面積ゼロ禁止) を確認
     - 不満なら `Err(InvalidBooleanInput { reason: ".." })`
   - 完了条件: T07/T08/T13/T16 + 既存 Plane×Plane Boolean (`t02-t20`) 全 green

3. **STEP 3 — partition.rs の Option<PlaneData> 化 (R01 part 2)** (`booleans/partition.rs` + T14)
   - `partition_faces` 冒頭の `target_planes: Vec<PlaneData> = ... .unwrap()` を `Vec<Option<PlaneData>>` に変更
   - 同様に `tool_planes`
   - **`.unwrap()` および `.as_ref().unwrap()` の残置は禁止** (R01 Round 3 反映): sphere/cylinder face で touch される経路で 1 箇所でも残ると panic 設計回避が破綻するため、`PlaneData` の参照は必ず `let Some(plane) = ... else { continue; }` または `if let Some(plane) = ...` の guard で囲む
   - 該当箇所: coplanar 判定ループ (line 155-194)、target/tool face iteration (line 205, 217, 519 等)、`PlaneData::from_surface(surface)` (line 478) — それぞれ Some-guard 化
   - 必要なら plane 投影が必要な処理を `pub(crate) fn coplanar_overlap_2d(tp: &PlaneData, up: &PlaneData, ...) -> Option<...>` のような **Some 前提 helper** に切り出し、unwrap を構文的に不可能にする
   - `PlaneData::from_surface` が `Option` を返さない場合は `Option<PlaneData>` を返すように変更
   - 完了条件: T14 (sphere face 入力で panic-free) + 既存 Plane×Plane 全 green、grep で `.as_ref().unwrap()` `\.unwrap()` が `partition.rs` の plane 関連経路にゼロであることを目視確認

4. **STEP 4 — cyl×sph face pair skip (R04 Round 3 反映)** (`booleans/partition.rs` + T15)
   - **`validate_boolean_input` 入口での全面 reject ではなく**、`partition_faces` 内部の face pair ループで「target.faces[ti].surface が Cylinder かつ tool.faces[ui].surface が Sphere、または逆」のペアに対して `intersect_surfaces` を呼ばずに `continue` + warn ログ (R04 反映)。混在入力全体は通る
   - これは #39 完成までの一時 skip (#39 で削除する旨をコメントに明記)
   - T15 を「cylinder+sphere 混在入力で全面 reject ではなく、cyl×sph face pair のみ無視されて他 face pair の partition は走り Ok 返却」に書き換え
   - 完了条件: T15 + A3 (box + sphere) + 既存 Plane×Plane 全 green

5. **STEP 5 — A3 Acceptance + Euler** (`mycad-build/tests/feature_dispatcher.rs` + A3, T22(A3), T32)
   - A3: box (中心原点、10×10×10) Cut sphere (center=(0,0,0)、radius=3、box 完全内包 (max coord 3 ≤ box half-extent 5)) → 2-shell void shell
   - T22(A3): 2 回 build で `assert_solids_equal_with_names` 一致
   - T32: `V - E + F - L_inner = 4`、`shells.len() == 2`、inner sphere face `same_sense == false`
   - 完了条件: 全テスト green、`cargo xtask ci` 最終 green

## Verification

実装完了後:
```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
cargo xtask ci
```

A3 の目視確認は #35 へ委譲。A1 (box-cylinder blind hole) は #39 で実装し、その後 #35 で A1+A3 まとめて目視確認 → Phase 4 close。
