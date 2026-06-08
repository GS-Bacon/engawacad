# test-spec: #101 api-state-private

## 不足テスト（plan 計画分）

GLM が state.rs のインライン `#[cfg(test)] mod tests` に T01〜T01_degen を実装済み。
state_invariants.rs には公開 API 経由の ensure_loaded 読み取り専用テストが追加済み。
4 件すべて実装済みのため追加実装不要。

## 実装差分から追加すべきテスト

なし。変更は:
1. `state.rs` — `doc` 非公開化・`ensure_loaded(&Document)` 変更・`snapshot`/`commit` 追加
2. `handler.rs` — `post_feature` を `snapshot`/`commit` 経由に変更

handler.rs の post_feature は既存の integration tests（`post_features_acceptance.rs`, `e2e_api_scenarios.rs`）が HTTP 経由でカバー。

## エッジケース・退化入力

- `commit()` をロード前に呼ぶ → T01_degen_commit_without_load_no_panic で検証済み
- `ensure_loaded()` が非存在パスでエラー → 既存 ensure_loaded_nonexistent_path_returns_error_and_doc_stays_none でカバー

## 数値境界

N/A（数値演算なし）

## 決定性

N/A（状態管理の public API 変更のみ）

## 結論

T01〜T01_degen が実装済みのため STEP 6.6 GLM テスト追加実装は不要。state shim で glm_impl を passed にセット。
