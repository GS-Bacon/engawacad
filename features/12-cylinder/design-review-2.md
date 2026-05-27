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
session id: 019e57c8-af7c-7c62-a08f-2ae556901aa0
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
# Issue #12: 円柱プリミティブ + 曲面テッセレーション土台

## Context（なぜやるか）

ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。

設計の根幹判断（壁打ち済み）:
- 解像度は STL 焼き出し側の関心事。B-rep は厳密なまま。本番出力は将来 STEP 前提なので、解像度は **外部化可能だが既定値で足りる**サブ経路。
- テッセレータは ADR-004 準拠で **variant を直接 `match` しない**。型ごとの分岐は `Surface`/`Curve` の多態メソッド（網羅 match はコンパイラが強制）に閉じ込め、アルゴリズム本体は型非依存に保つ。
- 2D 三角化は今回は自前（円柱は凸領域で自明）。任意トリム面/Boolean 面が出る Phase 3後半/4 でライブラリ(earcutr/spade 等)を差し込む余地を残す。

## 実装対象

- Issue: #12 / Milestone: Phase 3
- 影響ファイル:
  - 新規 `crates/mycad-kernel/src/primitives/cylinder.rs` — `make_cylinder`
  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
  - `crates/mycad-kernel/src/geometry/curve.rs` — `Curve::sample_segment` 追加
  - `crates/mycad-kernel/src/geometry/surface.rs` — `Surface::requires_interior_sampling` 追加
  - `crates/mycad-kernel/src/tessellation/mod.rs` — 曲線折れ線化 + 曲面 UV グリッド対応 + `TessellationOptions`
  - `crates/mycad-build/src/lib.rs` — `CreateCylinder` スタブを実差し替え
  - `crates/mycad-cli/src/main.rs` — `export` に任意 `--segments` を追加（既定値あり）
  - 新規 `examples/cylinder.mycad`
  - テスト: `cylinder.rs` 内インライン / `tessellation/mod.rs` 内 / `mycad-build/tests/feature_dispatcher.rs` / `mycad-cli/tests/export.rs`

## 設計方針

### 1. トポロジー（`make_cylinder(radius, height, id_gen) -> Solid`）

軸 = +Z、底面中心を原点、底 z=0 / 上 z=h。`orthonormal_basis(+Z) = (+Y, -X)` なので角度 u=0 は +Y 方向 → シーム頂点は (0, r, z)。

- 頂点 **2**: `v_bot=(0,r,0)`, `v_top=(0,r,h)`
- エッジ **3**:
  - `e_bot`: `Curve::Circle{center(0,0,0), normal +Z, radius r}`, vertices `[v_bot,v_bot]`, t_range `[0,2π]`（閉曲線。evaluate(0)=v_bot）
  - `e_top`: `Curve::Circle{center(0,0,h), normal +Z, radius r}`, vertices `[v_top,v_top]`, t_range `[0,2π]`
  - `e_seam`: `Curve::Line{origin(0,r,0), direction(0,0,h)}`, vertices `[v_bot,v_top]`, t_range `[0,1]`
- 面 **3**（同 `Surface` 既存 variant、enum 追加なし）:
  - 底キャップ: `Surface::Plane{origin(0,0,0), normal -Z, u_axis +X, v_axis +Y}`、loop = `e_bot` の HE 1 本
  - 上キャップ: `Surface::Plane{origin(0,0,h), normal +Z, u_axis +X, v_axis +Y}`、loop = `e_top` の HE 1 本
  - 側面: `Surface::Cylinder{origin(0,0,0), axis +Z, radius r}`、loop = 4 HE: seam上(e_seam,fwd,start v_bot)→top円(e_top,fwd,start v_top)→seam下(e_seam,rev,start v_top)→bot円(e_bot,rev,start v_bot)
- シェル **1**（closed=true）

**マニフォルド規約**: 各エッジは正確に 2 HE で、向きは逆（fwd/rev）。
- `e_seam`: 側面が fwd と rev（シームは同一面が両側で使用＝周期面の標準形）
- `e_bot`: 側面が rev、底キャップが fwd
- `e_top`: 側面が fwd、上キャップが rev
- HE 合計 = 6（= 3 edges × 2）。

**Euler–Poincaré**: V−E+F = 2−3+3 = 2 = 2(S−H), S=1,H=0 → 成立。

**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。

**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。

### 2. テッセレータ（ADR-004 準拠の加算拡張）

`tessellate_solid` を次の型非依存アルゴリズムに拡張。**`match Surface::Cylinder` 等は書かない**。

- 解像度の外部化（R02: 不正解像度のクランプ）:
  ```rust
  pub struct TessellationOptions { pub angular_segments: usize, pub axial_segments: usize }
  impl Default // angular=32, axial=1
  impl TessellationOptions { pub fn new(angular, axial) -> Self }  // angular を 3 未満なら 3、axial を 1 未満なら 1 にクランプ
  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
  pub fn tessellate_solid_with(solid, &TessellationOptions) -> TriangleMesh  // 内部でも angular.max(3)/axial.max(1) で防御
  ```
- 数値規約（R04: epsilon を共有定数化し実装・テストで同一閾値）:
  - `tessellation` に `const SEAM_EPS: f64`（u のシーム跳び検出は `|Δu| > π` を基準、定数で表現）、`const AREA_EPS: f64`（外積ノルムがこれ未満の三角形は縮退として index に積まない）、法線向き判定は `desired_normal.dot(geom_normal) < 0.0` で反転スワップ（厳密 0 跨ぎのみ、tolerance 不要）。
- 多態フック（型分岐をジオメトリ側の網羅 match に閉じ込め、コンパイラが variant 追加漏れを強制）:
  - `Curve::sample_segment(&self, t_start: f64, t_end: f64, segments: usize) -> Vec<Point>`（R01: HE 向き対応）: `t_start`（含む）から `t_end`（含まない）へ向かう点列。先頭は必ず `evaluate(t_start)`。`Line` は `vec![evaluate(t_start)]`（1 点）、`Circle` は `t_start→t_end` を `segments` 等分した点列。
  - `Surface::requires_interior_sampling(&self) -> bool`: `Plane=false`、`Cylinder/Sphere/Cone=true`。
- 面ごとの処理:
  1. 外側 loop の各 HE について、`forward` なら `(t0,t1)`、`!forward` なら `(t1,t0)` を `sample_segment` に渡して折れ線化 → 連結して境界点列。**先頭点が必ず `HalfEdge.start_vertex` に一致**するため、逆向き Line を含む cuboid でも従来と同一の角点列＝**平面出力不変**。
  2. `surface.requires_interior_sampling()` が **false（平面）** → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。
  3. **true（曲面）** → 境界点を `surface.uv_of` で (u,v) 化し、周期シーム（u が ±π で跳ぶ）を 2π アンラップして連続化 → パラメータ箱 [u_min,u_max]×[v_min,v_max] を取得 → `(angular_segments+1)×(axial_segments+1)` の UV グリッドを `surface.evaluate(u,v)` でサンプル → セルごとに 2 三角形。これで側面が滑らかな多面体に。
  4. 各頂点法線は `surface.normal_at_point`（`same_sense` で反転）— 既存 PR #11 の方式を踏襲。
- **STL 外向き保証**: 各三角形の幾何法線（辺の外積）が、その面の外向き法線（`normal_at_point` を `same_sense` で調整）と逆なら 2 頂点をスワップして向きを揃える。これでキャップ/側面とも STL facet が外向き。cuboid は既に外向きのためスワップ発生せず、出力は実質不変（決定性・三角形数も維持）。

### 3. ディスパッチ / CLI / フォーマット

- `mycad-build/src/lib.rs`: `Feature::CreateCylinder{radius,height}` 分岐で `radius>0 && height>0` を検証し、OK なら `Ok(make_cylinder(*radius, *height, gen))`、NG なら `Err(KernelError::InvalidParameter{..})`（`UnsupportedFeature` スタブ削除）。`Feature` enum・`id()` は既存のまま変更不要。
- `mycad-cli`: `export` に `--segments <N>`（任意、未指定なら `TessellationOptions::default`）。指定時は `TessellationOptions::new(N, 1)` でクランプ。export パスは `tessellate_solid_with` を呼ぶ。
- `examples/cylinder.mycad`:
  ```yaml
  version: "0.1.0"
  root_component:
    name: "Simple Cylinder"
    features:
      - type: create_cylinder
        id: cyl_1
        radius: 5.0
        height: 20.0
  ```

### derive / 依存 / エラー規約

- `TessellationOptions` に `Debug, Clone, Serialize, Deserialize`（既存 `TriangleMesh` と同様）。`Copy` も可。
- 新規 workspace 依存なし（nalgebra のみで完結）。
- `KernelError`（既存 `thiserror`）に `InvalidParameter { kind: &'static str }` を 1 variant 追加（R02 の境界検証用）。

## 設計レビュー反映メモ（Codex R01–R05）

- R01(critical) 反映: `Curve::sample_segment` を向き対応 API に変更（上記 §2）。
- R02(high) 反映: `radius/height>0` を境界検証 + `TessellationOptions` クランプ。
- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
- R04(medium) 反映: epsilon 共有定数 `SEAM_EPS`/`AREA_EPS` と比較規約を明記（上記 §2）。
- R05(medium) 反映: golden YAML ラウンドトリップテスト T13 追加。

## テスト計画

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
| T02 | トポロジー | 頂点2・エッジ3・面3・シェル1(closed)・HE6・loop3 | 各 `len()` 一致 |
| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
| T04 | マニフォルド+ループ閉包 | 各エッジがちょうど 2 HE に参照され向きが逆。**かつ**各 loop で連続 HE の終点 ≈ 次 HE の始点、最後の HE 終点 ≈ 先頭 HE 始点（`approx`）。seam 接続も検証 | 集計 + 幾何連続性検証 (R03) |
| T05 | 幾何 | シーム頂点が (0,r,0)/(0,r,h)、各 Surface/Curve の係数 | `approx` で一致 |
| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
| T07 | 決定性(tessellate) | 同一入力2回で positions/normals/indices 一致 | `assert_eq!` |
| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions が従来値と一致 |
| T09 | 外向き法線 | 全 facet の幾何法線が重心からの外向きと同符号（凸形状なので判定可） | 検証 |
| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
| T11 | export 統合 | `mycad export examples/cylinder.mycad -o tmp.stl` が成功し、facet 数が T06 と整合、ASCII STL として妥当 | `export.rs`（`CARGO_BIN_EXE_mycad`） |
| T12 | 解像度外部化+クランプ | `tessellate_solid_with` で分割数が変わり三角形数が増減。`TessellationOptions::new(2,0)` が `angular=3/axial=1` にクランプされる | 2 解像度で差分 + クランプ検証 (R02) |
| T13 | golden YAML ラウンドトリップ | `examples/cylinder.mycad` を `from_path`→`to_yaml`→`from_yaml`→`to_yaml` で equality。`create_cylinder` の serialize/schema 回帰を固定 | `assert_eq!` (R05) |
| T14 | 退化入力エラー | `radius<=0` / `height<=0` の `create_cylinder` を `build_solid_from_features` に渡すと `KernelError::InvalidParameter` | `assert!(matches!(..))` (R02) |

## 検証（エンドツーエンド）

1. `cargo test --workspace` で T01–T12 green。
2. `cargo run -p mycad-cli -- export examples/cylinder.mycad -o /tmp/cyl.stl` 成功。
3. STL を目視/メッシュビューア（Blender/MeshLab）で読み込み、閉じた円柱（側面が多面体近似・上下フタあり・法線外向き）を確認。
4. `cargo xtask ci`（fmt→clippy→test→build）green。

## スコープ外（将来）

- `Sphere`/`Cone`/`Extrude`、任意トリム面の 2D メッシュライブラリ導入、pcurve、ビューアの解析/GPU 適応レンダリング、`HalfEdge` への隣接(twin/next/prev)構造導入（R03、Boolean/フィレットが要求した時点で別途）。
</stdin>
codex
設計レビューとして、まず既存カーネルのトポロジー型・`make_cuboid`・テッセレーション実装・エラー/依存規約を確認します。文書内の前提が既存設計と噛み合っているかを見てから、重大度付きで指摘を整理します。
exec
/bin/bash -lc "rg --files /home/bacon/mycad | rg 'crates/mycad-kernel/src/(primitives|geometry|topology|tessellation)|crates/mycad-build/src/lib.rs|crates/mycad-cli/src/main.rs|Cargo.toml"'$|crates/mycad-kernel/src/.+error'"'" in /home/bacon/mycad
 succeeded in 0ms:
/home/bacon/mycad/Cargo.toml
/home/bacon/mycad/crates/mycad-format/Cargo.toml
/home/bacon/mycad/crates/mycad-cli/Cargo.toml
/home/bacon/mycad/crates/mycad-cli/src/main.rs
/home/bacon/mycad/crates/xtask/Cargo.toml
/home/bacon/mycad/crates/mycad-kernel/Cargo.toml
/home/bacon/mycad/crates/mycad-viewer/Cargo.toml
/home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs
/home/bacon/mycad/crates/mycad-kernel/src/tessellation/stl.rs
/home/bacon/mycad/crates/mycad-kernel/src/geometry/surface.rs
/home/bacon/mycad/crates/mycad-kernel/src/geometry/mod.rs
/home/bacon/mycad/crates/mycad-kernel/src/geometry/curve.rs
/home/bacon/mycad/crates/mycad-kernel/src/geometry/math.rs
/home/bacon/mycad/crates/mycad-kernel/src/primitives/mod.rs
/home/bacon/mycad/crates/mycad-kernel/src/primitives/cuboid.rs
/home/bacon/mycad/crates/mycad-build/Cargo.toml
/home/bacon/mycad/crates/mycad-build/src/lib.rs

exec
/bin/bash -lc 'git status --short' in /home/bacon/mycad
 succeeded in 0ms:
 M .gitignore
?? .claude/
?? features/

codex
既存実装の骨格が見えました。次に `make_cuboid`、B-rep 型、テッセレータ、`build`/CLI の現状を読み、今回の設計がどこで破綻しうるかを詰めます。
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
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
mod cuboid;

pub use cuboid::make_cuboid;

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/geometry/curve.rs" in /home/bacon/mycad
 succeeded in 0ms:
use super::math::orthonormal_basis;
use super::{Point, Vec3};
use serde::{Deserialize, Serialize};

/// A geometric curve in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Curve {
    /// A line defined by an origin point and a direction vector.
    Line { origin: Point, direction: Vec3 },
    /// A circle defined by a center, normal, and radius.
    Circle {
        center: Point,
        normal: Vec3,
        radius: f64,
    },
}

impl Curve {
    /// Evaluate the curve at parameter t.
    /// For a line: origin + t * direction
    /// For a circle: center + radius * (cos(t) * u + sin(t) * v)
    pub fn evaluate(&self, t: f64) -> Point {
        match self {
            Curve::Line { origin, direction } => origin + t * direction,
            Curve::Circle {
                center,
                normal,
                radius,
            } => {
                // Build a local coordinate system from the normal
                let (u, v) = orthonormal_basis(normal);
                center + *radius * (t.cos() * u + t.sin() * v)
            }
        }
    }
}

exec
/bin/bash -lc "sed -n '1,320p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod stl;
pub use stl::to_ascii_stl;

use crate::brep::topology::Solid;
use crate::geometry::Point;
use serde::{Deserialize, Serialize};

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

/// Tessellate a B-rep solid into a triangle mesh.
///
/// Each face's outer loop is triangulated as a fan from its first vertex.
/// Normals are evaluated per vertex from the underlying surface, so curved
/// faces get correct shading rather than a single per-face normal.
///
/// Not yet handled: interior subdivision of curved faces (uv sampling) — a
/// curved face is still approximated by its boundary fan — and inner loops
/// (holes). Both are deferred to the curved-primitive work (Phase 3).
pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    for face in &solid.faces {
        let outer_loop = &solid.loops[face.outer_loop];

        // Collect the vertices of this face's outer loop
        let loop_vertices: Vec<&Point> = outer_loop
            .half_edges
            .iter()
            .map(|&he_idx| {
                let he = &solid.half_edges[he_idx];
                &solid.vertices[he.start_vertex].point
            })
            .collect();

        if loop_vertices.len() < 3 {
            continue;
        }

        // Add vertices and triangulate as a fan from vertex 0.
        // Normals are evaluated per vertex from the surface so curved faces
        // are shaded correctly (planar faces yield a constant normal).
        let base_idx = mesh.positions.len() as u32;

        for &v in &loop_vertices {
            let normal = face.surface.normal_at_point(v);
            let n = if face.same_sense {
                [normal.x, normal.y, normal.z]
            } else {
                [-normal.x, -normal.y, -normal.z]
            };
            mesh.positions.push([v.x, v.y, v.z]);
            mesh.normals.push(n);
        }

        for i in 1..(loop_vertices.len() as u32 - 1) {
            mesh.indices.push(base_idx);
            mesh.indices.push(base_idx + i);
            mesh.indices.push(base_idx + i + 1);
        }
    }

    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::primitives::make_cuboid;

    #[test]
    fn test_tessellate_cuboid() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
        let mesh = tessellate_solid(&solid);

        // A cuboid has 6 faces, each tessellated as 2 triangles = 12 triangles
        assert_eq!(mesh.triangle_count(), 12);
        // 6 faces * 4 vertices = 24 vertices (not shared across faces for correct normals)
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

        let mesh1 = tessellate_solid(&solid1);
        let mesh2 = tessellate_solid(&solid2);

        assert_eq!(mesh1.positions, mesh2.positions);
        assert_eq!(mesh1.normals, mesh2.normals);
        assert_eq!(mesh1.indices, mesh2.indices);
    }
}

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/geometry/surface.rs" in /home/bacon/mycad
 succeeded in 0ms:
use super::math::orthonormal_basis;
use super::{Point, Vec3};
use serde::{Deserialize, Serialize};

/// A geometric surface in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Surface {
    /// An infinite plane.
    Plane {
        origin: Point,
        normal: Vec3,
        u_axis: Vec3,
        v_axis: Vec3,
    },
    /// A cylindrical surface.
    Cylinder {
        origin: Point,
        axis: Vec3,
        radius: f64,
    },
    /// A spherical surface.
    Sphere { center: Point, radius: f64 },
    /// A conical surface.
    Cone {
        apex: Point,
        axis: Vec3,
        half_angle: f64,
    },
}

impl Surface {
    /// Evaluate the surface at parameters (u, v).
    pub fn evaluate(&self, u: f64, v: f64) -> Point {
        match self {
            Surface::Plane {
                origin,
                u_axis,
                v_axis,
                ..
            } => origin + u * u_axis + v * v_axis,

            Surface::Cylinder {
                origin,
                axis,
                radius,
            } => {
                let (bu, bv) = orthonormal_basis(axis);
                origin + *radius * (u.cos() * bu + u.sin() * bv) + v * axis.normalize()
            }

            Surface::Sphere { center, radius } => {
                // u = longitude [0, 2π), v = latitude [-π/2, π/2]
                let x = radius * v.cos() * u.cos();
                let y = radius * v.cos() * u.sin();
                let z = radius * v.sin();
                Point::new(center.x + x, center.y + y, center.z + z)
            }

            Surface::Cone {
                apex,
                axis,
                half_angle,
            } => {
                let (bu, bv) = orthonormal_basis(axis);
                let r = v * half_angle.tan();
                apex + v * axis.normalize() + r * (u.cos() * bu + u.sin() * bv)
            }
        }
    }

    /// Get the normal vector at parameters (u, v).
    pub fn normal_at(&self, u: f64, v: f64) -> Vec3 {
        match self {
            Surface::Plane { normal, .. } => normal.normalize(),

            Surface::Cylinder { axis, .. } => {
                let (bu, bv) = orthonormal_basis(axis);
                (u.cos() * bu + u.sin() * bv).normalize()
            }

            Surface::Sphere { center, .. } => {
                let p = self.evaluate(u, v);
                (p - center).normalize()
            }

            Surface::Cone {
                apex,
                axis,
                half_angle,
            } => {
                let p = self.evaluate(u, v);
                let a = axis.normalize();
                let to_point = p - apex;
                let along_axis = to_point.dot(&a) * a;
                let radial = to_point - along_axis;
                if radial.norm() < 1e-12 {
                    return a;
                }
                let radial_n = radial.normalize();
                (radial_n * half_angle.cos() - a * half_angle.sin()).normalize()
            }
        }
    }

    /// Inverse of [`Surface::evaluate`]: find the parameters (u, v) whose
    /// evaluation lands on (or nearest to) `p`.
    ///
    /// Used so that geometry-consuming algorithms (e.g. tessellation) can ask
    /// for the surface normal at a concrete 3D point without assuming the
    /// surface is planar / has a constant normal.
    pub fn uv_of(&self, p: &Point) -> (f64, f64) {
        match self {
            Surface::Plane {
                origin,
                u_axis,
                v_axis,
                ..
            } => {
                let d = p - origin;
                (
                    d.dot(u_axis) / u_axis.dot(u_axis),
                    d.dot(v_axis) / v_axis.dot(v_axis),
                )
            }

            Surface::Cylinder { origin, axis, .. } => {
                let a = axis.normalize();
                let (bu, bv) = orthonormal_basis(axis);
                let d = p - origin;
                let v = d.dot(&a);
                let radial = d - v * a;
                (radial.dot(&bv).atan2(radial.dot(&bu)), v)
            }

            Surface::Sphere { center, radius } => {
                let d = p - center;
                let v = (d.z / radius).clamp(-1.0, 1.0).asin();
                let u = d.y.atan2(d.x);
                (u, v)
            }

            Surface::Cone { apex, axis, .. } => {
                let a = axis.normalize();
                let (bu, bv) = orthonormal_basis(axis);
                let d = p - apex;
                let v = d.dot(&a);
                let radial = d - v * a;
                (radial.dot(&bv).atan2(radial.dot(&bu)), v)
            }
        }
    }

    /// Surface normal at a concrete 3D point lying on (or near) the surface.
    ///
    /// Convenience over [`Surface::uv_of`] + [`Surface::normal_at`] so callers
    /// never have to special-case planar vs curved surfaces.
    pub fn normal_at_point(&self, p: &Point) -> Vec3 {
        let (u, v) = self.uv_of(p);
        self.normal_at(u, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
        let p = surface.evaluate(u, v);
        let (u2, v2) = surface.uv_of(&p);
        assert_relative_eq!(u, u2, epsilon = 1e-9);
        assert_relative_eq!(v, v2, epsilon = 1e-9);
    }

    #[test]
    fn test_uv_of_roundtrip_plane() {
        let surface = Surface::Plane {
            origin: Point::new(1.0, 2.0, 3.0),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        assert_uv_roundtrip(&surface, 0.7, -1.3);
    }

    #[test]
    fn test_uv_of_roundtrip_cylinder() {
        let surface = Surface::Cylinder {
            origin: Point::new(0.0, 0.0, 0.0),
            axis: Vec3::z(),
            radius: 2.5,
        };
        assert_uv_roundtrip(&surface, 0.9, 1.7);
    }

    #[test]
    fn test_uv_of_roundtrip_sphere() {
        let surface = Surface::Sphere {
            center: Point::new(-1.0, 0.5, 2.0),
            radius: 3.0,
        };
        assert_uv_roundtrip(&surface, 1.1, 0.6);
    }

    #[test]
    fn test_uv_of_roundtrip_cone() {
        let surface = Surface::Cone {
            apex: Point::new(0.0, 0.0, 0.0),
            axis: Vec3::z(),
            half_angle: 0.5,
        };
        assert_uv_roundtrip(&surface, 0.8, 2.0);
    }

    /// A curved surface must report different normals at different points —
    /// the property the tessellator now relies on instead of a single
    /// per-face normal.
    #[test]
    fn test_sphere_normal_varies_per_point() {
        let surface = Surface::Sphere {
            center: Point::origin(),
            radius: 1.0,
        };
        let p1 = surface.evaluate(0.0, 0.0);
        let p2 = surface.evaluate(std::f64::consts::FRAC_PI_2, 0.3);
        let n1 = surface.normal_at_point(&p1);
        let n2 = surface.normal_at_point(&p2);
        // Distinct surface points yield non-parallel outward normals.
        assert!((n1.dot(&n2)).abs() < 0.99);
    }

    #[test]
    fn test_plane_normal_constant_per_point() {
        let surface = Surface::Plane {
            origin: Point::origin(),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        let n1 = surface.normal_at_point(&Point::new(5.0, -2.0, 0.0));
        let n2 = surface.normal_at_point(&Point::new(-3.0, 4.0, 0.0));
        assert_relative_eq!(n1.dot(&n2), 1.0, epsilon = 1e-12);
    }
}

codex
`make_cuboid` は `HalfEdge.start_vertex` の順序だけでループを表現しており、現状テッセレータもそこを前提にしています。次は B-rep 型定義と `build` 側の feature/history の扱いを見ます。
exec
/bin/bash -lc "sed -n '1,320p' crates/mycad-build/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
use mycad_format::Feature;
use mycad_kernel::brep::topology::{IdGenerator, Solid};
use mycad_kernel::error::KernelError;
use mycad_kernel::primitives::make_cuboid;

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
        Feature::CreateCylinder { .. } => Err(KernelError::UnsupportedFeature {
            kind: "create_cylinder",
        }),
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
/bin/bash -lc "sed -n '1,280p' crates/mycad-cli/src/main.rs" in /home/bacon/mycad
 succeeded in 0ms:
use clap::{Parser, Subcommand};
use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::tessellation::{tessellate_solid, to_ascii_stl};
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
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Export { input, output } => {
            if let Err(msg) = run_export(&input, &output) {
                eprintln!("error: {msg}");
                process::exit(1);
            }
        }
    }
}

fn run_export(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
    let doc = Document::from_path(input)
        .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
    let mut gen = IdGenerator::new(0);
    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
        .map_err(|e| format!("failed to build solid: {e}"))?;
    let mesh = tessellate_solid(&solid);
    let stl = to_ascii_stl(&mesh, "model");
    std::fs::write(output, stl)
        .map_err(|e| format!("failed to write {}: {e}", output.display()))?;
    Ok(())
}

exec
/bin/bash -lc "rg -n \"struct Solid|struct HalfEdge|struct Loop|struct Face|struct Shell|struct Edge|struct Vertex|IdGenerator|same_sense|inner_loops\" crates/mycad-kernel/src -g '"'!target'"'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-kernel/src/tessellation/mod.rs:76:            let n = if face.same_sense {
crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/stl.rs:51:    use crate::brep::topology::IdGenerator;
crates/mycad-kernel/src/tessellation/stl.rs:57:        let mut g = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/stl.rs:74:            let mut g = IdGenerator::new(0);
crates/mycad-kernel/src/brep/topology.rs:12:pub struct Vertex {
crates/mycad-kernel/src/brep/topology.rs:21:pub struct HalfEdge {
crates/mycad-kernel/src/brep/topology.rs:34:pub struct Edge {
crates/mycad-kernel/src/brep/topology.rs:46:pub struct Loop {
crates/mycad-kernel/src/brep/topology.rs:55:pub struct Face {
crates/mycad-kernel/src/brep/topology.rs:62:    pub inner_loops: Vec<usize>,
crates/mycad-kernel/src/brep/topology.rs:64:    pub same_sense: bool,
crates/mycad-kernel/src/brep/topology.rs:69:pub struct Shell {
crates/mycad-kernel/src/brep/topology.rs:80:pub struct Solid {
crates/mycad-kernel/src/brep/topology.rs:160:        inner_loops: Vec<usize>,
crates/mycad-kernel/src/brep/topology.rs:161:        same_sense: bool,
crates/mycad-kernel/src/brep/topology.rs:168:            inner_loops,
crates/mycad-kernel/src/brep/topology.rs:169:            same_sense,
crates/mycad-kernel/src/brep/topology.rs:185:pub struct IdGenerator {
crates/mycad-kernel/src/brep/topology.rs:189:impl IdGenerator {
crates/mycad-kernel/src/brep/topology.rs:208:        let mut gen1 = IdGenerator::new(100);
crates/mycad-kernel/src/brep/topology.rs:209:        let mut gen2 = IdGenerator::new(100);
crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
crates/mycad-kernel/src/primitives/cuboid.rs:87:    struct FaceDef {
crates/mycad-kernel/src/primitives/cuboid.rs:91:        same_sense: bool,
crates/mycad-kernel/src/primitives/cuboid.rs:109:            same_sense: true,
crates/mycad-kernel/src/primitives/cuboid.rs:125:            same_sense: true,
crates/mycad-kernel/src/primitives/cuboid.rs:141:            same_sense: true,
crates/mycad-kernel/src/primitives/cuboid.rs:157:            same_sense: true,
crates/mycad-kernel/src/primitives/cuboid.rs:173:            same_sense: true,
crates/mycad-kernel/src/primitives/cuboid.rs:189:            same_sense: true,
crates/mycad-kernel/src/primitives/cuboid.rs:210:            face_def.same_sense,
crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);

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
/bin/bash -lc "sed -n '1,320p' crates/mycad-kernel/src/brep/topology.rs" in /home/bacon/mycad
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
/bin/bash -lc "sed -n '1,240p' crates/mycad-kernel/src/error.rs" in /home/bacon/mycad
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
}

exec
/bin/bash -lc "sed -n '1,240p' crates/mycad-kernel/Cargo.toml" in /home/bacon/mycad
 succeeded in 0ms:
[package]
name = "mycad-kernel"
description = "B-rep geometry kernel for MyCad"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
nalgebra = { workspace = true }
serde = { workspace = true }
thiserror = { workspace = true }
indexmap = { workspace = true }

[dev-dependencies]
approx = { workspace = true }

[[bench]]
name = "tessellation"
harness = false

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-format/Cargo.toml" in /home/bacon/mycad
 succeeded in 0ms:
[package]
name = "mycad-format"
description = "YAML schema definition and parser for MyCad feature files"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
serde_yaml = { workspace = true }
schemars = { workspace = true }
thiserror = { workspace = true }
indexmap = { workspace = true }

[dev-dependencies]
approx = { workspace = true }

codex
ここまでで、既存 B-rep はフラット配列+`start_vertex`/`forward` の最小表現で、`Face` は `inner_loops` を持っています。`mycad-format` 側のシリアライズ/スキーマ規約も確認して、テスト計画の抜けと API 設計の齟齬を洗います。
exec
/bin/bash -lc "sed -n '1,320p' crates/mycad-format/src/lib.rs" in /home/bacon/mycad
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
/bin/bash -lc 'rg -n "enum Feature|CreateCylinder|JsonSchema|Serialize|Deserialize|to_yaml|from_yaml|from_path|Document" crates/mycad-format/src crates/mycad-build tests examples' in /home/bacon/mycad
 exited 2 in 0ms:
rg: tests: No such file or directory (os error 2)
crates/mycad-format/src/lib.rs:7:pub use document::Document;
crates/mycad-format/src/feature.rs:1:use schemars::JsonSchema;
crates/mycad-format/src/feature.rs:2:use serde::{Deserialize, Serialize};
crates/mycad-format/src/feature.rs:5:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/feature.rs:14:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/feature.rs:16:pub enum Feature {
crates/mycad-format/src/feature.rs:28:    CreateCylinder {
crates/mycad-format/src/feature.rs:76:            | Feature::CreateCylinder { id, .. }
crates/mycad-format/src/component.rs:3:use serde::{Deserialize, Serialize};
crates/mycad-format/src/component.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
crates/mycad-format/src/component.rs:70:impl Serialize for ComponentRef {
crates/mycad-format/src/component.rs:71:    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
crates/mycad-format/src/component.rs:76:impl<'de> Deserialize<'de> for ComponentRef {
crates/mycad-format/src/component.rs:77:    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
crates/mycad-format/src/component.rs:83:impl schemars::JsonSchema for ComponentRef {
crates/mycad-format/src/component.rs:89:        <String as schemars::JsonSchema>::json_schema(gen)
crates/mycad-format/src/component.rs:96:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
crates/mycad-build/src/lib.rs:27:        Feature::CreateCylinder { .. } => Err(KernelError::UnsupportedFeature {
crates/mycad-format/src/document.rs:3:use schemars::JsonSchema;
crates/mycad-format/src/document.rs:4:use serde::{Deserialize, Serialize};
crates/mycad-format/src/document.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/document.rs:9:pub struct Document {
crates/mycad-format/src/document.rs:16:impl Document {
crates/mycad-format/src/document.rs:25:    /// Serialize to YAML string.
crates/mycad-format/src/document.rs:26:    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
crates/mycad-format/src/document.rs:30:    /// Deserialize from YAML string.
crates/mycad-format/src/document.rs:31:    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
crates/mycad-format/src/document.rs:35:    /// Load a Document from a `.mycad` file path.
crates/mycad-format/src/document.rs:36:    pub fn from_path(path: &Path) -> Result<Self, FormatError> {
crates/mycad-format/src/document.rs:42:        let doc = Self::from_yaml(&content)?;
crates/mycad-format/src/document.rs:56:        let mut doc = Document::new("Test Assembly");
crates/mycad-format/src/document.rs:63:        let yaml = doc.to_yaml().unwrap();
crates/mycad-format/src/document.rs:64:        let doc2 = Document::from_yaml(&yaml).unwrap();
crates/mycad-format/src/document.rs:76:        let mut doc = Document::new("Test");
crates/mycad-format/src/document.rs:84:        let yaml1 = doc.to_yaml().unwrap();
crates/mycad-format/src/document.rs:85:        let yaml2 = doc.to_yaml().unwrap();
crates/mycad-format/src/document.rs:90:    fn test_from_path_loads_simple_box() {
crates/mycad-format/src/document.rs:96:        let doc = Document::from_path(&path).unwrap();
crates/mycad-format/src/document.rs:102:    fn test_from_path_missing_file_returns_io_error() {
crates/mycad-format/src/document.rs:104:        let err = Document::from_path(path).unwrap_err();
crates/mycad-format/src/document.rs:109:    fn test_from_path_invalid_extension() {
crates/mycad-format/src/document.rs:111:        let err = Document::from_path(path).unwrap_err();
crates/mycad-format/src/document.rs:124:        let doc = Document::from_path(&path).unwrap();
crates/mycad-format/src/document.rs:138:        let yaml = doc.to_yaml().unwrap();
crates/mycad-format/src/document.rs:139:        let doc2 = Document::from_yaml(&yaml).unwrap();
crates/mycad-format/src/document.rs:140:        assert_eq!(doc.to_yaml().unwrap(), doc2.to_yaml().unwrap());
crates/mycad-build/tests/feature_dispatcher.rs:2:use mycad_format::Document;
crates/mycad-build/tests/feature_dispatcher.rs:14:    let doc = Document::from_path(&path).expect("load .mycad");

exec
/bin/bash -lc "sed -n '1,240p' crates/mycad-kernel/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod brep;
pub mod error;
pub mod geometry;
pub mod primitives;
pub mod tessellation;

pub use brep::topology;
pub use error::KernelError;

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-format/src/feature.rs" in /home/bacon/mycad
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

exec
/bin/bash -lc 'rg -n "golden|roundtrip|tessellate_solid_with|TessellationOptions|export" crates/mycad-cli tests crates/mycad-kernel crates/mycad-build' in /home/bacon/mycad
 exited 2 in 0ms:
rg: tests: No such file or directory (os error 2)
crates/mycad-cli/tests/export.rs:5:fn export_simple_box_produces_12_facets() {
crates/mycad-cli/tests/export.rs:16:        .arg("export")
crates/mycad-cli/tests/export.rs:21:        .expect("run mycad export");
crates/mycad-cli/tests/export.rs:22:    assert!(status.success(), "export should succeed");
crates/mycad-cli/src/main.rs:32:            if let Err(msg) = run_export(&input, &output) {
crates/mycad-cli/src/main.rs:40:fn run_export(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
crates/mycad-kernel/src/geometry/surface.rs:168:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
crates/mycad-kernel/src/geometry/surface.rs:176:    fn test_uv_of_roundtrip_plane() {
crates/mycad-kernel/src/geometry/surface.rs:183:        assert_uv_roundtrip(&surface, 0.7, -1.3);
crates/mycad-kernel/src/geometry/surface.rs:187:    fn test_uv_of_roundtrip_cylinder() {
crates/mycad-kernel/src/geometry/surface.rs:193:        assert_uv_roundtrip(&surface, 0.9, 1.7);
crates/mycad-kernel/src/geometry/surface.rs:197:    fn test_uv_of_roundtrip_sphere() {
crates/mycad-kernel/src/geometry/surface.rs:202:        assert_uv_roundtrip(&surface, 1.1, 0.6);
crates/mycad-kernel/src/geometry/surface.rs:206:    fn test_uv_of_roundtrip_cone() {
crates/mycad-kernel/src/geometry/surface.rs:212:        assert_uv_roundtrip(&surface, 0.8, 2.0);

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

codex
設計書は既存コードの延長上にありますが、いくつかは現行 API とぶつかる可能性があります。ADR と既存ドキュメントの規約を軽く確認して、レビュー所見を確定します。
exec
/bin/bash -lc "rg -n \"ADR-001|ADR-004|Feature history|source of truth|index-based|mod.rs|pub use\" -g '*.md' -g '*.rs' ." in /home/bacon/mycad
 succeeded in 0ms:
./ROADMAP.md:74:**備考**: 曲面同士の Boolean は交線として自由曲線を、フィレット・面取りは自由曲面を必要とする (交線は一般に円・直線でない)。自由曲面/NURBS は専用 Phase を立てず、本 Phase が要求する範囲から漸進的に導入する。数値モデル (トレラント vs 厳密) もここで決定する。詳細は ADR-004 参照。
./crates/mycad-format/src/lib.rs:6:pub use component::Component;
./crates/mycad-format/src/lib.rs:7:pub use document::Document;
./crates/mycad-format/src/lib.rs:8:pub use error::FormatError;
./crates/mycad-format/src/lib.rs:9:pub use feature::Feature;
./docs/decisions/001-brep-representation.md:1:# ADR-001: B-rep (Boundary Representation) の採用
./docs/decisions/001-brep-representation.md:28:- トポロジーエンティティはフラット配列に格納（index-based）
./crates/mycad-kernel/src/lib.rs:7:pub use brep::topology;
./crates/mycad-kernel/src/lib.rs:8:pub use error::KernelError;
./features/12-cylinder/design-review.md:49:- Feature history = source of truth の原則と矛盾しないか
./features/12-cylinder/design-review.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review.md:83:- テッセレータは ADR-004 準拠で **variant を直接 `match` しない**。型ごとの分岐は `Surface`/`Curve` の多態メソッド（網羅 match はコンパイラが強制）に閉じ込め、アルゴリズム本体は型非依存に保つ。
./features/12-cylinder/design-review.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
./features/12-cylinder/design-review.md:94:  - `crates/mycad-kernel/src/tessellation/mod.rs` — 曲線折れ線化 + 曲面 UV グリッド対応 + `TessellationOptions`
./features/12-cylinder/design-review.md:98:  - テスト: `cylinder.rs` 内インライン / `tessellation/mod.rs` 内 / `mycad-build/tests/feature_dispatcher.rs` / `mycad-cli/tests/export.rs`
./features/12-cylinder/design-review.md:129:### 2. テッセレータ（ADR-004 準拠の加算拡張）
./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/12-cylinder/design-review.md:274:crates/mycad-kernel/src/tessellation/mod.rs
./features/12-cylinder/design-review.md:279:crates/mycad-kernel/src/primitives/mod.rs
./features/12-cylinder/design-review.md:302:crates/mycad-kernel/src/brep/mod.rs
./features/12-cylinder/design-review.md:308:crates/mycad-kernel/src/geometry/mod.rs
./features/12-cylinder/design-review.md:427:CLAUDE.md:57:- **モジュール構造**: 1概念1ファイル。重要な型は `mod.rs` から `pub use` で再エクスポート。
./features/12-cylinder/design-review.md:435:docs/decisions/004-freeform-geometry-commitment.md:1:# ADR-004: 自由曲面・自由曲線を確定要件として扱う
./features/12-cylinder/design-review.md:440:ROADMAP.md:74:**備考**: 曲面同士の Boolean は交線として自由曲線を、フィレット・面取りは自由曲面を必要とする (交線は一般に円・直線でない)。自由曲面/NURBS は専用 Phase を立てず、本 Phase が要求する範囲から漸進的に導入する。数値モデル (トレラント vs 厳密) もここで決定する。詳細は ADR-004 参照。
./features/12-cylinder/design-review.md:447:features/12-cylinder/design-review.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review.md:451:features/12-cylinder/design-review.md:83:- テッセレータは ADR-004 準拠で **variant を直接 `match` しない**。型ごとの分岐は `Surface`/`Curve` の多態メソッド（網羅 match はコンパイラが強制）に閉じ込め、アルゴリズム本体は型非依存に保つ。
./features/12-cylinder/design-review.md:452:features/12-cylinder/design-review.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
./features/12-cylinder/design-review.md:456:features/12-cylinder/design-review.md:129:### 2. テッセレータ（ADR-004 準拠の加算拡張）
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
./features/12-cylinder/design-review.md:1926:# ADR-001: B-rep (Boundary Representation) の採用
./features/12-cylinder/design-review.md:1953:- トポロジーエンティティはフラット配列に格納（index-based）
./features/12-cylinder/design-review.md:1960:# ADR-004: 自由曲面・自由曲線を確定要件として扱う
./features/12-cylinder/design-review.md:1976:- **フィレット・面取り (ADR-001 が動機として挙げた操作)**: 一般のフィレット面や掃引面は解析式に乗らない。→ 自由**曲面** (NURBS 等) が必須。
./features/12-cylinder/design-review.md:1981:- ADR-001 のとおり **Feature history が真実の源で、B-rep は Feature から再生成される**。`.mycad` に永続化されるのは Feature 列であり B-rep ではないため、`Surface` / `Curve` / `Edge` の内部表現は後で改修してもファイル移行が不要。後付けコストが構造的に低い。
./features/12-cylinder/design-review.md:1998:- 本 ADR は ADR-001 (B-rep 採用) を拡張する位置づけ。番号 003 は ADR-002 で Viewer スタック用に予約済みのため 004 を使用。
./features/12-cylinder/design-review.md:2013:pub use brep::topology;
./features/12-cylinder/design-review.md:2014:pub use error::KernelError;
./features/12-cylinder/design-review.md:2017:/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/geometry/mod.rs" in /home/bacon/mycad
./features/12-cylinder/design-review.md:2096:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./features/12-cylinder/design-review.md:2490:- Feature history = source of truth の原則と矛盾しないか
./features/12-cylinder/design-review.md:2491:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review.md:2519:    section: "設計方針 > 2. テッセレータ（ADR-004 準拠の加算拡張）"
./features/12-cylinder/design-review.md:2549:    section: "設計方針 > 2. テッセレータ（ADR-004 準拠の加算拡張）"
./crates/mycad-kernel/CLAUDE.md:24:3. `primitives/mod.rs` に `mod` と `pub use` を追加
./docs/decisions/004-freeform-geometry-commitment.md:1:# ADR-004: 自由曲面・自由曲線を確定要件として扱う
./docs/decisions/004-freeform-geometry-commitment.md:17:- **フィレット・面取り (ADR-001 が動機として挙げた操作)**: 一般のフィレット面や掃引面は解析式に乗らない。→ 自由**曲面** (NURBS 等) が必須。
./docs/decisions/004-freeform-geometry-commitment.md:22:- ADR-001 のとおり **Feature history が真実の源で、B-rep は Feature から再生成される**。`.mycad` に永続化されるのは Feature 列であり B-rep ではないため、`Surface` / `Curve` / `Edge` の内部表現は後で改修してもファイル移行が不要。後付けコストが構造的に低い。
./docs/decisions/004-freeform-geometry-commitment.md:39:- 本 ADR は ADR-001 (B-rep 採用) を拡張する位置づけ。番号 003 は ADR-002 で Viewer スタック用に予約済みのため 004 を使用。
./crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./crates/mycad-kernel/src/tessellation/mod.rs:2:pub use stl::to_ascii_stl;
./.claude/skills/3ai/agents/codex-design-reviewer.md:36:- Feature history = source of truth の原則と矛盾しないか
./.claude/skills/3ai/agents/codex-design-reviewer.md:37:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-2.md:49:- Feature history = source of truth の原則と矛盾しないか
./features/12-cylinder/design-review-2.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
./features/12-cylinder/design-review-2.md:83:- テッセレータは ADR-004 準拠で **variant を直接 `match` しない**。型ごとの分岐は `Surface`/`Curve` の多態メソッド（網羅 match はコンパイラが強制）に閉じ込め、アルゴリズム本体は型非依存に保つ。
./features/12-cylinder/design-review-2.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
./features/12-cylinder/design-review-2.md:94:  - `crates/mycad-kernel/src/tessellation/mod.rs` — 曲線折れ線化 + 曲面 UV グリッド対応 + `TessellationOptions`
./features/12-cylinder/design-review-2.md:98:  - テスト: `cylinder.rs` 内インライン / `tessellation/mod.rs` 内 / `mycad-build/tests/feature_dispatcher.rs` / `mycad-cli/tests/export.rs`
./features/12-cylinder/design-review-2.md:129:### 2. テッセレータ（ADR-004 準拠の加算拡張）
./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
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
./CLAUDE.md:57:- **モジュール構造**: 1概念1ファイル。重要な型は `mod.rs` から `pub use` で再エクスポート。
./CLAUDE.md:63:- **Feature history = source of truth**: `mycad-format` の Feature 列が設計の真実の源。B-rep は Feature から再生成される。

exec
/bin/bash -lc "find . -maxdepth 3 -type f | rg 'ADR|adr|docs|design|architecture'" in /home/bacon/mycad
 succeeded in 0ms:
./features/12-cylinder/design-review-2.md
./features/12-cylinder/design-review.md
./docs/decisions/002-roadmap-management.md
./docs/decisions/004-freeform-geometry-commitment.md
./docs/decisions/001-brep-representation.md
./docs/file-format.md

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/geometry/math.rs" in /home/bacon/mycad
 succeeded in 0ms:
use super::Vec3;

/// Build an orthonormal basis (u, v) from a normal vector.
///
/// Given a normal vector, computes two perpendicular unit vectors
/// that together with the normal form a right-handed coordinate system.
pub fn orthonormal_basis(normal: &Vec3) -> (Vec3, Vec3) {
    let n = normal.normalize();
    // Choose a vector not parallel to n
    let not_parallel = if n.x.abs() < 0.9 {
        Vec3::x()
    } else {
        Vec3::y()
    };
    let u = n.cross(&not_parallel).normalize();
    let v = n.cross(&u);
    (u, v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_orthonormal_basis_z_axis() {
        let (u, v) = orthonormal_basis(&Vec3::z());
        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&Vec3::z()), 0.0, epsilon = 1e-12);
    }

    #[test]
    fn test_orthonormal_basis_x_axis() {
        let (u, v) = orthonormal_basis(&Vec3::x());
        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&Vec3::x()), 0.0, epsilon = 1e-12);
    }
}

codex
レビュー項目はほぼ固まりました。既存コードとの照合上、重大なのは「汎用曲面テッセレーションの抽象が cylinder 専用の仮定を漏らしている点」と「Euler テストが実体を検証していない点」です。最終的に YAML だけ返すので、最後に指摘を絞ります。
exec
/bin/bash -lc "sed -n '1,140p' docs/decisions/001-brep-representation.md" in /home/bacon/mycad
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
/bin/bash -lc 'rg -n "issues:|verdict:" features/12-cylinder/design-review*.md' in /home/bacon/mycad
 succeeded in 0ms:
features/12-cylinder/design-review.md:57:issues:
features/12-cylinder/design-review.md:67:verdict: pass  # pass | fail
features/12-cylinder/design-review.md:71:`issues` が空の場合は `issues: []` と書く。
features/12-cylinder/design-review.md:2498:issues:
features/12-cylinder/design-review.md:2508:verdict: pass  # pass | fail
features/12-cylinder/design-review.md:2512:`issues` が空の場合は `issues: []` と書く。
features/12-cylinder/design-review.md:2516:issues:
features/12-cylinder/design-review.md:2543:verdict: fail
features/12-cylinder/design-review.md:2546:issues:
features/12-cylinder/design-review.md:2573:verdict: fail
features/12-cylinder/design-review-2.md:57:issues:
features/12-cylinder/design-review-2.md:67:verdict: pass  # pass | fail
features/12-cylinder/design-review-2.md:71:`issues` が空の場合は `issues: []` と書く。

exec
/bin/bash -lc "sed -n '1,120p' docs/decisions/004-freeform-geometry-commitment.md" in /home/bacon/mycad
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
/bin/bash -lc "rg -n \"決定性|determin|same input|IdGenerator|HashMap|thread_rng|cross-platform|float\" CLAUDE.md crates/mycad-kernel/CLAUDE.md docs -g '*.md'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-kernel/CLAUDE.md:18:- EntityIDは `IdGenerator` で決定的に生成される
crates/mycad-kernel/CLAUDE.md:23:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
crates/mycad-kernel/CLAUDE.md:25:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）
CLAUDE.md:20:設計原則: **決定性** (同じ入力は常に同じ出力)、**パラメトリック履歴** (feature historyが真実の源)、**YAML-based format** (人間可読・diff可能)。
CLAUDE.md:53:- **決定性**: すべてのEntityIDは `IdGenerator` で決定的に生成。テストでは決定性を検証すること。
docs/decisions/004-freeform-geometry-commitment.md:30:3. **数値モデル (トレラント方式 vs 厳密方式) は本 ADR では保留**し、曲面 Boolean を実装する Phase 4 着手時に決定する。自由曲面の交線は数値的近似になるため、業界カーネル (Parasolid / ACIS) はエンティティ毎に公差を持つトレラントモデルを採る。これを後から全エンティティ・全比較へ導入するのは大改修であり、Phase 4 で意思決定する論点として明示しておく。なお `CLAUDE.md` の **決定性** 原則とは両立可能 (アルゴリズム固定で再現できる) だが、数値ロバスト性の確保が決定性維持の難所になる点に留意する。

exec
/bin/bash -lc "sed -n '1,120p' crates/mycad-kernel/CLAUDE.md" in /home/bacon/mycad
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
既存 cuboid の面順序も確認しました。設計書の「cuboid は既に外向きでスワップ不要」という前提はかなり怪しく、ここは重大な回帰前提のズレとして扱います。
exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/tessellation/stl.rs" in /home/bacon/mycad
 succeeded in 0ms:
use super::TriangleMesh;
use std::fmt::Write;

pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
    let tri_count = mesh.triangle_count();
    let mut out = String::with_capacity(64 + tri_count * 200);

    writeln!(out, "solid {}", solid_name).unwrap();

    for tri in 0..tri_count {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;

        let p0 = mesh.positions[i0];
        let p1 = mesh.positions[i1];
        let p2 = mesh.positions[i2];

        let n = face_normal(p0, p1, p2);

        writeln!(out, "  facet normal {} {} {}", n[0], n[1], n[2]).unwrap();
        writeln!(out, "    outer loop").unwrap();
        writeln!(out, "      vertex {} {} {}", p0[0], p0[1], p0[2]).unwrap();
        writeln!(out, "      vertex {} {} {}", p1[0], p1[1], p1[2]).unwrap();
        writeln!(out, "      vertex {} {} {}", p2[0], p2[1], p2[2]).unwrap();
        writeln!(out, "    endloop").unwrap();
        writeln!(out, "  endfacet").unwrap();
    }

    writeln!(out, "endsolid {}", solid_name).unwrap();
    out
}

fn face_normal(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3]) -> [f64; 3] {
    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let nx = u[1] * v[2] - u[2] * v[1];
    let ny = u[2] * v[0] - u[0] * v[2];
    let nz = u[0] * v[1] - u[1] * v[0];
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len > 0.0 {
        [nx / len, ny / len, nz / len]
    } else {
        [0.0, 0.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::primitives::make_cuboid;
    use crate::tessellation::tessellate_solid;

    #[test]
    fn ascii_stl_cuboid_structure() {
        let mut g = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
        let mesh = tessellate_solid(&solid);
        let stl = to_ascii_stl(&mesh, "test_cube");

        assert!(stl.starts_with("solid test_cube\n"));
        assert!(stl.trim_end().ends_with("endsolid test_cube"));
        assert_eq!(stl.matches("facet normal").count(), 12);
        assert_eq!(stl.matches("endfacet").count(), 12);
        assert_eq!(stl.matches("outer loop").count(), 12);
        assert_eq!(stl.matches("endloop").count(), 12);
        assert_eq!(stl.matches("\n      vertex ").count(), 36);
    }

    #[test]
    fn ascii_stl_deterministic() {
        let run = || {
            let mut g = IdGenerator::new(0);
            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
            to_ascii_stl(&tessellate_solid(&s), "cube")
        };
        assert_eq!(run(), run());
    }
}

exec
/bin/bash -lc "sed -n '260,340p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
 succeeded in 0ms:
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
issues:
  - id: R01
    severity: high
    section: "設計方針 > 2. テッセレータ（ADR-004 準拠の加算拡張）"
    finding: "`Surface::requires_interior_sampling()` の二値分岐だけで全ての非平面 face を UV 矩形グリッド化する設計になっており、`Cylinder` 以外の `Sphere`/`Cone`、inner loop 付き face、将来の trim face でも同じ経路に流れて誤メッシュを静かに生成する。ADR-004 が未決論点として挙げる pcurve/trim 情報もこの抽象では表現できない。"
    suggestion: "曲面内部分割の経路を今回の full cylinder side face に限定するか、少なくとも face/surface 側に周期性・特異点・trim 領域を返す抽象を追加し、未対応な curved face は明示エラーにすること。"
  - id: R02
    severity: high
    section: "設計方針 > 2. テッセレータ（ADR-004 準拠の加算拡張） > STL 外向き保証"
    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
    suggestion: "この変更を意図的な winding 修正として明記し、`T08` を『counts は維持するが indices / STL facet normal は変わり得る』前提に更新するか、先に cuboid 側の loop orientation を正規化して本当に不変にすること。"
  - id: R03
    severity: medium
    section: "テスト計画 > T03"
    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
    suggestion: "実際の `solid.vertices/edges/faces/shells` と hole 数から `V - E + F` と `2(S - H)` を計算して比較するテストに修正すること。"
  - id: R04
    severity: medium
    section: "設計方針 > 1. トポロジー（`make_cylinder`） > 退化入力の検証（R02）"
    finding: "退化入力の排除が `build_solid_from_features` 境界にしかなく、公開 API `make_cylinder(radius, height, id_gen) -> Solid` 自体は `radius<=0` / `height<=0` を受けても不正 B-rep を返せてしまう。`mycad-build` を経由しない kernel 利用では不変条件を守れない。"
    suggestion: "公開 API で `Result<Solid, KernelError>` を返すか、unchecked constructor を非公開化して validated wrapper だけを公開すること。少なくとも kernel 単体で invalid input を弾く経路を持たせること。"

verdict: fail
tokens used
75,980
issues:
  - id: R01
    severity: high
    section: "設計方針 > 2. テッセレータ（ADR-004 準拠の加算拡張）"
    finding: "`Surface::requires_interior_sampling()` の二値分岐だけで全ての非平面 face を UV 矩形グリッド化する設計になっており、`Cylinder` 以外の `Sphere`/`Cone`、inner loop 付き face、将来の trim face でも同じ経路に流れて誤メッシュを静かに生成する。ADR-004 が未決論点として挙げる pcurve/trim 情報もこの抽象では表現できない。"
    suggestion: "曲面内部分割の経路を今回の full cylinder side face に限定するか、少なくとも face/surface 側に周期性・特異点・trim 領域を返す抽象を追加し、未対応な curved face は明示エラーにすること。"
  - id: R02
    severity: high
    section: "設計方針 > 2. テッセレータ（ADR-004 準拠の加算拡張） > STL 外向き保証"
    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
    suggestion: "この変更を意図的な winding 修正として明記し、`T08` を『counts は維持するが indices / STL facet normal は変わり得る』前提に更新するか、先に cuboid 側の loop orientation を正規化して本当に不変にすること。"
  - id: R03
    severity: medium
    section: "テスト計画 > T03"
    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
    suggestion: "実際の `solid.vertices/edges/faces/shells` と hole 数から `V - E + F` と `2(S - H)` を計算して比較するテストに修正すること。"
  - id: R04
    severity: medium
    section: "設計方針 > 1. トポロジー（`make_cylinder`） > 退化入力の検証（R02）"
    finding: "退化入力の排除が `build_solid_from_features` 境界にしかなく、公開 API `make_cylinder(radius, height, id_gen) -> Solid` 自体は `radius<=0` / `height<=0` を受けても不正 B-rep を返せてしまう。`mycad-build` を経由しない kernel 利用では不変条件を守れない。"
    suggestion: "公開 API で `Result<Solid, KernelError>` を返すか、unchecked constructor を非公開化して validated wrapper だけを公開すること。少なくとも kernel 単体で invalid input を弾く経路を持たせること。"

verdict: fail
