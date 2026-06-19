## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `refs_resolve_in_state(f, sketches_at, live_bodies_at) -> bool` 新規 helper (Extrude/ExtrudeCut/Cut/Fuse/Intersect の direct ref が prefix 状態で resolve できるかを純判定) | CreateSketch.plane_ref=Entity 経路の broken ref defensive (#264 で既に追跡 + Cut 系の transitive case は #266) — 本 Issue は direct ref のみ |
| `simulate_history` の prefix walking で各 step に `refs_resolve_in_state` ガードを挟む (resolve できなければ consume も register もスキップ = atomic skip) | `Document::validate()` の semantic-aware 化 (ADR-015 #246 で議論中の `FeatureOp` 一元化と整合させて別 Issue で扱う) |
| 既存 `feature_crud_acceptance.rs` (#263 19 件) / `feature_crud_plane_ref_acceptance.rs` (#264 15 件) の回帰維持 | `build_bodies_from_features` の dry-run 連動 (engawa-build side チェック) — 本 Issue は format-level semantic gate に限定 |
| 新規 acceptance test (broken prefix で insert が BodyNotFound/InsertBeforeProducer 系を返すこと) | 新規 FeatureCrudError variant 追加 (既存 BodyNotFound/SketchNotFound/InsertBeforeProducer/InsertBeforeConsumer で意味論成立) |
| CreateSketch は引き続き sketches_at に常時 register (CreateSketch 自身は body ref を持たないため、`refs_resolve_in_state` で弾く必要がない) | CreateSketch.plane_ref 経由の sketch_at 抑制 (#264 で別レイヤとして扱う、本 Issue は body producer 群の挙動限定) |

## Non-Goals

- CreateSketch.plane_ref=Entity 経路の broken ref defensive — #264 で feature_implicit_body_refs 経由の lifetime tracking を実装済。#266 で sketch user (Extrude/ExtrudeCut) の transitive 追跡を scope-defer
- `Document::validate()` の semantic-aware 化 — ADR-015 (#246 needs-human) の `FeatureOp` 一元化議論と統合するため別 Issue
- `build_bodies_from_features` の dry-run / pre-flight build — engawa-build builder 側の挙動変更は別 Issue (本 Issue は engawa-build/feature_crud の semantic gate 強化)
- 新規 FeatureCrudError variant — 既存 BodyNotFound / SketchNotFound / InsertBeforeProducer 系で broken prefix 経由の検出を表現可能
- CreateSketch の sketches_at register 抑制 — CreateSketch 自身は body ref を持たず、broken の判定対象外 (plane_ref は #264/#266 系で別途扱う)

## 実装対象

Issue: #265
影響クレート/ファイル:
- `crates/engawa-build/src/feature_crud.rs` (新規 helper 1 個 + 既存 `simulate_history` への guard 挿入)
- `crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs` (新規 integration test)

### 変更箇所 1: 新規 helper `refs_resolve_in_state` を追加

**Before** (line 132-135 抜粋):
```rust
/// Extract body IDs consumed by a feature (same as body_refs in current spec).
fn feature_consumes(f: &Feature) -> Vec<&str> {
    feature_body_refs(f)
}
```

**After** (`feature_consumes` の直下に追加):
```rust
/// Check whether all direct refs of a body-producer feature resolve against the
/// running prefix state (`sketches_at` / `live_bodies_at`).
///
/// Returns `true` for features that have no refs to validate (CreateBox/Cylinder/Sphere/CreateSketch).
/// Returns `false` if any sketch ref is missing from `sketches_at`, or any body ref
/// (target/tool/fuse_target) is missing from `live_bodies_at`.
///
/// This is the gate used by `simulate_history` to decide whether a prefix feature
/// "really executed". A feature that fails this check is treated as inert — its
/// inputs are not consumed and its output is not registered (atomic skip).
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
                    .map_or(true, |t| live_bodies_at.contains_key(t))
        }
        Feature::ExtrudeCut {
            sketch, target, ..
        } => sketches_at.contains_key(sketch) && live_bodies_at.contains_key(target),
        Feature::Cut { target, tool, .. }
        | Feature::Fuse { target, tool, .. }
        | Feature::Intersect { target, tool, .. } => {
            live_bodies_at.contains_key(target) && live_bodies_at.contains_key(tool)
        }
        // CreateBox/Cylinder/Sphere have no refs; CreateSketch's plane_ref is handled by
        // #264 lifetime tracking and is orthogonal to prefix body-producer atomicity.
        _ => true,
    }
}
```

### 変更箇所 2: `simulate_history` の prefix walk に atomic skip guard を挿入

**Before** (line 137-197 抜粋、Extrude 以降の body-producer 分岐):
```rust
            Feature::Extrude {
                id, fuse_target, ..
            } => {
                if let Some(target) = fuse_target {
                    live_bodies_at.remove(target);
                }
                live_bodies_at.insert(id.clone(), i);
            }
            Feature::ExtrudeCut { id, target, .. } => {
                live_bodies_at.remove(target);
                live_bodies_at.insert(id.clone(), i);
            }
            Feature::Cut {
                id, target, tool, ..
            } => {
                live_bodies_at.remove(target);
                live_bodies_at.remove(tool);
                live_bodies_at.insert(id.clone(), i);
            }
            Feature::Fuse {
                id, target, tool, ..
            } => {
                live_bodies_at.remove(target);
                live_bodies_at.remove(tool);
                live_bodies_at.insert(id.clone(), i);
            }
            Feature::Intersect {
                id, target, tool, ..
            } => {
                live_bodies_at.remove(target);
                live_bodies_at.remove(tool);
                live_bodies_at.insert(id.clone(), i);
            }
```

**After** (各 body-producer arm の冒頭で `refs_resolve_in_state` チェック → 失敗なら continue):
```rust
            Feature::Extrude {
                id, fuse_target, ..
            } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    // Broken ref in prefix — atomic skip: no consume, no register.
                    continue;
                }
                if let Some(target) = fuse_target {
                    live_bodies_at.remove(target);
                }
                live_bodies_at.insert(id.clone(), i);
            }
            Feature::ExtrudeCut { id, target, .. } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    continue;
                }
                live_bodies_at.remove(target);
                live_bodies_at.insert(id.clone(), i);
            }
            Feature::Cut {
                id, target, tool, ..
            } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    continue;
                }
                live_bodies_at.remove(target);
                live_bodies_at.remove(tool);
                live_bodies_at.insert(id.clone(), i);
            }
            Feature::Fuse {
                id, target, tool, ..
            } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    continue;
                }
                live_bodies_at.remove(target);
                live_bodies_at.remove(tool);
                live_bodies_at.insert(id.clone(), i);
            }
            Feature::Intersect {
                id, target, tool, ..
            } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    continue;
                }
                live_bodies_at.remove(target);
                live_bodies_at.remove(tool);
                live_bodies_at.insert(id.clone(), i);
            }
```

`CreateBox` / `CreateCylinder` / `CreateSphere` / `CreateSketch` の分岐は無改変 (引き続き unconditional register)。

注: `refs_resolve_in_state` 内で `Feature::CreateSketch` は `_ => true` で fall-through。CreateSketch.plane_ref=Entity の defensive は #264 で扱う orthogonal レイヤなので本 Issue では介入しない。

## 設計方針

- **決定性**: `refs_resolve_in_state` は HashMap.contains_key のみ。simulate_history の prefix walk 順序は無改変 (`for (i, f) in features.iter().enumerate()`)。atomic skip による live_bodies_at の最終状態は同一入力 → 同一 (T01 で検証)
- **B-rep トポロジー妥当性**: 該当なし (semantic gate のみ。kernel 側 Euler-Poincaré 不変条件は不変)
- **退化幾何の扱い**: 該当なし (本 Issue は ref tracking)
- **derive 規約**: 新規型なし
- **エラーハンドリング**: 既存 `FeatureCrudError::BodyNotFound` / `SketchNotFound` / `InsertBeforeProducer` を再利用 (broken prefix で live_bodies_at に未登録 → 既存の lookup ロジックが BodyNotFound を返す経路を活かす)
- **workspace.dependencies**: 新規 dep 追加なし
- **atomic skip semantics**: 「broken prefix feature は実行されなかったとみなす」= 入力 consume なし + 出力 register なし。中途半端な状態 (consume only / register only) は不整合になるため不可

### 数値モデル

本 Issue は数値モデルを扱わない。tolerance なし。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | broken prefix history + 同 insert を 2 回 → エラー variant 同一 + Document YAML byte-equal | `assert_eq!` |
| T02 | 正常系 (Cut tool broken) | history = [box_b1, box_b2, Cut(c1, target=b1, tool=missing)] に Cut(f1, target=c1, tool=b2) を tail で insert | `Err(BodyNotFound { feature_id: "f1", body_ref: "c1" })` (c1 が atomic skip で live でない) |
| T03 | 正常系 (Extrude sketch broken) | history = [box_b1, Extrude(e1, sketch=missing_sk, fuse_target=box_b1)] に Cut(f1, target=e1, tool=box_b1) を tail で insert | `Err(BodyNotFound { feature_id: "f1", body_ref: "e1" })` (e1 が atomic skip で live でない、かつ box_b1 は consume されず live のまま) |
| T04 | 正常系 (Fuse target broken) | history = [box_b1, Fuse(g1, target=missing_a, tool=box_b1)] に Cut(f1, target=g1, tool=box_b1) を tail で insert | `Err(BodyNotFound { feature_id: "f1", body_ref: "g1" })` |
| T05 | 正常系 (Intersect tool broken) | history = [box_b1, box_b2, Intersect(i1, target=box_b1, tool=missing)] に Cut(f1, target=i1, tool=box_b2) を tail | `Err(BodyNotFound { feature_id: "f1", body_ref: "i1" })` |
| T06 | 正常系 (cascade — broken prefix 後の good feature も skip) | history = [box_b1, Cut(c1, target=b1, tool=missing), Extrude(e1, sketch=sk1, fuse_target=c1)] (CreateSketch sk1 も含む) に Cut(f1, target=e1, tool=box_b2) を tail で insert | `Err(BodyNotFound { feature_id: "f1", body_ref: "e1" })` (c1 skip → e1 の fuse_target=c1 が resolve せず e1 も skip → e1 not live) |
| T07 | 正常系 (broken でない box は consume されず live のまま) | history = [box_b1, box_b2, Cut(c1, target=b1, tool=missing)] に Cut(f1, target=box_b1, tool=box_b2) を tail で insert | `Ok(_)` (b1 は c1 が skip されたため依然 live) |
| T08 | 正常系 (ExtrudeCut sketch broken) | history = [box_b1, ExtrudeCut(ec1, sketch=missing_sk, target=box_b1, depth=5.0)] に Cut(f1, target=ec1, tool=box_b2) を tail (box_b2 を事前 push) | `Err(BodyNotFound { feature_id: "f1", body_ref: "ec1" })` |
| T_degen_clean_history | 退化 (broken なし) | clean な history で既存挙動と変わらないこと | `Ok(_)` (T_boundary_existing_acceptance と合わせて回帰確認) |
| T_degen_first_feature_broken | 退化 (先頭が broken) | history = [Cut(c1, target=missing_a, tool=missing_b)] に CreateBox(box_new) を tail で insert | `Ok(_)` (CreateBox 自身に ref がないため成功。c1 は skip された結果として無害) |
| T_boundary_existing_acceptance_passes | 境界 (既存 acceptance 回帰) | 既存 `feature_crud_acceptance.rs` / `feature_crud_plane_ref_acceptance.rs` 34 件 が回帰しないこと (rustdoc note + CI で確認) | 既存テスト全 pass |

注: T02-T08 は broken prefix → atomic skip の各 variant ごとの検証。T_degen_* は退化/境界の挙動。既存 34 件の回帰は CI 全体 (`cargo xtask ci`) で検証する。

## 幾何的不変条件チェックリスト

- N/A (本 Issue は engawa-build/feature_crud の semantic gate 拡張。kernel 側の Boolean / Partition / Assemble は touch しない)
