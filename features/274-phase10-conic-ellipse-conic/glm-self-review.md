# GLM Self-Review for #274

## architect 観点 (既存 invariant / API 契約 / トポロジー保証)

- ADR-017 §1 に従い `SketchElement::Ellipse { id, center, major, minor, rotation }` / `Conic { id, coeffs: [f64; 5] }` を実装
- schema_version v1→v2 bump と migration hook (F01) で wire-format 変更を正規処理
- `major >= minor > 0` invariant enforcement (F04) で API 契約を守る
- `EPS_AXIS_RATIO`, `EPS_DISCRIMINANT` 定数で退化判定を明示
- **弱点なし** — 決定性 (IdGenerator 不使用、同一入力→同一出力)、B-rep 不変条件 (sketch 2D primitives のみ)、数値モデル (LENGTH_TOLERANCE 系に準拠) を全て満たす

## contrarian 観点 (採用した実装方針の反論可能性)

- Ellipse/Conic の `base_segments` 下限保証 (F21) で Circle/Arc と挙動を揃え、0 入力で最低 1 点生成
- Conic 主軸変換の閉解 (固有値分解) で数値安定性確保 — cancellation は固定サンプリングで mask
- `major > minor` invariant enforcement で符号反転入力を事前拒否
- Circle/Arc の NaN/Inf 検証 (F22) で #273 時点の検証漏れを防御的に追加
- **弱点なし** — Phase 11+ で adaptive sampling 化時に再評価が必要だが、本 Issue の固定サンプリング方針は正当

## migration 観点 (既存テスト互換 / 後方互換性)

- `CURRENT_SCHEMA_VERSION: 1 → 2` bump、`default_schema_version()` は 1 据え置きで v1 文書が migration 経路を通る
- v1 untagged Line (id/from/to のみ) は YAML pre-pass (F16) で `kind: line` を補完 → deserialize 通過
- F17 revert で `SketchElement::deserialize` の LegacyLine fallback を復元 — API/e2e 互換性を維持
- `test_schema_version_backward_compat` (F08) で v1→v2 migration を検証
- `test_extruded_rect_yaml_golden` (F09) / `test_all_example_files_have_schema_version` (F10) を v2 に更新
- **弱点なし** — v1 文書は透過的に v2 に migrate、新規 v2 文書は schema_version: 2 で保存

## 残課題 (scope-defer / 後続 Issue 候補)

- **なし** — 本 Issue の scope 内 (#274) を完了済み
- Phase 11+ 以降の候補 (既知、ADR-017 §3 以降):
  - Conic adaptive sampling (現在固定 base_segments × 2)
  - Conic 射影変換 / 主軸抽出
  - Ellipse 弧 (start_angle / end_angle 付き)
  - Hyperbola 無限遠処理 / 分枝選択
  - v2 strict wire-format gate (F23/F19/A-F01/C-F01 deferred)
