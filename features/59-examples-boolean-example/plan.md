## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| 5 つの boolean example YAML の座標修正（視覚的変化が出る配置に変更） | CreateBox への origin フィールド追加（別 Issue） |
| golden_examples.rs の golden 文字列を新 YAML に合わせて更新 | 新しい example ファイルの追加 |
| examples_smoke.rs への影響確認 | ビューア実装 |

## Non-Goals
- CreateBox に origin パラメータを追加すること
- 既存の cylinder/sphere/fuse_box_cyl 等の変更
- テッセレーション品質の改善

## 実装対象

**影響ファイル:**
- `examples/boolean_box_fuse.mycad`
- `examples/boolean_box_cut.mycad`
- `examples/boolean_box_intersect.mycad`
- `examples/boolean_box_void.mycad`
- `examples/boolean_cut_sphere_dimple.mycad`
- `crates/mycad-format/tests/golden_examples.rs` (golden 文字列更新)

**変更方針:** CreateBox は origin なし（常に原点中心）。視覚的変化を出すために:
- fuse: 異なるサイズの box を fuse してクロス形状
- cut: cylinder (origin 使用) を tool にして貫通穴
- intersect: sphere を tool にして丸みある交差形状
- void: sphere (center 使用) を outer box 上面付近に配置
- sphere_dimple: sphere center を box 上面付近 (z=4.5) に設定

**before/after:**

### boolean_box_fuse.mycad
```yaml
# Before: box_a 2×2×2, box_b 2×2×2 → same shape
# After: box_a 6×2×2 (X方向), box_b 2×6×2 (Y方向) → クロス形状
```

### boolean_box_cut.mycad
```yaml
# Before: target 2×2×2, tool box 1×1×1 (内部空洞のみ)
# After: target 4×4×4, tool cylinder r=1 h=6 origin=[0,0,-3] (貫通穴)
```

### boolean_box_intersect.mycad
```yaml
# Before: box_a 2×2×2, box_b 2×2×2 → same shape
# After: box_a 4×4×4, tool sphere r=3 → 丸みのある直方体
```

### boolean_box_void.mycad
```yaml
# Before: outer 4×4×4, inner 2×2×2 同位置 (外見変化なし)
# After: outer 6×6×6, inner sphere r=2 center=[0,0,3.5] (上面ディンプル)
```

### boolean_cut_sphere_dimple.mycad
```yaml
# Before: sphere r=3 center=[0,0,0] (完全内部 → ディンプル見えない)
# After: sphere r=3 center=[0,0,4.5] (上面付近 → ディンプル可視)
```

## 設計方針
- 決定性: YAML は静的データ。変更前後で同一ロード結果。
- golden_examples.rs は round-trip テスト → YAML 変更後に実際の出力を確認してから更新。
- examples_smoke.rs の既存エントリは変更不要（smoke テストは「ビルド成功」のみ検証）。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_golden_roundtrip | 正常系 | 5 ファイルの golden round-trip が全通過 | 全 assert_eq! pass |
| T02_smoke_build | 正常系 | examples_smoke.rs の既存テストが全通過 | ok |
| T03_degen_boundary | 退化/境界 | cylinder tool が box を完全貫通する geometry | panic なし + 正常 solid |

## 幾何的不変条件チェックリスト
- N/A: テッセレーション層変更なし
- N/A: B-rep topology 変更なし (YAML データのみ)
