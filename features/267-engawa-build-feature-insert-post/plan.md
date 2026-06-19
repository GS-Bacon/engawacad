## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `check_no_downstream_break` の `executed_at_full` を **insert 後** の hypothetical history で再シミュレート (`features[..at] ++ [new_feat] ++ features[at..]`) | `check_refs_resolve_before` の post-insert re-simulation (新規 feature の direct/implicit refs check は self prefix で十分、broken-future 活性化は consumer 側でのみ問題化する) |
| Acceptance test: `[box_b1, box_b2, Cut(c1, target=new_box, tool=box_b1), Cut(c2, target=box_b1, tool=box_b2)]` に `CreateBox(new_box)` を idx 2 insert → `InsertBeforeConsumer` を返す | `simulate_history` の internal API 変更 (`refs_resolve_in_state` の sketch transitive 反映は #268 で対応) |
| Acceptance: ExtrudeCut/Fuse/Intersect 同パターン (新規 feature 挿入で previously skipped consumer が live 化) | `FeatureOp::apply` 一元化 (ADR-015 確定後の別 Issue) |
| Acceptance: 退化境界 — insert が **何も活性化させない** (broken-future が無い) ケースで挙動変化なしを確認 (回帰防止) | `FeatureCrud::update` / `FeatureCrud::remove` への適用 |
| Acceptance: 退化境界 — insert が broken-future を活性化するが、活性化後の consumer は新規 feature 自身の出力に依存しているため downstream break しないケース | パフォーマンス最適化 (1 history あたり insert ごとに 1 回の追加 simulate_history call が許容) |

## Non-Goals

- `check_refs_resolve_before` の post-insert re-simulation: 本 Issue scope 外。新規 feature の self ref check は insert 前 prefix で十分。
- `simulate_history::refs_resolve_in_state` の sketch transitive 化: #268 で対応
- `FeatureOp::apply` 一元化: ADR-015 (#246) 確定後の別 Issue
- パフォーマンス O(n) 追加 simulate_history call の最適化 (差分計算/incremental simulate): scope 外

## 実装対象

Issue: #267
影響クレート/ファイル:
- `crates/engawa-build/src/feature_crud.rs` (`check_no_downstream_break` 1 関数の修正)
- `crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs` (T_267 ファミリーを追記)

### 変更1: `check_no_downstream_break` を **insert 後 history** で executed_at_full を計算

before (現状、#266 で first-unprotected consumer semantics に直したもの):
```rust
fn check_no_downstream_break(f: &Feature, features: &[Feature], at: usize) -> Result<(), FeatureCrudError> {
    let (_, _, executed_at_full) = simulate_history(features, features.len());
    let consumed_bodies = feature_consumes(f);
    for body_id in consumed_bodies {
        for (consumer_idx, consumer_feat) in features.iter().enumerate().skip(at) {
            if !executed_at_full.contains(&consumer_idx) { continue; }
            // ... (per-consumer first-unprotected check)
        }
    }
    Ok(())
}
```

after (insert 後 hypothetical で再 simulate):
```rust
fn check_no_downstream_break(f: &Feature, features: &[Feature], at: usize) -> Result<(), FeatureCrudError> {
    // Hypothetical post-insert history: features[..at] ++ [f.clone()] ++ features[at..]
    let mut post_insert: Vec<Feature> = Vec::with_capacity(features.len() + 1);
    post_insert.extend_from_slice(&features[..at]);
    post_insert.push(f.clone());
    post_insert.extend_from_slice(&features[at..]);

    let (_, _, executed_at_full) = simulate_history(&post_insert, post_insert.len());
    let consumed_bodies = feature_consumes(f);

    // Walk consumers in post-insert index space: skip first (at+1) entries
    // (= features[..at] + the inserted feat itself which is at index `at`).
    // post-insert idx N+1 corresponds to original features idx N for N >= at.
    for body_id in consumed_bodies {
        for (post_idx, consumer_feat) in post_insert.iter().enumerate().skip(at + 1) {
            if !executed_at_full.contains(&post_idx) { continue; }
            let consumer_refs: Vec<&str> = feature_consumes(consumer_feat);
            let implicit_consumer_refs = feature_transitive_implicit_body_refs(consumer_feat, &post_insert);
            let direct_match = consumer_refs.contains(&body_id);
            let implicit_match = implicit_consumer_refs.iter().any(|r| r == body_id);
            if direct_match || implicit_match {
                // re_registered check in post-insert space, between (at+1) and post_idx
                let mut re_registered = false;
                for (reg_idx, reg_feat) in post_insert.iter().enumerate().skip(at + 1) {
                    if reg_idx >= post_idx { break; }
                    if !executed_at_full.contains(&reg_idx) { continue; }
                    if reg_feat.id() == body_id {
                        match reg_feat {
                            Feature::CreateBox { .. }
                            | Feature::CreateCylinder { .. }
                            | Feature::CreateSphere { .. }
                            | Feature::Extrude { .. }
                            | Feature::ExtrudeCut { .. }
                            | Feature::Cut { .. }
                            | Feature::Fuse { .. }
                            | Feature::Intersect { .. } => { re_registered = true; break; }
                            _ => {}
                        }
                    }
                }
                if !re_registered {
                    // Map back to ORIGINAL features index for error reporting.
                    let original_consumer_idx = post_idx - 1;
                    return Err(FeatureCrudError::InsertBeforeConsumer {
                        consumed_ref: body_id.to_string(),
                        displaced_feature_id: consumer_feat.id().to_string(),
                        consumer_at: original_consumer_idx,
                        requested_at: at,
                    });
                }
            }
        }
    }
    Ok(())
}
```

要点:
- `post_insert` を構築して `simulate_history` を 1 回呼ぶ追加コスト (1 history insert あたり 1 回)。決定的、副作用なし
- consumer 走査は post-insert 空間で `skip(at + 1)` (新規 feature の **次** から)
- re_registered check も post-insert 空間で、当該 consumer 手前まで
- `consumer_at` をエラーに乗せるときは原 features 空間に戻す (`post_idx - 1`)
- `displaced_feature_id` は feature id でそのまま意味を持つ

### 変更2: `feature_transitive_implicit_body_refs` の `features` 引数も `post_insert` へ統一

`feature_transitive_implicit_body_refs` は CreateSketch を lookup する。`post_insert` を渡すことで、新規 feature が `CreateSketch` の場合も transitive 経路に含まれる。これは #266 の意図と整合 (= insert 後の依存関係を見る)。

## 設計方針

- **決定性要件**: `post_insert` 構築は `extend_from_slice` + `push` で決定的。`simulate_history` も既存 deterministic。同一入力 → 同一出力
- **B-rep トポロジー妥当性**: N/A (validation 層)
- **退化幾何の扱い**: N/A
- **derive 規約**: 変更なし (関数 signature 不変)
- **エラーハンドリング**: 既存 `InsertBeforeConsumer` variant を流用、新規 variant 追加なし
- **workspace.dependencies 規約**: 依存追加なし

### 後方互換性 / 既存テスト影響

- `simulate_history` の挙動は不変 (atomic skip semantics 維持)。`post_insert` への入力切替のみ
- 既存 `feature_crud_acceptance.rs` / `feature_crud_plane_ref_acceptance.rs` (T02 等) は insert 前 history で broken-future がないため、post-insert と pre-insert で executed_at_full は同じ
- 既存 #265 / #266 の `feature_crud_prefix_validate_acceptance.rs` tests (broken prefix history で insert が新たな activation を起こさない場合) も挙動不変
- 唯一影響するのは「insert された feature が broken-future を活性化させる」シナリオで、これは #267 で初めて検出される

### consumer_at の意味

`consumer_at` は元 features 空間でのインデックスを返したい (caller が history index を解釈するため)。post-insert 空間の idx を `idx - 1` でマップして original 空間 (= insert 前 features) の idx に戻す。post_idx >= at + 1 を保証しているので `post_idx - 1 >= at` が成り立つ。

## テスト計画（ID 付き）

`tests/feature_crud_prefix_validate_acceptance.rs` の末尾に追記。

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| t_267_determinism | 決定性 | broken-future 活性化シナリオで同 history を 2 回 insert → エラー variant 同一 | `InsertBeforeConsumer` ×2、`format!("{:?}", err)` equal |
| t_267_cut_activates_broken_consumer | 正常系 | `[box_b1, box_b2, Cut(c1, target=new_box, tool=box_b1), Cut(c2, target=box_b1, tool=box_b2)]` に `CreateBox(new_box)` を idx 2 insert → c2 が壊れる | `Err(InsertBeforeConsumer { consumed_ref: "box_b1", displaced_feature_id: "c2", .. })` |
| t_267_extrudecut_activates_broken_consumer | 正常系 | 同パターンで ExtrudeCut が consumer 側 (broken→live 化) になるケース | `Err(InsertBeforeConsumer { consumed_ref: "box_*", displaced_feature_id: "ec*", .. })` |
| t_267_fuse_activates_broken_consumer | 正常系 | 同パターンで Fuse が consumer 側 | `Err(InsertBeforeConsumer { consumed_ref: "box_*", displaced_feature_id: "f*", .. })` |
| t_267_intersect_activates_broken_consumer | 正常系 | 同パターンで Intersect が consumer 側 | `Err(InsertBeforeConsumer { consumed_ref: "box_*", displaced_feature_id: "i*", .. })` |
| t_267_degen_no_activation_no_change | 退化境界 | broken-future が **無い** clean history で従来挙動を維持 (回帰防止) — `[box_1, box_2, Cut(c1, target=box_1, tool=box_2)]` に `CreateSphere(sp)` を idx 2 で insert → `Ok` | `Ok(new_doc)` |
| t_267_boundary_activated_consumer_safe_after_reregister | 退化境界 | 活性化される consumer の body が、その間で再登録される稀パターン → re_registered=true で Ok | `Ok(new_doc)` (新規 feature 挿入後、活性化される consumer の前で同 id が再登録されれば保護される) |

退化/境界 ID: `t_267_degen_no_activation_no_change`, `t_267_boundary_activated_consumer_safe_after_reregister` の 2 件。

## 幾何的不変条件チェックリスト

- N/A (validation 層のみ、partition / assemble / boolean のトポロジー操作を含まない)
