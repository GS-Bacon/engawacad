# Test Spec — #116 api-multistep-e2e

## 実装状況

コア実装 (STEP 6) で GLM が 3 テストをすべて実装済み。`cargo xtask ci` 全ステージ green 確認済み。

## plan テスト計画 vs 実装の突き合わせ

| ID | plan 期待結果 | 実装状況 | 備考 |
|----|--------------|----------|------|
| S08 | 200 OK, bodies >=2, cut face_id 含む | ✅ 実装済み | yz plane を使用（xy は extrude_0 底面と共面になるため）|
| S08_boundary | 200 OK (depth < target 辺長) | ✅ 実装済み | depth=2.9（yz 軸切断方向 x∈[-3,3] の内側）|
| S08_degen | 422 UNPROCESSABLE_ENTITY | ✅ 実装済み | `nonexistent_0` target → 422 |

## Plan からの適応

1. **S08 / S08_boundary: sketch_1 を yz 平面に変更**
   - plan では xy 平面の sketch_1 ([-1,1]×[-1,1]) + depth=2 を想定
   - xy 平面は extrude_0 底面 (z=0) と共面になるため void cut が不正確になる
   - GLM が yz 平面 (y∈[-1,1], z∈[1,4]) + x 方向 depth に変更 → 幾何的に正当

2. **S08_boundary: depth=4 → 2.9**
   - yz 平面で extrude すると tool が x 方向 [0, depth] に伸びる
   - extrude_0 は x∈[-3,3] → depth=3 で境界、2.9 で「ギリギリ内側」
   - plan の depth=4 は xy 軸想定の値であり、yz 軸変更後は 2.9 が正しい

## 追加すべきテスト

コア実装が plan の全 3 ID を網羅しているため、テスト追加は不要。

## エッジケース・退化入力

- S08_degen が nonexistent target を網羅済み
- S08_boundary が depth ギリギリ境界を網羅済み

## 数値境界

- extrude_0: x∈[-3,3], y∈[-3,3], z∈[0,5]
- cut tool (yz plane): y∈[-1,1], z∈[1,4], x∈[0, depth]
- S08_boundary depth=2.9: x∈[0,2.9] ⊂ [-3,3] ✓

## 決定性

- 各テストは `temp_copy("simple_box.mycad")` で独立した一時ファイルを使用
- `make_app(path)` は毎回新しい AppState を生成 → テスト間干渉なし
