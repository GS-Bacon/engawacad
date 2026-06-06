# Test Spec — Issue #78: Phase 5 close gate

## 不足テスト（plan 計画分）
T01〜T04 は `close_gate_acceptance.rs` に実装済み（GLM コア実装で全て実装）。

T01 export exit 0: mycad export assembly.mycad が exit 0 で成功
T02 STL ≥ 1KB: 生成 STL ファイルが 1024 bytes 以上
T03 HTTP 200: /api/v0/mesh が 200 を返し bodies が空でない
T04_boundary missing file: 存在しないファイルを export すると非ゼロ exit

## 実装差分から追加すべきテスト
- 追加テスト不要。T01〜T04 が plan の要件を全てカバーしている。

## エッジケース・退化入力
- T04_boundary: 存在しないファイルパスを渡した場合のエラー伝播を検証済み

## 数値境界
- T02: STL サイズ ≥ 1024 bytes (exact)

## 決定性
- T01/T02: CLI export の出力は決定的（実行毎に同じ STL を生成）
