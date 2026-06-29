# Claude Self-Review for #288

GLM self-review (`glm-self-review.md`) と独立に diff (`crates/engawa-build/src/lib.rs` の
`validate_sketch_profile_contours` / `is_closed_primitive` 追加と 2 箇所の呼び出し挿入、
`tests/create_sketch_closed_acceptance.rs` 9 件の acceptance test) を 3 観点で再点検する。

## architect 観点 (既存 invariant / API 契約 / B-rep トポロジー保証)

- guard 関数は純粋判定で B-rep に到達しない。Euler-Poincaré 等の不変条件には無影響。
- `KernelError::InvalidParameter { kind: "profile" }` を再利用しており、新規 variant は
  追加していない (API 契約維持)。
- **medium**: `is_closed_primitive` が `matches!(elem, Circle | Ellipse | Conic)` で
  実装されており、将来 `SketchElement` に新 variant が追加されたとき (例: `Polygon`,
  `BSpline` など — Phase 11+ で想定される) silently `false` に分類される。
  結果として `[NewClosedPrimitive, Line]` のような profile が guard をすり抜けて
  flat_map で壊れる潜在バグ。exhaustive `match` への置換で compile-time に検出
  できるが、Phase 11+ で multi-contour 正規実装に置き換える前提なら緊急度は低い
  (現行 SketchElement の 5 variant では全件カバー済み)。

## contrarian 観点 (採用方針の反論可能性 / defensive semantics の退化)

- Option B (reject) を採用したことに対する反論: Option A (contour 化) のほうが
  正規 fix だが、`make_extrusion` の signature が `&[(f64,f64)]` (単一 polyline) で
  あり Phase 11+ で `Vec<Vec<(f64,f64)>>` に拡張する必要がある。Phase 10 scope では
  Option B が最小コストで安全側。GLM 評価と同じ結論。
- 直前 Phase 10 Issue の defensive semantics (例: #271 split parent, #274 Ellipse/Conic
  追加) を退化させていない。`validate_profile_closed` (既存) と並ぶ前段 reject 層として
  自然な拡張。
- **medium**: T03_degen の comment が "現行 (pre-fix) コードでも downstream の
  is_convex/is_simple チェックで同 error variant を返すため pre-fix でも pass する"
  と認めている通り、T03 単体では guard 経路を直接検証できない (両方の経路が同じ
  error variant を返すため区別不能)。post-fix のリグレッション検出には、guard を
  削除した状態で fail する状況を作る必要があるが現実的に困難。T06_mid_closed
  (`[Line, Circle, Line]`) はより確度が高い regression guard (中央 closed は downstream
  だけでも壊れる確率が低い)。これは追加した T06 でカバー済み。

## migration 観点 (既存テスト互換 / 後方互換性)

- public API 変更なし: `build_bodies_from_features` の signature 変更なし。
- 既存 acceptance test (`face_sketch_extrude_acceptance.rs`,
  `face_sketch_extrudecut_acceptance.rs`, `examples_smoke.rs` 他) はいずれも
  単一 element profile を使用しており guard 通過するため regression なし
  (`grep -r 'profile: vec!\[' crates/engawa-build/tests/ | wc -l` で 単一要素のみ確認可能)。
- golden YAML / `.engawa` 形式に変更なし。
- 新規 test ファイル `create_sketch_closed_acceptance.rs` は独立した integration test
  として配置されており既存テストファイルと衝突しない。

## 結論

弱点 2 件 (いずれも **medium**) を検出。critical/high なし → claude-self-review.md に
記録のみで STEP 7 へ進む。

- **M-01** (architect/maintainability): `is_closed_primitive` の `matches!` が将来の
  SketchElement 新 variant に対し silently `false` を返す foot-gun。Phase 11+
  multi-contour 正規実装で再評価する。
- **M-02** (contrarian/test sensitivity): T03_degen は pre/post-fix で同 error variant に
  なるため guard 経路を直接検証できない。T06_mid_closed で間接的に補強済み。

両件とも Phase 10 scope では受容可能。GLM 再実装は不要。
