# Codex Findings (medium / low, non-blocking)

本 Issue で受け入れ判断した non-blocking 指摘の記録。`gate:adr-review` などの大きな決定には ADR を起こすが、ここはコード品質メモ。

## STEP 7.5 Round 2 — contrarian persona, C-F01 (medium)

**finding**: `FeatureCrud::insert` で semantic validation (`check_self_reference` / `check_refs_resolve_before` / `check_no_downstream_break`) が `next.validate()` (format-level) より先に走るため、空/不正 id を持つ feature がいきなり semantic エラーで上書きされる。

例:
- `Extrude { id: "", sketch: "unknown", .. }` → 期待: `InvalidName { value: "", reason: "..." }` だが、実際は `SketchNotFound { feature_id: "", sketch_ref: "unknown" }`
- `Cut { id: "", target: "", tool: "anything" }` → 期待: `InvalidName` だが、実際は `SelfReference { feature_id: "", ref_kind: "body" }` (id == target == "")

**判断**: 採用 (medium、非 block で merge をブロックしない)。

実害は軽微 (CLI が "insert failed: <semantic error>" を返すため、エラーメッセージの specificity がやや低い)。format-level エラーを先に出した方が診断が明快なので **改善余地あり** だが、Issue #263 の射程外 (4 variants は semantic 側で、validate 順序は別 axis)。本 Issue では記録のみ、修正は別 Issue または同一 PR の余力で対応する。

**suggested fix (将来)**: `FeatureCrud::insert` 入口で `check_self_reference` の前に `validate_identifier(feature.id(), "feature_id")` 相当の最小 sanity check を入れるか、`next.validate()` を semantic check より前に呼ぶ (前者の方が clone コストなし)。
