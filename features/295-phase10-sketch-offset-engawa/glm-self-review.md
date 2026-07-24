# GLM Self-Review for #295

## architect 観点 (既存 invariant / API 契約 / トポロジー保証)
- 満たしている事項:
  - `Feature::SketchOffset` の YAML 型定義は `serde` tagged 実装で決定的
  - `apply_sketch_offset` / `apply_sketch_offset_build` は純関数で決定性保証 (T01 100 runs で検証済み)
  - `selection` 順序不変性: BTreeSet + source 順 iteration (T01a で検証済み)
  - Circle-only 契約は `apply_sketch_offset_build` で enforce、build-level で Line/Arc を拒否
  - A01 fix: `refs_resolve_in_state()` と `check_refs_resolve_before()` で Circle-only validation を二重化
  - `Feature::SketchOffset` に rustdoc で build-level 契約（Circle 単一のみ許可）を明記
  - `golden_sketch_offset` byte-identical テストで YAML serialization の決定性を検証
  - `sketch: String` で Extrude/ExtrudeCut と対称な wire format を維持
- 弱点 / リスク:
  - `check_refs_resolve_before()` に `SketchOffset` 固有ロジックが追加されており、`refs_resolve_in_state()` との重複がある (`feature_crud.rs:233-257` と `:607-620`)
  - 将来的に `refs_resolve_in_state()` が「ref が存在する」だけをチェックする設計に戻る場合、双方の変更が必要

## contrarian 観点 (採用した実装方針の反論可能性)
- 採用した方針と理由:
  - Line/Arc offset を kernel-level pure function に留め、build-level は Circle-only に制限
    - 理由: corner join/trim が Trim/Extend (#276) 依存で Phase 10 scope 外。closed loop の連結性維持は本 Issue 範囲外
  - 負 sweep Arc を `InvalidParameter { kind: "arc_negative_sweep" }` で拒否
    - 理由: 符号規約が正 sweep 前提で負 sweep は #276 or #278 で扱うべき
  - `SelfIntersection` → `DegenerateSketchElement` 代替
    - 理由: 一般判定は Trim/Fillet と共に別 Issue、collapse は `<= EPS_LENGTH` で十分検出可能
  - Plan narrowing（Circle-only build contract）は仕様上の妥協点だが、Line/Arc の corner 非連結問題を Trim/Extend (#276) に先送りするという明示的設計判断として妥当
  - Schema version non-bump は additive variant 追加として既存 pattern (#274, #275) と整合
- 直前 Issue や同 Phase の defensive semantics との整合性:
  - ADR-018 (`<= EPS_LENGTH` 統一規約) を遵守
  - ADR-005 EntityRef scope を遵守 (`Sketch.sketch: String`、EntityRef は B-rep 専用)

## migration 観点 (既存テスト互換 / 後方互換性)
- 変更した public API:
  - `Feature::SketchOffset` variant 追加 (additive、既存 variant に影響なし)
  - `apply_sketch_offset_build` / `apply_sketch_offset` (新規公開関数、既存コードは呼ばない)
  - `refs_resolve_in_state()` の `SketchOffset` 分岐 (既存コードは `_ => true` 経路を使用)
  - `check_refs_resolve_before()` に `SketchOffset` 追加チェック (既存 feature は影響なし)
- golden YAML 変更: なし (新規 `golden_sketch_offset` test のみ追加)
- 既存 acceptance test 改変: なし (新規 `t19_sketch_offset_line_profile_rejected` のみ追加)
- `Feature::SketchOffset` variant 追加は既存 variant に非破壊
- `examples/sketch_offset.engawa` の schema_version: 2 は既存 Circle/Arc 表現と整合
- golden test 追加のみで既存テスト改変なし

## 残課題 (scope-defer / 後続 Issue 候補)
- Line/Arc offset の build-level 接続 (corner join/trim #276 依存)
- 負 sweep Arc の offset (#276 or #278)
- Ellipse/Conic offset の数値反復解法 (別 ADR + Issue 提案)
- `refs_resolve_in_state()` と `check_refs_resolve_before()` の validation 重複解消 (refactor pass)
- Multi-contour offset (Phase 11+)
- Property test 網羅 (Refactor Pass)
- selection per-element split は Trim 系と別 Issue で検討
