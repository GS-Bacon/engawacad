# test(api): HTTP エンドポイントへのファジングで未知パニック・クラッシュを自動探索する

## 位置づけ
`type: foundation` — Phase 7 の完了条件（ブラウザ操作 → API → カーネル）の品質保証として、API 入力の想定外パターンによるパニック・クラッシュを事前に排除する。Phase 7 の CreateSketch 等の新エンドポイントも本ハーネスを拡張して検証する前提で設計する。

## 背景・動機
Playwright テストは「事前に設計したシナリオ」しか叩けないため、未知のエッジケースは発見できない。API ファジング（ランダム入力の大量投入）を導入することで、設計外の入力パターンによるパニック・500 エラーを自動探索する。

## 完了条件
- `/api/v0/features` POST エンドポイントにランダム JSON を大量投入するファズハーネスが `crates/mycad-api/tests/fuzz_features.rs` に実装されていること
- パニックおよび HTTP 500 レスポンスを検出してレポートを出力すること
- `cargo xtask acceptance --fuzz` で実行でき、デフォルト（`cargo xtask acceptance` のみ）では走らないこと

## ファジング対象
- `type` フィールド: 未知の Feature 型、null、空文字
- `depth`: NaN、Infinity、-Infinity、0、1e15、-1e15、1e-15
- `face_id`: 存在しない ID、不正形式、空文字、null
- ランダムな JSON 構造（余分なキー、ネスト、配列）

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| /api/v0/features POST のファジング | LLVM ベースの cargo-fuzz 導入 |
| パニック/500 の検出とレポート | B-rep 意味論的正しさの検証 |
| cargo xtask acceptance --fuzz への組み込み | 発見したバグの修正（別 Issue） |

## Non-Goals
- LLVM ベースの cargo-fuzz は導入しない（Axum test client で十分）
- ファジングで発見したバグの修正は別 Issue

## 設計方針
- Axum test client（tower ServiceExt）でサーバを立ててランダム JSON を投入
- `rand` crate でランダム生成（workspace 依存に追加）
- 実行時間: デフォルト 30 秒（`FUZZ_DURATION_SECS` 環境変数で変更可）
- 検出: HTTP 500 レスポンスをエラーとして記録し、最終的にパニックなし・500 件数 0 をアサート

### 数値モデル
- tolerance: NaN/Inf は入力として拒否（HTTP 400 を期待）、極大値（1e15）は 400 または正常処理（kernel が棄却）どちらも許容
- 退化判定基準: HTTP 500 またはプロセスパニックを "未処理の退化" として検出
- ADR-004 準拠: kernel 側が ε チェックで拒否するケースは 400 を正常レスポンスとみなす
