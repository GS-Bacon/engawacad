# Test Spec for #265

GLM core dispatch が plan.md の T01〜T_boundary_existing_acceptance_passes 11 件を全て実装済み (`crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs`)。CI 全体も green (`cargo xtask ci`)。

以下は **追加すべきテスト** の指示書 (STEP 6.6 で GLM test-implementer に任せる差分のみ列挙)。

## 不足テスト（plan 計画分）

なし — plan T01〜T08 + T_degen_clean_history / T_degen_first_feature_broken / T_boundary_existing_acceptance_passes をすべて実装済み。

## 実装差分から追加すべきテスト

GLM core 実装の `refs_resolve_in_state` ヘルパと `simulate_history` 各 body-producer arm の atomic skip guard により、broken prefix feature は inputs を consume せず outputs も register しない。以下の差分由来パターンを追加:

- **DIFF01 (Extrude with broken sketch but Some(fuse_target))**: history = `[box_b1, Extrude(e1, sketch=missing_sk, fuse_target=Some(box_b1))]` → e1 が skip されると box_b1 は consume されず live のまま。tail で `Cut(target=box_b1, tool=box_b2)` を insert すると `Ok` を返すことを確認。(T03 は f1 が e1 を target にして BodyNotFound を assert するが、別軸として「fuse_target だった box_b1 が consume されずに残る」副作用の不存在を確認する)
- **DIFF02 (Cut with both target and tool broken)**: history = `[Cut(c1, target=missing_a, tool=missing_b)]` で c1 が skip されること (live_bodies_at に c1 が入らない) を確認。tail で `Cut(target=c1, tool=any)` を insert → `BodyNotFound { feature_id, body_ref: "c1" }` で `body_ref` が "c1" であることを assert。

## エッジケース・退化入力

- **EDGE01 (broken prefix → 後で同 id の正常 register が出るとどうなるか)**: history = `[Cut(c1, target=missing_a, tool=missing_b), CreateBox(c1)]` で c1 は最初の Cut で skip されるが、後の CreateBox(c1) で改めて live になる。tail で `Cut(target=c1, tool=any)` を insert → live_bodies_at で c1 が見つかるため `InsertBeforeProducer` ではなく `BodyNotFound { body_ref: "any" }` (tool=any のため) が返ることを確認。(*c1 の再登録 を確認するため、tool=box_real のような実際に存在する body を使う variant も用意して `Ok(_)` を assert する 2 ケース構成)*
- **EDGE02 (broken prefix が複数連鎖)**: history = `[Cut(c1, target=missing_a, tool=missing_b), Cut(c2, target=c1, tool=missing_d), Cut(c3, target=c2, tool=missing_e)]` で c1, c2, c3 すべて skip されることを確認 (cascade)。tail で `CreateBox(box_new)` を insert → `Ok(_)` (新規 box は ref を持たない)

## 数値境界

該当なし (本 Issue は ref tracking。tolerance 値を扱わない)。

## 決定性

- T01 で同一入力 → 同一エラー variant の assert 済み。
- 追加 (`T01b_determinism_yaml_byteequal`): history = `[box_b1, box_b2, Cut(c1, target=b1, tool=missing)]` に Cut(f1, target=box_b1, tool=box_b2) を insert (Ok path) → 2 回の `to_yaml()` が byte-equal であることを assert (T07 と同じ history で Ok path を 2 回実行する determinism test)。
