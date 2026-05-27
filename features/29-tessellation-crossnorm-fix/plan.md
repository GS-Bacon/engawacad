# バグ修正: push_triangle 外積二乗ノルム式の .sqrt() 誤適用

## Issue

GitHub Issue #29: `bug(tessellation): push_triangle の cross_norm 式が不正（.sqrt() が最後項のみ適用）`

## 問題

`crates/mycad-kernel/src/tessellation/mod.rs` の `push_triangle` 関数で、外積ノルムの計算式が誤っている。

**現在のコード（誤り）**:
```rust
let cross_norm = (u[1] * v[2] - u[2] * v[1]).powi(2)
    + (u[2] * v[0] - u[0] * v[2]).powi(2)
    + (u[0] * v[1] - u[1] * v[0]).powi(2).sqrt(); // ← .sqrt() は最後の項だけに掛かる
if cross_norm < AREA_EPS * AREA_EPS {
```

Rust の演算子優先度により `.sqrt()` はメソッドチェーンで最後の `.powi(2)` 項だけに適用され、結果は `a² + b² + |c|` という次元混在量になっている。

## 調査結果

- 閾値は `AREA_EPS * AREA_EPS`（`AREA_EPS = 1e-14`, 閾値 = 1e-28）
- テスト `test_t09_near_degenerate_triangle_added`（L1414-1428）のコメント: `// cross_norm >> AREA_EPS² but still very small`
- `tiny = AREA_EPS.sqrt() * 10.0 = 1e-6` のとき、`cross_norm` は外積の**二乗ノルム** = (1e-12)² = 1e-24 >> 1e-28 ✓

よって `cross_norm` は外積の**二乗ノルム**（三成分それぞれ `.powi(2)` の和、`.sqrt()` なし）であるべき。

## 修正内容

**ファイル**: `crates/mycad-kernel/src/tessellation/mod.rs`

`push_triangle` 関数内の該当箇所を以下に変更する:

```rust
// 変更前
let cross_norm = (u[1] * v[2] - u[2] * v[1]).powi(2)
    + (u[2] * v[0] - u[0] * v[2]).powi(2)
    + (u[0] * v[1] - u[1] * v[0]).powi(2).sqrt();
if cross_norm < AREA_EPS * AREA_EPS {

// 変更後
let cross_norm_sq = (u[1] * v[2] - u[2] * v[1]).powi(2)
    + (u[2] * v[0] - u[0] * v[2]).powi(2)
    + (u[0] * v[1] - u[1] * v[0]).powi(2);
if cross_norm_sq < AREA_EPS * AREA_EPS {
```

変更点:
1. `.sqrt()` を末尾の `.powi(2)` から削除（三成分すべて二乗和のみ）
2. 変数名 `cross_norm` → `cross_norm_sq`（二乗量であることを名前で明示）
3. `if` の比較変数名も `cross_norm_sq` に合わせる

**新規コードは追加しない。テストも追加しない**（既存テストが正しく通ることを確認するだけ）。

## テスト計画

1. `cargo test -p mycad-kernel` を実行
   - `test_t09_near_degenerate_triangle_added` が pass（near-degenerate は追加される）
   - 退化三角形テスト（identical-point triangle should not be added）が pass
   - その他の既存 tessellation テストが pass
2. `cargo xtask ci` が green になること
