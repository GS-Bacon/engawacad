# Test Spec — #237 ADR-015 draft

ADR は docs (純粋テキスト) なので、acceptance test は ADR file の **存在 + 必須セクション + Decision Matrix の Option 記載 + Status flag** を smoke check する。

## 不足テスト (plan 計画分)

| ID | 場所 | 状態 |
|----|------|------|
| T01 | `tests/adr_015_phase9_doc_acceptance.rs` | 未実装 (skeleton 状態) |
| T_DOC_required_sections | 同上 | 未実装 |
| T_DOC_decision_matrix_has_options | 同上 | 未実装 |
| T_DEG_file_missing | 同上 | 未実装 (test 名はネガティブだが、本テストは file が存在することを assert) |
| T_BOUNDARY_status_accepted | 同上 | 未実装 |

## 実装差分から追加すべきテスト

- (なし — ADR は単独 file。実装差分が plan 計画と一致)

## エッジケース・退化入力

- ADR file が存在しない場合 → integration test が `file not found` で panic → 検出される (T_DEG_file_missing)

## 数値境界

N/A

## 決定性

T01 で 2 回 read 結果が一致することを assert

## 期待値乖離

なし
