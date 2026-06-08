## 不足テスト（plan 計画分）

| ID | 実装状況 | 備考 |
|----|----------|------|
| T01 | ✅ 実装済み (`t01_prop_cut_never_produces_degenerate_manifold`, 32 cases) | |
| T01_boundary | ✅ 実装済み (`t01_boundary_coplanar_tool_does_not_panic`, 32 cases) | |
| T02 | ✅ 実装済み — surface cut に変更 (`t02_prop_surface_cut_reduces_volume`) | plan は void cut (x_offset ∈ [-2,2]) だったが void cut で vol_after > vol_before になることが判明。表面カット (x_offset ∈ [4.1, 5.4]) に修正して vol_after < vol_before を確認 |
| T02_degen | ✅ 実装済み (`t02_degen_minimum_tool_size_ok_or_err`) | |
| T03 | ✅ 実装済み (`t03_determinism_fixed_seed`) | |

## 実装差分から追加すべきテスト

- **T02 void cut semantics**: plan の T02 は void cut での体積減少を想定していたが、tessellate_solid の内殻 winding が positive signed volume を返すため vol_after > vol_before になることが確認された。この挙動を regression として記録するテストを追加すべき:
  ```rust
  fn t02_void_cut_volume_exceeds_initial() {
      // x_offset=0 (fully internal) → vol_after > vol_before (inner shell adds abs volume)
  }
  ```

## エッジケース・退化入力

- **T01_boundary** では coplanar tool (x_offset ± 5.0 で target face に接触) をカバー済み
- **T02_degen** では 1×1×1 の最小ツールをカバー済み
- **パニックなし**: T01_boundary で catch_unwind を使って verify 済み

## 数値境界

- T01: x_offset ∈ [-3, 3] (target X face は ±5 なので tool w=1..4 で部分的に内部埋没)
- T01_boundary: x_offset ∈ [-5, 5] (完全 coplanar から内部まで網羅)
- T02 (surface cut): x_offset ∈ [4.1, 5.4] で tool X range = [x_offset-1, x_offset+1] が target X=+5 を必ず貫通

## 決定性

- T03: `IdGenerator::new(99)` + 固定入力で 2 回実行 → vertices/edges/faces 数と vertex ID が一致を確認
- proptest の shrinkage seed: `boolean_proptest.proptest-regressions` ファイルに保存 (git 管理外、.gitignore)

## 追加推奨テスト（GLM test-implementer 向け）

1. **T02_void_semantics**: x_offset=0 の void cut で vol_after > vol_before を assert (現挙動の regression ガード)
2. **T01_large_tool**: tool_w=9.9 (target の幅 10 より小さい) で surface cut → validate_manifold OK or clean Err
3. **T03_boundary_seed**: `IdGenerator::new(200)` で T01_boundary と同一入力 → 2 回実行で同一結果
