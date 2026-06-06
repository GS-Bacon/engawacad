# Test Spec — Issue #77: kernel 回転 transform

## 不足テスト（plan 計画分）
T01-T09 は `#[ignore]` スケルトンとして `rotate_acceptance.rs` に存在。
GLM テスト実装ではこれらを実装する。GLM コア実装では11件の inline テストを追加済み。

T01 決定性100x: 同一Solidを100回rotate → 全頂点一致
T02 x90° 法線確認: Cuboidの面法線が厳密に軸に揃う (精度1e-12)
T03 逆変換: rotate(M) → rotate(M^T) で元の幾何に戻る (精度1e-12)
T04 恒等行列: euler_to_matrix(0,0,0) → 単位行列
T05 snap確認: euler_to_matrix(90,0,0) 各要素がスナップ済み整数
T06 Cylinder basis一貫性: orthonormal_basis(new_axis)が元の基底の回転と一致 (精度1e-12)
T07_boundary_zero_rotation: (0,0,0)回転Solidが元と完全一致
T08_boundary_180: x軸180°でy/z座標が符号反転 (精度1e-12)
T09_degen_pivot_at_vertex: pivot頂点の座標が不変 (精度1e-15)

## 実装差分から追加すべきテスト
- GLMコア実装が `topology.rs` に `#[cfg(test)]` で11件追加済み。
  追加された inline テストをもとに見落としを確認する。
- 追加テスト候補: `euler_to_matrix(0,90,0)` のY回転行列の各要素確認

## エッジケース・退化入力
- T09_degen_pivot_at_vertex: pivot 点が頂点座標と一致する場合
- T07_boundary_zero_rotation: ゼロ度回転は恒等変換

## 数値境界
- T02/T03/T06/T08: 精度 1e-12 (approx crate の epsilon)
- T09_degen: 精度 1e-15 (pivot 点の厳密不変性)
- T05: exact integer snap (sin90=1.0, cos90=0.0)

## 決定性
T01: 100回反復で全頂点座標が完全一致。euler_to_matrix は純粋関数。
