<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## Final Round
- FN01: 棄却 — `fuse_box_cyl_acceptance.rs::t02_manifold()` が既に `solid.validate_manifold().expect(...)` で manifold 検証を実施している。smoke テストの #[ignore] 解除は build 成功を確認するものだが、acceptance T02 は manifold validation の成功を直接 assert している。FN01 の前提（manifold 検証テストが不在）は誤認識。
