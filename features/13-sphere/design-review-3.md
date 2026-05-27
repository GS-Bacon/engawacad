issues:
  - id: R01
    severity: high
    section: "設計方針 > テッセレーション > canonical 球 face の検証"
    finding: "canonical 判定が seam の `Curve::Circle` の center/radius と半周 span に寄りすぎており、`normal = -Vec3::y()` の canonical seam であること、および `curve.evaluate(t_range[0/1])` が `edge.vertices[0/1]` に一致することが明記されていない。これだと別の大円 seam や、頂点と一致しない `t_range` を持つ不整合な face を full sphere として受理しうる。"
    suggestion: "canonical 条件に `Circle.normal == -Vec3::y()`（符号許容なら向きも含めて規定）と、`evaluate(t_range[0]) ≈ vertices[0]` / `evaluate(t_range[1]) ≈ vertices[1]` の検証を追加すること。"
  - id: R02
    severity: medium
    section: "API・型設計 > TessellationOptions"
    finding: "`UvSphere` が `n_v = (angular_segments / 2).max(2)` を内部導出するため、公開 API の `axial_segments` が sphere では実質無視される。`tessellate_solid_with` の解像度契約が surface 種別ごとに変わるが、その方針が設計として固定されていない。"
    suggestion: "sphere でも `axial_segments` を stack 数として尊重するか、少なくとも『sphere は angular のみを使い axial は無視する』契約を明記し、CLI/API の既定値とテストをそれに合わせて固定すること。"
  - id: R03
    severity: medium
    section: "テスト計画 > T16 self-adjacent 許容"
    finding: "T16 は「妥当性検証を通る」とあるが、現行 kernel には self-adjacent periodic face を判定する明示的 validator が見当たらない。`CLAUDE.md` の 1 行追記だけでは、この新しい例外形を実行時不変条件として守れない。"
    suggestion: "`topology.rs` のコメント/不変条件も更新したうえで、self-adjacent face を許容する具体的な validator か、少なくとも edge-HE 対応・loop closure・face/shell 整合を機械的に検証する helper を追加すること。"
  - id: R04
    severity: medium
    section: "テスト計画 > 球面テッセレーション"
    finding: "T07/T09/T15 は default 32 分割前提の確認に寄っており、重点確認点に挙がっている `n_v = (angular/2).max(2)` の妥当性が、最小値や奇数分割で検証されていない。`push_triangle` は退化三角形を silent skip するため、default 960 枚だけでは一般式と watertight 性の担保として弱い。"
    suggestion: "`angular_segments = 3, 4, 5, 7, 32` などを含む表形式/parameterized テストを追加し、各ケースで `2*n_u*(n_v-1)`、watertight、外向き winding を確認すること。"
verdict: fail