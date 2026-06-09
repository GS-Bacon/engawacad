## 不足テスト（plan 計画分）— GLM が実装済み

| ID | 実装状況 | 備考 |
|----|---------|------|
| t01_kernel_neg_depth_determinism | ✅ extrusion.rs inline test | ID・座標一致を確認 |
| t02_kernel_neg_depth_manifold | ✅ extrusion.rs inline test | validate_manifold + V-E+F==2 |
| t02_reg_negative_extrude_centroid | ✅ feature_dispatcher.rs（#[ignore] 解除 + depth=-3.0）| centroid_x < -5.0 |
| t01_positive_extrude_centroid | ✅ 既存テスト（変更なし） | リグレッション確認 |
| t_boundary_zero_depth_rejected | ✅ extrusion.rs inline test | depth=0 → Err |
| t_degen_nonfinite_depth_rejected | ✅ extrusion.rs inline test | NaN/Inf → Err |
| front_neg_sign | ✅ extrude.test.ts（f_z_neg fixture 修正）| depth < 0 |
| front_pos_sign | ✅ extrude.test.ts（f_z_pos fixture 確認）| depth > 0 |

## 実装差分から追加すべきテスト

### 追加観察: extrusion.rs L513 の既存テスト修正
GLM が `-1.0` depth を「以前は Err だったが now is_ok()」に変更した。
これは符号付き depth 契約への正しい対応。

### 追加テスト提案（STEP 6.6 で対応）

1. **t03_positive_and_negative_symmetric**: `depth=+3.0` と `depth=-3.0` で生成した solid の頂点 bounding box が平面に対して対称であることを assert
   - yz 平面で origin=(0,0,0), depth=+3 → x∈[0,3] / depth=-3 → x∈[-3,0]
2. **t04_negative_extrude_fuse_integration（dispatcher）**: `offset=-5, depth=-3` の押し出しを既存 box に fuse する dispatcher テスト（fuse_target あり）
   - 現状 t02_reg は fuse_target なしのスタンドアローン。fuse パスもカバーする。

## エッジケース・退化入力

| ケース | 実装状況 |
|--------|---------|
| depth が ε 未満の極小正値 | ✅ t_boundary_zero_depth_rejected でカバー |
| depth = -ε（極小負値、abs ≤ ε）| ✅ 同テスト内（`depth.abs() <= length_eps` で Err） |
| depth = NEG_INFINITY | ✅ t_degen_nonfinite_depth_rejected |
| faceId に face パターンなし → デフォルト +1 | ✅ extrude.test.ts `no face pattern → sign +1` |

## 数値境界
- `LENGTH_TOLERANCE = 1e-9`（extrusion.rs） → depth=1e-10 は Err、depth=1e-8 は OK

## 決定性
- t01_kernel_neg_depth_determinism で同 seed 2 回実行 → 全 ID・座標一致を確認済み
