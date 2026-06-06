## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `@playwright/test` を web/ に追加 | CI サーバー自動起動（mycad-api の常駐は不要、API を mock） |
| playwright.config.ts の作成 | assembly.mycad（v0 未対応） |
| コンソールエラー検査（全 non-assembly example） | 逐次 CI ステップへの組み込み（cargo xtask ci の web ステップに追加） |
| スクリーンショット回帰テスト + golden 画像 commit | Three.js シーン直接検査（オプション機能） |

## Non-Goals
- mycad-api を CI で実際に起動してテストする（Playwright の `page.route()` で /api/* をモック）
- 100% pixel perfect 比較（maxDiffPixelRatio: 0.03 の許容値を設定）
- assembly.mycad のテスト（v0 未対応）

## 実装対象
- Issue: #61
- 新規ファイル:
  - `web/playwright.config.ts` — 設定ファイル
  - `web/tests/viewer.spec.ts` — テストスクリプト
  - `web/tests/viewer.spec.ts-snapshots/` — golden 画像（Playwright が自動生成）
- 変更ファイル:
  - `web/package.json` — `@playwright/test` を devDependencies に追加、scripts に `"playwright": "playwright test"` 追加
  - `crates/xtask/src/main.rs` — web CI ステップに `npx playwright test` を追加

### Playwright 設定（playwright.config.ts）

```ts
import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests',
  use: {
    launchOptions: { args: ['--use-gl=swiftshader'] },
    headless: true,
    baseURL: 'http://127.0.0.1:4173',  // vite preview
  },
  webServer: {
    command: 'npm run preview',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: !process.env.CI,
  },
});
```

### テスト構成（viewer.spec.ts）

対象 example（assembly 除く 16 件）:
`simple_box`, `cylinder`, `sphere`, `boolean_box_cut`, `boolean_box_fuse`,
`boolean_box_intersect`, `boolean_box_void`, `boolean_cut_cylinder_hole`,
`boolean_cut_sphere_dimple`, `boolean_fuse_box_cyl`, `boolean_intersect_box_cyl`,
`boolean_intersect_cyl_sphere`, `cylinder_offset`, `sphere_offset`,
`extruded_rect`, `two_bodies`

各テストで:
1. `page.route('/api/**', ...)` で API をモック → example の mesh データを返す
2. `page.on('console', msg => ...)` でコンソールエラーを収集
3. ページをロード
4. コンソールエラー数 = 0 をアサート
5. スクリーンショットを取りゴールデンと比較（maxDiffPixelRatio: 0.03）

API モックは Rust の `mycad-cli` を使って .mycad を事前 JSON 変換するか、
簡易な方法として `simple_box.mycad` を直接 parse して mesh を生成する CLI を使う。

**現実的な代替**: API を mock せず、`page.goto('?file=simple_box.mycad')` ではなく
Playwright で直接 `/api/render` を呼ぶ前に intercept して静的 JSON を返す。
静的 JSON は `cargo run -p mycad-cli -- simple_box.mycad` の出力をあらかじめ生成して
`web/tests/fixtures/` に配置する。

## 設計方針
- API モック: `page.route('/api/**', route => route.fulfill({ json: fixture }))` でサーバー不要
- fixture: `web/tests/fixtures/<example>.json` に mesh JSON を事前コミット
  - `cargo run -p mycad-cli -- render examples/<name>.mycad` で生成する
- WebGL: `--use-gl=swiftshader` で CI 対応
- flaky 対策: `maxDiffPixelRatio: 0.03`、タイムアウト 30 秒
- 決定性: golden は初回 `--update-snapshots` で生成してコミット

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_console_no_error_simple_box | 正常系 | simple_box: コンソールエラー 0 件 | assert === 0 |
| T02_screenshot_simple_box | 正常系 | simple_box: golden と比較 | maxDiffPixelRatio < 0.03 |
| T03_console_no_error_boolean | 正常系 | boolean 系 example のコンソールエラー 0 件 | assert === 0 |
| T04_boundary_degen_empty_response | 境界 | API が空配列を返す → "No bodies found" エラーのみでクラッシュなし | assert エラーメッセージ 1 件以下 |

## 幾何的不変条件チェックリスト
- N/A（ビューア・テスト実装のみ）
