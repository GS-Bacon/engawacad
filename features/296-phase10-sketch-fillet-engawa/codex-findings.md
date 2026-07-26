# Codex final gate (STEP 7.5) — non-blocking findings

verdict: pass (blocking=0, medium 2件)

## A01 (medium)
- file: `crates/engawa-kernel/src/geometry/sketch_fillet.rs:57`
- finding: ゼロ長入力の判定が `len_a || len_b` を1本化しており、`elem_b` だけが退化している場合でも `DegenerateSketchElement.element_id` に常に `a_id` を返す。退化要素の特定が誤るため、呼び出し側の診断と退化テストが実入力とずれる。
- suggestion: `len_a` と `len_b` を個別に判定し、それぞれ `a_id` / `b_id` を返すよう分岐する。あわせて `elem_b` のみゼロ長な回帰テストを追加する。
- 対応: non-blocking のため本 Issue では見送り。次の kernel/geometry 系 Issue または Refactor Pass で対応候補として記録。

## M01 (medium)
- file: `crates/engawa-build/tests/sketch_fillet_acceptance.rs:163`
- finding: `t08_cw_profile_negative_sweep` は前半で pure helper の負 sweep を確認しているが、後半の build 経路では `Feature::SketchFillet` を入れずに生の CW 矩形をそのまま `Extrude` している。テスト名と計画が要求する「負 sweep fillet が build まで通る」統合経路は未検証。
- suggestion: T08 の feature 列に `Feature::SketchFillet` を含め、`SketchFillet -> tessellate_sketch_element -> make_extrusion` の実経路で build 成功を検証する。
- 対応: non-blocking のため本 Issue では見送り。テストカバレッジの既知ギャップとして記録 (次 Issue で強化候補)。
