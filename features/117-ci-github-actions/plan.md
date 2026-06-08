## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `.github/workflows/ci.yml` 作成 | matrix ビルド（複数 OS / Rust バージョン） |
| push / pull_request トリガー | デプロイ / リリース自動化 |
| Rust toolchain キャッシュ（actions/cache） | GitHub Packages への publish |
| Node.js 20 セットアップ | `test-results/` アーティファクト保存 |
| Playwright chromium インストール | — |
| `cargo xtask ci` 実行 | — |

## Non-Goals

- Linux 以外の OS での CI 実行（Playwright ベースライン画像が linux 依存のため）
- Rust nightly / beta ビルド
- ベンチマーク実行

## 実装対象

新規ファイル: `.github/workflows/ci.yml`

```yaml
name: CI
on:
  push:
    branches: ["**"]
  pull_request:
jobs:
  ci:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy,rustfmt
      - uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: ${{ runner.os }}-cargo-
      - uses: actions/setup-node@v4
        with:
          node-version: "20"
          cache: "npm"
          cache-dependency-path: web/package-lock.json
      - name: Install Playwright browsers
        run: npx playwright install --with-deps chromium
        working-directory: web
      - name: Run cargo xtask ci
        run: cargo xtask ci
```

## 設計方針

- Rust は `rust-toolchain.toml` の `stable` を使用。`dtolnay/rust-toolchain@stable` が `.toml` を自動読み込む
- Node.js キャッシュは `web/package-lock.json` をキーに `npm` キャッシュを利用
- Playwright は CI 環境で `--with-deps` が必要（システムライブラリ込みインストール）
- 決定性・B-rep・数値モデルセクションは N/A（CI 設定ファイルのため）

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T01 | 正常系 | workflow YAML が valid YAML として parse できる | python3 yaml.safe_load で例外なし |
| T01_boundary_trigger | 境界 | push / pull_request 両トリガーが定義されている | grep で確認 |

## 幾何的不変条件チェックリスト

N/A（CI 設定ファイルのため幾何処理なし）
