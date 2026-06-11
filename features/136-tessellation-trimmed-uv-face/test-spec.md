# Test Spec — #136 trimmed UV face u shift

## サマリ

`trim_surface_uv_shift_acceptance.rs` の T01-T09 すべて pass。`periodic_u_shift` を `pub fn` として公開し plan 通りの周期保存シフト (`shift = round((outer_center - avg_inner) / 2π) * 2π`) を実装。期待値乖離なし。

## 不足テスト (plan 計画分)

なし。T01-T09 すべて実装済み。

## 実装差分から追加すべきテスト

なし。GLM 実装 (`tessellation/mod.rs:418-441` `periodic_u_shift` + L500-505 呼出し置換) は plan 通りで、想定外の分岐なし。

## エッジケース・退化入力

実装済み:
- T07_degen_empty_outer (outer 空 → no-op)
- T08_boundary_inner_centered (shift exact 0.0)

追加チェック不要。

## 数値境界

- `round(Δ / 2π)` の挙動: half-away-from-zero (T05/T06 で ±0.5 サイクル正確にカバー)
- T01 で決定性 (同一入力 → 同一出力) 確認済み

## 決定性

T01 (`t01_determinism`) で同一 outer/inner 入力を 2 回呼び結果完全一致を assert。pure function なので明白に決定的。

## 類似ケース (未カバー)

bug 修正 Issue としての類似コードパス探索:

1. **`unwrap_periodic_uv` 関数本体**: 内側ループ unwrap で呼ばれる。本 Issue は呼び出し直後の shift ロジックのみ修正。`unwrap_periodic_uv` 自体は周期保存ではない (各点の sequential 差分処理) が、これは別ロジックで本 Issue 範囲外 (Non-Goal)。
2. **外側ループの unwrap (L443-450)**: 同じ `unwrap_periodic_uv` を呼ぶ。outer は inner と異なり shift を行わない (`outer_center` の基準) ため、本修正の影響範囲外。
3. **sphere の trimmed UV face**: `tessellate_trimmed_uv_face` は cylinder と sphere の両方で呼ばれる。sphere の uv は (φ, θ) で u が経度 [-π, π]、v が緯度 [-π/2, π/2]。本修正で u (経度) の周期保存は sphere でも有効に働く。v (緯度) は周期性なし (極で詰まる) ため影響なし。**sphere の trimmed face で本修正は同じく正しく動く**ことを論理的に確認 (#137 別 Issue で integration テスト追加候補)。
4. **cone の trimmed UV**: cone は現在 trimmed tessellation 未実装。本 Issue 範囲外。

**結論**: 構造的類似バグなし。test-spec として追加要件なし。

## 期待値乖離

なし。plan の T ID と acceptance test の assertion が一致。

## STEP 6.6 GLM テスト実装への指示

**スキップ可**: T01-T09 全て pass しているため追加テスト実装不要。state を直接 glm_impl=passed に set 済み。
