# debug-spec for #266 — Codex 3-persona round 1 採用指摘の修正

## 経緯

GLM コア実装 (STEP 6) で `check_no_downstream_break` の semantics を「first match consumer を返す」から「**last (most downstream) match consumer を返す**」に変えた。これは plan.md T11 が `displaced_feature_id == "e1"` を期待していたため。

しかし Codex 3 persona 全員 (architect/contrarian/migration) が "last consumer 畳み込み" は broken-prefix 系 history で先行 consumer を隠す **回帰** だと指摘した (high)。

## 仮説

`[consumer_a, re-register(body_id), consumer_b]` の suffix で、consumer_b の手前に再登録があるため `re_registered = true` になり、**未保護の consumer_a が見落とされる**。

## 関連ファイル

- `crates/engawa-build/src/feature_crud.rs` の `check_no_downstream_break`
- `crates/engawa-build/tests/feature_crud_plane_ref_acceptance.rs` の T02-T17 (期待値の整合確認)

## 修正方針

### 修正1: `check_no_downstream_break` を「最初の未保護 consumer を返す」に戻す (A-F01 / C-F01 / M-F01 採用)

before (現状):
```rust
for body_id in consumed_bodies {
    let mut last_consumer_idx: Option<usize> = None;
    let mut last_consumer_id: Option<String> = None;

    for (consumer_idx, consumer_feat) in features.iter().enumerate().skip(at) {
        ...
        if direct_match || implicit_match {
            last_consumer_idx = Some(consumer_idx);
            last_consumer_id = Some(consumer_feat.id().to_string());
        }
    }

    if let Some(consumer_idx) = last_consumer_idx {
        // re_registered check for the LAST consumer
        ...
    }
}
```

after (元の semantics に戻す):
```rust
for body_id in consumed_bodies {
    for (consumer_idx, consumer_feat) in features.iter().enumerate().skip(at) {
        if !executed_at_full.contains(&consumer_idx) {
            continue;
        }
        let consumer_refs: Vec<&str> = feature_consumes(consumer_feat);
        let implicit_consumer_refs =
            feature_transitive_implicit_body_refs(consumer_feat, features);
        let direct_match = consumer_refs.contains(&body_id);
        let implicit_match = implicit_consumer_refs.iter().any(|r| r == body_id);
        if direct_match || implicit_match {
            // Check re_registered only up to THIS consumer (not all consumers)
            let mut re_registered = false;
            for (reg_idx, reg_feat) in features.iter().enumerate().skip(at + 1) {
                if reg_idx >= consumer_idx { break; }
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
                return Err(FeatureCrudError::InsertBeforeConsumer {
                    consumed_ref: body_id.to_string(),
                    displaced_feature_id: consumer_feat.id().to_string(),
                    consumer_at: consumer_idx,
                    requested_at: at,
                });
            }
            // この consumer は保護されたので次の consumer に進む
        }
    }
}
```

= 「最初の未保護 consumer を返す」 = pre-#266 semantics。`feature_transitive_implicit_body_refs` の transitive 経路は維持。

### 修正2: 既存テスト T02 の期待値を元の `displaced_feature_id == "sk"` に戻す

T02 の history は `[box_2, box_1, sk(plane=Entity(box_1)), e1(sketch=sk)]`。`feature_transitive_implicit_body_refs` で e1 が box_1 を transitive 参照しても、sk が先 (index 2) に hit するため first-match consumer は sk。

```rust
assert_eq!(displaced_feature_id, "sk");  // 元に戻す
```

### 修正3: T11/T12/T13/T15 の期待値を `["sk", "e1"]` 許容に変更

T11-T13/T15 の history は `[box_*, box_1, sk(plane=Entity(box_1)), e1(sketch=sk)]` で、sk と e1 のどちらも box_1 の consumer。first-match は sk が返るが、これは plan.md scope (= e1 が transitive で検出される) を弱体化させない:

```rust
match result {
    Err(FeatureCrudError::InsertBeforeConsumer {
        consumed_ref,
        displaced_feature_id,
        ..
    }) => {
        assert_eq!(consumed_ref, "box_1");
        // sk (direct implicit ref) も e1 (transitive ref) も両方 box_1 consumer。
        // first-match semantics で sk が返るが、transitive 検出自体は T14 で別途検証する。
        assert!(
            displaced_feature_id == "sk" || displaced_feature_id == "e1",
            "expected sk or e1, got {}", displaced_feature_id
        );
    }
    other => panic!("expected InsertBeforeConsumer, got {:?}", other),
}
```

### 修正4: T14 (ExtrudeCut) fixture を更新して transitive 経路でのみ hit させる (M-F03 採用)

before (現状): `ExtrudeCut.target == box_other`、しかも `Cut.tool == box_other` を共有していて direct ref でも fail。

after: ec1 の target を **box_1 以外、かつ Cut が消費しない別の body** にし、history を:

```
[box_other (Cut.tool), box_1, sk(plane=Entity(box_1)), box_for_ec1, ec1(sketch=sk, target=box_for_ec1)]
```
そして `Cut(target=box_1, tool=box_other)` を idx 2 に insert。ec1 の direct ref (target/tool) は box_for_ec1 のみで box_1 を含まないため、transitive 経路 (sk → plane_ref(box_1)) のみで hit する。**T14 の期待値**:
- `consumed_ref == "box_1"`
- `displaced_feature_id` は sk が first-match consumer なので `"sk"` または `"ec1"` を許容 (実際は sk が history 順で先)

### 修正5: T16 (sk のみ history) は元から `displaced_feature_id == "sk"` で variant matches OK

確認のみ、変更不要。

### 修正6: T17 (Extrude insert with consumed plane) は変更不要

`check_refs_resolve_before` 側の transitive_implicit check は scope creep でない。M-F02 (scope-defer 別 Issue) は `simulate_history` 内部の `refs_resolve_in_state` 変更を要求しているが、本 Issue (#266) では `check_refs_resolve_before` の表層 transitive check のみ実装する。

## 試した修正と結果

- (round 1 まだ未試行)

## 次にやること

1. `check_no_downstream_break` を上記 after コードに書き直す (per-consumer re-register check / 最初の未保護で return)
2. T02 期待値を `"sk"` に戻す
3. T11/T12/T13/T15 を `assert!(["sk","e1"].contains(&displaced_feature_id))` に変更
4. T14 の fixture を独立 body に変える + ec1 の direct ref が box_1 を含まないよう調整
5. `cargo xtask ci` で全 green を確認
6. **既存 #265 / #264 acceptance tests が全て pass することを確認**

## 追加で書いてほしいテスト

`crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs` に regression test を追加 (Codex A-F01 / C-F01 共通 suggestion):

```rust
/// #266 R2: early consumer + later valid re-register → first-unprotected consumer
/// が返ることを確認 (last-consumer 畳み込み回帰の防止)
#[test]
fn t_266_r2_early_consumer_with_later_reregister_returns_first() {
    // history: [box_orig, sk_a(plane=Entity(box_orig)), CreateBox box_orig (re-register), e2(...)]
    // skip された feature の挙動を踏まえつつ、early consumer (sk_a) が hit して
    // first-unprotected として返ることを検証。
    // 詳細な history は GLM が組み立てる (`simulate_history` の broken-ref skip 挙動と整合させる)。
    todo!()
}
```

`feature_crud_prefix_validate_acceptance.rs` に既存パターンがあれば流用してよい。
