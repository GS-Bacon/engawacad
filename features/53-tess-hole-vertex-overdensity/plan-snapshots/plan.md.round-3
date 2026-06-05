## Context

Issue #53 (bug, kernel): `examples/boolean_cut_cylinder_hole.mycad` をメッシュ化すると、穴あき天面（正方形に円穴のドーナツ面）の穴フチに頂点が約2081個（正常の約58倍）生成され、極薄三角形が密集して「白いエッジ」アーティファクトが出る。原因は二重離散化:

1. Boolean cut が穴フチの円を **64本の小円弧エッジ**（各 `Curve::Circle`、t_range が約2π/64の部分弧）に分割して保存（`partition.rs:11` `ANGULAR_SEGMENTS_DEFAULT=64`、`assemble.rs:443-481` `circle_curve_for_edge`）。
2. テッセレーションの `collect_loop_points` (`tessellation/mod.rs:380`) が **エッジ1本ごとに `segments`(=32)点**サンプリング → 64×32≈2048点。

直線エッジは `sample_segment` が1点しか返さないため無傷で、円弧エッジだけが膨張する。

修正方針: テッセレーション側で、円弧エッジを**弧の角度幅に比例した点数**でサンプリングする（上流の Boolean トポロジーは変更しない）。

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `collect_loop_points` の円弧サンプリングを弧幅比例化（edge.curve / pcurve 両経路） | Boolean cut の上流トポロジー変更（穴フチを円エッジ1本に統合する案B） |
| 共有ヘルパー `arc_segment_count` を `geometry/math.rs` に追加 | `Curve::sample_segment` / `Curve2D::sample_segment` の既存シグネチャ・意味論の変更 |
| `boolean_cut_cylinder_hole` の頂点数回帰テスト追加 | 隣接面間の watertight 性の新規保証（現状未保証、本Issueで新たに保証しない） |
| 全円エッジ(2π)の非回帰（従来通り `segments` 点） | `tessellate_face_earcut` の二重 collect 等のリファクタ |

## Non-Goals
- 案B（Boolean が穴フチを単一円エッジで持つ）: 全 Boolean 共通の `partition.rs`/`assemble.rs` 中枢を触り回帰リスクが高いため本Issueでは行わない。
- 隣接面の T-junction 解消・watertight 厳密化: 既存の振る舞い（未保証）を変えない。
- `tessellate_face_earcut` 内の inner loop 二重 collect の最適化（インデックス整合のため現状維持）。

## 実装対象
<!-- Issue: #53 -->
影響クレート/ファイル:
- `crates/mycad-kernel/src/geometry/math.rs` — 新規ヘルパー追加
- `crates/mycad-kernel/src/tessellation/mod.rs` — `collect_loop_points` の2ブランチ修正
- `crates/mycad-build/tests/hole_tessellation_acceptance.rs` — 新規 acceptance テスト
- `crates/mycad-build/tests/examples_smoke.rs` — 既存 `boolean_cut_cylinder_hole` は変更不要（acceptance で別途検証）

新規関数シグネチャ:
```rust
/// 弧の角度幅 |t_end - t_start| に比例した分割数を返す（最低1）。
/// base_segments は全周(2π)に対する目標分割数。
pub fn arc_segment_count(t_start: f64, t_end: f64, base_segments: usize) -> usize
```
実装:
```rust
pub fn arc_segment_count(t_start: f64, t_end: f64, base_segments: usize) -> usize {
    let span = (t_end - t_start).abs();
    let n = (base_segments as f64 * span / (2.0 * std::f64::consts::PI)).ceil() as usize;
    n.max(1)
}
```

`collect_loop_points` の修正（`crates/mycad-kernel/src/tessellation/mod.rs:362-381`）:

before:
```rust
let seg_points = if let Some(ref pcurve) = he.pcurve {
    let face = &solid.faces[face_idx];
    let uv_samples = match pcurve.curve_2d() {
        Curve2D::Line2D { .. } => {
            vec![pcurve.evaluate(pcurve.t_range()[0])]
        }
        Curve2D::Circle2D { .. } => pcurve.sample(segments),
    };
    uv_samples
        .into_iter()
        .map(|(u, v)| face.surface.evaluate(u, v))
        .collect()
} else {
    let (t_start, t_end) = if he.forward {
        (edge.t_range[0], edge.t_range[1])
    } else {
        (edge.t_range[1], edge.t_range[0])
    };
    edge.curve.sample_segment(t_start, t_end, segments)
};
```
after:
```rust
let seg_points = if let Some(ref pcurve) = he.pcurve {
    let face = &solid.faces[face_idx];
    let uv_samples = match pcurve.curve_2d() {
        Curve2D::Line2D { .. } => {
            vec![pcurve.evaluate(pcurve.t_range()[0])]
        }
        Curve2D::Circle2D { .. } => {
            let [ts, te] = pcurve.t_range();
            let n = arc_segment_count(ts, te, segments);
            pcurve.sample(n)
        }
    };
    uv_samples
        .into_iter()
        .map(|(u, v)| face.surface.evaluate(u, v))
        .collect()
} else {
    let (t_start, t_end) = if he.forward {
        (edge.t_range[0], edge.t_range[1])
    } else {
        (edge.t_range[1], edge.t_range[0])
    };
    let n = match &edge.curve {
        Curve::Circle { .. } => arc_segment_count(t_start, t_end, segments),
        Curve::Line { .. } => segments,
    };
    edge.curve.sample_segment(t_start, t_end, n)
};
```
（`use` に `crate::geometry::math::arc_segment_count` と `crate::geometry::curve::Curve` を追加。`tessellate_face_earcut` の `all_points` 再 collect も同じ `collect_loop_points` を通るため自動的に整合し、earcut の flat_coords と頂点バッファの点数は一致する。）

## 設計方針
- **決定性**: `arc_segment_count` は `ceil` を用いた純粋関数で、同一入力→同一の整数分割数。`IdGenerator` 非依存。サンプリング点座標も従来の `evaluate` を流用するため決定的。
- **B-rep トポロジー妥当性**: トポロジー（V/E/F）は変更しない。メッシュ化の点数のみ変わる。Euler-Poincaré は B-rep 側で従来通り成立（本修正は brep を触らない）。
- **退化幾何の扱い**: `arc_segment_count` は `n.max(1)` で最低1点を保証。`span≈0` の極小弧でも1点。`segments` は呼び出し元で `angular_segments.max(3)` 済み。
- **derive 規約**: 新規型なし。既存型のシグネチャ不変。
- **エラーハンドリング**: 新規エラー条件なし（`thiserror` 追加不要）。
- **workspace.dependencies**: 新規依存なし。

### 数値モデル
- 本修正はサンプリング点数の tolerance を新規導入しない。分割数 = `ceil(base * |Δt| / 2π)`（base=`angular_segments`、既定32）。
- 全周(2π)→ base 点（現状維持）、1/64弧→ `ceil(32/64)=1` 点。穴フチ計 約64点。
- 最小弧幅は `2π/base_segments`。base_segments=N のとき 1/N 弧 → `ceil(N/N)=1`、1/(2N) 弧 → 1点（境界の挙動）。base=64 のとき 1/64 弧は `ceil(64/64)=2` 点になる点に留意（既定 base=32 では 1 点）。(AM03)
- **ε_area（T04 退化三角形判定）**: 既存先例に従い `area_eps = LENGTH_TOLERANCE * LENGTH_TOLERANCE` (= 1e-18) を使用。出典: `crates/mycad-kernel/src/booleans/mod.rs:146`、`LENGTH_TOLERANCE=1e-9` (`crates/mycad-kernel/src/geometry/math.rs:10`)。新規定数は定義せず既存定数を参照する。(NU01/AM02)
- ADR-004: tolerant/exact の判定ロジックには非干渉（点数のみ）。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `boolean_cut_cylinder_hole` を2回 build+tessellate し全 positions/indices 一致（修正後の決定性検証。修正前との比較ではない） | `assert_eq!` |
| T02 | 正常系/全頂点数の粗ガード | 同例の全頂点数が `< 400`（バグ時 2202）。導出: 天面68 + 底面4 + 箱側面4×4 + 穴側壁（≤128: 上下リム各64） ≈ 216 が上限見積り、安全側に 400 を粗上限とする | `assert!(total < 400)` |
| T03 | 頂点数境界(天面) | z≈5.0(±LENGTH_TOLERANCE) の頂点数 `<= 80`。導出: 外周正方形 4 (直線4辺×1点) + 内周リム 64 (円弧64サブエッジ×1点) = **68** | `assert!(top_count <= 80)` |
| T04 | 退化三角形なし | 天面に属する三角形の面積がすべて `> ε_area`（ε_area = `LENGTH_TOLERANCE*LENGTH_TOLERANCE` = 1e-18） | `assert!(no zero-area)` |
| T05 | 非回帰(全円) | `arc_segment_count(0, 2π, 32)==32`、`(0, π/2, 32)==8`、`(0, 2π/64, 32)==1`、`(0,0,32)==1` | 単体 unit test (`geometry/math.rs` inline) |

- T01〜T04 は `crates/mycad-build/tests/hole_tessellation_acceptance.rs`（integration、STEP 5.5 で `#[ignore]` スケルトン先置き）。
- T05 は `geometry/math.rs` の `#[cfg(test)] mod tests`（STEP 6 で GLM が inline 追加）。
- ε_area は既存先例 `LENGTH_TOLERANCE*LENGTH_TOLERANCE` (= 1e-18, `booleans/mod.rs:146` と同方式) を使用。新規定数は定義しない。

## 幾何的不変条件チェックリスト
本Issueは Boolean/Partition/Assemble を変更せず、テッセレーションのサンプリング点数のみ変更するため大半 N/A。
- [ ] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか → **N/A（partition/assemble 非変更）**
- [ ] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか → **N/A**
- [ ] flip_normals / same_sense の意味論が明確か → **N/A（法線処理は既存 `same_sense` 分岐を踏襲）**
- [ ] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか → **N/A**
- [x] サンプリング点数削減後も earcut の flat_coords と頂点バッファ（`all_points`）の点数・順序が一致するか（両者とも `collect_loop_points` 経由で整合）

## Verification
- `cargo test -p mycad-build --test hole_tessellation_acceptance`（T01〜T04）
- `cargo test -p mycad-kernel geometry::math`（T05）
- `cargo xtask ci` で全体 green
- 目視: `mycad export examples/boolean_cut_cylinder_hole.mycad -o /tmp/hole.stl` の STL サイズが 411KB から大幅減、`mycad view` で白エッジが消える
