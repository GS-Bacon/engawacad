## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `playwright.config.ts` を複数 `webServer` 構成に変更 | E2E シナリオテスト追加（#116） |
| Rust API サーバー (`mycad-api`) の自動起動・終了設定 | Playwright retries / matrix 化 |
| API サーバーの startup timeout を 120s に設定 | fixture ファイルの追加 |

## Non-Goals

- E2E テストの追加（#116 で対応）
- 認証・CORS 設定の変更
- API サーバーのポート変更

## 実装対象

`web/playwright.config.ts` の `webServer` を配列に変更。

**Before**:
```ts
webServer: {
  command: "npm run preview",
  url: "http://127.0.0.1:4173",
  reuseExistingServer: !process.env.CI,
},
```

**After**:
```ts
webServer: [
  {
    command: "npm run preview",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
  },
  {
    command: "cargo run -p mycad-api -- ../examples/simple_box.mycad",
    url: "http://127.0.0.1:7878/api/v0/mesh",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    cwd: "..",
  },
],
```

- `cwd: ".."` で `web/` から一つ上のリポジトリルートを指定し、`../examples/` へのパスが通るようにする
- `timeout: 120_000`: CI での Rust コンパイル時間（初回）に対応。`cargo xtask ci` が先に `cargo build` するため実際の起動は速い
- `reuseExistingServer: !process.env.CI`: ローカルでは既存サーバーを再利用、CI では必ず新規起動

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T01 | 正常系 | `npm run preview` + API サーバーが起動し T01 が pass | PASS |
| T01_boundary_degen | 境界 | API サーバーが起動しない場合 url-check が失敗してテストが skip/fail | timeout error |

## 幾何的不変条件チェックリスト

N/A（Playwright 設定のみ）
