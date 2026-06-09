# Test Spec — #123 e2e-phase-extrude-extrudecut

## 実装済みテスト

| ID | 実装 | ファイル |
|----|------|---------|
| T01_extrude_normal | ✅ depth=2.0、頂点数 > baseline + type:extrude 確認 | acceptance_extrude.spec.ts |
| T02_extrude_min | ✅ depth=0.01、最小値 | acceptance_extrude.spec.ts |
| T03_extrude_large | ✅ depth=10.0、大値 | acceptance_extrude.spec.ts |
| T04_degen_extrude_neg_face | ✅ depth=-2.0、#110 回帰 | acceptance_extrude.spec.ts |
| T05_degen_extrude_neg_min | ✅ depth=-0.01、#110 回帰 | acceptance_extrude.spec.ts |
| T06_extrude_cut_normal | ✅ depth=1.0 ExtrudeCut | acceptance_extrude.spec.ts |
| T07_extrude_cut_near_boundary | ✅ depth=14.999、境界手前 | acceptance_extrude.spec.ts |
| T08_degen_extrude_cut_at_boundary | ✅ depth=15.0、#111 回帰 | acceptance_extrude.spec.ts |
| T09_extrude_cut_beyond_boundary | ✅ depth=15.1、境界超え | acceptance_extrude.spec.ts |

## 設計ノート

- `test.describe.configure({ mode: "serial" })` で直列実行（共有サーバの状態競合防止）
- 各テストは `getTotalVertices()` で現在ベースラインを取得してから POST
- `getTotalVertices()` は `request` fixture を型 `any` で受けるシンプル実装

## Phase 7 拡張ポイント

`acceptance_extrude.spec.ts` 末尾の TODO コメントを参照。
CreateSketch 起点（ブラウザ面選択 → スケッチ描画 → 押出）のケースを追加する。
