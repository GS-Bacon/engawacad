<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## Round 2

- IN01 (critical): 「Arc ID 生成に IdGenerator を使うべき」を棄却 — SketchElement::id (String, format layer) と EntityId=u64 (IdGenerator, B-rep layer) の混同による誤検知。ADR-017:93-96/153 が文字列派生 ID を明示規定し、カウンタ再採番を ADR-005 と衝突するとして棄却済み。Opus 4.7 subagent が topology.rs/feature.rs/sketch_offset.rs/ADR-017 を実地確認済み。plan.md 設計方針節にレイヤー区別の doc note を追記し、round 3 以降での再発を防止。

## Round 3

- AM03 (low): 「ANGLE_TOLERANCE 妥当性の根拠記述なし」を棄却 — ペルソナ自身が自己完結的に「指摘不要」と結論、数値モデル節で値固定済み。
- AM04 (low): 「再fillet防止ガードがコード例に見当たらない」を棄却 — ペルソナ自身が「実装レベルで担保済み」と結論、as_line の UnsupportedFeature 経路で自動的に拒否される。
- AM05 (medium): 「geometry/math.rs 再提案を排除する記述がない」を棄却 — ペルソナ自身が「必須ではない」と結論、既存記述で確定事項と明記済み (冗長な念押しは追加しない)。

## Round 4

- NU01 (medium): 「### 数値モデル セクションが存在しない」を棄却 — 事実誤認、290行目に実在。
- NU02 (medium): 「共面2面 dihedral angle / KernelError::DegenerateInput 未定義」を棄却 — Boolean/Partition (3D) 概念の誤爆、本 Issue (2D Line-Line fillet) に無関係。DegenerateInput は error.rs に実在しない架空 variant。

## STEP 7.5 round 2 (Codex)

- A01 (high): 「CRUD gate が共有コーナー座標を検証すべき」を棄却 (実装変更は行わない) — plan.md:350 の既存 scope 決定の再指摘。Codex の帰結 (build で sketch_fillet_no_shared_corner に落ちる) は事実誤認、全 Line profile は validate_profile_closed が先に InvalidParameter{kind:"profile"} で捕捉する。到達可能な mixed profile ケースは Non-Goals 領域。同種の gap は Extrude 分岐にも既存で fillet 固有ではない。代わりに T13 回帰テストを追加しplan記述を訂正 (部分採用)。
