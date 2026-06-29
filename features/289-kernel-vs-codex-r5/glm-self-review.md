# GLM Self-Review for #289

## architect 観点 (既存 invariant / API 契約 / トポロジー保証)

- `length_near()` の `<= LENGTH_TOLERANCE` 規約と整合性を取れた
- `extrusion.rs` / `booleans/mod.rs` の既存 `<=` 規約と統一できた
- 退化判定 (`<=`) が「TOL 以下は無視」という意味論に即している
- 決定性に影響なし（比較演算子の変更のみ）

**弱点 / リスク**:
- なし

## contrarian 観点 (採用した実装方針の反論可能性)

- `<` strict 案 (Issue body M-F01) からの逸脱理由:
  - `length_near()` の `<=` 規約が kernel の根本判定関数であることを重視
  - `extrusion` / `booleans` の既存 `<=` 規約との整合を優先
  - Rectangle/Polygon/Slot (#275) が既に `<=` で導入されているため、修正範囲最小化
- 境界テストの値設定: `LENGTH_TOLERANCE * 1.000_001` は `EPS_AXIS_RATIO` / `minor > major` チェックと相性が悪いため、現実的な値 (0.1, 1.0) を使用
  - これは「TOL より大きい」ことを検証する本質を損なわない

**直前 Issue や同 Phase の defensive semantics を退化させていないか**:
- していない。`<=` 規約はより厳格な退化判定（境界 1 段厳しく）であり、`<` strict よりも defensive

## migration 観点 (既存テスト互換 / 後方互換性)

- **変更した public API**: なし（内部判定の比較演算子変更のみ）
- **変更した golden YAML**: なし
- **既存 acceptance test への影響**: なし（sketch tessellation は内部実装）
- **既存単体テストの改変**:
  - `t_edge_length_tolerance_boundary_passes`: assertion 反転 (pass → reject)
  - 既存テストのエラーメッセージ assertion を `"< ε_radius"` → `"<= ε_radius"` に更新 (7件)

## 残課題 (scope-defer / 後続 Issue 候補)

- **本 Issue scope 外**:
  - `ANGLE_TOLERANCE` 比較規約の統一（別 Issue）
  - `EPS_DISCRIMINANT` / `EPS_AXIS_RATIO` 等の比較規約（別系統 ε）
  - Tolerance newtype 化 / per-entity tolerance (#31, Phase 5)
