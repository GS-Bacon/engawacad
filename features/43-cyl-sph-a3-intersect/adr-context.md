# ADR-004 抜粋（#43 関連: トレランス方針）

本 Issue はテスト追加のみで新規幾何生成はないが、T34/T18 の浮動小数比較で参照する公差定数:

| 定数 | 値 | 用途 |
|------|----|------|
| `LENGTH_TOLERANCE` | `1e-9` (mm) | 距離・座標の絶対比較 |
| `ANGLE_TOLERANCE` | `1e-9` (rad) | 角度・パラメータの絶対比較 |
| `RELATIVE_TOLERANCE` | `1e-9` (無次元) | スケール比例の相対比較 |

- Phase 4 は per-entity トレラント方式（Parasolid/ACIS 流）。グローバル定数は `Tolerance::DEFAULT` 値として残存。
- #34（曲面 Boolean MVP）の対象に Cylinder×Sphere の軸整列 Boolean が含まれる。per-entity tolerance 埋め込みは Non-Goal。
- 本 Issue (#43) は #34 の sub-step（テスト hardening）であり、tolerant 比較を踏襲。T34 の center.z / normal / t_range 比較は `LENGTH_TOLERANCE` / `ANGLE_TOLERANCE` 近傍で近似 assert する。
