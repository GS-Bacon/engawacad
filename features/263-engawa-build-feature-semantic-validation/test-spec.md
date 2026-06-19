# Test Spec (Issue #263)

## 不足テスト（plan 計画分）

plan T01〜T08 はすべて acceptance test に実装済み。T09 (互換維持) は新規 acceptance test では検証せず、`cargo test -p engawa-build` で既存 4 inline tests (`tests` mod 内) が pass し続けることで担保 (CI green ですでに検証済み)。

## 実装差分から追加すべきテスト

`feature_crud.rs:84-93` の `feature_body_refs` は **Extrude.fuse_target** / **ExtrudeCut.target** / **Cut.{target,tool}** / **Fuse.{target,tool}** / **Intersect.{target,tool}** の 7 経路を扱う。実装には全 variant branch があるが、acceptance test は `Feature::Cut` (T04/T05/T06/T07) と `Feature::Extrude { fuse_target: None }` (T01/T02/T03) しかカバーしていない。以下を追加:

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T10_extrude_cut_target_not_found | 異常系 | 空 doc に `ExtrudeCut(id=ec1, sketch=unknown, target=unknown, depth=5)` を at=0 → `SketchNotFound` (sketch refs が body refs より先にチェックされるため) | `matches!(err, SketchNotFound { sketch_ref: "unknown", .. })` |
| T11_extrude_cut_body_not_found | 異常系 | `[CreateSketch(s1)]` に `ExtrudeCut(id=ec1, sketch=s1, target=unknown, depth=5)` を at=1 → `BodyNotFound { body_ref: "unknown" }` | `matches!(err, BodyNotFound { .. })` |
| T12_extrude_fuse_target_not_found | 異常系 | `[CreateSketch(s1)]` に `Extrude(id=e1, sketch=s1, fuse_target=Some("unknown"))` を at=1 → `BodyNotFound { body_ref: "unknown" }` | `matches!(err, BodyNotFound { .. })` |
| T13_fuse_both_refs_ok | 正常系 | `[CreateBox(b1), CreateBox(b2)]` に `Fuse(id=f1, target=b1, tool=b2)` を at=2 → `Ok`、features.len() == 3 | `Ok` |
| T14_intersect_ref_consumed | 異常系 | `[CreateBox(b1), CreateBox(b2), Cut(c1, target=b1, tool=b2)]` に `Intersect(id=i1, target=b1, tool=b2)` を at=3 → `b1`/`b2` は既に c1 で consume 済み → `BodyNotFound` | `matches!(err, BodyNotFound { .. })` |
| T15_self_reference_sketch | degen | `Extrude(id=x, sketch=x, depth=5)` を任意位置に挿入 → `SelfReference { ref_kind: "sketch" }` | `matches!(err, SelfReference { ref_kind: "sketch", .. })` |
| T16_insert_at_tail_full_history | 境界 | `[CreateSketch(s1), CreateBox(b1), Extrude(e1, sketch=s1)]` に `Fuse(f1, target=b1, tool=e1)` を at=3 (= len) → `Ok` | `Ok` && features.len() == 4 |

## エッジケース・退化入力

- **dead branch in `check_no_downstream_break` の `re_registered` 経路**: `DuplicateFeatureId` ガードが先に走るため `features[at..]` に同じ id の producer は存在し得ない。実装は防御的に残してあるが論理的に到達不能。test-spec に追加テストは不要 (Non-Goals)。

## 類似ケース（未カバー）

`feature_consumes(f)` が `feature_body_refs(f)` と同一実装である点について、もし将来 `Extrude` が「sketch を CONSUME しない参照」と区別される変更が入ったら `feature_consumes` の分離が必要。**現状は同一**なので追加テスト不要。

## 数値境界

N/A (semantic validation は ID 文字列比較のみ、数値計算なし)。

## 決定性

T01 で同一 input 2 回呼び → 同一 yaml で検証済み。追加は不要。
