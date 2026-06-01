issues:
  - id: R01
    severity: high
    section: "設計方針 > 4. 交線エンティティの安定名 (Derived 名)"
    finding: "`FaceFragment.partner_parent_name` を 1 つ増やすだけでは edge 単位 provenance を表現できない。`partition_faces` は複数 partner face 由来の intersection segment を 1 本の `segments` に平坦化してから `pslg_subdivide` しているため、1 fragment の境界に別々の partner が混在し得る。この形のままでは assemble 側で全 intersection edge の 2 親を total/unique に復元できない。"
    suggestion: "partner provenance は fragment ではなく、subdivision に入る各交線 segment もしくは subdivide 後の各 boundary edge に保持する設計へ上げること。assemble が edge ごとに 2 親を読める形にすること。"
  - id: R02
    severity: medium
    section: "設計方針 > 8. derive 規約"
    finding: "`Curve2D` の『variant payload も private にして `try_*` 経由のみで構築させる』という前提は Rust の `pub enum` では成立しない。外部コードから `Curve2D::Line2D { direction: (0.0, 0.0) }` のような不正値を直接構築でき、custom Deserialize だけでは不変条件を守り切れない。"
    suggestion: "`Curve2D` を公開 `enum` のままにせず、private enum + public wrapper/newtype などの opaque 表現に変更し、公開 API では validated constructor だけを通すこと。"
  - id: R03
    severity: medium
    section: "実装対象 > `crates/mycad-kernel/src/brep/topology.rs` / テスト計画 T12-T13"
    finding: "現行の `validate_manifold` 契約は `Result<(), &'static str>` だが、計画では `KernelError::PcurveSurfaceMismatch` などの構造化エラーを返す前提でテストしている。戻り値契約の変更が設計に明記されておらず、このままでは T13/T13b/T13c の期待値と一致しない。"
    suggestion: "`validate_manifold` を `Result<(), KernelError>` に変えるか、pcurve 整合性専用の structured validator を別で定義し、その call site と tests まで計画に含めること。"
  - id: R04
    severity: medium
    section: "設計方針 > 5. 曲面交線テッセレーション"
    finding: "`collect_loop_points` で `pcurve.sample(segments)` をそのまま使う案は、既存 `Curve::Line.sample_segment` が直線で 1 点しか返さない契約と噛み合っていない。提案どおり `Line2D` が N 点を返すと、直線 HE の loop 点数が増えて fan/earcut 前提と T14 の『既存挙動同等』が崩れる。"
    suggestion: "`Line2D` の sampling 契約を既存 3D line と揃えるか、`collect_loop_points` 側で直線 pcurve を特別扱いして straight edge の点数を増やさないこと。"

verdict: fail