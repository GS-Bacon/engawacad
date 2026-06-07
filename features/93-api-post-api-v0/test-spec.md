# Test Spec — #93 POST /api/v0/features

## 不足テスト（plan 計画分）

acceptance skeleton の T01〜T06 は全て実装済み（`#[ignore]` 解除済み）。
また、T05_boundary / T06_degen も追加実装済み（auto-modified skeleton に含まれていなかったため Claude が追記）。

| ID | 種別 | 内容 | テスト関数 | 状態 |
|----|------|------|-----------|------|
| T01 | 決定性 | 2 つの独立 temp コピーに同一 Feature を POST → JSON 一致 | `t01_determinism` | 実装済み |
| T02 | 正常系 | 有効 CreateBox Feature を POST → 200、bodies 非空、box_1+box_2 確認 | `t02_post_creates_body` | 実装済み |
| T03 | 冪等性 | POST 直後レスポンス == 後続 GET /api/v0/mesh | `t03_idempotency_post_eq_get` | 実装済み |
| T04 | 累積 | 連続 POST × 2（異なる id）→ 3 bodies（box_1+box_a+box_b） | `t04_accumulation` | 実装済み |
| T05_boundary | 異常 | 重複 feature_id の Feature を POST → 422、ファイル不変 | `t05_boundary_duplicate_id_rollback` | 実装済み |
| T06_degen | 異常 | shape 不正 body（`id` 欠落）を POST → 422、状態不変 | `t06_degen_invalid_body_shape` | 実装済み |

## 実装差分から追加すべきテスト

実装差分 (crates/mycad-api/src/{state.rs, handler.rs, router.rs, error.rs}) を確認した結果:

1. **`write_atomic` の tmp 残留**（AM02 指摘）: `rename` 失敗時に best-effort `remove_file` している。
   `rename` 失敗シナリオは OS レベルのテストが難しいため、統合テストでは検証不可。
   実装コードのコメント（IN01）で順序不変条件を明記済み。追加テスト不要。

2. **`ensure_loaded` の再ロード抑制**: 同一 app インスタンスで GET × 2 回の場合、2 回目は
   `from_path` を呼ばない（doc が Some の場合はスキップ）。内部 unit test としては有効だが、
   統合テストから証明が難しいため本スコープでは省略（STEP 6 の GLM テスト実装で対処可）。

3. **POST 後の GET 冪等性**（T03）: 別 app インスタンスを使って disk から再ロードさせる手法で確認済み。
   同一インスタンスでの GET は T03 が間接的にカバー（in-memory doc が update 済みのため）。

4. **`assemble_and_tessellate` の empty assembly ガード**: `bodies.is_empty()` → 422 のパス。
   現 fixture（simple_box.mycad）は常に 1 body を持つため直接テストしにくい。GET 側で
   既存テスト `mesh_api.rs` がカバー済み（リグレッション T07 に含まれる）。

## エッジケース・退化入力

| ケース | 対応 | テスト |
|--------|------|--------|
| 重複 feature_id | `candidate.validate()` → `FormatError::DuplicateFeatureId` → 422 | T05_boundary |
| shape 不正 JSON（`id` 欠落等） | axum `Json<Feature>` デシリアライズエラー → 422 | T06_degen |
| 完全な invalid JSON（non-JSON body） | axum `JsonSyntaxError` → 422 | T06_degen の拡張（未テスト、同じ 422 経路） |
| ファイル書き戻し失敗 | `write_atomic` → `ApiError::Internal` → 500 | OS レベルで再現困難、実装コメントで保証 |

## 数値境界

N/A。API 層のみ。数値閾値なし。

## 決定性

- T01: 独立 temp コピー 2 つで同一 Feature を POST → JSON レスポンスが `assert_eq!` で一致。
- `IdGenerator::new(0)` を毎回 `assemble_and_tessellate` 内で生成するため、同一 Document → 同一 Vec<BodyMesh> は構造的に保証される。
- `face_ids` 決定性は #92 で保証済み（`canonical_name()` は pure function）。

## GLM への実装指示（STEP 6.6 — 追加テスト）

以下の追加テストが有効:

1. **`ensure_loaded` のキャッシュ挙動**: `post_feature` 後に同一 app インスタンスで GET を呼ぶと
   `doc.is_some()` のため再ロードなしで POST 後の状態を返すことを、inline unit test で確認。
   ただし handler は async 且つ State 経由のため、mockless な unit test が難しい。
   `state.rs` の `AppState::ensure_loaded` 単体テストとして追加すること:
   ```rust
   // crates/mycad-api/src/state.rs 末尾 #[cfg(test)] mod tests
   // 存在しない path で new → ensure_loaded で error、doc は None のまま
   // 存在する path で new → ensure_loaded で Ok、doc が Some になる
   // 2 回目 ensure_loaded は from_path を呼ばない（doc は Some のまま）
   ```

2. **T07 リグレッション（GET /mesh）**: 既存 `mesh_api.rs` のテスト群が State 型変更後も全通ることを確認。
   実装後の `cargo test -p mycad-api --test mesh_api` が green であることが確認済み。追加テスト不要。
