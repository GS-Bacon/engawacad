# test-spec.md — #96 ExtrudeCut

## 実装済みテスト(GLM core で追加・CI green 確認済み)
| plan ID | 実装場所 | 実装関数 | 備考 |
|---------|----------|----------|------|
| U01 | `crates/mycad-build/tests/feature_dispatcher.rs` | `u01_extrude_cut_determinism` | assert_solids_equal byte 一致 |
| U02 | 同上 | `u02_extrude_cut_void_shell` | tool 埋没 → shells==2, euler==0 |
| U03 | 同上 | `u03_extrude_cut_partial_l` | 面貫通 → shells==1, manifold OK |
| U04 | `crates/mycad-format/src/feature.rs` (inline test) | `test_extrude_cut_yaml_golden` | YAML 往復 + target/depth フィールド |

## 不足テスト(plan 計画分 — STEP 6.6 で追加)

### U05_degen — dispatcher 退化ケース
場所: `crates/mycad-build/tests/feature_dispatcher.rs`
```rust
// u05_extrude_cut_degen_depth: depth <= 0 は InvalidParameter を返すべき
// → build_bodies_from_features が Err(KernelError::InvalidParameter{..}) を返すことを assert
// プロファイルは正常な矩形。depth = 0.0 と -1.0 の 2 ケース。
#[test]
fn u05_extrude_cut_degen_depth() { ... }
```

### U06_boundary — dispatcher 境界ケース
場所: `crates/mycad-build/tests/feature_dispatcher.rs`
```rust
// u06a_extrude_cut_missing_target: 存在しない target → BodyNotFound or 相当する Err
// u06b_extrude_cut_nonintersecting: 非交差 tool(target ボックス外に配置)
//   → STEP 6 で boolean の実挙動(t07/t08 参照)を確認して期待値を確定する
//   t07 は `Ok(...)` shells==0 or empty を返す実装、t08 は `Err(EmptyBooleanResult)` の可能性
//   → 実装時に `boolean` の戻り値を grep して具体 variant を特定すること
#[test]
fn u06a_extrude_cut_missing_target() { ... }
#[test]
fn u06b_extrude_cut_nonintersecting() { ... }
```

### T01_ui — vitest 純関数 決定性
場所: `web/src/extrude.test.ts` に追加
```ts
// buildExtrudeCutFeatures: 同入力で 2 回呼んで toEqual
// result.extrudeCut.type === "extrude_cut"
// result.extrudeCut.target === provided target
```

### T02_ui_degen — vitest 退化
場所: `web/src/extrude.test.ts` に追加
```ts
// target "" → null
// depth <= 0 → null
// depth NaN → null
// insetRect で collapse するケース: 幅 0 の face → null
```

### E01_cut — Playwright E2E (面クリック → btn-extrude-cut visible)
場所: `web/tests/extrude_panel.spec.ts` に追加(E01-E04 の extrudecut セクション)
```ts
// 面クリック後 [data-testid=btn-extrude-cut] が visible
```

### E02_cut — Playwright E2E (POST 順序 `["create_sketch","extrude_cut"]`)
場所: `web/tests/extrude_panel.spec.ts`
```ts
// intercept /api/v0/features
// 深さ入力 + btn-extrude-cut クリック
// postedBodies.toEqual(["create_sketch","extrude_cut"])
```

### A01 — acceptance (mycad-api, 実バックエンド)
場所: `crates/mycad-api/tests/extrude_cut_acceptance.rs`(既にスケルトン済み)
- `#[ignore]` を外し実装を完成させる
- simple_box(10×20×30)の xy 面に インセット sketch `[-2,2]×[-4,4]`, depth=20 → blind pocket
- POST create_sketch → POST extrude_cut → 頂点数変化 + `.mycad` に `type: extrude_cut` 1 件

## 実装差分から追加すべきテスト

### Playwright: `buildExtrudeCutFeatures` が `target` を正しく解決することの E2E
E02_cut の intercept で `body.target == <expected_feature_id>` を追加 assert する。
`currentBodies` から face_ids でボディを引く解決ロジックが正しく動作しているか確認。

### `insetRect` の単体テスト
`insetRect` は独立した純関数なので vitest で:
- 標準矩形 10×10 → ratio 0.25 後に 5×5(中央 50%)になることを確認
- collapse ケース(extent が ε_guard 以下)が null を返すことを確認

## エッジケース・退化入力(追加確認事項)
- `buildExtrudeCutFeatures`: `existingFeatureIds` が空 Set の時 `extrude_cut_1` が生成されること
- U06_boundary の非交差 tool: `boolean` 実挙動が `Ok(noop)` の場合、返ってきたソリッドが元の target と
  `assert_solids_equal` で一致することを assert(Err の場合は variant 名を assert)

## 数値境界(ADR-004 §3.1 準拠)
- depth ≤ 0.0 → `InvalidParameter` (LENGTH_TOLERANCE = 1e-9 より)
- inset 後 extent ≤ EPSILON_GUARD (1e-9) → `buildExtrudeCutFeatures` returns null
- U02 の tool は depth=3, box 半幅=5 → tool が [-5,5]×[-5,5]×[-5,5] の内側に収まることを座標確認で補強

## 決定性
U01 で網羅。IdGenerator seed=0 で 2 回ビルドして byte 一致。

## CI グリーン後の全テスト数
- rust: U01/U02/U03(dispatcher) + U04(format inline) + U05_degen + U06a/U06b + A01 = 計 8 integration tests
- vitest: T01_ui + T02_ui_degen + insetRect 単体 = 既存 70 + 新規 N 件
- Playwright: E01_cut + E02_cut = 既存 E01-E04 + 新規 2
