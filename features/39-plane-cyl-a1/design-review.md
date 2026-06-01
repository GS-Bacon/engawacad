issues:
  - id: R01
    severity: high
    section: "設計方針 > 4. assemble.rs の Face 構成 / 5. edge curve 復元 / 7. Cylinder trim"
    finding: "per-chord provenance を導入している一方で、FaceFragment に outer boundary 用の t_range / pcurve 配列が定義されておらず、pcurve attach も `build_inner_loop` に限定されている。A1 の cylinder 側面は outer_loop 上の circle edge の pcurve から `v_min`/`v_max` を読む設計なので、このままでは trimmed cylinder face の外周 HE に必要な pcurve と t_range が載らない。"
    suggestion: "outer boundary にも `boundary_t_ranges` と per-side pcurve/t_range を保持し、通常の outer-loop edge 組み立て時にも各 face 側 half-edge へ pcurve を付与すること。`build_inner_loop` 専用ではなく、outer/inner 共通の provenance attach 経路に揃える。"
  - id: R02
    severity: high
    section: "テスト計画 > T04"
    finding: "Euler 検証が `V-E+F-L_inner = 2` になっており、レビュー基準の Euler-Poincaré `V - E + F = 2(S - H)` と一致していない。inner loop は face 境界の表現であって、閉じた solid のグローバル Euler 特性から直接減算する量ではない。A1 の blind-hole なら `S=1, H=0` なので期待値は `V-E+F=2`。"
    suggestion: "T04 は `V - E + F = 2(S - H)` をそのまま検証し、A1 では `2` を期待値にすること。loop 数の確認が必要なら別アサーションに分離する。"
  - id: R03
    severity: high
    section: "設計方針 > 7. tessellation/mod.rs — Cylinder trim 分岐"
    finding: "partial-height cylinder tessellation の発火条件が『outer_loop が 4 HE より多い場合』になっているが、同じプラン内で A1 の cylinder 側面は `outer_loop = 4 HE` と定義されている。記述どおり実装すると A1 自身が trimmed-cylinder 分岐に入らない。"
    suggestion: "分岐条件を A1 の実トポロジーと一致させること。たとえば `4 HE` を含めるか、HE 数ではなく『constant-v の円周 pcurve 2 本と seam 2 本を持つ non-full-height cylinder face』のような幾何条件で判定する。"

verdict: fail