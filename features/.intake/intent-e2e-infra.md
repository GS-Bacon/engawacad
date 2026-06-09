# test(e2e): Playwright に実 API サーバを接続し cargo xtask acceptance を追加する

## 位置づけ
`type: foundation` — Phase 7 の完了条件（ブラウザ上で正準平面スケッチ → Extrude/ExtrudeCut → `.mycad` 記録）を自動 E2E テストで検証するための基盤 Issue。この Issue 単体では Phase 7 機能は実装しないが、Phase 7 完了判定の自動テストをこの基盤なしに書けない。

## 背景・動機
現在の Playwright テストは `/api/v0/mesh` と `/api/v0/features` を fixture でモックしており、実カーネルを経由しない。そのため「ブラウザ操作 → 実 API → カーネル → レンダリング」の全スタック疎通が自動テストで保証されていない。Phase 6 のバグ（#103-#111 等）の多くは人間が実際に触って発見しており、Phase 7 着手前にこの gap を埋める。

## 完了条件
- `playwright.config.ts` の `webServer` に `cargo run -p mycad-api -- <file>` を設定し、テスト実行前に実サーバが自動起動する
- 実サーバに接続するヘルパー `setupPageWithServer()` を追加する（既存モック版 `setupPageWithFixture()` は互換性のため残す）
- `cargo xtask acceptance` タスクを追加する（`cargo xtask ci` とは独立して実行できる）
- 既存の Playwright テスト（T01/T02/E01-E04 等）が実サーバに対して通ること

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| playwright.config.ts への webServer 設定追加 | 新規テストケースの実装（別 Issue） |
| 実サーバ接続ヘルパー関数の追加 | API ファジング（別 Issue） |
| cargo xtask acceptance タスク追加 | タイル動画の生成（別 Issue） |
| 既存テストの実サーバ対応確認 | CI/GitHub Actions への組み込み |

## Non-Goals
- 新しいテストシナリオの実装はしない
- CI パイプラインへの組み込みはしない（ローカル実行のみ）

## 設計方針
- `webServer.command`: `cargo build -p mycad-api` 後に実行バイナリを起動
- tmpdir に `simple_box.mycad` をコピーしてサーバに渡す
- `webServer.port`: 3001（開発時 vite の 3000 と競合しないよう変更）
- 既存の `setupPageWithFixture()` ヘルパーはそのまま残し、`setupPageWithServer()` を追加する
