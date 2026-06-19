## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `feature_transitive_implicit_body_refs(consumer_feat, features)` ヘルパ追加 (consumer の `feature_sketch_refs` を辿り、参照先 `CreateSketch` の `feature_implicit_body_refs` を合算) | `Sweep` / `Revolve` 等の Phase 10+ Feature variants |
| `check_no_downstream_break` で downstream consumer の implicit body refs を **transitive** に解決 | `EntityRef::Derived` chain 自体の修正 (既存 `collect_named_feature_ids` をそのまま使う) |
| `check_refs_resolve_before` で Extrude/ExtrudeCut insert 時、sketch ref が指す `CreateSketch` の implicit body refs が live かを確認 | `Document::apply_op` / `FeatureOp` enum 化 (ADR-015 確定後の別 Issue) |
| Acceptance test: `[box_1, sk(plane_ref=Entity(box_1)), e1(sketch=sk)]` で Cut(target=box_1) を `idx 2` (e1 の直前) insert → `InsertBeforeConsumer { consumed_ref: "box_1", displaced_feature_id: "e1" }` | パフォーマンス最適化 (O(n²) のままで OK、history 規模が小さい) |
| Acceptance: 同 history で Fuse/Intersect 経路 | Component RefPlane (ADR-014 / #207) との相互作用 |
| Acceptance: `Extrude` insert 時に referenced sketch の plane_ref body が consumed なら error | `Sketch` 直接の plane_ref 変更 (CreateSketch 自体の update — Phase 9+ の別 op) |
| Acceptance: Derived chain を含む sketch でも transitive resolution が走ること | `simulate_history` の semantics 変更 (broken-ref skip ロジックはそのまま) |

## Non-Goals

- `Sweep` / `Revolve` 等の Phase 10+ Feature variants の sketch ref 追跡: scope 外
- `EntityRef::Derived` の provenance chain そのものの修正: 既存実装をそのまま流用
- `Document::apply_op` / `FeatureOp` enum 化: ADR-015 確定後の別 Issue
- Component RefPlane (ADR-014 / #207) との相互作用
- パフォーマンス最適化 (O(history_len * features_len) のままで OK)
- `simulate_history` の broken-ref skip ロジック変更
- `CreateSketch` 自体の plane_ref を update する op: Phase 9+ の別 Issue

## 実装対象

Issue: #266
影響クレート/ファイル:
- `crates/engawa-build/src/feature_crud.rs` (helper 追加 + 2 関数の transitive 化)
- `crates/engawa-build/tests/feature_crud_plane_ref_acceptance.rs` (本 #266 用 T01-T05 を追記)

### 変更1: `feature_transitive_implicit_body_refs` ヘルパ追加 (新規)

`feature_implicit_body_refs` の直後に追加:

```rust
/// Extract implicit body refs **transitively** reachable from a consumer feature.
///
/// Includes:
/// - `feature_implicit_body_refs(f)` (direct: CreateSketch.plane_ref=Entity)
/// - For each sketch_id in `feature_sketch_refs(f)`, look up the matching
///   `CreateSketch` in `features` (full history) and add its `feature_implicit_body_refs`.
///
/// This captures the transitive lifetime dependency where e.g.
/// `Extrude { sketch: sk }` indirectly depends on the body that sk's plane is
/// attached to, even though Extrude itself carries no implicit body ref.
fn feature_transitive_implicit_body_refs(f: &Feature, features: &[Feature]) -> Vec<String> {
    let mut refs = feature_implicit_body_refs(f);
    for sketch_id in feature_sketch_refs(f) {
        for feat in features {
            if let Feature::CreateSketch { id, .. } = feat {
                if id == sketch_id {
                    refs.extend(feature_implicit_body_refs(feat));
                    break;
                }
            }
        }
    }
    refs
}
```

### 変更2: `check_no_downstream_break` 内の `implicit_consumer_refs` を transitive 化

before (line 469):
```rust
let implicit_consumer_refs = feature_implicit_body_refs(consumer_feat);
```

after:
```rust
let implicit_consumer_refs = feature_transitive_implicit_body_refs(consumer_feat, features);
```

### 変更3: `check_refs_resolve_before` 内の implicit ref check を transitive 化

before (line 398):
```rust
for implicit_ref in feature_implicit_body_refs(f) {
```

after:
```rust
for implicit_ref in feature_transitive_implicit_body_refs(f, features) {
```

`features` には新規 feature `f` 自身は含まれていない (insert 前の state)。`f` の sketch_ref が指す CreateSketch は `features` から lookup 可能。

## 設計方針

- **決定性要件**: helper は `Vec` の append 順が決定的 (sketch_refs の順 → features の出現順)。同一入力で同一出力。
- **B-rep トポロジー妥当性**: N/A (validation 層、トポロジー操作なし)
- **退化幾何の扱い**: N/A (validation 層)
- **derive 規約**: 関数追加のみ、新規型なし
- **エラーハンドリング**: 既存 `FeatureCrudError` 4 variant を流用。新規 variant 追加なし。
- **workspace.dependencies 規約**: 依存追加なし

### 後方互換性

- 既存 acceptance test (`feature_crud_acceptance.rs` / `feature_crud_prefix_validate_acceptance.rs` / `feature_crud_plane_ref_acceptance.rs`) は **全て pass 維持**:
  - direct ref のケースは helper の `feature_implicit_body_refs(f)` ベースで同等に動く
  - sketch を介さない feature (CreateBox/Cylinder/Sphere/Cut/Fuse/Intersect の direct ref) は `feature_sketch_refs` が空なので transitive 拡張なし
- 性能影響: history 規模が <100 features の現状では O(n*sketch_refs) の追加 lookup は無視できる

## テスト計画（ID 付き）

`tests/feature_crud_plane_ref_acceptance.rs` の末尾に追記。

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| t10_266_determinism | 決定性 | `[box_1, sk(plane=Entity(box_1)), e1(sketch=sk)]` で Cut(target=box_1) を idx 2 に 2 回 insert → エラー variant 同一 | `InsertBeforeConsumer` ×2、`format!("{:?}", err)` が equal |
| t11_266_cut_blocked_via_sketch_user | 正常系 | 同 history + Cut(target=box_1) idx 2 insert → e1 が downstream consumer として hit | `Err(InsertBeforeConsumer { consumed_ref: "box_1", displaced_feature_id: "e1", .. })` |
| t12_266_fuse_blocked_via_sketch_user | 正常系 | 同 history + Fuse(target=box_1, tool=box_2) idx 2 insert → e1 が downstream consumer | `Err(InsertBeforeConsumer { consumed_ref: "box_1", displaced_feature_id: "e1", .. })` |
| t13_266_intersect_blocked_via_sketch_user | 正常系 | 同 history + Intersect(target=box_1, tool=box_2) idx 2 insert → e1 が downstream consumer | `Err(InsertBeforeConsumer { consumed_ref: "box_1", displaced_feature_id: "e1", .. })` |
| t14_266_extrudecut_via_sketch_user | 正常系 | history `[box_1, sk(plane=Entity(box_1)), ec1(ExtrudeCut sketch=sk target=box_other)]` で Cut(target=box_1) idx 2 insert → ec1 が transitive 経路で hit | `Err(InsertBeforeConsumer { consumed_ref: "box_1", displaced_feature_id: "ec1", .. })` |
| t15_266_derived_chain_resolves_transitively | 退化境界 | `EntityRef::Derived` chain を持つ sketch でも plane_ref body が transitive に解決される | `InsertBeforeConsumer` (chain の最深 leaf body_id を consumed_ref として認識) |
| t16_266_degen_no_sketch_user_unblocks | 退化境界 | history が `[box_1, sk(plane=Entity(box_1))]` (e1 なし) で Cut(target=box_1) を idx 2 insert → sk 自身が downstream consumer (#264 の既存挙動と同等で blocked) | `Err(InsertBeforeConsumer { consumed_ref: "box_1", displaced_feature_id: "sk", .. })` (既存 T02 と同等、回帰検出) |
| t17_266_boundary_extrude_insert_with_consumed_plane | 退化境界 | history `[box_1, Cut(box_1, box_other), sk(plane=Entity(box_1))]` (sk の plane が consumed 後を参照) → Extrude(sketch=sk) を末尾に insert → `check_refs_resolve_before` の transitive implicit check が live でないことを検出 | `Err(BodyNotFound { feature_id: "e1", body_ref: "box_1" })` または `InsertBeforeProducer` (どちらでも transitive 経路の挙動を確認できれば OK) |

退化/境界 ID: `t15_266_derived_chain_resolves_transitively`, `t16_266_degen_no_sketch_user_unblocks`, `t17_266_boundary_extrude_insert_with_consumed_plane` の 3 件。

## 幾何的不変条件チェックリスト

- N/A (validation 層のみ、partition / assemble / boolean のトポロジー操作を含まない)
