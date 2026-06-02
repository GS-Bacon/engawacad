issues:
  - id: R01
    severity: high
    section: "テスト計画 > T08-T09"
    finding: "disjoint 系の期待値が広すぎる。T08 は `Ok(_) | Err(KernelError::_)`、T09 は `Err(KernelError::_)` を許容しており、既存の `DisjointFuseResult` / `EmptyBooleanResult` 契約と違う no-op・誤成功・誤 variant でもテストが通ってしまう。"
    suggestion: "Fuse の disjoint は `Err(KernelError::DisjointFuseResult)`、Intersect の disjoint は `Err(KernelError::EmptyBooleanResult)` まで固定し、退化系の受け入れ条件を具体化すること。"
  - id: R02
    severity: medium
    section: "テスト計画"
    finding: "golden YAML ラウンドトリップ検証が計画にない。T01 は 2 回 build の同一性だけで、`Solid` の `Curve::Circle` / pcurve / inner_loop を含む直列化互換性を検証できない。"
    suggestion: "Fuse/Intersect の結果 `Solid` に対して `serde_yaml::to_string` → `from_str` → `to_string` の byte-identical か、少なくとも roundtrip 後の topology と tessellation の同値を確認するテストを追加すること。"
  - id: R03
    severity: medium
    section: "設計方針 > 5. 退化幾何の扱い"
    finding: "`LENGTH_TOLERANCE` / `ANGLE_TOLERANCE` をどこで使うかが明文化されていない。`v_cut` の `[v_bot,v_top]` 判定、`normal ≈ +Z` 判定、disjoint / multi-plane 境界付近の扱いが実装者依存のままになる。"
    suggestion: "既存 tolerance 定数に結び付けて、範囲判定・法線比較・体積/座標比較の閾値をプランに明記すること。"

verdict: fail