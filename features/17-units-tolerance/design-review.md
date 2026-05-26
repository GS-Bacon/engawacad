issues:
  - id: R01
    severity: high
    section: "決定事項 1 / 実装対象 > docs/decisions/004-freeform-geometry-commitment.md"
    finding: "ADR-004 で『角度=rad』を全体前提のように固定しているが、公開フォーマットは既に `docs/file-format.md` と `crates/mycad-format/src/component.rs` で `Transform.rotation` を『degrees』と定義している。`Document` を変えない方針自体はよいが、外部フォーマット(deg)と kernel 内部幾何(rad)の境界変換/責務が設計に書かれておらず、単位契約が二重化したままになる。"
    suggestion: "ADR に『kernel 内部幾何は rad、`.mycad` の `Transform.rotation` は引き続き deg』を明記し、deg↔rad 変換境界を format/build 層の責務として固定すること。あわせて `docs/file-format.md` と `component.rs` の記述整合も同じ Issue で確定すること。"
  - id: R02
    severity: medium
    section: "決定事項 2 / 実装対象 > crates/mycad-kernel/src/geometry/math.rs, surface.rs, lib.rs"
    finding: "`AREA_EPS` は heuristic だから非公開に据え置く一方で、`APEX_TOLERANCE` は `Cone::normal_at` の特異点ガードでしかないのに crate root まで再エクスポートして『4 公開公差』に含めている。一般的な長さ/角度/相対公差と性質が異なる実装詳細を public API に固定してしまい、Phase 4 の数値モデル見直し時の差し替え点を増やす。"
    suggestion: "`LENGTH_TOLERANCE` / `ANGLE_TOLERANCE` / `RELATIVE_TOLERANCE` だけを共通公開契約にし、`APEX_TOLERANCE` は `surface` 内部の `pub(crate)` か private 定数として扱うこと。ADR には『cone apex singularity guard は内部実装詳細』として別枠で記録する程度に留めること。"

verdict: fail