# Plan: Extrude (スケッチ→ソリッド) feature — Issue #22

## Context

Issue #22 は Phase 3（Milestone 4「円柱・球・押し出しが作れる」）最後の `type: feature`。
平面上の閉じた 2D 多角形輪郭（スケッチ）を法線方向に押し出して B-rep ソリッドを生成し、
`.mycad` → geometry → STL まで動作させる。依存先 #14（ADR-005）は完了済み。

ユーザーとの壁打ちで確定した方針:
- **A案**: `create_sketch` を独立 Feature にし、`extrude` が feature_id で参照（ADR-005 の参照原則に整合）。
- **辺idは明示**: profile の各セグメントに `id` を書く。
- **スコープは分割**: #22 は Extrude 本体（スケッチ型 + 複数 feature ディスパッチ + 押し出し幾何 + STL）に集中。
  ADR-005 の名札土台工事（`EntityRef` enum 化・読み込み検証経路の一本化・全 primitive への
  face role 付与/保存 + API・T01–T11）は **後続の新規 foundation issue** に切り出す。
- **実装順は Extrude が先**: Extrude は Phase 3 で既存面を参照しない（ADR-005 43–47行）ため名札土台なしで完結する。
  土台の本当の利用者は Phase 4（Boolean）でまだ遠い。

### ADR-005 との関係（範囲の明示 / Codex 指摘 R01）
- **#22 は ADR-005 への「完全準拠」を主張しない**。ユーザー合意のスコープ分割により、ADR-005 Decision 8 /
  F01 が要求する「format 層検証・未検証 Document を public API から出さない（custom Deserialize / validated
  newtype）」は **後続 foundation issue の acceptance** とする。#22 はその前提で進める。
- #22 が行うのは **build 解決時（kernel 境界）の暫定検証のみ**（KernelError）。これは
  `build_solid_from_features` を通る経路（CLI export 含む）での不正入力を確実に弾くが、
  build を経由しない `Document::from_yaml`/`from_path`/mutation API レベルの不変条件保証は **#22 の範囲外**。
  検証ルール（charset・一意性・閉路・凸性・単純性）自体は後続 issue が format 層へ「引っ越す」際に再利用できる
  （ルールは不変、置き場所のみ移動）。
- Extrude の face role（`cap_start`/`cap_end`/`side_<seg>`）の確定・保存は ADR-005 Decision 7 が
  「role 付与実装 issue で確定」と明記しているため **後続 issue**。#22 では face を**決定的順序**で組む
  （Decision 9）だけ守り、後から role 名を貼っても並べ替え不要にする。
- **側面 ↔ segment の対応保持（重要 / Codex R02）**: kernel は profile 配列を**並べ替えない**。
  CW/CCW の補正は配列反転ではなく **HE 向きの選択**で行う（後述 winding）。これにより
  「側面 face k ↔ profile edge k ↔ `SketchSegment.id[k]`」の対応が入力順のまま不変に保たれ、
  後続 issue が `side_<seg>` role を貼る際に並べ替え・再構成が不要。

## 実装対象

### 1. format: `CreateSketch` variant + スケッチ型（`crates/mycad-format/src/feature.rs`）

新規型（すべて `Debug, Clone, Serialize, Deserialize, JsonSchema, TS` を derive）:
```rust
#[serde(rename_all = "lowercase")]
pub enum SketchPlane { Xy, Xz, Yz }   // YAML: xy / xz / yz

pub struct SketchSegment { pub id: String, pub from: [f64; 2], pub to: [f64; 2] }
```
`Feature` enum に追加:
```rust
#[serde(rename = "create_sketch")]
CreateSketch { id: String, plane: SketchPlane, profile: Vec<SketchSegment> },
```
- `Feature::id()` の match arm に `CreateSketch { id, .. }` を追加（feature.rs:76-83）。
- 既存 `Extrude { id, sketch, depth }` はそのまま（変更不要 = 既存 golden を壊さない）。
- テスト: `create_sketch` の round-trip + **exact `assert_eq!` golden**（T08。`contains` ではなく全文一致で drift 検出）。

YAML 形（`examples/extruded_rect.mycad`）:
```yaml
version: "0.1.0"
root_component:
  name: "Extruded Rect"
  features:
    - type: create_sketch
      id: sketch_1
      plane: xy
      profile:
        - { id: seg_a, from: [0.0, 0.0], to: [10.0, 0.0] }
        - { id: seg_b, from: [10.0, 0.0], to: [10.0, 5.0] }
        - { id: seg_c, from: [10.0, 5.0], to: [0.0, 5.0] }
        - { id: seg_d, from: [0.0, 5.0], to: [0.0, 0.0] }
    - type: extrude
      id: extrude_1
      sketch: sketch_1
      depth: 8.0
```

### 2. build: 複数 feature ディスパッチ（`crates/mycad-build/src/lib.rs`）

`build_solid_from_features` を「順次処理 + sketch 置き場」型に改修:
- 空 → `EmptyFeatureList`。
- features を **Vec 順（決定的）** に走査。`CreateSketch` は `HashMap<&str, (&SketchPlane, &[SketchSegment])>`
  に登録し**ソリッドを生成しない**（HashMap は id 引きのみ、反復に使わない → 決定性維持・Decision 9）。
- ソリッド生成 feature（CreateBox/Cylinder/Sphere/Extrude）は最大 1 個。
  0 個（sketch のみ）→ `EmptyFeatureList`、2 個以上 → `MultipleFeatures { count }`（= 生成 feature 数）。
- `Extrude { sketch, depth }`: HashMap から sketch を解決 →
  **build 側で軽量検証**（後述）→ `SketchPlane` を kernel `Plane` に写像 →
  profile 頂点列（各 seg.from を**ドキュメント順のまま**抽出。**並べ替えない**）→ `make_extrusion(&plane, &profile_uv, depth, gen)`。
  build は `seg.id[k]` を同じ index k で保持し、将来の side face 対応に備える。
  未解決 sketch → 新 `KernelError::SketchNotFound { sketch: String }`。
- 既存の単一 feature 経路（box/cyl/sphere）は従来通り動く（加算的・ADR-004）。

build 側軽量検証（KernelError にマップ。**静黙上書きを必ず防ぐ / Codex R03**）:
- **Feature.id の重複禁止**: 走査中に既出の `Feature.id`（Component 内）を検出したら
  `DuplicateFeatureId { id }` でエラー（HashMap 静黙上書き禁止）。charset `[A-Za-z0-9_-]` 違反は
  `InvalidParameter { kind: "feature_id" }`。
- **sketch id の重複禁止**: 同一 sketch id の二重登録もエラー（`DuplicateFeatureId`）。
- segment id がスケッチ内で一意・charset `[A-Za-z0-9_-]`（違反 → `InvalidParameter { kind: "sketch_segment_id" }`）。
- 輪郭が閉じている（`seg[i].to ≈ seg[i+1].from`、`last.to ≈ first.from`、`LENGTH_TOLERANCE`）→ 違反は `InvalidParameter { kind: "profile" }`。

### 3. kernel: `make_extrusion`（新規 `crates/mycad-kernel/src/primitives/extrusion.rs`）

`make_cylinder`（cylinder.rs）を手本に。`primitives/mod.rs` に `mod extrusion; pub use extrusion::make_extrusion;`。
```rust
pub fn make_extrusion(
    plane: &Plane,            // geometry::Plane（build が xy/xz/yz から構築）
    profile: &[(f64, f64)],   // uv 平面上の頂点列（閉路は暗黙、最後→最初）
    depth: f64,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError>
```
- kernel は format 非依存（依存グラフ厳守）→ 入力は kernel ネイティブ型のみ。seg id は渡さない。
- **公差の定義元（Codex R03）**: 既存 `geometry::LENGTH_TOLERANCE`（`geometry/math.rs`、`geometry/mod.rs` から re-export 済み）を唯一の長さ基準とする。新規定数を増やさず以下で導出:
  - **長さ epsilon** = `LENGTH_TOLERANCE`（ゼロ長辺・閉路判定・頂点一致に使用）。
  - **面積 epsilon** = `LENGTH_TOLERANCE²`（`|signed_area|`・共線 3 点の 2 倍三角形面積の閾値）。長さ²の次元に合わせる。
  - **交差 epsilon** = `LENGTH_TOLERANCE`（segment 交差判定のパラメータ/距離比較）。
  - **shared-endpoint 規則**: 隣接辺（edge k と k+1、及び last↔first）は端点共有が正当なので単純性検査から除外。**非隣接辺**のみ交差検査し、交差点が双方の開区間（端点を含まない、`LENGTH_TOLERANCE` 内のかすめ接触も交差扱い）に入れば自己交差として拒否。重複頂点（非隣接で端点が `LENGTH_TOLERANCE` 内一致）も単純性違反として拒否。
- **検証（Codex R05）**: `depth` は有限かつ `depth > LENGTH_TOLERANCE` を必須（≤0・非有限 → `InvalidParameter { kind: "depth" }`）。
  既存 primitive（cylinder/box の非正値拒否）と統一。**負押し出しは Phase 3 非対応**。
  頂点 < 3、ゼロ長エッジ、`|signed_area| <= 面積公差`（退化）→ `InvalidParameter { kind: "profile" }`（ADR-005 Decision 10）。
- **共線頂点の拒否（Codex R03）**: 連続 3 点の外積（= 2 倍三角形面積）が面積公差以下なら、辺中間の冗長点や
  零面積三角形を生むため `InvalidParameter { kind: "profile" }` で拒否（ゼロ長辺でも全体面積ゼロでもない退化を排除）。
- **凸かつ単純の検査（Codex R02・R04）**: 現 tessellator のキャップは `BoundaryFan`（先頭頂点扇形）で、
  **凸かつ単純（自己交差なし）な輪郭しか正しく三角化できない**。放置すると `mycad export` が不正 STL を
  成功扱いで出力する。よって 2 条件を両方検査し、いずれか違反なら `InvalidParameter { kind: "profile" }` で拒否:
  1. **凸性**: 全ターンの外積符号が一致、かつ巻き数 = 1（外角和 = ±360°）。符号一致のみだと五芒星型を通すため巻き数で締める。
  2. **単純性**: 非隣接辺同士の交差を明示検査（N 小のため O(N²) で十分）。凸＋単純で `BoundaryFan` の正当性を保証。
  ear-clipping による凹多角形対応は後 Phase。
- **向き補正（配列を反転しない / Codex R01・R02）**: 手系を考慮した winding で **HE 向きを選ぶ**。
  `handedness = sign((u_axis × v_axis)·normal)`（xy/yz=+1, **xz=-1**）、
  `winding = sign(signed_area_uv) × handedness`。この `winding` で底/天キャップと側面の HE 向きを一括決定し、
  キャップ法線を必ず外向き（底=-normal, 天=+normal）に保つ。**profile 配列は並べ替えない**ため
  「側面 k ↔ profile edge k」の対応が不変（後続 role 付与で並べ替え不要）。
- トポロジー（N 頂点 → V=2N, E=3N, F=N+2, HE=6N, Loops=N+2; Euler `V-E+F=2`）:
  - 頂点: 底 N（`plane.origin + u·u_axis + v·v_axis`）+ 天 N（底 + `depth·normal`）。
  - エッジ: 底 N（Line）+ 天 N（Line）+ 縦 N（Line）。
  - 底キャップ: N-HE ループ、`Surface::Plane` 法線 `-plane.normal`（HE 向きは `winding` で決定）。
  - 天キャップ: N-HE ループ、`Surface::Plane` 法線 `+plane.normal`。
  - 側面 N 枚: 各 `[底エッジ, 縦, 天エッジ, 縦]` の 4-HE ループ、`Surface::Plane` 外向き法線。
  - 全エッジが正確に 2 HE（正逆）/ ループ閉路 / shell `closed: true`（kernel 不変条件）。
- id はすべて `id_gen.next()` を固定順で発番（決定性）。

### 4. example + STL 経路
- `examples/extruded_rect.mycad`（上記）。CLI `mycad export` 経路（`crates/mycad-cli/src/main.rs` の
  `run_export`）は build → tessellate → STL で**改修不要**（ディスパッチャ改修が効く）。

## テスト計画

| ID | 種別 | 内容 | 期待 |
|----|------|------|------|
| T01 | 決定性 | `make_extrusion` を同一入力で2回 → V/E/F/HE/loop/shell 数・id・座標が完全一致 | assert_eq! |
| T02 | 正常系 | 矩形 profile（10×5, depth 8）→ V=8,E=12,F=6,shells=1 | counts |
| T03 | Euler | `V-E+F = 2(S-H)`（S=1,H=0 → 2） | assert_eq! |
| T04 | 多様体 | 各エッジ 2 HE 正逆 + ループ閉路（頂点インデックス連結） | manifold |
| T05 | 幾何 | 底キャップ法線 -Z / 天キャップ法線 +Z / 天頂点 z=depth | normal/coord |
| T06 | 退化 | depth=0/負/NaN/Inf → `InvalidParameter{depth}`; 頂点<3/ゼロ長辺/面積≈0/**共線3点**(R03) → `InvalidParameter{profile}` | unwrap_err |
| T07 | 向き(手系) | xy/yz/**xz** の3平面 × CW/CCW 両入力で、底=-normal・天=+normal の外向きキャップに統一（R01） | 法線一致 |
| T08 | format golden(厳密) | `Feature::CreateSketch` 単体 YAML と `examples/extruded_rect.mycad` の `Document::to_yaml()` を**exact `assert_eq!`** で固定（field順・省略規則・rename drift 検出）。生成 TS/JsonSchema も exact drift（R04） | assert_eq! |
| T09 | build 統合 | `examples/extruded_rect.mycad` load → build → V=8,E=12,F=6,shells=1 | counts |
| T10 | build エラー | sketch 未解決 → `SketchNotFound`; 生成 feature 2個 → `MultipleFeatures`; seg id 重複/不正文字 → `InvalidParameter` | matches! |
| T11 | build 回帰 | 既存 box/cylinder/sphere の単一 feature build が従来通り通る | 既存テスト維持 |
| T12 | CLI E2E 正常 | `mycad export examples/extruded_rect.mycad -o out.stl` → STL facet 数 = 12（キャップ2×2 + 側面4×2） | facet count |
| T13 | 非凸/自己交差拒否(R02) | 凹多角形 と 自己交差(五芒星型)profile → `InvalidParameter{profile}`; CLI export も失敗（不正 STL を出さない） | unwrap_err / exit≠0 |
| T14 | 重複id拒否(R03) | 重複 `Feature.id`・重複 sketch id → `DuplicateFeatureId`（静黙上書きしない） | matches! |
| T15 | 側面↔seg対応(R02) | 入力 profile edge k と side face k の幾何対応が入力順で安定（並べ替え無し） | 順序検証 |
| T16 | 前方参照禁止(R04) | `extrude` が**後続**の `create_sketch` を参照（features 配列で extrude が先・sketch が後）→ `SketchNotFound`。Vec 順走査＝sketch は参照時点で未登録、という順序依存を仕様として固定 | matches! |
| T17 | api 成功系(R01) | `crates/mycad-api/tests/mesh_api.rs` の旧 unsupported(422) を extrude 成功(200 + facet=12)へ更新し回帰維持 | status/facet |

## 影響ファイル
- `crates/mycad-format/src/feature.rs` — 型追加・`Feature::id()` 更新・テスト
- `crates/mycad-build/src/lib.rs` — ディスパッチャ改修・軽量検証・テスト（`tests/feature_dispatcher.rs`）
- `crates/mycad-kernel/src/primitives/extrusion.rs`（新規）+ `primitives/mod.rs`
- `crates/mycad-kernel/src/error.rs` — `SketchNotFound { sketch: String }`、`DuplicateFeatureId { id: String }` 追加
- `examples/extruded_rect.mycad`（新規）
- `crates/mycad-cli/tests/export.rs` — extrude E2E テスト追加
- **`crates/mycad-api/tests/mesh_api.rs`（R01・CI 破壊回避で必須）** — 現 `t07_unsupported_feature`(164行) / `t18_extrude_unsupported`(226行) は extrude を「unsupported(422)」前提のまま。extrude **成功系**（200 + mesh 検証）へ書き換える。放置すると `cargo test --workspace` / `cargo xtask ci` が落ちる。
- **`crates/mycad-api/tests/fixtures/extrude.mycad`（R01・必須）** — 旧 `type: extrude` 単体 fixture を `create_sketch` + `extrude` 形へ差し替え。
- **`crates/xtask/src/main.rs`（R02・必須）** — `FEATURE_GOLDEN`(286行) の TS 型文字列に `create_sketch` variant が増えるため exact `assert_eq!`(298行) が落ちる。golden 文字列を更新。
- **`web/src/generated/Feature.ts`（R02・必須）** — `cargo xtask gen-ts` で再生成（6行の Feature 型に `create_sketch` が追加）。再生成差分を必ずコミットに含める。
- 生成物: 上記 TS に加え JsonSchema golden があれば再生成（新型追加でスキーマ変化）
- docs: `crates/mycad-format/CLAUDE.md` の Feature→Kernel 表で Extrude を「`make_extrusion`」に更新

## 後続 foundation issue（#22 完了後に起票）
ADR-005 名札土台: `EntityRef` enum 化（`Named{feature_id,kind,role}`）、単一 validated load path
（重複 feature_id・charset・sketch seg id を format 層で検証、未検証 Document 非公開）、
全 primitive（box/cyl/sphere/extrude）への face/edge/vertex role 付与 + 名前引き API、
epsilon 集約確認、acceptance T01–T11（ADR-005 246–264行）。Phase 4 伝播は別途 #27。

## 検証方法
1. `cargo xtask ci`（build + test + clippy + fmt）green。
2. `cargo run -p mycad-cli -- export examples/extruded_rect.mycad -o /tmp/rect.stl` → STL 生成、
   facet 数 12、Blender/MeshLab で 10×5×8 の角柱を確認。
3. `make_extrusion` 決定性テスト（IdGenerator::new(0) 2回で完全一致）。
