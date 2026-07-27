# GLM Self-Review for #299 (fix phase)

本レビューは STEP 7.5 / 6.7 合流後の修正 (Fix 1 + Fix 2) に対する自己批評。
実装本体は前段で CI green 済み。本フェーズでは `crates/**/src/` に 2 点のみ変更。

## architect 観点 (既存 invariant / API 契約 / トポロジー保証)

- 満たしているもの:
  - `sketch_selection_resolves` を `feature_crud.rs` に新設し、`refs_resolve_in_state`
    (edit/suppress/delete/reorder 経路) と `check_refs_resolve_before` (insert 経路) の
    selection 検証ロジックを統一した。SketchFillet の T10 契約
    (`sketch_fillet_acceptance.rs::t10_crud_gate_rejects_rename_breaking_fillet`)
    と同じ element-level 検証が SketchMirror / SketchPattern{Linear,Circular} でも
    働くようになった (`t11_crud_gate_edit_breaks_mirror_selection`,
    `t_crud_pattern_edit_breaks_selection_{linear,circular}` が green)。
  - Pattern の `count >= 2` 限定 gate は insert 経路と edit 経路で一貫
    (`t_crud_pattern_edit_n1_selection_ignored` green)。count=1 は両経路で no-op。
- 弱点 / リスク:
  - `sketch_selection_resolves` は `feature_crud.rs` 内の既存 `element_id_of` を再利用
    しているが、新設した helper が `feature_crud.rs` のみで使われている (他クレート
    からの再利用可能性はあるが本 Issue の scope 外)。ファイルを跨ぐ DRY 機会の放置
    ではなく、適切なカプセル化。
  - SketchMirror arm は仕様上 `count` を持たないため、Pattern のような count gate
    が存在しない。これは Mirror の仕様 (selection 空でなければ常に要素を複製しようと
    する) と整合しているが、Mirror の `axis_p1==axis_p2` のような幾何退化は
    引き続き `apply_sketch_mirror` の kernel fail-fast に依存している
    (本 Issue の scope 外だが、edit 経路でも事前に弾けるチャンス was 意図的に棄却)。

## contrarian 観点 (採用した実装方針の反論可能性)

- 却下した代替案と理由:
  - 「`sketch_selection_resolves` を SketchFillet/SketchChamfer にも適用して
    共通化する」—— SkFillet/Chamfer は `find_adjacent_pair` による「隣接性」
    という追加の不変条件を持つため、単なる id 解析とは別物。共通化すると
   後者の契約を損なうため不採用。
  - 「count > MAX_PATTERN_COUNT を `u32::MAX` まで許す」—— self-review A1 が
    実測した OOM リスクを考えると明らかに不適切。10_000 は実用上十分大きい
    (単一 sketch の 2D 要素でこの数を超えることは現実的でない)。
- defensive semantics の退化チェック:
  - 既存の insert 経路 gate が `count >= 2 && !selection.is_empty()` だったのに対し、
    edit 経路は `count >= 2` だけ (`selection.is_empty()` チェックなし)。これは
    意図的: `sketch_selection_resolves` 内部で空 selection を "all resolves" と
    扱うため、edit 経路でも空 selection は常に true となり、insert 経路の
    `!selection.is_empty()` gate と**意味論的に等価** (空 selection なら gate は発動しない)。
    したがって semantic 退化はしていない。

## migration 観点 (既存テスト互換 / 後方互換性)

- 触った public API:
  - `engawa_kernel::geometry::sketch_pattern::MAX_PATTERN_COUNT` (新規 `pub const`)。
    既存関数のシグネチャ変更なし。新 const の追加のみで後方互換。
  - `engawa_build::feature_crud::FeatureCrud::insert/edit/suppress/reorder/delete` の
    public API 変更なし。内部ロジックの強化 (Mirror/Pattern arm の element-level 検証) のみ。
  - `KernelError::InvalidParameter` の kind 文字列のみ新規追加
    (`sketch_pattern_{linear,circular}_count_too_large`)。既存 kind 文字列の変更なし。
- golden YAML / 既存 acceptance test の改変:
  - `examples/sketch_pattern.engawa` は今回触っていない (前段で作成済み)。
  - `crates/engawa-build/tests/sketch_pattern_acceptance.rs` /
    `sketch_mirror_acceptance.rs` は Claude が確定済みで、本フェーズでは**無変更**。
    (テスト側が要求する仕様に src 側を合わせた形なので、テストを書き換える必要なし。)

## 残課題 (scope-defer / 後続 Issue 候補)

1. **#331 (横断 false-reject)**: `sketch_selection_resolves` は元
   `CreateSketch.profile` 基準なので、先行 Fillet/Chamfer/Offset/Mirror が挿入した
   派生 id (`{a}_{b}_fillet_arc`, `{id}_offset`, `{id}_mirror_1`,
   `{id}_pattern_linear_{k}` 等) を edit 後経路でも解決できない。本 Issue では
   Mirror と同じ制約として受容 (`t_known_limitation_mirror_derived_elem_false_reject`
   green で既存契約を保持)。根本修正は current profile を逐次構築する simulate_history
   の拡張が必要 (#331 で追跡)。
2. **Codex suggestion 後半 (未採用)**: 「`selection=[]` かつ profile が
   Ellipse/Conic を含む場合は reject」という insert/edit 横断の追加軸。
   別軸の false-accept 対策だが、insert gate も同時に変えないと非対称を生むため
   本 Issue では見送り。#331 で対応可能。
3. **Mirror arm の幾何退化の事前検出**: axis_p1==axis_p2 を edit 経路でも弾けるが、
   本 Issue の scope 外 (kernel fail-fast がカバー)。
4. **`MAX_PATTERN_COUNT` の調整可能性**: 現状 10_000 で固定。ユーザがパターン駆動の
   周期構造をしたい場合 (例: 歯車の 1000歯、grating の 5000 line) には十分だが、
   将来的に「ユーザ設定で上限を引き上げる」オプションが必要になるかもしれない。
5. **`sketch_pattern_*_count_too_large` のエラーメッセージ**: 現状 kind 文字列のみで
   ユーザへの案内が薄い。CLI でこのエラーを見たとき「上限 N です」と出すには
   別 layer でのメッセージ整形が必要 (本 Issue の scope 外)。
