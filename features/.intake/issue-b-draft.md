# tessellation/mod.rs: 死に分岐削除 + 共有境界の直接比較テスト追加 (ADR-009 関連)

## 位置付け

**`type: foundation`** (ADR-002 §「ラベル運用: 2 軸ラベル制」) — Phase 7 milestone 内に置くが、**Phase 7 完了判定の対象外**。本 Issue は Phase 7 スケッチ描画の完了条件 (xy/xz/yz スケッチ → Extrude/ExtrudeCut → `CreateSketch` Feature 記録) には直接寄与せず、ADR-009 決定の即時実施可能分を消化する横断的保守作業として扱う。

## 背景

ADR-009 で「Boolean 交線円を周期エッジで保持し離散化をテッセレーション層に遅延する」(案 A) を決定した。実装本体 (Implementation Outline Phase 1 + 2) は Phase 4 再訪タイミングで起票するが、Implementation Outline Phase 3 の**ADR 待ち不要な即時クリーンアップ 2 件**を本 Issue で実施する。

実施動機: 死に分岐 (#131 試行錯誤跡) と共有境界比較テスト不在 (codex-review #129-F02 提案) は ADR-009 実装着手前でも独立に解消でき、放置すると `/3ai` の GLM ワーカーが当該分岐を意味のあるロジックと誤認するリスクがある。

## 作業内容

### 1. tessellation/mod.rs:649-680 の死に分岐削除

`adj_is_sphere` 分岐 (`if adj_is_sphere { arcs_per_rev } else { arcs_per_rev }`) は両腕とも同一値 `arcs_per_rev` を返す死に分岐。#131 試行錯誤跡として残存している。

- `adj_is_sphere` 計算と if/else 構造を削除し、`n_u = arcs_per_rev` に簡略化する
- 周辺の長文コメント (`adj_is_sphere is computed to enable future differentiation...`) を簡潔化し、「将来 ADR-009 実装で差分化されうる」旨を 1〜2 行で残す

### 2. 共有境界の直接比較テスト追加

codex-review #129-F02 の提案。Boolean 結果の隣接面間で「両側がエッジ境界上に同一サンプル列を生成しているか」を直接比較する acceptance テストを追加する:

- 対象: 既存 Boolean fixture (例: cyl×sphere の交線を含むモデル) を tessellate し、隣接面の共有境界頂点を直接比較する
- 失敗時のメッセージで「どのエッジ・どのインデックスで何 mm ずれているか」を出すこと
- ADR-009 実装 (Phase 1+2) が入ったらこのテストは構造的に green になる。それまでは「現状の規約依存が動いていること」のセーフティネットとして機能する

配置: `crates/mycad-kernel/tests/shared_boundary_acceptance.rs` 新規作成。

## 完了条件

- `cargo xtask ci` green
- 新テストが追加され、既存 Boolean モデル (e.g. cyl_sph) で pass する
- 死に分岐削除後もすべての既存 tessellation テストが pass する

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `tessellation/mod.rs:649-680` 死に分岐削除 | ADR-009 Implementation Outline Phase 1 (partition.rs 周期エッジ化) |
| 共有境界の直接比較テスト追加 (新規 1 ファイル) | ADR-009 Phase 2 (テッセレーション側のエッジ駆動化全面実装) |
| 周辺コメント簡潔化 | `ANGULAR_SEGMENTS_DEFAULT` の意味を変えること |

## Non-Goals

- Boolean partition.rs の `ANGULAR_SEGMENTS_DEFAULT` 参照箇所 8 件への手出し (これは ADR-009 実装 Issue の範囲)
- テッセレーションの境界サンプル導出ロジックを Edge 駆動に書き換えること (同上)
- `n_u` ヒューリスティクスの撤去 (同上)

## 関連

- ADR-009 (本作業の親 ADR)
- #138 (ADR-009 起票元、本 Issue 起票で close)
- #129 / #130 / #131 (連鎖バグの症状、本 Issue では直接修正しない)
- codex-review #129-F02 (共有境界比較テスト提案)
