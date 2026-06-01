issues:
  - id: R01
    severity: high
    section: "実装対象 > 変更する型・シグネチャ"
    finding: "`EntityRef::Derived` に `kind` が無く、派生参照が Face/Edge/Vertex のどれを指すかを型として保持できない。`canonical_name` も `D(<op>;<selector>;[...])` だけだと、同じ provenance と selector を共有する異種エンティティが同一名に衝突し得る。"
    suggestion: "`Derived` に `kind: EntityKind` を追加し、`canonical_name` にも `F/E/V` を含めること。もし `selector` 側で kind を表現する設計にするなら、その一意性契約を明文化し、異種エンティティが衝突しないテストを追加すること。"
  - id: R02
    severity: medium
    section: "設計方針 > 検証経路 / テスト計画"
    finding: "本 Issue では `EntityRef` は依然としてどの `Feature` にも埋め込まれないため、`Document::from_yaml`/`from_path` では T05 の `role`/`op`/`selector` 不正や T14 の「不正な `EntityRef` を持つ `Document`」を表現できない。さらに standalone `EntityRef` の custom `Deserialize` が返すのは `serde` 系 error なので、T09a の「T05 を typed `FormatError` で返す」という期待値は現スコープのままでは成立しない。"
    suggestion: "`Document` 経路の typed error テスト対象は `feature_id`/重複 `feature_id` に限定し、`EntityRef` 側は `validate()`・`TryFrom<RawEntityRef>`・直接 deserialize 拒否を別テストに分離すること。将来 `EntityRef` も `Document` 経路で typed error 化したいなら、埋込時に `RawEntityRef` を経由する設計を追加すること。"
  - id: R03
    severity: low
    section: "実装対象 > 影響クレート/ファイル"
    finding: "新しい公開型 `EntityRef` / `EntityKind` を追加する計画なのに、`crates/mycad-format/src/lib.rs` の再エクスポート更新が影響範囲に含まれていない。現行 crate は root で公開型を `pub use` しており、このままだと公開 API の出し方が不統一になる。"
    suggestion: "`crates/mycad-format/src/lib.rs` を影響ファイルに追加し、`pub use feature::{EntityKind, EntityRef, Feature, SketchPlane, SketchSegment};` の形で再エクスポートを揃えること。"

verdict: fail