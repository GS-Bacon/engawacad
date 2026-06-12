# Test Spec for Issue #145

## サマリ

Issue #145 (Playwright T03 console-no-error 14 件 ERR_CONNECTION_REFUSED) の core 実装は完了。
plan の T01〜T05 の期待値はすべて実装 assertion と一致しており、期待値乖離なし。

## 不足テスト (plan 計画分)

なし。plan に列挙した T01〜T05 はすべて実装済みかつ pass している:

| ID | 実装場所 | 結果 |
|---|---|---|
| T01_startup_log | `crates/mycad-api/tests/startup_log_acceptance.rs:t01_startup_log` | ✅ pass |
| T02_release_bin_runs | `crates/mycad-api/tests/startup_log_acceptance.rs:t02_release_bin_runs` | ✅ pass |
| T03_boundary_invalid_path | `crates/mycad-api/tests/startup_log_acceptance.rs:t03_boundary_invalid_path` | ✅ pass |
| T04_degen_no_args | `crates/mycad-api/tests/startup_log_acceptance.rs:t04_degen_no_args` | ✅ pass |
| T05_e2e_playwright_green | `cargo xtask ci` 内の `npx playwright test` で 52 passed (T03 14 件 cover) | ✅ pass |

## 期待値乖離チェック

| T ID | plan の期待値 | 実装 assertion | 乖離 |
|---|---|---|---|
| T01_startup_log | stderr に "listening on 127.0.0.1:7878" | `assert!(buf.contains("listening on 127.0.0.1:7878"))` | なし |
| T02_release_bin_runs | release バイナリで起動 + port 7878 listen | `assert!(found && port_ready)` | なし |
| T03_boundary_invalid_path | 不正パスで `expect` panic 終了 | `assert!(!status.success())` | なし |
| T04_degen_no_args | 引数なしで `expect` panic 終了 | `assert!(!status.success())` | なし |
| T05_e2e_playwright_green | T03 console-no-error 14 件 pass | `cargo xtask ci` 内 Playwright 52 passed | なし |

## 実装差分から追加すべきテスト

なし。差分 4 ファイル (main.rs / playwright.config.ts / helpers.ts / viewer.spec.ts) と新規 1 ファイル (startup_log_acceptance.rs) は plan の方針通りで、計画外の分岐は発生していない。

## エッジケース・退化入力

- T03_boundary_invalid_path (`/nonexistent/path.mycad`) と T04_degen_no_args (引数なし) で `expect` panic 経路を cover 済み。
- `mycad-api` の追加引数バリエーション (例: 巨大ファイル / シンボリックリンク先消失) は本 Issue のスコープ外 (Out-of-Scope: webServer 起動方式の再設計)。

## 数値境界

N/A (infrastructure 修正、数値モデルなし)。

## 決定性

N/A (infrastructure 修正、`IdGenerator` には触れていない)。既存テスト群が決定性を引き続き保証。

## 類似ケース（未カバー）

`web/tests/` 配下に **viewer.spec.ts のローカル `setupPageWithFixture` と類似構造** の test が他にも存在する:

| ファイル | 該当パターン | console.error 厳密検査 | 本 Issue で対応 |
|---|---|---|---|
| `web/tests/extrude_panel.spec.ts:25` | inline `page.route("/api/v0/mesh", ...)` | なし | 不要 (現状 pass) |
| `web/tests/face_picking.spec.ts:20` | inline `page.route("/api/v0/mesh", ...)` | なし | 不要 (現状 pass) |
| `web/tests/promo_chain.spec.ts:34` | ローカル `setupPage()` 関数 | なし | 不要 (現状 pass) |

**判定:** これらは console.error を `toHaveLength(0)` で厳密検査していないため、port 7879 接続失敗が起きても test fail しない。よって本 Issue で予防的 mock 追加は **行わない** (Out-of-Scope: 「他の Playwright テストの追加 / E2E マトリクス拡張」)。

**観察 / 後続作業候補:** 将来 console-no-error 系の assertion をこれらの spec に追加するケースに備え、`helpers.ts` から export した `mockLoggerSidecar` を呼び出すよう各 spec を refactor する preventive fix を別 Issue として起票することは検討に値する。ただし本 Issue では扱わない。

## 追加 GLM 実装の要否

**不要。** plan の T01〜T05 はすべて pass しており、類似ケースの予防的 fix も Out-of-Scope。

STEP 6.6 (GLM test 実装) はスキップして STEP 7 (GLM final review) へ進む。
