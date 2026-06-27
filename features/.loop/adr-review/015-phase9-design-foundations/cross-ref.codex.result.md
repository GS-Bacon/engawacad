missing_refs:
  - ADR-003: `Feature` command 一本化と `schema_version` / migration はここで親方針が置かれており、ADR-015 §1/§3 の直接の先例。
  - ADR-009: §4 が Boolean / Tessellation の不変量検証と bench を主要目的にしており、その品質対象の先例を引くべき。
conflicts:
  - ADR-005 §2: `FeatureOp::Edit { mutate: Box<dyn FnOnce(&mut Feature)> }` は `Feature.id` を不変 identity anchor とする前提を API 形で担保しておらず、rename や variant 変更を無制約に許しうる。
  - ADR-008 §Decision 3: closure ベースの `Edit` は CLI/API/AI から運べる concrete な書き込み command にならず、Phase 6 で固めた replayable な Feature command 契約と噛み合わない。
suggestions:
  - `FeatureOp::Edit` は closure ではなく `new_feature: Feature` か variant 別 patch enum に置き換え、`feature_id` 不変・variant 不変・参照再検証を本文で明記する。
  - `Related` と「既存 ADR との関係」に ADR-003 / ADR-005 / ADR-008 / ADR-009 を追加し、どの決定を継承しているかを 1 行ずつ書く。
  - ADR-014 との関係は「直交」だけで済ませず、child 間 datum 同期は未解決で trigger 2 の再評価条件に残すと明記する。
  - `schema_version` 据え置き条件に「新しい wire-level enum variant / sketch kind を追加した場合の旧 reader 互換」を追記し、v1 据え置きの境界を明確化する。
  - Open Questions の Q2 から ADR-016 参照を外し、variable/format 論点として本 ADR か別 Issue に寄せる。
verdict: misaligned