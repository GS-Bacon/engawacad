# ADR-004 抜粋: A2 (Plane×Sphere Cut) に関連する決定事項

## 数値モデル (Decision 3 — #31 で確定)

Phase 4 はトレラント方式 (per-entity tolerance) を採用。ただし A2 の Plane×Sphere は:
- per-entity tolerance フィールドの埋め込みは **Non-Goal** (Phase 5 候補)
- グローバル定数 `LENGTH_TOLERANCE = 1e-9 (mm)` を引き続き使用する
- 大円ガードの条件 `d.abs() < LENGTH_TOLERANCE` は絶対比較として適切 (ADR-004 §公開公差定数)

`RELATIVE_TOLERANCE` ではなく `LENGTH_TOLERANCE` を使う理由: `d` は距離次元を持つ長さ値であり、スケール非依存の相対比較より絶対比較が適切。大円の実ユースケースは設計意図の間違いがほとんどで、誤差範囲内の "ほぼ大円" を通過させる必要はない。

## アルゴリズムの曲面型依存禁止 (Decision 1)

「アルゴリズムは曲面・曲線の型に非依存であること。平面・直線前提をアルゴリズムへ埋め込まない」

A2 への適用:
- partition.rs の sphere UV クリップ skip は A2 の対称性を生かした一時的な workaround。Cylinder と Sphere の両方に `!is_seam_periodic()` のような抽象的な判定を使うことが長期的目標だが、A2 では既存 `is_cyl` / `is_sph` フラグで動かす。
- この trade-off は Non-Goals に明記済み。

## Pcurve (Decision 3 補足)

`HalfEdge.pcurve: Option<Pcurve>` — #31 で `Curve2D::Line2D` + `Circle2D` 導入済み。

A2 で生成する:
- plane face inner loop の pcurve: `Curve2D::Circle2D` (plane UV 上の 2D 円)
- sphere face inner loop の pcurve: `Curve2D::Line2D` (緯度線 v=const) — assemble で格納するが tessellation では 3D circle から直接 UV project (seam ラップ回避)
