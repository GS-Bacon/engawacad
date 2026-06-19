## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `refs_resolve_in_state` の `Extrude` / `ExtrudeCut` 分岐で、参照する `CreateSketch` の `feature_implicit_body_refs` (= plane_ref body) を `live_bodies_at` で再評価する | 既存の `feature_transitive_implicit_body_refs` ヘルパの semantics 変更 |
| `refs_resolve_in_state` のシグネチャに `features: &[Feature]` 引数を追加し、呼び出し側 (`simulate_history` 内 6 箇所) を全て更新する | `check_refs_resolve_before` / `check_no_downstream_break` (line 436 / 521 / 581) で既に transitive 経路は反映済みのため触らない |
| `feature_crud_prefix_validate_acceptance.rs` に `T_269_*` 系の transitive plane_ref liveness 用テストを追加する | `CreateSketch` 自身の plane_ref check 経路 (#267 で導入済み) の変更 |
| 既存 acceptance test (#264 / #265 / #266 / #267 ＋全 prefix validate suite) の回帰なしを確認する | 新しい `FeatureCrudError` バリアント追加 (既存の `BodyNotFound` / 既存メッセージで足りる) |

## Non-Goals

- 新しい error variant の追加: 既存 `BodyNotFound` / `InsertBeforeProducer` が再利用される (Issue #265 で defensive validate を追加する別経路がある — 本 Issue では `simulate_history` の skip 判定のみ修正)
- `feature_transitive_implicit_body_refs` のキャッシュ化・パフォーマンス改善: `refs_resolve_in_state` は `simulate_history` 内で各 feature step ごとに `features` を線形走査する形になるが、本 Issue ではアルゴリズム的単純化を優先 (history 長 N に対し O(N^2) のままで進める)
- 名前付き helper の新規追加: 既存 `feature_transitive_implicit_body_refs` を直接呼ぶ
- pre-insert validation (`check_refs_resolve_before` line 436-485) の touch: 既に line 436 で `feature_transitive_implicit_body_refs` ベースの implicit_ref check が完備
- post-insert re-simulation (`#267` `simulate_history` 流) の追加実装: 本 Issue は `simulate_history` 内 `refs_resolve_in_state` の判定強化のみ
- engawa-format/engawa-kernel への変更: 本 Issue は engawa-build/feature_crud.rs 内に閉じる

## 実装対象

Issue: #269
影響クレート/ファイル: `crates/engawa-build/src/feature_crud.rs` (実装), `crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs` (テスト)

変更する関数のシグネチャ:
- `fn refs_resolve_in_state(f: &Feature, sketches_at: &HashMap<String, usize>, live_bodies_at: &HashMap<String, usize>) -> bool`
  → `fn refs_resolve_in_state(f: &Feature, features: &[Feature], sketches_at: &HashMap<String, usize>, live_bodies_at: &HashMap<String, usize>) -> bool`

### Before (現状, feature_crud.rs:158-196)

```rust
fn refs_resolve_in_state(
    f: &Feature,
    sketches_at: &HashMap<String, usize>,
    live_bodies_at: &HashMap<String, usize>,
) -> bool {
    match f {
        Feature::Extrude {
            sketch,
            fuse_target,
            ..
        } => {
            sketches_at.contains_key(sketch)
                && fuse_target
                    .as_ref()
                    .is_none_or(|t| live_bodies_at.contains_key(t))
        }
        Feature::ExtrudeCut { sketch, target, .. } => {
            sketches_at.contains_key(sketch) && live_bodies_at.contains_key(target)
        }
        Feature::Cut { target, tool, .. }
        | Feature::Fuse { target, tool, .. }
        | Feature::Intersect { target, tool, .. } => {
            live_bodies_at.contains_key(target) && live_bodies_at.contains_key(tool)
        }
        Feature::CreateSketch { plane_ref, .. } => {
            if let Some(PlaneRef::Entity(eref)) = plane_ref {
                for named_id in collect_named_feature_ids(eref) {
                    if !live_bodies_at.contains_key(&named_id) {
                        return false;
                    }
                }
            }
            true
        }
        _ => true,
    }
}
```

### After

```rust
fn refs_resolve_in_state(
    f: &Feature,
    features: &[Feature],
    sketches_at: &HashMap<String, usize>,
    live_bodies_at: &HashMap<String, usize>,
) -> bool {
    // 全 variant 共通の transitive implicit body refs check:
    // Extrude/ExtrudeCut が参照する CreateSketch.plane_ref body が live でなければ skip
    for implicit_ref in feature_transitive_implicit_body_refs(f, features) {
        if !live_bodies_at.contains_key(&implicit_ref) {
            return false;
        }
    }

    match f {
        Feature::Extrude {
            sketch,
            fuse_target,
            ..
        } => {
            sketches_at.contains_key(sketch)
                && fuse_target
                    .as_ref()
                    .is_none_or(|t| live_bodies_at.contains_key(t))
        }
        Feature::ExtrudeCut { sketch, target, .. } => {
            sketches_at.contains_key(sketch) && live_bodies_at.contains_key(target)
        }
        Feature::Cut { target, tool, .. }
        | Feature::Fuse { target, tool, .. }
        | Feature::Intersect { target, tool, .. } => {
            live_bodies_at.contains_key(target) && live_bodies_at.contains_key(tool)
        }
        // CreateSketch の直接 plane_ref も上の transitive ループでカバーされる
        // (feature_implicit_body_refs が CreateSketch 自身の plane_ref を返す)
        _ => true,
    }
}
```

呼び出し側 (`simulate_history` 内 line 233 / 248 / 259 / 269 / 280 / 291) は引数 `features` (= `simulate_history` がループしている slice) を追加で渡す形に書き換える。`simulate_history` 自体の引数は変更不要 (既に `features: &[Feature]` を持つ)。

## 設計方針

- **決定性**: 既存 `feature_transitive_implicit_body_refs` が `features` を順次走査する純関数で、同じ history に対して同じ結果を返すため、決定性は保たれる。
- **B-rep トポロジー妥当性**: 本 Issue は feature history の simulate 中の skip 判定強化のみで、生成される B-rep には触れない。Euler-Poincaré 不変条件は `crates/engawa-kernel` 側で別途維持される (N/A)。
- **退化幾何の扱い**: N/A (履歴 simulate 層の判定強化のため、幾何退化とは別軸)。
- **derive 規約**: 新しい公開型は追加しない (private fn の引数変更のみ)。
- **エラーハンドリング**: `refs_resolve_in_state` は `bool` を返す純関数のため、`thiserror` は使わない。`simulate_history` 側で broken-prefix を atomic skip する既存挙動を維持する。
- **workspace.dependencies**: 新規依存なし (engawa-format / std::collections のみ既存利用)。

### 数値モデル

N/A — 本 Issue は履歴 simulate 層の判定強化のみで、tolerance / ε 値は登場しない (Phase 8 の Phase 4/6+ 数値判定対象外)。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一 history `[box_1, sk(plane=Entity(box_1)), Cut(c1, target=box_1, tool=box_other), e1(sketch=sk), Extrude(...)]` を `FeatureCrud::insert` 2 回実行し全 ID 順序が一致 | assert_eq! 連続 2 回の post-insert document feature 順序 |
| T02 | 正常系 | clean history `[box_1, box_2, sk(plane=Entity(box_1)), e1(sketch=sk, fuse_target=box_2)]` で末尾 (idx=4) に追加 Extrude を insert → e1 は executed のまま、Extrude も resolve OK で Ok | `result.is_ok()` |
| T03 | 正常系 | clean history で末尾 insert (refs_resolve_in_state strict 化が clean path に影響しないこと) | `result.is_ok()` |
| T_269_extrude_transitive_plane_ref_dead | バグ捕捉 | history `[box_1, sk(plane=Entity(box_1)), Cut(c1, target=box_1, tool=box_other), e1(sketch=sk, fuse_target=None), Extrude(e2)]` を末尾 (idx=5) に `Cut(target=e1, tool=box_other2)` insert → e1 は sk の plane_ref body=box_1 が dead で skip されているため executed_at_full に無く、`BodyNotFound { body_ref: "e1" }` が返る | `Err(BodyNotFound { body_ref: "e1" })` |
| T_269_extrudecut_transitive_plane_ref_dead | バグ捕捉 | 同じく ExtrudeCut 経路: `[box_1, box_t, sk(plane=Entity(box_1)), Cut(c1, target=box_1, tool=box_2), ec1(sketch=sk, target=box_t)]` 末尾 insert で `Cut(target=ec1, tool=box_3)` → ec1 skip → `BodyNotFound { body_ref: "ec1" }` | `Err(BodyNotFound { body_ref: "ec1" })` |
| T_269_degen_clean_plane_ref_alive | 退化境界 (回帰防止) | `[box_1, sk(plane=Entity(box_1)), e1(sketch=sk, fuse_target=None)]` (plane_ref body box_1 は alive) で末尾 insert → e1 は executed のまま、Extrude(target=e1, ...) も成功 | `result.is_ok()` |
| T_269_degen_plain_planeref_unaffected | 退化境界 | `[box_1, box_2, sk(plane=PlaneRef::XYPlane), Cut(c1, target=box_1, tool=box_2), e1(sketch=sk)]` (PlaneRef::Entity でない sk は transitive 不要) で末尾 insert → 既存挙動と同じく e1 は executed (sk は box_1 に依存しないため) | `result.is_ok()` |
| T_269_boundary_self_dependency | 退化境界 | `[box_1, sk(plane=Entity(box_1)), e1(sketch=sk, fuse_target=box_1)]` (e1 が box_1 を直接 fuse) → 既存 `feature_body_refs` の Extrude.fuse_target check で broken: e1 自身が live_bodies_at に box_1 を必要とするが、box_1 は e1 自身の fuse_target で消費される直前。これは現行 behavior 維持 — Cut 等で box_1 が consume されていない限り e1 は OK | `result.is_ok()` (clean history 扱い) |
| T_269_regression_267_full | 回帰確認 | #267 の `t_267_normal_activates_broken_consumer` 系シナリオが新ロジック下で同じ verdict を返すこと | 既存 assertion がそのまま通る |
| T_269_regression_266_full | 回帰確認 | #266 の sketch transitive 系シナリオが同じ verdict を返すこと | 既存 assertion がそのまま通る |

退化/境界 ID: `T_269_degen_clean_plane_ref_alive`, `T_269_degen_plain_planeref_unaffected`, `T_269_boundary_self_dependency` の 3 件で 1 件以上の `_degen_` / `_boundary_` 要件を満たす。

## 幾何的不変条件チェックリスト

N/A — 本 Issue は履歴 simulate 層の skip 判定強化で、Boolean / Partition / Assemble 系の幾何不変条件には影響しない。
- [ ] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか → N/A
- [ ] 各プリミティブの face ごとの outer_loop 2D 向き → N/A
- [ ] flip_normals / same_sense の意味論 → N/A
- [ ] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか → N/A
