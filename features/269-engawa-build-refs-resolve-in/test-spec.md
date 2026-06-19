# test-spec.md — Issue #269

## 概況

STEP 6 (GLM コア実装) で `refs_resolve_in_state` の transitive plane_ref liveness 反映を実装済み。
GLM は plan.md の T_269_* テスト群を `crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs` に追加した (skeleton で先行 Write した `refs_resolve_transitive_plane_ref_acceptance.rs` には書かれなかった)。

`cargo test --workspace --test feature_crud_prefix_validate_acceptance`: **35 件 pass, 0 fail, 0 ignored**。
plan.md のテスト計画 ID 表は全て実装済みで、 関数名 mapping は以下:

| plan ID | 実装関数名 (prefix_validate file) | 状態 |
|---------|----------------------------------|------|
| T01 (決定性) | `t_269_determinism` | ✓ pass |
| T02 (正常系 / clean tail insert) | `t_269_clean_history_tail_insert` | ✓ pass |
| T03 (正常系 / strict 化が clean 回帰しない) | `t_269_clean_history_unaffected` | ✓ pass |
| T_269_extrude_transitive_plane_ref_dead | 同名 | ✓ pass |
| T_269_extrudecut_transitive_plane_ref_dead | 同名 | ✓ pass |
| T_269_degen_clean_plane_ref_alive | 同名 | ✓ pass |
| T_269_degen_plain_planeref_unaffected | 同名 | ✓ pass |
| T_269_boundary_self_dependency | 同名 | ✓ pass |
| T_269_regression_267_full | (暗黙) — 既存 `t_267_*` 系が prefix_validate file に同居しているため、新ロジックで pass しないと CI red になる | ✓ pass (回帰検出) |
| T_269_regression_266_full | (暗黙) — 同上で `t_266_*` 系が同居 | ✓ pass |

## 不足テスト (plan 計画分)

- なし。plan 表の全 ID をカバー済み。
- ただし `T_269_regression_266_full` と `T_269_regression_267_full` は単独の関数を追加せず、同居する既存 `t_266_*` / `t_267_*` テストの pass で代替している。これは「同一 test binary 内に regression 用 fixture が既にあるため、再実装すると単なる duplication」と判断したと推測される (GLM コミットメッセージで判断推定)。回帰検出は十分。

## 実装差分から追加すべきテスト

- なし。`refs_resolve_in_state` のシグネチャに `features: &[Feature]` 引数を追加した点と、全 variant 共通の transitive implicit body refs check を冒頭に置いた点について、`Cut` / `Fuse` / `Intersect` 経路は **direct ref に sketch を取らない** ため transitive ループは空になり、既存 `t_02_broken_cut_tool_skips_output` / `t_04_broken_fuse_target_skips_output` / `t_05_broken_intersect_tool_skips_output` で挙動不変が回帰確認済み (= 既存 pass = 回帰なし)。
- 新しいエラーパス・新しい error variant は追加されていない。

## エッジケース・退化入力

- `T_269_degen_clean_plane_ref_alive`: clean history で plane_ref body が alive な場合に skip されないこと
- `T_269_degen_plain_planeref_unaffected`: `PlaneRef::Entity` でない sketch (= `PlaneRef::XYPlane` 等) では transitive ループが空になるため、新ロジック導入で挙動が変わらないこと
- `T_269_boundary_self_dependency`: Extrude が `fuse_target=box_1` で box_1 を直接 consume するが、box_1 自身は alive のままなので clean扱いになる境界

## 数値境界

N/A — 本 Issue は履歴 simulate 層の bool 判定で、tolerance / ε は登場しない (Phase 8 数値判定対象外)。

## 決定性

`T_269_determinism` を追加: 同一 Document に `FeatureCrud::insert` を 2 回呼んで結果の `Document.root_component.features` を比較し、order・id が完全一致することを assert している。

## 類似ケース（未カバー）

本 Issue は `bug` ラベルでなく `type: foundation` のため STEP 6.5 §3.5 (類似ケース追加チェック) は対象外。
ただし参考として:
- `refs_resolve_in_state` の transitive 経路は `Cut` / `Fuse` / `Intersect` でも空ループとして通過するため、これら 3 variant の挙動不変は既存 t02/t04/t05 acceptance で回帰確認されている。
- `feature_transitive_implicit_body_refs` 自身は #266 で導入されたヘルパで、本 Issue は `refs_resolve_in_state` 内での **呼び出し位置を増やす** のみ。helper のアルゴリズム変更は無いため、helper 単体のテスト追加は不要 (既存 #266 acceptance がカバー)。

## 期待値乖離

`check-spec-divergence.ts` 実行: plan.md に T ID 周辺の数値段落なし。implementation 側の diff は `git diff main..HEAD` 経由で取得できない (本 repo の base が `claude/add-claude-guidelines-BKKtD` のため false negative) が、Claude が手動 grep した結果、新規 assertion は plan の `Err(BodyNotFound { body_ref: ... })` 期待と一致している (T_269_extrude/extrudecut テストで `panic!` メッセージとして verify)。**乖離なし** — STEP 6.6 へ進める。

## Follow-up メモ

- `crates/engawa-build/tests/refs_resolve_transitive_plane_ref_acceptance.rs` (STEP 5.5 で生成した skeleton) が **未削除のまま残っている**。10 件の `#[ignore = "STEP 6 で実装後に解除"]` `todo!()` stub が ignored 扱いで CI に出力される。挙動には影響しないが視認性が悪い。
  - 削除を試みたが crates/** への破壊操作 guard でブロックされた (`/3ai` フロー)。
  - 次サイクル B-7 sweep か手動で `git rm` で除去するのが望ましい。今 Issue では merge を優先し、本 follow-up を残す。
