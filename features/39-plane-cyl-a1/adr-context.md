# ADR-004 抜粋 (Issue #39 設計レビュー用)

## 数値モデル: トレラント方式 (Decision 3 — Phase 4 (#31) で確定)

```
LENGTH_TOLERANCE = 1e-9  # 内部長さ比較の基準 (Rust constant)
ANGLE_TOLERANCE  = 1e-9  # ラジアン単位
```

Phase 4 (#34) は **per-entity tolerance ではなくグローバル公差** (上記定数) を使用。per-entity tolerance は Phase 5 以降の別 Issue。

## #34 の scope (ADR-004 §80 Decision 3 補足より)

> #34 曲面 Boolean MVP: Plane×Cylinder / Plane×Sphere / Cylinder×Sphere の軸整列 Boolean。per-entity tolerance 埋め込みは Non-Goal

## pcurve 採用 (ADR-004 §101)

`HalfEdge.pcurve: Option<Pcurve>` により half-edge ごとに面のパラメータ空間上の 2D 曲線を持つ (OpenCASCADE 慣行)。曲面交線エッジは `Curve2D::Line2D` (for Plane×Plane) / `Curve2D::Circle2D` (for Plane×Cylinder circle) を格納する。`Sampled2D` / `NURBS2D` は Phase 4 非対象。

## tessellation の面法線評価

`tessellate_solid` は頂点ごとに `Surface::normal_at_point` で法線を評価する (以前の 1 回評価は廃止)。trim 対応後も同様に頂点毎評価を維持する。

## earcutr 採用判断 (ADR-004 §80 Decision 3 との整合)

earcutr は f64 ベースで polygon-only の三角形分割ライブラリ、閾値フリー。ADR-004 のトレラント方式 (グローバル公差で入力を事前 gate する) と整合。
