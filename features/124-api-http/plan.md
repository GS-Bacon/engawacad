## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `/api/v0/features` POST エンドポイントへのランダム JSON 大量投入ファズハーネス | LLVM ベースの cargo-fuzz 導入 |
| パニック・HTTP 500 の検出とレポート出力 | B-rep 意味論的正しさの検証 |
| `cargo xtask acceptance --fuzz` サブコマンド追加 | ファジングで発見したバグの修正（別 Issue）|
| `rand` crate を workspace.dependencies に追加 | `/api/v0/mesh` GET など他エンドポイントのファジング |

## Non-Goals
- LLVM ベースの cargo-fuzz は導入しない（Axum test client で十分）
- ファジングで発見したバグの修正は別 Issue として起票する
- B-rep 意味論的正しさ（返却メッシュの頂点数など）は検証しない

## 実装対象
<!-- Issue: #124 -->
影響クレート/ファイル:
- `Cargo.toml` (workspace): `rand = "0.9"` を追加
- `crates/mycad-api/Cargo.toml`: dev-dependencies に `rand = { workspace = true }` を追加
- `crates/mycad-api/tests/fuzz_features.rs`: 新規作成（ファズハーネス本体）
- `crates/xtask/src/main.rs`: `acceptance` サブコマンドと `--fuzz` フラグを追加

新規追加のみ（既存関数の修正なし）。

## 設計方針
- **Axum test client**: `tower::ServiceExt::oneshot` で HTTP サーバ不要のテスト（既存 post_features_acceptance.rs と同パターン）
- **rand による生成**: `rand` 0.9 + `rand::thread_rng()` でランダム JSON 値を生成
- **実行時間制御**: デフォルト 30 秒 (`FUZZ_DURATION_SECS` 環境変数で変更可)
- **検出条件**: HTTP 500 レスポンスをエラーとして記録し、最終アサートで 500 件数 == 0 を確認
- **ignore 属性**: 通常の `cargo test --workspace` でスキップするため `#[ignore = "fuzz: run with cargo xtask acceptance --fuzz"]` を付与
- **xtask acceptance**: `cargo xtask acceptance` はデフォルト受け入れテスト群（`--test post_features_acceptance` 等）を実行。`--fuzz` を付けると `fuzz_features -- --include-ignored` を追加実行
- **決定性要件**: ファズハーネス自体は非決定的だが、T01_determinism テストで同一入力→同一レスポンスを検証する
- **エラーハンドリング**: thiserror 不要（テストクレートのみ）
- **derive 規約**: テストクレートのみのため最小限

### ファジング対象 (Issue 仕様より)
- `type` フィールド: 未知の Feature 型, null, 空文字
- `depth`: NaN, Infinity, -Infinity, 0, 1e15, -1e15, 1e-15
- `face_id`: 存在しない ID, 不正形式, 空文字, null
- ランダムな JSON 構造（余分なキー, ネスト, 配列）

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一の無効 JSON ペイロードを2つの独立アプリに送り、レスポンスコードと形式が一致 | assert_eq!(status1, status2) |
| T02 | 正常系ファズ | 30秒間ランダム JSON を大量投入し HTTP 500 が 0 件 | assert_eq!(fuzz_errors, 0) |
| T03_degen_special_values | 退化/境界 | NaN, Infinity, -Infinity を含む depth フィールドで 400 または正常処理（500 でない） | status != 500 |
| T04_degen_unknown_type | 退化/境界 | 未知の `type` 文字列で 422 (Unprocessable) が返ること | status == 422 |
| T05_degen_null_body | 退化/境界 | body が null の JSON を送り 400 または 422 が返ること | status != 500 |

## 幾何的不変条件チェックリスト
- N/A（ファズハーネスはトポロジー不変条件を検証しない）
- N/A
- N/A
- N/A
