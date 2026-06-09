## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| Extrude/ExtrudeCut × 境界値テストマトリクス（9ケース） | CreateSketch 起点の E2E |
| #110/#111 回帰テスト（負 depth / 境界値 ExtrudeCut） | Boolean/永続テスト |
| `cargo xtask acceptance --workers N` フラグ追加 | タイル動画（別 Issue） |
| Phase 7 拡張ポイントコメント | CI パイプライン組み込み |

## Non-Goals
- CreateSketch 起点のテストは本 Issue に含めない（Phase 7 実装後に追加）
- Boolean/多段/永続テストは別 Issue
- CI パイプライン組み込みはしない

## 実装対象
<!-- Issue: #123 -->
影響ファイル:
- `web/tests/acceptance_extrude.spec.ts`: 新規作成 — 9ケース × テストマトリクス
- `crates/xtask/src/main.rs`: `acceptance()` に `--workers N` フラグ追加

## 設計方針
### テスト実行方針
- Playwright API テスト（`request.post()`）でブラウザ不要の HTTP テスト
- `test.describe.configure({ mode: 'serial' })` で直列実行（共有サーバの状態競合を防ぐ）
- 各テストの冒頭で `request.get('/api/v0/mesh')` して現在のベースライン頂点数を取得
- POST レスポンスの頂点数を baseline と比較（累積状態でも相対比較が成立）

### simple_box の幾何
- width=10, height=20, depth=30、原点中心: z∈[-15,15]
- XY 平面 (z=0) に sketch を作成 → extrude は z 正方向 or 負方向
- ExtrudeCut: sketch[-2,2]×[-4,4], target=box_1, depth=3 → void カット（内部に穴）
- "face距離" = 15 (XY 平面から top face z=15 までの距離)

### cargo xtask acceptance --workers
Before:
```rust
fn acceptance() -> ExitCode {
    ...
    let status = Command::new("npx")
        .args(["playwright", "test"])
        ...
```

After:
```rust
fn acceptance() -> ExitCode {
    let workers = args.iter()
        .find(|a| a.starts_with("--workers"))
        .and_then(|a| a.split('=').nth(1).or_else(|| ...))
        .unwrap_or("1");
    ...
    let status = Command::new("npx")
        .args(["playwright", "test", "--workers", workers])
        ...
```

### 数値モデル (Phase 4 / 6+ 必須)
- tolerance: face 距離 = 15.0（simple_box XY 平面から top face z=+15 まで）
- 退化判定基準: HTTP 500 または JSON レスポンスに `"error"` キーが含まれる
- ADR-004 準拠: tolerant モデル（kernel 内部 ε と整合）
- 境界値 ε: ±1e-3（±0.001）を "ちょうど" として使用

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_extrude_normal | 正常系 | Extrude 正側面 depth=2.0 | HTTP 200, 頂点数 > baseline, type:extrude |
| T02_extrude_min | 正常系 | Extrude 正側面 depth=0.01 | HTTP 200, 頂点数 > baseline |
| T03_extrude_large | 正常系 | Extrude 正側面 depth=10.0 | HTTP 200, 頂点数 > baseline |
| T04_degen_extrude_neg_face | 退化/境界 (#110 回帰) | Extrude 負側面 depth=-2.0 | HTTP 200, error キーなし |
| T05_degen_extrude_neg_min | 退化/境界 (#110 回帰) | Extrude 負側面 depth=-0.01 | HTTP 200, error キーなし |
| T06_extrude_cut_normal | 正常系 | ExtrudeCut depth=1.0 target=box_1 | HTTP 200, type:extrude_cut |
| T07_extrude_cut_near_boundary | 正常系 | ExtrudeCut depth=face距離-0.01=14.999 | HTTP 200, error キーなし |
| T08_degen_extrude_cut_at_boundary | 退化/境界 (#111 回帰) | ExtrudeCut depth=face距離=15.0 | HTTP 200, error キーなし |
| T09_extrude_cut_beyond_boundary | 正常系 | ExtrudeCut depth=face距離+0.1=15.1 | HTTP 200, error キーなし |

## 幾何的不変条件チェックリスト
- N/A（Playwright API テスト、カーネル不変条件は Rust テスト層で検証済み）
- N/A
- N/A
- N/A
