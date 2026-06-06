# Test Spec — Issue #56 tessellation-cap

## 不足テスト（plan 計画分）
全 T01〜T07 は acceptance テストで実装済み。未実装なし。

## 実装差分から追加すべきテスト
なし — 変更は 3 行の条件式のみ（trim_lower XOR、same_sense の正規化）。

## エッジケース・退化入力
- T06: v_lat = 0 (赤道切断) → asin(0)=0 → v_boundary=0, pole_v=±π/2。通常の cap と同じコードパスで処理。スタックオーバーフロー・NaN なし。
- T07: trim_lower=false + same_sense=false (upper cap, inward sphere) → XOR=false → `(a0, b0, a1)` パス。外向き法線 dot > 0 を確認済み。

## 数値境界
- pole fan が push_triangle の AREA_EPS ガード（1e-14）に引っかかる可能性: n_u=32 の pole fan は十分な面積あり。問題なし。
- 球面 r が極小（1e-10 以下）時: 既存 `test_sphere_tiny_radius_tessellation` がカバー。

## 決定性
T01 で確認済み。2 run 完全一致。
