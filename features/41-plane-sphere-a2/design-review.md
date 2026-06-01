issues:
  - id: R01
    severity: high
    section: "設計方針 > tessellation sphere trim 戦略"
    finding: "`tessellate_sphere_face_trimmed` は緯度線を 3D circle から `surface.project_uv(p)` で UV 化して earcut に渡す設計だが、緯度線は full-sphere の周期 UV 上で `u=0/2π` をまたぐため、平面の hole として閉じた単純多角形にならない。文中で問題化している seam 断裂を解消できておらず、trimmed sphere の三角形分割が不正になる。"
    suggestion: "earcut 前に経線で face を切って局所 chart に展開するか、`u` を unwrap して seam 複製込みで loop を矩形内部の単純多角形へ正規化すること。"
  - id: R02
    severity: high
    section: "テスト計画 > T02"
    finding: "T02 の体積期待値が誤っている。`center=(5,5,11), R=3, plane z=10` なら box 内に入る球冠の高さは `h=2` で、除去体積は `πh^2(R-h/3)=28π/3≈29.32`。記載の `cap vol ≈ 16.76` は一致しないため、正しい実装でも受け入れテストが落ちる。"
    suggestion: "T02 の期待値を `1000 - 28π/3 ≈ 970.68` に修正し、`h` の定義も明記すること。"
  - id: R03
    severity: medium
    section: "テスト計画 > T07"
    finding: "T07 が tangent ケースになっていない。`center z=15, R=3` では top plane `z=10` からの距離が 5 で、T08 と同じ disjoint になるため、Acceptance にある `tangent / disjoint は no-op` のうち tangent 境界が未検証のまま残る。"
    suggestion: "tangent は `center z=13` など `|d|=R` となる配置へ修正し、disjoint は別ケースとして維持すること。"
  - id: R04
    severity: medium
    section: "実装対象 > geometry/surface_intersect.rs:214-218"
    finding: "大円 reject を共有幾何 API の `intersect_plane_sphere` 自体へ入れて `UnsupportedBooleanCase` を返す設計だと、幾何学的には正当な plane×sphere 交線まで geometry 層で表現不能になる。A2 の Boolean 制約が低レイヤへ漏れており、他 call site での有効な大円利用まで巻き込む。"
    suggestion: "大円の reject は Boolean 側の partition/dispatch で行い、`intersect_plane_sphere` は円交線を返す純粋幾何 API のまま保つこと。"

verdict: fail