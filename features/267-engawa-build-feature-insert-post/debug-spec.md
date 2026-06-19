# debug-spec for #267 — GLM round 1 が scope ずれ + test fixture バグ

## 経緯

GLM round 1 は plan.md の **post-insert re-simulation** を実装せず、代わりに `refs_resolve_in_state` の sketch (CreateSketch.plane_ref) transitive 化を行った。これは **#268 で scope-defer したはずの作業 (M-F02)**。

結果として:
- 主要 5 tests (t_267_cut/extrudecut/fuse/intersect_activates_broken_consumer + t_267_determinism) が **`Ok` を返してしまい failed** ← post-insert re-sim 未実装のため
- t_267_boundary_activated_consumer_safe_after_reregister が `DuplicateFeatureId { id: "box_b1" }` で fail ← test fixture が `box_b1` を 2 度作っている

## 仮説

1. `simulate_history` を **pre-insert features (= insert 前)** で 1 回しか呼ばないため、insert された feature が previously skipped consumer を活性化する効果が反映されない
2. GLM の test fixture (`t_267_boundary_activated_consumer_safe_after_reregister`) に `CreateBox(box_b1)` 二重登録あり (元 history の `box_b1` と再 register 用の `box_b1`)

## 関連ファイル

- `crates/engawa-build/src/feature_crud.rs` の `check_no_downstream_break`
- `crates/engawa-build/src/feature_crud.rs` の `refs_resolve_in_state` (← #268 scope の作業を巻き戻し)
- `crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs` の `t_267_boundary_*`

## 修正方針

### 修正1: `refs_resolve_in_state` の CreateSketch transitive 追加を **revert** する (#268 scope-defer 維持)

GLM round 1 で追加した以下のロジックを巻き戻す:
- `refs_resolve_in_state` の `Feature::CreateSketch { plane_ref, .. }` 分岐
- `refs_resolve_in_state` への `_features: &[Feature]` 引数追加
- `collect_named_feature_ids` → `collect_named_feature_ids_from` リネーム + 全 caller 更新
- `simulate_history` 内の `Feature::CreateSketch` 経路に refs_resolve_in_state チェック追加

これらは #268 で別途実装する範囲。**本 #267 では `refs_resolve_in_state` の表面 API と semantics を維持** する。

revert 後の `refs_resolve_in_state` シグネチャ:
```rust
fn refs_resolve_in_state(
    f: &Feature,
    sketches_at: &HashMap<String, usize>,
    live_bodies_at: &HashMap<String, usize>,
) -> bool {
    match f {
        Feature::Extrude { sketch, fuse_target, .. } => {
            sketches_at.contains_key(sketch)
                && fuse_target.as_ref().map_or(true, |t| live_bodies_at.contains_key(t))
        }
        Feature::ExtrudeCut { sketch, target, .. } => {
            sketches_at.contains_key(sketch) && live_bodies_at.contains_key(target)
        }
        Feature::Cut { target, tool, .. }
        | Feature::Fuse { target, tool, .. }
        | Feature::Intersect { target, tool, .. } => {
            live_bodies_at.contains_key(target) && live_bodies_at.contains_key(tool)
        }
        // CreateBox/Cylinder/Sphere/CreateSketch have no prefix-resolvable refs in #267 scope.
        // (CreateSketch.plane_ref liveness is #268 / M-F02.)
        _ => true,
    }
}
```

`collect_named_feature_ids` (元 API) は復旧。

### 修正2: `check_no_downstream_break` で post-insert re-simulation を実装 (本 Issue の core)

post-insert で実際に「previously executing consumer が broken になる」シナリオを検出するロジック:

```rust
fn check_no_downstream_break(
    f: &Feature,
    features: &[Feature],
    at: usize,
) -> Result<(), FeatureCrudError> {
    // 1. Pre-insert simulation
    let (_, _, executed_at_pre) = simulate_history(features, features.len());

    // 2. Post-insert hypothetical
    let mut post_insert: Vec<Feature> = Vec::with_capacity(features.len() + 1);
    post_insert.extend_from_slice(&features[..at]);
    post_insert.push(f.clone());
    post_insert.extend_from_slice(&features[at..]);
    let (_, _, executed_at_post) = simulate_history(&post_insert, post_insert.len());

    let consumed_bodies_by_f = feature_consumes(f);

    // 3a. Cases where the inserted f directly consumes a body still needed by some
    //     downstream consumer (= original semantics, pre-insert view).
    //     This was the existing pre-#267 behavior — keep it for direct-conflict detection.
    for body_id in &consumed_bodies_by_f {
        for (consumer_idx, consumer_feat) in features.iter().enumerate().skip(at) {
            if !executed_at_pre.contains(&consumer_idx) { continue; }
            let consumer_refs: Vec<&str> = feature_consumes(consumer_feat);
            let implicit_consumer_refs = feature_transitive_implicit_body_refs(consumer_feat, features);
            let direct_match = consumer_refs.contains(body_id);
            let implicit_match = implicit_consumer_refs.iter().any(|r| r == body_id);
            if direct_match || implicit_match {
                // re_registered check (pre-insert space, between at+1 and consumer_idx)
                let mut re_registered = false;
                for (reg_idx, reg_feat) in features.iter().enumerate().skip(at + 1) {
                    if reg_idx >= consumer_idx { break; }
                    if !executed_at_pre.contains(&reg_idx) { continue; }
                    if reg_feat.id() == *body_id {
                        match reg_feat {
                            Feature::CreateBox { .. } | Feature::CreateCylinder { .. }
                            | Feature::CreateSphere { .. } | Feature::Extrude { .. }
                            | Feature::ExtrudeCut { .. } | Feature::Cut { .. }
                            | Feature::Fuse { .. } | Feature::Intersect { .. } => {
                                re_registered = true; break;
                            }
                            _ => {}
                        }
                    }
                }
                if !re_registered {
                    return Err(FeatureCrudError::InsertBeforeConsumer {
                        consumed_ref: body_id.to_string(),
                        displaced_feature_id: consumer_feat.id().to_string(),
                        consumer_at: consumer_idx,
                        requested_at: at,
                    });
                }
            }
        }
    }

    // 3b. Post-insert activation case (#267 scope):
    //     Some consumer that was executing in pre-insert sim becomes inert in post-insert sim,
    //     because newly-activated features consumed the bodies it depends on.
    //     This catches the "[box_b1, box_b2, Cut(c1, target=new_box, tool=box_b1), Cut(c2, target=box_b1, tool=box_b2)]
    //     + insert CreateBox(new_box) @ idx 2" scenario.
    for (orig_idx, consumer_feat) in features.iter().enumerate().skip(at) {
        let post_idx = orig_idx + 1;  // shift by inserted feature
        if !executed_at_pre.contains(&orig_idx) { continue; }  // wasn't working before
        if executed_at_post.contains(&post_idx) { continue; }  // still works after
        // Was working pre, broken post — identify which body of consumer_feat is no longer live post.
        // Pick the first direct body ref that was live pre but not live post.
        // For simplicity (and to satisfy the error variant contract), pick the first body ref that
        // was consumed by some newly-activated feature in post-insert.
        let consumer_refs: Vec<&str> = feature_consumes(consumer_feat);
        let implicit_consumer_refs = feature_transitive_implicit_body_refs(consumer_feat, &post_insert);

        // Combine direct + implicit refs into a single ordered list (direct first).
        let mut all_refs: Vec<String> = Vec::new();
        for r in &consumer_refs { all_refs.push(r.to_string()); }
        for r in &implicit_consumer_refs { all_refs.push(r.clone()); }

        // For each ref the consumer uses: was it consumed by an activator?
        // An activator is a feature at index in (at..post_idx) in post-insert space that
        // wasn't in executed_at_pre (at original idx, i.e. post_idx-1) but is in executed_at_post.
        // The inserted feature itself counts (executed_at_post contains `at`).
        for body_id in &all_refs {
            // Find activator that consumes body_id within post-insert prefix up to post_idx-1.
            let activator_consumed = post_insert.iter().enumerate().take(post_idx).skip(at).any(|(pi, pf)| {
                // pi == at: inserted feature. pi > at: shifted from original idx pi-1.
                let executed_now = executed_at_post.contains(&pi);
                let executed_before = if pi == at { false } else { executed_at_pre.contains(&(pi - 1)) };
                let is_activator_or_inserted = executed_now && !executed_before;
                if !is_activator_or_inserted { return false; }
                feature_consumes(pf).contains(&body_id.as_str())
            });
            if activator_consumed {
                return Err(FeatureCrudError::InsertBeforeConsumer {
                    consumed_ref: body_id.to_string(),
                    displaced_feature_id: consumer_feat.id().to_string(),
                    consumer_at: orig_idx,
                    requested_at: at,
                });
            }
        }
        // If no activator found but consumer became inert, fall through (could be the consumer
        // ref something the inserted feature replaced — rare; pre-#267 wouldn't catch this either).
    }

    Ok(())
}
```

要点:
- **2 段階チェック**: (3a) f が直接 consume する body → existing semantics 維持。(3b) f が previously skipped consumer を activate → 新規ロジック
- (3b) は orig features を走査し、pre-insert で executed だった consumer が post-insert で inert になっているケースを検出
- 「なぜ inert になったか」の body_id 特定は、post-insert で **newly activated** な feature が consume した body を探す方法を採る
- Pre/post 両方の simulate_history を呼ぶため、追加コストは insert あたり 1 回の simulate_history call (許容)

### 修正3: `t_267_boundary_activated_consumer_safe_after_reregister` の fixture を `DuplicateFeatureId` にしないよう修正

`box_b1` を 2 回 `CreateBox` で push しているのが原因。代替案:
- history: `[box_b1, box_b2, Cut(c1, target=new_box, tool=box_b1), CreateBox(box_b1_v2), Cut(c2, target=box_b1_v2, tool=box_b2)]`
- insert `CreateBox(new_box)` at idx 2
- c1 が活性化 → box_b1 consume。だが c2 は `box_b1_v2` (別 id) を target にしているので、c2 は壊れない → `Ok`

または、`Document::validate` でなく `FeatureCrud::insert` の semantic check のみで判定する fixture に組み替える (validation 層を skip させる test API があれば)。

または、テスト の意図 (= re-register があれば保護される) を実現する別パターン:
```
[box_b1, box_b2, Cut(c1, target=new_box, tool=box_b1), CreateBox(box_b1_v2 with same id? no — duplicate),
 Cut(c2, target=box_b1, tool=box_b2)]
```
これは元々の意図に近いが、`DuplicateFeatureId` が validate で reject される。

最も clean な案: `id` は一意にしつつ、`re_registered` ロジックの「同 id の register」が触れないケースとして書く。具体的には:
- history: `[box_b1, box_b2, Cut(c1, target=new_box, tool=box_b1), Cut(c2, target=box_b1, tool=box_b2)]`
- insert `CreateBox(new_box)` で `c1` 活性化、`c2` 破損 → `InsertBeforeConsumer` を期待
- これは `t_267_cut_activates_broken_consumer` と同じシナリオなので、t_267_boundary は **削除** するか、別の趣旨に変更

**推奨**: `t_267_boundary_activated_consumer_safe_after_reregister` を **削除** する。boundary check は plan.md スコープには入れていたが、`DuplicateFeatureId` 制約と整合する simple な fixture が組めない (Document::validate で reject されるため)。`Document::validate` の `DuplicateFeatureId` チェックは別 layer の不変条件で、re_registered semantics は「id 単位の lifecycle」ではなく「feature instance の lifecycle」を扱う検査に近いため、本 Issue のテスト範囲では実装困難。

plan.md の boundary 項目から削除して `t_267_degen_no_activation_no_change` のみ残す。

## 試した修正と結果

- (round 1 試行 — GLM が scope ずれを起こした)

## 次にやること

1. `refs_resolve_in_state` の sketch transitive 追加を完全 revert (#268 scope-defer 維持)
2. `check_no_downstream_break` で pre/post-insert simulate_history を併用して **two-phase detection**
3. `t_267_boundary_activated_consumer_safe_after_reregister` を **削除** (DuplicateFeatureId 制約で組めない)
4. plan.md のテスト計画から t_267_boundary を削除
5. 残り 6 tests (determinism, cut/extrudecut/fuse/intersect activates, degen_no_activation) を pass させる
6. 既存 #265 / #266 / #264 / 全 acceptance test の回帰なしを `cargo xtask ci` で確認

## 追加で書いてほしいテスト

無し。boundary は削除して合計 6 tests に絞る。
