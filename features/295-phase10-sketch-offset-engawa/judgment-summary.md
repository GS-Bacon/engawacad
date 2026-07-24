<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R01: 採用 → plan の「...」節を修正 -->
<!-- - R02: 棄却 → Non-Goals に記載済みの蒸し返し -->
<!-- - R03: 部分採用 → epsilon 比較は採用。HalfEdge 循環チェックは後続 issue に委譲 -->

## Round 1
- SC01 (medium, scope): 採用 — Non-Goals に SCOPE DEFENSE 節を追加し「SelfIntersection → DegenerateSketchElement 代替」の理由・整合を明記
- AM01 (low, ambig): 採用 — T06 に baseline (offset なし) fixture との bounding box radius 比較を明記
- invariant: issues なし
- numeric: issues なし


## Round 2
- IN01 (critical, invariant): 採用 — 疑似コードで HashSet を BTreeSet に変更、コメントで「出力は source 順、selection の順序・重複は結果に影響しない」旨を明記
- IN02 (high, invariant): 採用 — T01a 追加 (selection=[a,b,c] vs [c,b,a] で同一出力を assert)
- scope: issues なし
- ambig: issues なし
- numeric: issues なし

## Round 3
- SC01 (medium, scope): 棄却 (rejection.md 参照)
- IN01 (medium, invariant): 棄却 (rejection.md 参照)
- 全 4 ペルソナで critical/high 0 件

## Round 4
- SC02 (medium, scope): 採用 — Non-Goals の SelfIntersection 項に SCOPE DEFENSE 節への参照を追記
- invariant / ambig / numeric: issues なし
- **Round 3 も C/H=0、Round 4 も C/H=0 → 2 round 連続 C/H=0 で収束**

## STEP 7.5 (実装最終レビュー) round 3 — #303 escalation 解消 (人間判断で再開)
- C-F02 (high) / M-F01 (high): schema_version bump 要否 → **棄却** (詳細は rejection.md Round 5 参照。#274/#275 precedent に基づく)
- C-F01 (high): public 契約 vs build 実装 (Circle-only) の乖離 → **部分採用 (doc のみ)** — wire 型は変更せず、rustdoc + plan.md In-Scope 表で契約を明示
- C-F03 (medium) / M-F02 (medium): golden YAML 追加 → **採用**
- 3 論点とも決着、STEP 7.5 を fresh 1-shot gate で再実行して STEP 8 へ進む

## STEP 7.5 round 4 (rebase 後 fresh gate)
- A01 (high): CRUD が SketchOffset 参照 sketch の Circle 単一性を未検証 → **採用** — `refs_resolve_in_state`/`check_refs_resolve_before` に検査追加、回帰テスト追加 (rejection.md Round 6 参照)

## STEP 7.5 round 5 (fix 後 fresh gate)
- A01 (high): CRUD が複数回 offset の数値 collapse を未検出 → **棄却** — CRUD 層はどの Feature 型についても数値/幾何的 buildability を検証しない (Cut/Fuse/Intersect/Extrude も同様) という既存 architecture 境界に従う。build_bodies_from_features() が既存の DegenerateSketchElement で正しく Err を返す。詳細は rejection.md Round 6 参照。
