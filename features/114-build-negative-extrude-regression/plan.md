## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `feature_dispatcher.rs` に negative extrude リグレッションテスト追加 | #110 の実際の修正 |
| `#[ignore = "known bug: #110"]` でマーク | UI 側の変更 |
| 正側押し出し（offset > 0）が正しく動作することを確認するテストも追加 | proptest |

## Non-Goals

- #110 負側押し出しバグの修正（#110 で対応）
- UI 側の変更
- E2E テスト

## 実装対象

`crates/mycad-build/tests/feature_dispatcher.rs` の末尾に追加:
- `t_reg_negative_extrude`: 負側面（offset=-5）から depth=3 で押し出し → 重心 x < -5 を期待
  - `#[ignore = "known bug: #110"]` でマーク
- `t_positive_extrude_centroid`: 正側面（offset=+5）から depth=3 で押し出し → 重心 x > 5 を確認

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T01 | 正常系 | 正側（offset=+5）押し出し → 重心 x > 5 | pass |
| T01_degen_boundary | 境界 | offset=0（xy/xz中心面）から押し出し | 既存テストで確認済み |
| T02 | リグレッション | 負側（offset=-5）押し出し → 重心 x < -5 | `#[ignore]` (#110 修正後に有効化) |

## 幾何的不変条件チェックリスト

N/A（テスト追加のみ）
