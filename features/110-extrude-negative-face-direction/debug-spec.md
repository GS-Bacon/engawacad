## 仮説

vitest `front_neg_sign` / `front_pos_sign` テストが失敗している。
`buildExtrudeFeatures("N(v0;face:f_x_neg)", positions, indices, faceIds, 5, ...)` で result が null になっている。

## 根本原因

GLM が書いたテスト (`extrude.test.ts` の `#110 faceSignFromFaceId` describe) で、`boxTopFaceData()` fixture を使いながら `f_x_neg` / `f_x_pos` の faceId を渡している。`boxTopFaceData()` の `faceIds` は全て `"N(v0;face:f_z_pos)"` のみなのでマッチせず、`footprintProfile` → null → `buildExtrudeFeatures` → null になる。

## 関連ファイル
- `web/src/extrude.test.ts` 行 1007-1025（失敗テスト）
- `web/src/extrude.test.ts` 行 30-43（`boxTopFaceData` fixture）

## 修正方針

`front_neg_sign` / `front_pos_sign` テストを正しい fixture で書き直す。

**方針 A（推奨）**: `f_z_neg` face 用の fixture を作り、`buildExtrudeFeatures("N(v0;face:f_z_neg)", ...)` で `result.extrude.depth < 0` を確認。`f_z_pos` は既存 T01/T07 が通っているので depth > 0 の間接確認として使える。

具体的には:
1. `boxNegFaceData()` を追加（z=-5 付近の `f_z_neg` 面、triangles の z=0 固定で OK）:
```ts
function boxNegFaceData() {
  const positions = new Float32Array([0,0,0, 10,0,0, 10,10,0, 0,10,0]);
  const indices = new Uint32Array([0,1,2, 0,2,3]);
  const faceIds = ["N(v0;face:f_z_neg)", "N(v0;face:f_z_neg)"];
  return { positions, indices, faceIds };
}
```
2. `front_neg_sign` テストで `faceId = "N(v0;face:f_z_neg)"` + `boxNegFaceData()` を使う
3. `front_pos_sign` テストで `faceId = "N(v0;face:f_z_pos)"` + `boxTopFaceData()` を使う（faceIds が一致するので通る）

## 試した修正と結果
- GLM 1 回目: fixture 不一致（`f_x_neg` + `boxTopFaceData()`） → vitest 2 失敗

## 次にやること
- `web/src/extrude.test.ts` の `#110 faceSignFromFaceId` describe を上記方針 A で修正し、vitest 全 pass + CI green にする
- `faceSignFromFaceId` の単体テスト（`f_z_neg → -1`, `f_y_pos → +1`）は fixture 不要なので現状維持

## 追加で書いてほしいテスト
- 特になし（方針 A の修正で足りる）

---

## Codex R1 指摘（F01 high / F02 medium）

### F01 (high, blocking): 極端な depth で NaN 法線生成

**問題**: `depth.abs() <= eps` && `depth.is_finite()` の検証は通過するが、`depth = ±f64::MAX` のとき `p + depth * normal` でオーバーフロー → side face の cross product が `inf × inf - inf × inf = NaN` になる。`validate_manifold()` は法線有限性を見ないため見逃される。

**修正方針**: depth の絶対値に実用上限を設ける。CAD の実用範囲は 1e9 mm 以内（= 1000 km）なので `depth.abs() > 1e12` を `InvalidParameter` にする。

`extrusion.rs` L145 の検証を以下に変更:
```rust
if !depth.is_finite() || depth.abs() <= length_eps || depth.abs() > 1e12 {
    return Err(KernelError::InvalidParameter { kind: "depth" });
}
```

**テスト追加**: `t_degen_nonfinite_depth_rejected` に `f64::MAX` / `-f64::MAX` を追加:
```rust
assert!(matches!(
    make_extrusion(&plane, &profile, f64::MAX, &mut gen),
    Err(KernelError::InvalidParameter { kind: "depth" })
));
assert!(matches!(
    make_extrusion(&plane, &profile, -f64::MAX, &mut gen),
    Err(KernelError::InvalidParameter { kind: "depth" })
));
```

### F02 (medium): t04_negative_extrude_fuse_integration が無効な検証

**問題**: `feature_dispatcher.rs` の `t04_negative_extrude_fuse_integration` テストが `build_features(features).ok()` または同等の形で Err を全許容しており、`depth reject` への回帰も見逃す。

**修正方針**: `.expect("should succeed")` または `unwrap()` に変更し、成功を厳密にアサートする。もし boolean 演算失敗（manifold/geometric）が既知なら `#[ignore = "known limitation: ..."]` を付けて将来の修正を明示する。

まず現テストの内容を確認し、適切に修正する。
