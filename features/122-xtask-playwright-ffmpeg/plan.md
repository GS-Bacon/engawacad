## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| cargo xtask acceptance --record オプション追加 | ライブ20分割画面（リアルタイム表示） |
| Playwright video: 'on' の設定切り替え（env var） | CI への動画アップロード |
| FFmpeg xstack フィルタによるタイル合成 | 動画トリミング・編集 |
| FFmpeg 未インストール時の graceful skip | Windows/macOS 対応 |
| 動的グリッド（4列固定、行数 = ceil(N/4)） | 固定 4×5 強制パッディング |

## Non-Goals
- リアルタイムのライブ分割画面表示はしない
- Windows/macOS での動作は保証しない（Linux 環境想定）
- CI への動画アップロードはしない
- `cargo xtask ci` の変更なし（acceptance --record は補助ツール）

## 実装対象
<!-- Issue: #122 -->
影響ファイル:
- `crates/xtask/src/main.rs`: `acceptance()` に `--record` フラグ追加 + `tile_videos()` 関数追加
- `web/playwright.config.ts`: `PLAYWRIGHT_VIDEO === '1'` 時に `video: 'on'` を設定

## 設計方針

### --record フラグ
- `acceptance()` で `--record` フラグを検出
- npx playwright test 呼び出し時に `.env("PLAYWRIGHT_VIDEO", "1")` を追加
- Playwright 完了後に `tile_videos()` を呼んで FFmpeg タイル合成

### Playwright video 設定
playwright.config.ts の `use:` ブロックに追加:
```typescript
video: process.env.PLAYWRIGHT_VIDEO === "1" ? "on" : "off",
```

### FFmpeg タイル合成
- `web/test-results/` 以下から `*.webm` を再帰探索
- N=0: `WARNING: --record が指定されましたが動画ファイルが見つかりませんでした` を出力し続行
- ffmpeg 未インストール: `WARNING: ffmpeg が見つかりません — タイル合成をスキップ` を出力し SUCCESS
- N=1: `ffmpeg -y -i v0.webm output.mp4`（xstack 不要）
- N≥2: xstack フィルタを動的構築

### xstack レイアウト計算
列数 = 4（固定）、行数 = ceil(N/4)。全動画が同解像度と仮定:
- position[i]: col = i%4, row = i/4
  - x部: col==0 → "0", col==1 → "w0", col==2 → "w0+w1", col==3 → "w0+w1+w2"（同解像度なので w0=w1=w2=w3）
  - y部: row==0 → "0", row≥1 → "h0*(row)"
- filter = `xstack=inputs=N:layout=<positions joined by |>:fill=black`

出力先: `web/test-results/acceptance-tiled.mp4`

### before / after（main.rs: acceptance() の npx playwright 呼び出し部分）
Before:
```rust
let status = Command::new("npx")
    .args(["playwright", "test", "--workers", &workers])
    .current_dir(&pw_web_dir)
    .status()
    .expect("failed to execute playwright test");
if !status.success() {
    eprintln!("FAILED: Playwright E2E tests");
    return ExitCode::FAILURE;
}
```

After:
```rust
let mut cmd = Command::new("npx");
cmd.args(["playwright", "test", "--workers", &workers])
   .current_dir(&pw_web_dir);
if record {
    cmd.env("PLAYWRIGHT_VIDEO", "1");
}
let status = cmd.status().expect("failed to execute playwright test");
if !status.success() {
    eprintln!("FAILED: Playwright E2E tests");
    return ExitCode::FAILURE;
}
if record {
    tile_videos(&pw_web_dir);
}
```

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_record_flag_parsed | 正常系 | acceptance() が --record を認識 | record = true のコードパス確認（Rust コンパイル通過で検証） |
| T02_no_record_default | 正常系 | --record なしのデフォルト | PLAYWRIGHT_VIDEO 未設定で通常 headless |
| T03_degen_ffmpeg_missing | 境界/退化 | ffmpeg 未インストール時 | WARNING 表示して SUCCESS（tile_videos は NG を返さない） |
| T04_degen_no_videos | 境界/退化 | .webm が 0 件の場合 | WARNING 表示して続行 |

## 幾何的不変条件チェックリスト
- N/A（xtask / Playwright 設定変更のみ、幾何カーネル無関係）
- N/A
- N/A
- N/A
