<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## Round 2

### IN01 (低、棄却)
- persona: invariant
- 指摘: plan.md は math.rs に EPS_AXIS_RATIO/EPS_DISCRIMINANT を追加すると記載するが、ADR-017 §2 では tolerances.rs に追加と記載されている
- 棄却理由: 実コード上 `LENGTH_TOLERANCE` `ANGLE_TOLERANCE` は `crates/engawa-kernel/src/geometry/math.rs` に存在 (#273 と整合)。`tolerance.rs` (singular) は `Tolerance` 型用で定数置き場ではない。ADR-017 §2 の "tolerances.rs" は方向性表記で、ファイルは未存在。本 Issue では既存配置 (math.rs) に統一する方が分割コストを払わずに済む

## Codex deferred findings (post-merge follow-up Issue 候補)

本 Issue (#274) で Codex から繰り返し指摘されたが、scope/互換性配慮で deferral とした項目:

### Codex C-F01 / A-F01 / A-F01 (Round 1, 3, 4): SketchElement::deserialize LegacyLine fallback の v2 strict gate

- 内容: `schema_version: 2` 文書でも `kind:` 無し sketch 要素を `Line` として受理する (= legacy fallback 経由)
- 本 Issue では deferred (F19 / F23 / F27 で文書化済)
- 理由:
  1. F13 で一度 strict gate を実装したが、axum Json extractor (API) / Playwright e2e / 全 examples の broad 互換性を壊した
  2. Codex の指摘は legitimate だが「v2 で kind 必須」は architectural decision で別 ADR が必要
  3. 本 Issue scope は "Ellipse + Conic 追加" であり、wire-format strictness は別 Issue 化が適切
- post-merge 対応: Phase 11+ で wire-format strictness が必要になったタイミングで別 ADR + Issue 起票
