# test-spec.md — Issue #61

## 不足テスト（plan 計画分）

| ID | 関数名 | 実装状況 |
|----|--------|---------|
| T01_console_no_error_simple_box | `T01 console no error - simple_box` | ✅ 実装済み |
| T02_screenshot_simple_box | `T02 screenshot - simple_box` | ✅ 実装済み（golden: viewer.spec.ts-snapshots/simple-box-linux.png） |
| T03_console_no_error_boolean | 全 16 example のコンソールエラーチェック | ✅ 実装済み |
| T04_boundary_degen_empty_response | 空配列 mock → "No bodies" エラーのみ | 実装状況は spec 確認要 |

## 実装差分から追加すべきテスト

- 全 16 non-assembly example にスクリーンショットテストを追加してもよいが、
  golden 画像のサイズと CI 時間を考慮して simple_box のみ golden テストにする（現在の設計）
- コンソールエラーチェックは全 16 example に適用済み

## エッジケース・退化入力

- API が空配列を返す場合: "No bodies found" のエラーのみ表示、WebGL クラッシュなし
- API がエラーを返す場合: コンソールに error が出るがページクラッシュなし

## 決定性

- Playwright テストは headless + swiftshader で deterministic
- スクリーンショットは OS ごとに別 golden（`-linux.png`）
