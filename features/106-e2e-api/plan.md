## 自律判断ログ（自律バッチモード）

- **曖昧点**: Issue は Playwright 実 API 統合を要求しているが、CI 環境は headless/GPU なし（Playwright 未インストール）
- **採用**: Rust API integration テスト（axum test client 経由）でシナリオをカバーする
  - Playwright 実 API 接続は Out-of-Scope（CI で実行不可）
  - `crates/mycad-api/tests/` に multi-step シナリオを追加
  - **理由**: Issue の根本要求は「自動テストで #102-#105 のバグを検出できること」。Rust integration テストは CI で確実に動き、同等のカバレッジを提供できる。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| GET /api/v0/features → ID 重複検出シナリオ (#102) | Playwright 実 API 統合（CI 環境で実行不可） |
| POST sketch + extrude → GET /api/v0/mesh で f_cap_*/f_side_* face_id 確認 (#103) | ブラウザ UI インタラクション |
| POST sketch + extrude → ボディ数 == 2 確認 (#104) | WebGL レンダリング確認 |
| POST sketch(offset=5) + extrude_cut(depth≈5) → 境界値確認 (#105) | Phase 8 以降の機能 |
| multi-step シーケンスの決定性検証 | |

## Non-Goals

- Playwright E2E テストの追加（CI 環境非対応）
- `playwright.realapi.config.ts` の正式化
- UI コンポーネントのテスト
- 単一ソリッドへのマージ（#104 の kernel limitation は既知）

## 実装対象

- Issue: #106
- 影響ファイル:
  - `crates/mycad-api/tests/e2e_api_scenarios.rs` — 新規 multi-step integration テスト

## 設計方針

- `temp_copy()` + `make_app()` + `app.oneshot()` パターン（既存 extrude_ui_acceptance.rs と同じ）
- 各ステップは独立 `make_app(path.clone())` + `app.oneshot()` で状態を .mycad ファイル経由で受け渡す
- 決定性: 同一入力シーケンスを 2 回実行して出力が同一であることを確認

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| S01_reload_no_duplicate_id | 正常系 | GET /api/v0/features → 既存 ID 取得 → POST sketch(同 ID)→ 409/重複エラー確認 | 200 で既存 ID 含む、同 ID POST は 422 |
| S02_extruded_face_ids_contain_cap | 正常系 | POST sketch + extrude → GET /api/v0/mesh → face_ids に cap/side あり | 少なくとも 1 つ f_cap_* or f_side_* |
| S03_extrude_creates_two_bodies | 正常系 | POST sketch + extrude(no fuse) → ボディ数 == 2 | bodies.len() == 2 |
| S04_degen_extrudecut_depth_boundary | 境界 | sketch at offset=5.0, extrude_cut depth=5.0 (== face dist) → 422 | StatusCode::UNPROCESSABLE_ENTITY |
| S05_determinism_multi_step | 決定性 | sketch + extrude シーケンスを 2 回 → vertex count 完全一致 | 同一出力 |

## 幾何的不変条件チェックリスト

- N/A（カーネル変更なし）
- N/A
- N/A
- N/A
