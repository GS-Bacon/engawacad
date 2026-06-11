# test(tessellation): 隣接面の共有境界サンプルを直接比較する acceptance テストを追加する (ADR-009 / codex #129-F02)

## 位置付け

**`type: foundation`** (ADR-002「ラベル運用: 2 軸ラベル制」)。Phase 7 milestone への差し込み作業として起票する。Phase 7 完了判定 (xy/xz/yz スケッチ → Extrude/ExtrudeCut → CreateSketch Feature) には Phase 7 type: feature Issue 群が直接寄与するが、本 Issue は Phase 7 で **「描いたスケッチから Extrude/ExtrudeCut を実行する」完了条件の支え**として、Extrude 結果が既存 Boolean 形状と隣接するケース (スケッチからの押出が既存ソリッドの面に乗る場合等) で「面間の共有境界がずれていないか」を構造的に検出するセーフティネットを敷く。現状は naked-edge アサーションが上流で立つが、ずれた index・mm が出ないため Phase 7 シナリオで再発した際の調査コストが高い。ADR-002 §「差し込み作業は奉仕する Phase の Milestone に入れる」運用に沿う。

## 背景

ADR-009 で「Boolean 交線円を周期エッジで保持し、両側面はエッジ駆動で境界サンプルを共有する」(案 A) を決定した。しかし実装本体は Phase 4 再訪タイミングで起票する予定であり、それまで現状の「両側が独立に同じサンプル列を再導出する」規約に依存する状態が続く。

この状態における**規約違反 (両側のサンプル列がずれる) の検出能力が現状ない**。テッセレーション結果が watertight でなくなった瞬間に既存テストが落ちる (`bool_naked_edge_acceptance.rs` 等) が、failure mode が「naked edge カウント」のレイヤーで上がるため、**どのエッジ・どの index でずれたかが分からない**。

codex-review #129-F02 でも「共有境界の直接比較テスト」が提案されている。

## 作業内容

新規 acceptance テスト `crates/mycad-kernel/tests/shared_boundary_acceptance.rs` を追加する:

1. 既存 Boolean fixture (cyl×sphere の交線、box×sphere の交線等、Boolean 内 enum で網羅) を `make_*` で生成
2. tessellate した結果から隣接面ペアを列挙し、共有エッジに沿った境界頂点列を両側から取得
3. 両側の頂点列を index 順に直接比較し、`LENGTH_TOLERANCE` 以内で一致することを assert
4. 不一致時のエラーメッセージで `エッジ index / 頂点 index / 期待値 vs 実測値 / 距離 mm` を出力

ADR-009 実装本体が入ったら本テストは構造的に green になる。それまでは「現状の規約依存が動いていること」のセーフティネット + 偽陽性ガード (CI green ≠ 規約遵守) として機能する。

## 完了条件

- `cargo xtask ci` green
- 新ファイル `shared_boundary_acceptance.rs` 追加、最低 2 fixture を pass
- 既存テスト群に回帰なし

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| 新規 acceptance テスト 1 ファイル | 既存テストの修正 / 削除 |
| 既存 Boolean fixture (cyl×sphere, box×sphere 等) の利用 | 新規 Boolean fixture の作成 |
| 失敗時診断メッセージの整備 | tessellation/partition の実装変更 |

## Non-Goals

- 共有境界のずれを修正すること (ADR-009 実装本体の責務)
- 規約依存の解消 (同上)
- ADR-009 Implementation Outline Phase 1 / Phase 2 の作業

## 数値モデル

- 比較公差: `LENGTH_TOLERANCE` (`geometry::math`, 1e-9 mm) を使用 — ADR-004 既存定数を流用
- 退化判定: 退化 fixture は本テストの対象外 (Non-Goals)、既存テストに委ねる

## 関連

- ADR-009 §Implementation Outline Phase 3 即時実施分の片方
- codex-review #129-F02 (本提案の出典)
- ADR-004 §LENGTH_TOLERANCE
- #129 / #130 / #131 (連鎖バグの症状)
