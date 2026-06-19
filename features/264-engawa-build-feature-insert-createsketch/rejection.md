<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## STEP 7.5 Codex Round 1

- **C-F01 (contrarian, high) + M-F02 (migration, high)**: 「`PlaneRef::Entity` を `Named { kind: Face, .. }` 限定に restrict すべき (builder が Face しか resolve できないため Derived や Edge/Vertex を semantic-gate で reject せよ)」を **棄却**。
  - 理由: Issue #264 body が "EntityRef::Derived の chain 再帰 traversal (Derived.from の元 feature を辿る)" を明示要求している。Derived を弾く方向への変更は Issue 本来の要求 (lifetime tracking) と直接矛盾する。
  - builder 側の face 解決制約 (Named{Face} 限定) は別レイヤの懸念。本 Issue は「lifetime tracking」であり、builder 側の対応拡張 (将来 Derived chain の face 解決を実装) は別 Issue (ADR-015 #246 で議論中の `FeatureOp` 一元化での lifetime 集約) に追跡する。
  - T06 (Derived chain 成功 → Cut で InsertBeforeConsumer) は本 Issue の Derived traversal 要件を直接テストしており、Ok 経路の存在は仕様通り。
