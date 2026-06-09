# feat(xtask): Playwright 録画 + FFmpeg タイル合成で並列テスト実行を可視化する（補助ツール）

## 位置づけ
`type: foundation`（フェーズ完了ゲート外の補助ツール / milestone なし）— 特定 Phase の完了条件には紐づかない。受け入れテスト実行時の操作内容を動画で目視確認できる補助インフラとして提供する。

## 背景・動機
20 ワーカーで並列実行される Playwright テストが「本当に正しい操作を自動化しているか」を動画で確認したい。テスト設計の妥当性レビューとデバッグを支援する補助ツール。

## 前提
- Extrude/ExtrudeCut 境界値マトリクス Issue が closed であること

## 完了条件
- `cargo xtask acceptance --record` を実行すると Playwright が `video: 'on'` モードで走り、各 worker の録画が `test-results/videos/` に保存されること
- FFmpeg で全動画を 4×5 グリッドにタイル合成した `test-results/acceptance-tiled.mp4` が生成されること
- `--record` なし（デフォルト）では録画なし・headless で通常通り実行されること
- FFmpeg が未インストールの場合はスキップしてメッセージを出力し、xtask は正常終了すること

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| cargo xtask acceptance --record オプション追加 | ライブ20分割画面（リアルタイム表示） |
| Playwright video: 'on' の設定切り替え | CI への動画アップロード |
| FFmpeg xstack フィルタによるタイル合成 | 動画トリミング・編集 |
| FFmpeg 未インストール時の graceful skip | Windows/macOS 対応 |

## Non-Goals
- 特定 Phase の完了条件を検証する Issue ではない（補助ツール）
- リアルタイムのライブ分割画面表示はしない
- Windows/macOS での動作は保証しない（Linux 環境想定）

## 設計方針
- `--record` フラグで環境変数 `PLAYWRIGHT_VIDEO=1` を設定
- playwright.config.ts 側で `process.env.PLAYWRIGHT_VIDEO === '1'` なら `use: { video: 'on' }` に切り替え
- FFmpeg タイル合成: `xstack` フィルタで 4×5 グリッド（20 動画）
- `which ffmpeg` で存在確認し、なければ警告のみ
