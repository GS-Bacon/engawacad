# test(e2e): Phase 7 完了条件の Extrude/ExtrudeCut を検証する E2E テストマトリクスを構築する

## 位置づけ
`type: foundation` — Phase 7 完了条件「描いたスケッチから Extrude / ExtrudeCut を実行できる」を自動 E2E で検証するためのテストマトリクス骨格を整備する。Phase 7 で CreateSketch が実装されたら、本テストにスケッチ起点のケースを追加する前提で設計する。

## 前提
- 実サーバ接続基盤 Issue（intent-e2e-infra）が closed

## 完了条件
- `web/tests/acceptance_extrude.spec.ts` に下記テストマトリクスが実装されていること
- `cargo xtask acceptance --workers 20` で全テストが通ること
- Phase 7 実装時に CreateSketch 起点のケースを追加できる拡張ポイントがコメントで明示されていること

## テストマトリクス（Extrude/ExtrudeCut × 境界値）

Phase 7 の Extrude/ExtrudeCut 完了条件を構成する操作パターンを網羅する。CreateSketch 起点のケースは Phase 7 実装後に別 PR で追加する。

| ケース | 入力値 | アサート |
|---|---|---|
| Extrude 正側面 正常値 | depth=2.0 | HTTP 200 / mesh 頂点数 > 初期値 / response に `type:extrude` feature |
| Extrude 正側面 最小値 | depth=0.01 | HTTP 200 / 頂点数 > 初期値 |
| Extrude 正側面 大値 | depth=10.0 | HTTP 200 / 頂点数 > 初期値 |
| Extrude **負側面** (#110 回帰) | depth=-2.0 | HTTP 200 / 頂点数 > 初期値 / `"error"` キーなし |
| Extrude **負側面** 最小値 (#110 回帰) | depth=-0.01 | HTTP 200 / `"error"` キーなし |
| ExtrudeCut 正常値 | depth=1.0 | HTTP 200 / 頂点数 < Extrude 後 / `type:extrude_cut` feature |
| ExtrudeCut 境界手前 | depth=face距離-0.01 | HTTP 200 / `"error"` キーなし |
| ExtrudeCut **境界値** (#111 回帰) | depth=face距離ちょうど | HTTP 200 / `"error"` キーなし（500 が出ないこと） |
| ExtrudeCut 境界超え | depth=face距離+0.1 | HTTP 200 / `"error"` キーなし |

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| Extrude/ExtrudeCut × 境界値マトリクス実装 | CreateSketch 起点の E2E（Phase 7 実装後に追加） |
| #110/#111 回帰テスト | Boolean/永続テスト（別 Issue） |
| 20 ワーカー並列実行設定 | タイル動画（別 Issue） |

## Non-Goals
- CreateSketch 起点のテストは本 Issue に含めない
- Boolean/多段/永続テストは別 Issue

## 設計方針
- `setupPageWithServer()` を使用し、各テストは独立した tmp ファイルでサーバを起動（並列競合なし）
- `simple_box.mycad` top face (z=1.0) を face 距離の基準とする
- `"error"` キーの有無でマニフォールド違反を検出

### 数値モデル
- tolerance: face 距離 = 1.0（simple_box top face の z 座標）、±1e-3 を "境界ギリギリ" とする
- 退化判定基準: HTTP 500 または `"error"` キー付きレスポンスを「manifold 違反」として検出
- ADR-004 準拠: tolerant モデル、kernel 内部 ε と整合した境界値を設定
