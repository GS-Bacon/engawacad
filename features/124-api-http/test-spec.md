# Test Spec — #124 api-http

## 不足テスト（plan 計画分）

| ID | 実装状況 | 備考 |
|----|---------|------|
| T01 決定性 | ✅ 実装済み (`t01_determinism`) | 9種のペイロードで status/body が2アプリ間一致 |
| T02 正常系ファズ | ✅ 実装済み (`t02_fuzz_no_http_500`) | FUZZ_DURATION_SECS 秒間ランダム投入、500 == 0 を検証 |
| T03_degen_special_values | ✅ 実装済み (`t03_degen_special_float_values`) | 0, 1e15, -1e15, 1e-15, MIN の各depth で 500 でないこと |
| T04_degen_unknown_type | ✅ 実装済み (`t04_degen_unknown_type`) | 未知type で 4xx かつ非500 |
| T05_degen_null_body | ✅ 実装済み (`t05_degen_null_body`) | null/配列/数値/文字列ボディで 非500 |

## 実装差分から追加すべきテスト

特になし。T01-T05 で plan 記載の全ファジング対象を網羅している。

## エッジケース・退化入力

- NaN/Infinity は `serde_json::Number::from_f64` が None を返すため null として送信される設計（T03 に反映済み）
- 極大値 `f64::MIN`（約 -1.8e308）は JSON 数値として有効だが kernel が棄却する（T03）
- 空文字 type は有効な JSON だが Feature デシリアライズ失敗で 422（T04）

## 数値境界

| 値 | 期待動作 |
|----|---------|
| 0.0 | 400 または 422（ゼロサイズは kernel が拒否） |
| 1e15 | 400/422 または正常（kernel が棄却） |
| -1e15 | 400/422 |
| 1e-15 | 400/422 または正常 |
| f64::MAX | 400/422 |
| NaN | null 扱い → 400/422 |
| Infinity | null 扱い → 400/422 |

## 決定性

T01 で確認済み。同一ペイロード → 同一 status + body（2回独立実行）。
