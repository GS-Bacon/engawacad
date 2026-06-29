# GLM Self-Review for #288

## architect 観点 (既存 invariant / API 契約 / トポロジー保証)

- 既存の `validate_profile_closed` と同じパターンで `validate_sketch_profile_contours` を実装。純粋関数で副作用なし、同一入力→同一出力が保証される。
- `KernelError::InvalidParameter { kind: "profile" }` は既存のエラー variant を再利用しており、新規 variant 追加なしで API 契約維持。
- `is_closed_primitive` は exhaustive match で実装し、Phase 11+ で新 variant が追加されたとき compile-time で検出できる構造にした。
- `ANGLE_TOLERANCE` は既存定数を再利用し、kernel と整合した判定基準を維持。
- **弱点**: Conic を closed 扱いに含めたが、これは保守的な判定。実際には ellipse (closed) / parabola (open) / hyperbola (open) を区別するには係数解析が必要で、今回は scope 外とした。Phase 11+ で精密化時に影響する可能性があるが、defensive reject の前提上後から緩和しても破壊的変更ではない。

## contrarian 観点 (採用した実装方針の反論可能性)

- **Option A (正規実装)** ではなく **Option B (reject)** を採用した理由: Phase 10 は SketchElement 1 要素を前提としており、multi-contour 正規実装は `make_extrusion` signature 変更が必要な Phase 11+ scope。defensive guard は将来の拡張で削除/緩和可能。
- **反論可能性**: `[Circle, Line]` を reject しているが、Line が Circle の接線として機能するような特殊ケース（例: Line が Circle の一点から伸びる）であれば、幾何的には意味を持つ profile かもしれない。しかし現行実装ではどのみち flat_map で壊れるため、Phase 11 multi-contour 実装まで正当な reject。
- Arc の full-circle 判定で `n >= 1.0 && (sweep - n * TAU).abs() <= ANGLE_TOLERANCE` を採用 — `sweep >= TAU - ANGLE_TOLERANCE` の簡易判定でも十分では？（ただし現行実装は `n * TAU` で正整数倍対応しているため、整合を優先）
- 直前 Issue や同 Phase の defensive semantics を退化させていない: `validate_profile_closed` (既存) と同レイヤーで早期 reject しており、既存挙動（make_extrusion downstream の is_convex/is_simple チェックで同 error を返す）より明示的。

## migration 観点 (既存テスト互換 / 後方互換性)

- public API 変更なし: 既存の `build_bodies_from_features` signature 変更なし。
- golden YAML 変更なし: `.engawa` ファイル形式に変更なし。
- 既存 acceptance test を改変していない: `create_sketch_closed_acceptance.rs` は新規ファイルで既存 test を触っていない。
- 既存 extrude/extrude_cut テスト群 (face_sketch_extrude_acceptance.rs, face_sketch_extrudecut_acceptance.rs) はいずれも単一 primitive profile で使用しているため、今回 guard の影響を受けず regression なし。

## 残課題 (scope-defer / 後続 Issue 候補)

1. **Conic closed/open 厳密判定**: Phase 11+ multi-contour 実装時に、Conic 係数を解析して ellipse / parabola / hyperbola を区別し、ellipse のみを closed 扱いにする精密化が必要。
2. **Multi-contour `make_extrusion`**: Phase 11+ で `Vec<Vec<[f64;2]>>` のような contour-by-contour 入力へ signature 変更し、複数閉曲線 profile を正規サポート。
3. **T03 が downstream luck で pass する**: 現行 make_extrusion の is_convex/is_simple チェックでも同 error を返すため pre-fix でも pass する。post-fix は早期に reject する設計で、これは改善だが regression guard としての価値は将来的に減る（downstream 実装が変われば fail する可能性）。
4. **Arc の sweep == 0 (角度差ゼロ) 場合**: 既存 `tessellate_sketch_element` が `DegenerateSketchElement` で reject するため guard では false で OK としているが、将来の実装変更時は要注意。
