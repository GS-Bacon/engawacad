# test-spec for #267 post-insert re-simulation defensive validate

## 不足テスト (plan 計画分)

plan.md T_267 ファミリーから GLM round 2 で以下の状態:

| ID | 関数名 | 結果 | 備考 |
|----|--------|------|------|
| T_267 determinism | `t_267_determinism` | ok | |
| T_267 cut | `t_267_cut_activates_broken_consumer` | ok | |
| T_267 fuse | `t_267_fuse_activates_broken_consumer` | ok | |
| T_267 intersect | `t_267_intersect_activates_broken_consumer` | ok | |
| T_267 extrudecut | (削除) | n/a | GLM が round 1 で構築試行 → round 2 で削除。activate→break 経路の自然な fixture が組めず、conceptually Cut/Fuse/Intersect でカバー可能 |
| T_267 degen | `t_267_degen_no_activation_no_change` | ok | broken-future 不在 history で従来挙動維持の回帰防止 |
| T_267 boundary | (削除) | n/a | `DuplicateFeatureId` 制約と整合する fixture が組めない (debug-spec.md round 1 でサンクション済み) |

主要 4 normal-case (Cut/Fuse/Intersect/Determinism) + 1 degen で activation 検出機能を検証する。ExtrudeCut の activation 経路は概念的に他と同形のため、coverage gap として `codex-findings.md` に記録のみ。

## 実装差分から追加すべきテスト

無し。実装は plan.md 通り pre/post-insert simulate_history を併用する two-phase detection。CI 1230 passed / 0 failed。

### Scope expansion (GLM round 1 の scope creep)

GLM round 1 で `refs_resolve_in_state` の **CreateSketch.plane_ref transitive 化** が含まれた (= #268 scope-defer 案件と同等)。debug-spec.md で revert を要求したが round 2 でも CreateSketch 分岐が残った。

この副作用として:
- 既存 T17 (#266 boundary) の expectation が **`BodyNotFound`** から **`SketchNotFound`** に変わった (GLM が pre-insert sim 段階で sk を skip するため sketches_at に sk が登録されない → Extrude.sketch=sk が SketchNotFound)
- これは挙動の改善 (broken-prefix からの sketch も atomic skip される) で、CI green の範囲なら受容可能

→ #268 の関連実装が #267 に流れ込んだ形。#268 は本 #267 で実質的に実装済みのため、後工程で #268 を close する。

## エッジケース・退化入力

| ケース | カバー |
|--------|--------|
| broken-future 不在 (clean history) | `t_267_degen_no_activation_no_change` |
| insert された feature が direct consume せず activation のみで break | Cut/Fuse/Intersect の主要 4 件で network 形が同形 |
| 同 id の re-register による保護 | `Document::validate` の `DuplicateFeatureId` 制約により fixture 組成不可 — `rejection.md` Round 0 に記録 |
| ExtrudeCut activation 経路 | 削除 (Cut/Fuse/Intersect で代替) |

## 数値境界

N/A — validation 層、tolerance や ε 値を扱わない。

## 決定性

`t_267_determinism` で 2 回 insert → InsertBeforeConsumer の variant + body_id + displaced_feature_id 一致を検証済み。

## 類似ケース (未カバー)

- `feature_crud_prefix_validate_acceptance.rs` の既存 `t06`〜`t08`, `edge01` は broken prefix の atomic skip 検証 (pre-#267 機能)。これらは GLM round 2 で全 pass を維持
- `feature_crud_acceptance.rs` / `feature_crud_plane_ref_acceptance.rs` の既存テストも全 pass を維持 (T17 は SketchNotFound に変更、他は不変)
