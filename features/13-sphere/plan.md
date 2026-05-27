# Issue #13: 球プリミティブ + 球面テッセレーション

## Context

Phase 3「円柱・球・押し出し」の一環。#12 円柱が確立した「曲面サンプリング土台」の上に
球プリミティブを**加算的に**追加する (ADR-004 準拠: 他 variant の動作を壊さない)。

完了条件 (Issue #13):
- `mycad export examples/sphere.mycad -o out.stl` が STL を出力し MeshLab 等で読める
- 決定性テスト通過
- `cargo xtask ci` 通過

設計原則: **データ層 (.mycad / B-rep) は厳密な球** (`Surface::Sphere`) として保持し解像度に依存しない。
分割数はテッセレーション (STL 出力 / ビューア) 時のみの関心事。

## 設計方針

### トポロジー (標準的な UV 球)
- **頂点 2**: 南極 `center + (0,0,-r)`、北極 `center + (0,0,+r)` (center=原点)
- **エッジ 1**: メリディアン seam (+X 経線、XZ 平面内の半円)。**具体パラメータを固定**:
  - `Curve::Circle { center: origin, normal: -Vec3::y(), radius: r }`、`t_range = [π, 2π]`
  - 検算 (curve.rs / orthonormal_basis(-Y) より): `evaluate(π)=南極(0,0,-r)`、`evaluate(3π/2)=+X赤道(r,0,0)`、`evaluate(2π)=北極(0,0,r)`
  - `add_edge(id, [v_south, v_north], 上記 Circle, [π, 2π])` (t_range[0]→v_south と整合)
  - surface の u=0(+X)経線と seam が一致するため整合的
- **面 1**: `Surface::Sphere { center: origin, radius }`、`same_sense: true`、`inner_loops: vec![]`
- **shell 1**: closed
- **ループ**: `[he_seam_up (forward, start=南極), he_seam_down (reversed, start=北極)]` の 2-HalfEdge
  - ループ閉性: 末尾 HE 終点(南極) = 先頭 HE 始点(南極) ✓
  - 各エッジ 2 HalfEdge・逆向き ✓
- **Euler-Poincaré**: V−E+F = 2−1+1 = 2 = 2(S−H) (S=1,H=0) ✓
- 極は seam 両端の頂点。seam の曲線ジオメトリはトポロジー妥当性と幾何テストのみで使用。
  **テッセレーションは seam を経由せず surface を直接サンプルする**ため結合は最小。
- **(R04) self-adjacent periodic face を不変条件として明示許容**: full sphere は 1 本の seam edge に
  正逆 2 HalfEdge を載せる自己隣接面。これをカーネルの正当な表現として認め、専用妥当性テスト (T16) を置く。
  `crates/mycad-kernel/CLAUDE.md` の不変条件節にこの許容を 1 行追記する。

### 決定性
- `make_sphere(radius, id_gen)` で `id_gen.next()` を固定順に消費:
  solid → v_south → v_north → e_seam → he_up → he_down → loop → face → shell
- 同一 `IdGenerator` 初期値で 2 回生成 → 全 ID・座標一致 (cylinder T01 を踏襲)

### 退化幾何・エラー
- `radius` が非有限 or `<= 0` → `KernelError::InvalidParameter { kind: "radius" }` (cylinder と同形)

### テッセレーション (球専用処理を新規追加)
- `TessellationStrategy` に **`UvSphere` variant を新設**
- `Surface::Sphere` の `tessellation_strategy()` を `Unsupported` → `UvSphere` に変更。**`Cone` は `Unsupported` のまま**
- 新関数 `tessellate_face_sphere`:

  **(R02) canonical 球 face の検証 — 満たさなければ `TrimmedFaceUnsupported` を返す (強化版):**
  - `face.inner_loops` が空
  - outer loop が 2 HalfEdge・両方が同一 Edge を参照・その Edge が `Curve::Circle`
  - 2 HalfEdge が同一 edge の正逆 (`forward` が true/false 各 1)
  - 各 HalfEdge の `start_vertex` が `forward` と `edge.vertices` に整合 (forward なら start=vertices[0]、reverse なら start=vertices[1])
  - loop が閉じている (末尾 HE 終点 = 先頭 HE 始点)
  - `edge.vertices` の 2 頂点が両極 (`surface.uv_of` の v が ±π/2 近傍、tol 1e-9)
  - seam `Circle` の center/radius が sphere の center/radius と一致し、`t_range` が期待形 (半周) に入る
  - **(round3 R01)** `Circle.normal == -Vec3::y()` (canonical seam 向き) かつ
    `curve.evaluate(t_range[0]) ≈ edge.vertices[0]` / `evaluate(t_range[1]) ≈ edge.vertices[1]` (tol 1e-9) を検証 —
    別の大円 seam や頂点と不整合な t_range を full sphere として受理しない
  - いずれか満たさなければ `TrimmedFaceUnsupported` を返し、非 canonical な球面 face を silent に full sphere 化しない

  **(R01) 極を明示頂点へスナップし極は fan で閉じる (f64 で cos(π/2)≠0 のため格子任せにしない):**
  - `n_u = angular_segments.max(3)` (経度), `n_v = (angular_segments / 2).max(2)` (緯度=stack 数)
  - 南北極は `center ± (0,0,r)` を**厳密に 1 頂点ずつ**生成 (surface.evaluate(_, ±π/2) は使わない)
  - 内部リング `iv = 1..=n_v-1` のみ格子化: `v = -π/2 + (π/n_v)*iv`、各リング `n_u` 点 (周期、iu=0..n_u-1、`(iu+1)%n_u` で巻く)、`surface.evaluate/normal_at` でサンプル
  - 三角形生成:
    - 南極 fan: 南極 → 最下内部リング (`n_u` 枚)
    - 中間バンド `n_v-2` 本: 隣接リング間を quad=2三角形 (`2*n_u` 枚/バンド)
    - 北極 fan: 最上内部リング → 北極 (`n_u` 枚)
  - **(R03) winding を外向きに固定**: `to_ascii_stl` は `mesh.normals` でなく三角形 winding から facet normal を再計算するため、
    三角形の index 順 (winding) が外向き法線を生むよう構成する。`same_sense=false` の場合は法線符号だけでなく
    **三角形 index 順も反転**させ、STL が inside-out にならないようにする。`push_triangle` を使う (退化は原則発生しない)
- 円柱用 `tessellate_face_uv_grid` には**一切手を入れない** (脆弱な full-rev ガードを球で流用しない)

### (round3 R02) 解像度契約を明記
- **sphere は `angular_segments` のみを使い `axial_segments` は無視する** (緯度=stack 数は `(angular/2).max(2)` で内部導出)。
  `tessellate_solid_with` の解像度契約が surface 種別ごとに異なることを設計判断として固定。
- この契約を `TessellationOptions` のドキュメントコメントに明記し、CLI/API 既定値 (angular=32) とテストをこれに整合させる。

### (round3 R03) self-adjacent 妥当性 helper
- `CLAUDE.md` 追記だけに頼らず、`crates/mycad-kernel/src/brep/topology.rs` に
  機械的検証 helper (例 `Solid::validate_manifold()` 相当) を追加し、edge↔HE 対応・loop 閉性・face/shell 整合を検証。
  self-adjacent periodic face を許容する形で実装し、T16 はこの helper を呼ぶ。topology.rs のコメントも更新。

### (R04) facet 数はアルゴリズムから事前確定
- 三角形数 = 南北 fan (`2*n_u`) + 中間バンド (`(n_v-2)*2*n_u`) = **`2*n_u*(n_v-1)`**
- default (angular=32 → n_u=32, n_v=16) → `2*32*15 = 960`。実測でなく式で期待値を固定
- 別途 **watertight 検証** (全無向エッジが厳密に 2 三角形で共有 = 境界エッジ 0) を独立テストで置く

### derive 規約
- 新規型追加なし (`Surface::Sphere` / `Feature::CreateSphere` は既存)。`TessellationStrategy::UvSphere` のみ追加

## 影響ファイル

| ファイル | 変更 |
|---------|------|
| `crates/mycad-kernel/src/primitives/sphere.rs` | **新規**: `make_sphere` + 単体テスト群 |
| `crates/mycad-kernel/src/primitives/mod.rs` | `mod sphere; pub use sphere::make_sphere;` 追加 |
| `crates/mycad-kernel/src/geometry/surface.rs` | `tessellation_strategy()`: Sphere→`UvSphere` (Cone は Unsupported 維持) |
| `crates/mycad-kernel/src/tessellation/mod.rs` | `TessellationStrategy::UvSphere` 追加 + `tessellate_face_sphere` + dispatch arm。`test_unsupported_surface_error` の例を Sphere→Cone に差し替え |
| `crates/mycad-build/src/lib.rs` | `CreateSphere` スタブを `make_sphere(*radius, gen)` に差し替え、import 追加 |
| `crates/mycad-kernel/CLAUDE.md` | 不変条件節に self-adjacent periodic face 許容を 1 行追記 (R04) |
| `examples/sphere.mycad` | **新規** |
| `crates/mycad-build/tests/` | sphere 統合テスト追加 |
| `crates/mycad-cli/tests/export.rs` | sphere export E2E (facet count) 追加 |
| **`crates/mycad-api/tests/mesh_api.rs`** | **(R01)** T07 が今 `create_sphere.mycad` を 422 期待 → ① sphere 成功テスト (200, mesh) を新設、② T07 の「unsupported feature→422」coverage は `extrude.mycad` に付け替え |
| `crates/mycad-api/tests/fixtures/extrude.mycad` | **新規** (T07 の未対応 feature ケース維持用、`type: extrude`) |

### 再利用ポイント
- `Surface::Sphere` の `evaluate/normal_at/normal_at_point/uv_of` (実装済み, surface.rs)
- `push_triangle` / `TriangleMesh` (tessellation/mod.rs) — 退化三角形破棄ロジック
- `make_cylinder` のトポロジー構築 + テスト構成 (cylinder.rs) をテンプレートに
- `Curve::Circle` (curve.rs) — seam メリディアン

### examples/sphere.mycad
```yaml
version: "0.1.0"
root_component:
  name: "Simple Sphere"
  features:
    - type: create_sphere
      id: sphere_1
      radius: 5.0
```

## テスト計画

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一 `IdGenerator` で 2 回 `make_sphere` → 全 ID・座標一致 | フィールド毎 `assert_eq!` |
| T02 | トポロジー | V=2, E=1, F=1, HE=2, loops=1, shell=1 | `assert_eq!` |
| T03 | Euler | V−E+F = 2(S−H) | 成立 |
| T04 | 多様体/閉性 | 各エッジ 2 HE 逆向き、ループ閉性 | 成立 |
| T05 | 幾何 | 極が `center±(0,0,r)`、面 `Surface::Sphere` 係数 | `assert!` |
| T06 | 退化入力 | radius=0/負/NaN/Inf → `InvalidParameter{"radius"}` | `matches!` |
| T07 | メッシュ数 | 三角形数 = `2*n_u*(n_v-1)`、default で 960 (式から事前確定) | `assert_eq!(960)` |
| T08 | メッシュ決定性 | tessellate 2 回 → positions/normals/indices 一致 | `assert_eq!` |
| T09 | watertight | 全無向エッジが厳密に 2 三角形で共有 (境界エッジ 0) | 成立 |
| T10 | 極健全性 | 北/南極が厳密に `center±(0,0,r)` の単一頂点、極 fan が `n_u` 枚 | `assert!` |
| T11 | 統合 (build) | `examples/sphere.mycad` → build → トポロジー検証 (V=2,E=1,F=1) | `assert_eq!` |
| T12 | 非 canonical 球面 | trimmed/壊れた loop の Sphere face → `TrimmedFaceUnsupported` | `matches!` |
| T13 | Cone 未対応維持 | Cone 面の tessellate → `UnsupportedSurface{"cone"}` (回帰なし) | `matches!` |
| T14 | E2E (CLI) | `mycad export examples/sphere.mycad` → STL facet 数 = 960 | `assert_eq!(960)` |
| T15 | 外向き法線 (R03) | 各三角形の winding 法線 `face_normal` が `centroid − center` と同符号 (外向き) | `assert!(dot>0)` |
| T16 | self-adjacent 許容 (R04) | full sphere の自己隣接面が妥当性検証を通る | 成立 |
| T17 | API 成功 (R01) | `GET /api/v0/mesh?file=…sphere.mycad` → 200 + mesh (triangle 960) | `assert_eq!` |
| T18 | API 未対応維持 (R01) | T07 を `extrude.mycad` に付替え → 422 unsupported | `assert_eq!(422)` |
| T19 | 分割数一般化 (round3 R04) | `angular = 3,4,5,7,32` の parameterized: 各で `2*n_u*(n_v-1)`・watertight・外向き winding | 全ケース成立 |

(facet 数は実装由来でなく式 `2*n_u*(n_v-1)` から事前に固定。T09 で watertight、T15 で外向き法線を独立検証。
 T19 で最小値・奇数分割まで一般式と watertight を担保)

## 検証手順

1. `cargo xtask ci` (fmt → clippy → test → build) が green
2. `cargo run -p mycad-cli -- export examples/sphere.mycad -o /tmp/sphere.stl`
   → ファイル生成、`facet normal` 数が T07 と整合
3. `/tmp/sphere.stl` を MeshLab/Blender で開き球形を目視確認 (watertight、極で破綻なし)
4. 決定性テスト T01/T08 通過
5. Cone が依然 `Unsupported` であること (回帰なし) を T10 で確認

## Codex 設計レビューで重点確認したい点
- seam `Curve::Circle` の normal / t-range の正しさ (南極→北極の半周)
- 2-HE 単一 seam ループがカーネルの多様体/検証前提を満たすか
- 緯度分割数 `(angular/2).max(2)` の妥当性
- 極の退化三角形破棄後にメッシュが watertight か (極でファン状に閉じるか)
- T07/T11 の期待ファセット数
