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
session id: 019e57d4-80db-7011-aaea-bbfade28b148
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
  - `crates/mycad-kernel/src/geometry/surface.rs` — `Surface::tessellation_strategy` 追加
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
- 面 **3**（同 `Surface` 既存 variant、enum 追加なし）。**STL 外向きを構築時の巻き方向で保証する**（テッセレータ側の実行時スワップは行わない、§2 参照）:
  - 底キャップ: `Surface::Plane{origin(0,0,0), normal -Z, u_axis +X, v_axis +Y}`、loop = `e_bot` の HE **1 本(reversed)**。`Circle::evaluate` は +Z 周りで CCW なので、e_bot を **逆向き**に巻くと上から見て CW = 外向き法線 −Z の fan になる。`same_sense=true`（面法線 −Z = 構築巻きの幾何法線と一致）。
  - 上キャップ: `Surface::Plane{origin(0,0,h), normal +Z, u_axis +X, v_axis +Y}`、loop = `e_top` の HE **1 本(forward)**。forward = 上から見て CCW = 外向き法線 +Z。`same_sense=true`。
  - 側面: `Surface::Cylinder{origin(0,0,0), axis +Z, radius r}`、loop = 4 HE: seam上(e_seam,fwd,start v_bot)→top円(e_top,**rev**,start v_top)→seam下(e_seam,rev,start v_top)→bot円(e_bot,**fwd**,start v_bot)。`same_sense=true`（外向き=半径方向外向き、UV グリッドの ∂u×∂v 向きと一致するよう構築）。
- シェル **1**（closed=true）

**マニフォルド規約**: 各エッジは正確に 2 HE で、向きは逆（fwd/rev）。
- `e_seam`: 側面が fwd と rev（シームは同一面が両側で使用＝周期面の標準形）
- `e_bot`: 底キャップが **rev**、側面が **fwd**
- `e_top`: 上キャップが **fwd**、側面が **rev**
- HE 合計 = 6（= 3 edges × 2）。各エッジで 2 HE の向きが逆になっていることを確認（マニフォルド条件）。

**Euler–Poincaré**: V−E+F = 2−3+3 = 2 = 2(S−H), S=1,H=0 → 成立。

**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。

**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。前提条件はドキュメントコメントに明記し、`debug_assert!(radius > 0.0 && height > 0.0)` を先頭に置いて開発時の契約違反を検出（リリースでは境界検証済み前提で素通り）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。

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
  - `tessellation` に `const SEAM_EPS: f64`（u のシーム跳び検出は `|Δu| > π` を基準、定数で表現）、`const AREA_EPS: f64`（外積ノルムがこれ未満の三角形は縮退として index に積まない）。**実行時の向きスワップは行わない**（外向きは §1 の構築時巻き方向で保証）。
- 多態フック（型分岐をジオメトリ側の網羅 match に閉じ込め、コンパイラが variant 追加漏れを強制）:
  - `Curve::sample_segment(&self, t_start: f64, t_end: f64, segments: usize) -> Vec<Point>`（R01: HE 向き対応）: `t_start`（含む）から `t_end`（含まない）へ向かう点列。先頭は必ず `evaluate(t_start)`。`Line` は `vec![evaluate(t_start)]`（1 点）、`Circle` は `t_start→t_end` を `segments` 等分した点列。
  - `Surface::tessellation_strategy(&self) -> TessellationStrategy`（**bool ではなく enum**、R01 反映）。`enum TessellationStrategy { BoundaryFan, UvGrid }`。`Plane=>BoundaryFan`、`Cylinder/Sphere/Cone=>UvGrid`。テッセレータ側は `match strategy { ... }` を**網羅**で書く。将来 NURBS 等で別戦略（適応細分）が必要になったら variant を足す→未対応 arm がコンパイルエラーになり、catch-all で静かに誤メッシュ化する事故を防ぐ（bool の二分岐だと新 variant が既定経路へ流れてしまう、というのが R01 の指摘）。
- **トリム面ガード（R01）**: UvGrid 経路は「外側 loop がパラメータ箱全体を覆う**非トリム**面」を前提とする。`face.inner_loops` が非空（穴あき）の面は #12 のスコープ外なので、`UvGrid` 経路で穴を**無視して静かに塗りつぶさない** — `debug_assert!(face.inner_loops.is_empty())` で開発時に検出し、リリースでも穴を含む面は処理対象外として明示スキップ（誤った中実メッシュを出さない）。トリム済み外側 loop を持つ面が出るのは Phase 3後半/4（Boolean・押し出し断面）で、そこで trim 対応メッシャを `UvGrid` とは別 strategy として追加する。
- 面ごとの処理:
  1. 外側 loop の各 HE について、`forward` なら `(t0,t1)`、`!forward` なら `(t1,t0)` を `sample_segment` に渡して折れ線化 → 連結して境界点列。**先頭点が必ず `HalfEdge.start_vertex` に一致**するため、逆向き Line を含む cuboid でも従来と同一の角点列＝**平面出力不変**。
  2. `BoundaryFan`（平面） → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。巻き方向は §1 で構築済み（底キャップ rev / 上キャップ fwd）なので fan の幾何法線が外向きになる。**スワップなし → cuboid のインデックス列もバイト不変**（T08 で固定）。
  3. `UvGrid`（曲面） → 境界点を `surface.uv_of` で (u,v) 化し、周期シーム（u が ±π で跳ぶ）を `SEAM_EPS` 基準で 2π アンラップして連続化 → パラメータ箱 [u_min,u_max]×[v_min,v_max] を取得 → `(angular_segments+1)×(axial_segments+1)` の UV グリッドを `surface.evaluate(u,v)` でサンプル → セルごとに 2 三角形。セル三角形の頂点順は **(u,v) 増加方向で固定**し、`same_sense=true` の側面で ∂u×∂v が外向き（半径方向外向き）になるよう u/v 軸を取る。これで側面が滑らかな多面体に。
  4. 各頂点法線は `surface.normal_at_point`（`same_sense` で反転）— 既存 PR #11 の方式を踏襲。

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

## 設計レビュー反映メモ（Codex）

### Loop 1
- R01(critical) 反映: `Curve::sample_segment` を向き対応 API に変更（上記 §2）。
- R02(high) 反映: `radius/height>0` を境界検証 + `TessellationOptions` クランプ。
- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
- R04(medium) 反映: epsilon 共有定数 `SEAM_EPS`/`AREA_EPS` と比較規約を明記（上記 §2）。
- R05(medium) 反映: golden YAML ラウンドトリップテスト T13 追加。

### Loop 2
- R01(high) 反映: `is_planar() -> bool` の二分岐は将来 variant を既定経路へ静かに流す危険がある → `tessellation_strategy() -> TessellationStrategy`（enum・網羅 match）へ変更。さらに UvGrid 経路にトリム面ガード（`inner_loops` 非空はスキップ＋`debug_assert`）を追加し、穴あき面の誤メッシュ化を防止（§2）。
- R02(high) **検証して反映**: cuboid.rs / stl.rs を読み確認 → 「実行時の向き整合スワップは cuboid 出力不変」という前回主張は **誤り**（現行 cuboid のインデックス巻きが右手系外向き規約と一致する保証はなく、スワップ導入で index 列が変わり T08 が回帰しうる）。→ **テッセレータからスワップを撤去**し、外向きは §1 の**構築時巻き方向**（底キャップ=e_bot rev / 上キャップ=e_top fwd / 側面グリッド=∂u×∂v 外向き）で保証。平面 fan 経路は現行コードとバイト同一 → cuboid 完全不変（T08 で固定）。
- R04(再掲) 反映: `make_cylinder` に `debug_assert!(radius>0 && height>0)` と前提コメントを追加（境界検証は呼び出し側に集約のまま）。
- T03 を「実カウントから Euler を計算」へ修正（定数ハードコード撤廃）。

## テスト計画

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
| T02 | トポロジー | 頂点2・エッジ3・面3・シェル1(closed)・HE6・loop3 | 各 `len()` 一致 |
| T03 | Euler | 実 solid の `vertices.len() − edges.len() + faces.len() == 2 * (shells.len() − genus)`（genus=0）を**実カウントから**計算して検証（定数ハードコードでなく構造体から） | `assert_eq!` |
| T04 | マニフォルド+ループ閉包 | 各エッジがちょうど 2 HE に参照され向きが逆。**かつ**各 loop で連続 HE の終点 ≈ 次 HE の始点、最後の HE 終点 ≈ 先頭 HE 始点（`approx`）。seam 接続も検証 | 集計 + 幾何連続性検証 (R03) |
| T05 | 幾何 | シーム頂点が (0,r,0)/(0,r,h)、各 Surface/Curve の係数 | `approx` で一致 |
| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
| T07 | 決定性(tessellate) | 同一入力2回で positions/normals/indices 一致 | `assert_eq!` |
| T08 | 回帰(平面バイト不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。**positions も indices も従来値と完全一致**（スワップ撤去によりバイト不変、R02）。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions/indices 一致 |
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
設計ドキュメントをコードベースの前提と照らしてレビューします。まず既存の `make_cuboid`、トポロジー型、テッセレーション実装、エラー型を確認して、この提案が MyCad の規約と実装実態に噛み合うかを見ます。
exec
/bin/bash -lc "rg -n \"make_cuboid|tessellate_solid|IdGenerator|KernelError|HalfEdge|inner_loops|Surface::|Curve::|TriangleMesh|Feature::CreateCylinder|CreateCylinder|UnsupportedFeature|InvalidParameter|mod.rs|pub use\" crates/mycad-kernel crates/mycad-build crates/mycad-cli -g '"'!target'"'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-cli/src/main.rs:4:use mycad_kernel::brep::topology::IdGenerator;
crates/mycad-cli/src/main.rs:5:use mycad_kernel::tessellation::{tessellate_solid, to_ascii_stl};
crates/mycad-cli/src/main.rs:43:    let mut gen = IdGenerator::new(0);
crates/mycad-cli/src/main.rs:46:    let mesh = tessellate_solid(&solid);
crates/mycad-kernel/src/lib.rs:7:pub use brep::topology;
crates/mycad-kernel/src/lib.rs:8:pub use error::KernelError;
crates/mycad-kernel/src/error.rs:4:pub enum KernelError {
crates/mycad-kernel/src/error.rs:12:    UnsupportedFeature { kind: &'static str },
crates/mycad-build/src/lib.rs:2:use mycad_kernel::brep::topology::{IdGenerator, Solid};
crates/mycad-build/src/lib.rs:3:use mycad_kernel::error::KernelError;
crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::make_cuboid;
crates/mycad-build/src/lib.rs:8:    gen: &mut IdGenerator,
crates/mycad-build/src/lib.rs:9:) -> Result<Solid, KernelError> {
crates/mycad-build/src/lib.rs:11:        return Err(KernelError::EmptyFeatureList);
crates/mycad-build/src/lib.rs:15:        return Err(KernelError::MultipleFeatures {
crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
crates/mycad-build/src/lib.rs:27:        Feature::CreateCylinder { .. } => Err(KernelError::UnsupportedFeature {
crates/mycad-build/src/lib.rs:30:        Feature::CreateSphere { .. } => Err(KernelError::UnsupportedFeature {
crates/mycad-build/src/lib.rs:33:        Feature::Extrude { .. } => Err(KernelError::UnsupportedFeature { kind: "extrude" }),
crates/mycad-build/src/lib.rs:34:        Feature::Cut { .. } => Err(KernelError::UnsupportedFeature { kind: "cut" }),
crates/mycad-build/src/lib.rs:35:        Feature::Fuse { .. } => Err(KernelError::UnsupportedFeature { kind: "fuse" }),
crates/mycad-build/src/lib.rs:36:        Feature::Intersect { .. } => Err(KernelError::UnsupportedFeature { kind: "intersect" }),
crates/mycad-kernel/benches/tessellation.rs:1:use mycad_kernel::brep::topology::IdGenerator;
crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
crates/mycad-kernel/benches/tessellation.rs:3:use mycad_kernel::tessellation::tessellate_solid;
crates/mycad-kernel/benches/tessellation.rs:14:        let mut id_gen = IdGenerator::new(i as u64);
crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/benches/tessellation.rs:16:        let _mesh = tessellate_solid(&solid);
crates/mycad-kernel/src/tessellation/mod.rs:2:pub use stl::to_ascii_stl;
crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
crates/mycad-kernel/CLAUDE.md:14:- すべてのEdgeは正確に2つのHalfEdgeを持つ
crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
crates/mycad-kernel/CLAUDE.md:17:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
crates/mycad-kernel/CLAUDE.md:18:- EntityIDは `IdGenerator` で決定的に生成される
crates/mycad-kernel/CLAUDE.md:23:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
crates/mycad-kernel/CLAUDE.md:24:3. `primitives/mod.rs` に `mod` と `pub use` を追加
crates/mycad-kernel/CLAUDE.md:25:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）
crates/mycad-kernel/src/tessellation/stl.rs:1:use super::TriangleMesh;
crates/mycad-kernel/src/tessellation/stl.rs:4:pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
crates/mycad-kernel/src/tessellation/stl.rs:51:    use crate::brep::topology::IdGenerator;
crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
crates/mycad-kernel/src/tessellation/stl.rs:53:    use crate::tessellation::tessellate_solid;
crates/mycad-kernel/src/tessellation/stl.rs:57:        let mut g = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
crates/mycad-kernel/src/tessellation/stl.rs:59:        let mesh = tessellate_solid(&solid);
crates/mycad-kernel/src/tessellation/stl.rs:74:            let mut g = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
crates/mycad-kernel/src/tessellation/stl.rs:76:            to_ascii_stl(&tessellate_solid(&s), "cube")
crates/mycad-build/tests/feature_dispatcher.rs:3:use mycad_kernel::brep::topology::IdGenerator;
crates/mycad-build/tests/feature_dispatcher.rs:15:    let mut g = IdGenerator::new(0);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:3:use mycad_kernel::tessellation::tessellate_solid;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
crates/mycad-kernel/src/geometry/surface.rs:35:            Surface::Plane {
crates/mycad-kernel/src/geometry/surface.rs:42:            Surface::Cylinder {
crates/mycad-kernel/src/geometry/surface.rs:51:            Surface::Sphere { center, radius } => {
crates/mycad-kernel/src/geometry/surface.rs:59:            Surface::Cone {
crates/mycad-kernel/src/geometry/surface.rs:74:            Surface::Plane { normal, .. } => normal.normalize(),
crates/mycad-kernel/src/geometry/surface.rs:76:            Surface::Cylinder { axis, .. } => {
crates/mycad-kernel/src/geometry/surface.rs:81:            Surface::Sphere { center, .. } => {
crates/mycad-kernel/src/geometry/surface.rs:86:            Surface::Cone {
crates/mycad-kernel/src/geometry/surface.rs:105:    /// Inverse of [`Surface::evaluate`]: find the parameters (u, v) whose
crates/mycad-kernel/src/geometry/surface.rs:113:            Surface::Plane {
crates/mycad-kernel/src/geometry/surface.rs:126:            Surface::Cylinder { origin, axis, .. } => {
crates/mycad-kernel/src/geometry/surface.rs:135:            Surface::Sphere { center, radius } => {
crates/mycad-kernel/src/geometry/surface.rs:142:            Surface::Cone { apex, axis, .. } => {
crates/mycad-kernel/src/geometry/surface.rs:155:    /// Convenience over [`Surface::uv_of`] + [`Surface::normal_at`] so callers
crates/mycad-kernel/src/geometry/surface.rs:177:        let surface = Surface::Plane {
crates/mycad-kernel/src/geometry/surface.rs:188:        let surface = Surface::Cylinder {
crates/mycad-kernel/src/geometry/surface.rs:198:        let surface = Surface::Sphere {
crates/mycad-kernel/src/geometry/surface.rs:207:        let surface = Surface::Cone {
crates/mycad-kernel/src/geometry/surface.rs:220:        let surface = Surface::Sphere {
crates/mycad-kernel/src/geometry/surface.rs:234:        let surface = Surface::Plane {
crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
crates/mycad-kernel/src/geometry/curve.rs:24:            Curve::Line { origin, direction } => origin + t * direction,
crates/mycad-kernel/src/geometry/curve.rs:25:            Curve::Circle {
crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
crates/mycad-kernel/src/primitives/cuboid.rs:75:                Curve::Line {
crates/mycad-kernel/src/primitives/cuboid.rs:103:            surface: Surface::Plane {
crates/mycad-kernel/src/primitives/cuboid.rs:119:            surface: Surface::Plane {
crates/mycad-kernel/src/primitives/cuboid.rs:135:            surface: Surface::Plane {
crates/mycad-kernel/src/primitives/cuboid.rs:151:            surface: Surface::Plane {
crates/mycad-kernel/src/primitives/cuboid.rs:167:            surface: Surface::Plane {
crates/mycad-kernel/src/primitives/cuboid.rs:183:            surface: Surface::Plane {
crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
crates/mycad-kernel/src/brep/topology.rs:21:pub struct HalfEdge {
crates/mycad-kernel/src/brep/topology.rs:62:    pub inner_loops: Vec<usize>,
crates/mycad-kernel/src/brep/topology.rs:83:    pub half_edges: Vec<HalfEdge>,
crates/mycad-kernel/src/brep/topology.rs:138:        self.half_edges.push(HalfEdge {
crates/mycad-kernel/src/brep/topology.rs:160:        inner_loops: Vec<usize>,
crates/mycad-kernel/src/brep/topology.rs:168:            inner_loops,
crates/mycad-kernel/src/brep/topology.rs:185:pub struct IdGenerator {
crates/mycad-kernel/src/brep/topology.rs:189:impl IdGenerator {
crates/mycad-kernel/src/brep/topology.rs:208:        let mut gen1 = IdGenerator::new(100);
crates/mycad-kernel/src/brep/topology.rs:209:        let mut gen2 = IdGenerator::new(100);
crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};

exec
/bin/bash -lc 'rg --files crates/mycad-kernel/src crates/mycad-build/src crates/mycad-cli/src crates/mycad-build/tests crates/mycad-cli/tests examples' in /home/bacon/mycad
 succeeded in 0ms:
examples/assembly.mycad
examples/simple_box.mycad
crates/mycad-build/tests/feature_dispatcher.rs
crates/mycad-kernel/src/lib.rs
crates/mycad-build/src/lib.rs
crates/mycad-kernel/src/tessellation/mod.rs
crates/mycad-kernel/src/tessellation/stl.rs
crates/mycad-cli/src/main.rs
crates/mycad-kernel/src/error.rs
crates/mycad-kernel/src/brep/mod.rs
crates/mycad-kernel/src/brep/topology.rs
crates/mycad-kernel/src/geometry/curve.rs
crates/mycad-kernel/src/geometry/math.rs
crates/mycad-kernel/src/geometry/mod.rs
crates/mycad-kernel/src/geometry/surface.rs
crates/mycad-cli/tests/export.rs
crates/mycad-kernel/src/primitives/cuboid.rs
crates/mycad-kernel/src/primitives/mod.rs

codex
既存実装では `Surface` に円柱系の評価関数はすでにあり、トポロジーは `HalfEdge` の順序列だけで表現されています。ここからは、提案がその前提で本当に閉じるかと、テスト計画が実構造を十分に検証できるかを詰めます。
exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/brep/topology.rs" in /home/bacon/mycad
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
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
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

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/geometry/curve.rs" in /home/bacon/mycad
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
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/error.rs && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-build/src/lib.rs && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-cli/src/main.rs" in /home/bacon/mycad
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

---
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

---
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

codex
`KernelError` と CLI 側はかなり素直で、提案どおりの差し替え余地があります。一方で、穴付き面を「スキップ」する案と、円周エッジを 1 本の閉曲線で表す案は、B-rep 妥当性や出力完全性の観点で少し危ないので、そのあたりを確認しています。
exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-build/tests/feature_dispatcher.rs && printf '\\n---\\n' && sed -n '1,260p' crates/mycad-cli/tests/export.rs && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
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

---
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

---
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
    let mesh = tessellate_solid(&solid);

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
        tessellate_solid(&solid)
    };

    let mesh1 = run();
    let mesh2 = run();

    assert_eq!(mesh1.positions, mesh2.positions);
    assert_eq!(mesh1.normals, mesh2.normals);
    assert_eq!(mesh1.indices, mesh2.indices);
}

exec
/bin/bash -lc "rg -n \"enum Feature|CreateCylinder|Serialize|Deserialize|JsonSchema|to_yaml|from_yaml|from_path|root_component\" crates -g '"'!target'"'" in /home/bacon/mycad
 succeeded in 0ms:
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
crates/mycad-format/src/document.rs:3:use schemars::JsonSchema;
crates/mycad-format/src/document.rs:4:use serde::{Deserialize, Serialize};
crates/mycad-format/src/document.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/document.rs:13:    pub root_component: Component,
crates/mycad-format/src/document.rs:21:            root_component: Component::new(name),
crates/mycad-format/src/document.rs:25:    /// Serialize to YAML string.
crates/mycad-format/src/document.rs:26:    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
crates/mycad-format/src/document.rs:30:    /// Deserialize from YAML string.
crates/mycad-format/src/document.rs:31:    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
crates/mycad-format/src/document.rs:36:    pub fn from_path(path: &Path) -> Result<Self, FormatError> {
crates/mycad-format/src/document.rs:42:        let doc = Self::from_yaml(&content)?;
crates/mycad-format/src/document.rs:57:        doc.root_component.features.push(Feature::Extrude {
crates/mycad-format/src/document.rs:63:        let yaml = doc.to_yaml().unwrap();
crates/mycad-format/src/document.rs:64:        let doc2 = Document::from_yaml(&yaml).unwrap();
crates/mycad-format/src/document.rs:67:        assert_eq!(doc.root_component.name, doc2.root_component.name);
crates/mycad-format/src/document.rs:69:            doc.root_component.features.len(),
crates/mycad-format/src/document.rs:70:            doc2.root_component.features.len()
crates/mycad-format/src/document.rs:77:        doc.root_component.features.push(Feature::CreateBox {
crates/mycad-format/src/document.rs:84:        let yaml1 = doc.to_yaml().unwrap();
crates/mycad-format/src/document.rs:85:        let yaml2 = doc.to_yaml().unwrap();
crates/mycad-format/src/document.rs:90:    fn test_from_path_loads_simple_box() {
crates/mycad-format/src/document.rs:96:        let doc = Document::from_path(&path).unwrap();
crates/mycad-format/src/document.rs:98:        assert_eq!(doc.root_component.name, "Simple Box");
crates/mycad-format/src/document.rs:102:    fn test_from_path_missing_file_returns_io_error() {
crates/mycad-format/src/document.rs:104:        let err = Document::from_path(path).unwrap_err();
crates/mycad-format/src/document.rs:109:    fn test_from_path_invalid_extension() {
crates/mycad-format/src/document.rs:111:        let err = Document::from_path(path).unwrap_err();
crates/mycad-format/src/document.rs:124:        let doc = Document::from_path(&path).unwrap();
crates/mycad-format/src/document.rs:128:            .root_component
crates/mycad-format/src/document.rs:138:        let yaml = doc.to_yaml().unwrap();
crates/mycad-format/src/document.rs:139:        let doc2 = Document::from_yaml(&yaml).unwrap();
crates/mycad-format/src/document.rs:140:        assert_eq!(doc.to_yaml().unwrap(), doc2.to_yaml().unwrap());
crates/mycad-build/src/lib.rs:27:        Feature::CreateCylinder { .. } => Err(KernelError::UnsupportedFeature {
crates/mycad-cli/src/main.rs:41:    let doc = Document::from_path(input)
crates/mycad-cli/src/main.rs:44:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
crates/mycad-format/CLAUDE.md:5:`.mycad` ファイルはYAML形式。`Document` がトップレベルで、`root_component` にComponent階層を持つ。各Componentは `features` (操作履歴) と `children` (子コンポーネント) を持てる。
crates/mycad-format/CLAUDE.md:11:root_component:
crates/mycad-format/CLAUDE.md:34:| `CreateCylinder` | (未実装)    |
crates/mycad-build/tests/feature_dispatcher.rs:14:    let doc = Document::from_path(&path).expect("load .mycad");
crates/mycad-build/tests/feature_dispatcher.rs:17:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
crates/mycad-kernel/src/brep/topology.rs:4:use serde::{Deserialize, Serialize};
crates/mycad-kernel/src/brep/topology.rs:11:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:20:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:33:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:45:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:54:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:68:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:79:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/geometry/curve.rs:3:use serde::{Deserialize, Serialize};
crates/mycad-kernel/src/geometry/curve.rs:6:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/geometry/mod.rs:6:use serde::{Deserialize, Serialize};
crates/mycad-kernel/src/geometry/mod.rs:15:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/tessellation/mod.rs:6:use serde::{Deserialize, Serialize};
crates/mycad-kernel/src/tessellation/mod.rs:9:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/geometry/surface.rs:3:use serde::{Deserialize, Serialize};
crates/mycad-kernel/src/geometry/surface.rs:6:#[derive(Debug, Clone, Serialize, Deserialize)]

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/CLAUDE.md && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-kernel/src/primitives/mod.rs" in /home/bacon/mycad
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

---
mod cuboid;

pub use cuboid::make_cuboid;

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

exec
/bin/bash -lc "sed -n '1,200p' crates/mycad-format/src/feature.rs && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-format/src/document.rs" in /home/bacon/mycad
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

---
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

codex
テスト計画も見ていますが、現状の型定義だと `Solid` や `Face` に `PartialEq` がないので、記述どおりの「丸ごと `assert_eq!`」はそのままでは書けません。これは重大ではないですが、実装前にテストの比較単位を固めておかないと計画どおり進まない箇所です。
exec
/bin/bash -lc "sed -n '1,260p' Cargo.toml && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-kernel/Cargo.toml && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-build/Cargo.toml && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-cli/Cargo.toml" in /home/bacon/mycad
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

---
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

---
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

---
[package]
name = "mycad-cli"
description = "Command-line interface for MyCad"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "mycad"
path = "src/main.rs"

[dependencies]
mycad-kernel = { workspace = true }
mycad-format = { workspace = true }
mycad-build = { workspace = true }
clap = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }

exec
/bin/bash -lc "rg -n \"approx =|thiserror =|schemars =|serde_yaml =|tempfile =|workspace.dependencies|\\[workspace.dependencies\\]\" Cargo.toml crates/*/Cargo.toml" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-format/Cargo.toml:10:serde_yaml = { workspace = true }
crates/mycad-format/Cargo.toml:11:schemars = { workspace = true }
crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
crates/mycad-format/Cargo.toml:16:approx = { workspace = true }
crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
crates/mycad-kernel/Cargo.toml:15:approx = { workspace = true }
Cargo.toml:18:[workspace.dependencies]
Cargo.toml:23:serde_yaml = "0.9"
Cargo.toml:25:schemars = "0.8"
Cargo.toml:27:approx = "0.5"
Cargo.toml:28:tempfile = "3"
Cargo.toml:30:thiserror = "2"
crates/mycad-cli/Cargo.toml:19:tempfile = { workspace = true }

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

codex
issues:
  - id: R01
    severity: critical
    section: "設計方針 > 2. テッセレータ > トリム面ガード"
    finding: "UvGrid 経路で未対応の面を release では『明示スキップ』する設計になっており、`mycad export` が成功したまま面欠落の開いた STL を静かに出力しうる。これは watertight 性と出力完全性を壊す。"
    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
  - id: R02
    severity: high
    section: "設計方針 > 2. テッセレータ > UvGrid"
    finding: "`face.inner_loops.is_empty()` だけでは『非トリムのパラメータ矩形面』を判定できない。外側 loop だけで構成された trimmed face でも UvGrid がその UV bbox 全体を塗り、実面積より大きい誤メッシュを生成する。"
    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
  - id: R03
    severity: medium
    section: "設計方針 > 2. テッセレータ > Surface::tessellation_strategy"
    finding: "`Sphere` と `Cone` も `UvGrid` に割り当てているが、提案アルゴリズムとテスト計画は円柱前提で、球の極や円錐 apex の特異点処理が未設計・未検証のまま公開 API の振る舞いだけ先に広がっている。"
    suggestion: "#12 の範囲では `Plane` と `Cylinder` のみを明示対応に留め、`Sphere`/`Cone` は未対応エラーにするか、特異点専用の分割規則とテストを用意してから有効化すること"
  - id: R04
    severity: medium
    section: "設計方針 > 1. トポロジー / 退化入力の検証"
    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
  - id: R05
    severity: low
    section: "テスト計画 > T01"
    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
    suggestion: "比較対象を ID 列・座標列・面構成のフィールド単位に分解するか、必要ならテスト専用に比較ヘルパーを用意すること"

verdict: fail
tokens used
94,395
issues:
  - id: R01
    severity: critical
    section: "設計方針 > 2. テッセレータ > トリム面ガード"
    finding: "UvGrid 経路で未対応の面を release では『明示スキップ』する設計になっており、`mycad export` が成功したまま面欠落の開いた STL を静かに出力しうる。これは watertight 性と出力完全性を壊す。"
    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
  - id: R02
    severity: high
    section: "設計方針 > 2. テッセレータ > UvGrid"
    finding: "`face.inner_loops.is_empty()` だけでは『非トリムのパラメータ矩形面』を判定できない。外側 loop だけで構成された trimmed face でも UvGrid がその UV bbox 全体を塗り、実面積より大きい誤メッシュを生成する。"
    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
  - id: R03
    severity: medium
    section: "設計方針 > 2. テッセレータ > Surface::tessellation_strategy"
    finding: "`Sphere` と `Cone` も `UvGrid` に割り当てているが、提案アルゴリズムとテスト計画は円柱前提で、球の極や円錐 apex の特異点処理が未設計・未検証のまま公開 API の振る舞いだけ先に広がっている。"
    suggestion: "#12 の範囲では `Plane` と `Cylinder` のみを明示対応に留め、`Sphere`/`Cone` は未対応エラーにするか、特異点専用の分割規則とテストを用意してから有効化すること"
  - id: R04
    severity: medium
    section: "設計方針 > 1. トポロジー / 退化入力の検証"
    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
  - id: R05
    severity: low
    section: "テスト計画 > T01"
    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
    suggestion: "比較対象を ID 列・座標列・面構成のフィールド単位に分解するか、必要ならテスト専用に比較ヘルパーを用意すること"

verdict: fail
