issues:
  - id: R01
    severity: high
    section: "設計方針 > 6. assemble.rs の edge curve / face surface dispatch"
    finding: "`reverse_face_orientation` の仕様が破綻している。`face-effective outward normal = if same_sense { n } else { -n }` と定義した直後に、Plane だけ `same_sense` をトグルしつつ `Surface::Plane.normal/u_axis/v_axis` も反転すると、反転後の effective normal が元と同じ向きのままになる。加えて同じ文書内の STEP D・幾何的不変条件チェックリスト・T28 では Cylinder の axis を反転する版と不変にする版、void shell の法線を中心/軸へ向ける版と外向きにする版が混在している。"
    suggestion: "face 向きの表現を 1 つに統一すること。`same_sense` を正規の向き表現にするなら Plane/Cylinder/Sphere すべて Surface 本体は不変にして HE 順と `same_sense` だけで反転するか、逆に Surface を反転するなら `same_sense` は据え置く。STEP D、チェックリスト、T28 も同じ規約に合わせ、inner shell / inner hole の法線期待値は中心・軸方向へ向く形に修正すること。"

  - id: R02
    severity: high
    section: "テスト計画 > A1 / T17 / T27 / T31"
    finding: "A1 の入力形状と期待結果が一致していない。文書中の他テストと同じく cylinder の `origin` が底面中心なら、`origin=(0,0,-1), height=12` は box の内部で `z=-1..5` しか削らず、through hole ではなく blind hole になる。それにもかかわらず期待値は『上下面に inner_loop』『除去体積 = π·4·10』『単一 shell・貫通穴 1 個の Euler』『内側円柱面 2 個』になっており、同一ケースとして両立しない。"
    suggestion: "A1 を through hole ケースにしたいなら cylinder の位置を box の下面より下へ下げるなど入力形状を修正すること。現パラメータを維持するなら blind hole 用に volume、face 構成、Euler 期待値を全面的に書き換えること。A1 依存の T17/T19/T21/T22/T23/T27/T30/T31 も同時に更新すること。"

  - id: R03
    severity: medium
    section: "テスト計画 > A4a / T04 / T04b"
    finding: "`UnsupportedSurfaceIntersection` の `reason` 文字列が一貫していない。設計方針・退化幾何・T04/T04b は `non-perpendicular plane × cylinder` だが、A4a は `non-axis-aligned plane × cylinder` を期待している。`&'static str` をそのまま比較する設計なので、このままだと少なくともどちらかのテスト仕様が必ず外れる。"
    suggestion: "`reason` の文言を 1 つに正規化し、受け入れテストと単体テストの期待値を全て同じ文字列に揃えること。"

  - id: R04
    severity: low
    section: "設計方針 > 10. derive 規約"
    finding: "新規公開型 `IntersectionLoop` が `Debug, Clone, PartialEq` のみで、レビュー基準の `Serialize, Deserialize` 規約を満たしていない。説明では『kernel-internal』としているが、シグネチャは `pub` で公開 API になっている。"
    suggestion: "外部公開が不要なら `pub(crate)` に落とすこと。公開型として残すなら `Serialize, Deserialize` を追加し、必要なら `JsonSchema` も付与すること。"

verdict: fail