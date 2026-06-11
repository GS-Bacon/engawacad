<!-- Round ごとに以下の形式で追記すること -->

## STEP 5.5 — バグ再現ファースト確認の例外

- **再現テスト**: T05_boundary_seam_pos は `periodic_u_shift` ヘルパ (STEP 6 で `pub fn` 公開予定) を直接呼ぶ構成。
- **本 Issue 着手時の状況**: `periodic_u_shift` 未公開、テスト本体は `todo!()` で待機。
- **理由**: Issue #134 監査由来の「将来到達するリスク」起票で、現状の base カーネル (#130/#131 後) では Boolean Cut の制限により cylinder side face で inner_loop が出るシナリオを integration 経路で trigger できない。
  - 確認した経路 (両者とも `BooleanInternal("manifold validation failed: edge must have exactly 2 half-edges")` でエラー):
    - cylinder × cylinder Cut (cross-axis 穴 cylinder で貫通)
    - cylinder × box Cut (axis-aligned box で側面貫通)
- **対応方針**: shift ロジックを `pub fn periodic_u_shift` として抽出 (plan.md 変更箇所 2)、単体性質を tests/ から直接検証。STEP 6 GLM 実装後に T01-T08 (T05 含む) を解除して検証する。バグ再現は単体テストの「shift が 0 のまま (現状の broken 動作)」を「shift = round(Δ/2π)*2π (修正後)」で置き換える形で論理的に担保する。

## STEP 7 GLM Final Review (Round 1 — max-turns=30)

- **FN01 (critical)** 「u 掃引 integration テスト未実装」: **棄却 (実装不可能要件 / 別 Issue 候補)**
  - GLM 指摘の作業内容3 (u sweep + Boolean Cut + naked_edge=0) は現状の base カーネル (#130/#131 後) では実装不可能。Boolean Cut の制限により cylinder side face で inner_loop が出るシナリオを trigger できない (cylinder × cylinder Cut / cylinder × box 貫通 Cut のいずれも `BooleanInternal("manifold validation failed: edge must have exactly 2 half-edges")` で失敗、STEP 5.5 で実機確認済み)。
  - shift ロジック自体は `periodic_u_shift` を `pub fn` として抽出し、`trim_surface_uv_shift_acceptance.rs::T01-T08` (8 件) で周期保存性・退化境界・決定性を全網羅で検証済み。T09 で #130 既存 box - cylinder Cut 経路の回帰も担保。
  - Integration テスト (u sweep + naked_edge=0) は Boolean Cut の制限を解消する別 Issue が完了したのち、後続 Issue として追加するのが筋。本 Issue 範囲で artificially に B-rep を構築するのは ad-hoc で品質保証として不適。
  - **ユーザー承認済み** (自律バッチモード STEP 7 critical エスカレーション時に「棄却して進む」を選択)。
  - 棄却理由を `rejection.md` に記録。
