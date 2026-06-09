# Test Spec — #122 xtask-playwright-ffmpeg

## 実装済みテスト

| ID | 実装 | 確認方法 |
|----|------|---------|
| T01_record_flag_parsed | ✅ `--record` フラグ検出 | cargo build 通過 + コードレビュー |
| T02_no_record_default | ✅ デフォルト挙動不変 | ビルド通過・既存テストへの影響なし |
| T03_degen_ffmpeg_missing | ✅ `tile_videos()` が `which("ffmpeg")` で guard | コードレビュー |
| T04_degen_no_videos | ✅ `collect_webm_files()` が空配列 → WARNING + continue | コードレビュー |

## 設計ノート

- `tile_videos()` は失敗しても ExitCode::FAILURE を返さない（補助ツール）
- `build_xstack_filter()` は 4 列固定、行数 = ceil(N/4) で動的計算
- xstack layout は `col0=0|col1=w0|col2=w0+w1|col3=w0+w1+w2` × row で構成
- Playwright `video: 'off'` がデフォルト（env 未設定時）

## Phase 7 拡張ポイント
- `--tile-cols N` 引数で列数を変更可能にする（現在は 4 固定）
- `--video-dir` 引数で収集ディレクトリを変更する
