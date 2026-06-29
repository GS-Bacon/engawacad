# Debug Spec for STEP 7.5 codex round 2 (F01 + F02 修正)

Codex 3 persona から 2 件指摘。両方を 1 round で fix する。

## F01 (high) — Ellipse 境界テストの値再設計

Codex 3 persona 全員が「Ellipse の境界テスト 4 件は実際には `<=` 境界を踏んでいない」と指摘。実装を `<` strict に戻してもテストが pass してしまう = 境界の偽陽性ガード。

### 仮説 / 関連ファイル

- `crates/engawa-kernel/src/tessellation/sketch.rs` の inline tests (line 1033 周辺):
  - `t_boundary_above_tolerance_ellipse_major` (現状: major=1e-6, minor=1e-6)
  - `t_boundary_above_tolerance_ellipse_minor` (現状: major=1, minor=1e-5)
  - `t_boundary_exact_tolerance_ellipse_major` (現状: major=TOL, minor=TOL*1000) → major < minor で ADR-017 不変違反による reject の可能性
  - `t_boundary_exact_tolerance_ellipse_minor` (現状: major=1e-5, minor=TOL) → minor 境界を踏むが reject 理由を matches で限定していない

### 修正方針 (Codex contrarian F01 提案を採用)

inline test 4 件と acceptance test 4 件で同一入力 + reject 理由まで match する。

**T_BOUNDARY_above_tolerance_ellipse_major / minor (pass)**:
```rust
let major = LENGTH_TOLERANCE * 1.000_001;
let minor = LENGTH_TOLERANCE * 1.000_001;
// rotation: 0.0, id: ...
let result = tessellate_sketch_element(&ellipse, 32);
assert!(result.is_ok());
```
- major == minor で `minor / major == 1` → EPS_AXIS_RATIO 判定 pass
- 両方 TOL より一段上 → pass

**T_BOUNDARY_exact_tolerance_ellipse_major (reject by major <= ε_radius)**:
```rust
let major = LENGTH_TOLERANCE;
let minor = LENGTH_TOLERANCE;
let result = tessellate_sketch_element(&ellipse, 32);
assert!(matches!(
    result,
    Err(KernelError::DegenerateSketchElement { reason, .. }) if reason == "major <= ε_radius"
));
```
- major == TOL → 実装の `*major <= LENGTH_TOLERANCE` 判定が先に reject、reason "major <= ε_radius"
- 実装を `<` strict に戻すと major < TOL = false → 続く → minor < TOL = false → ratio check pass → mesh ok = テスト fail → 偽陽性ガード成立

**T_BOUNDARY_exact_tolerance_ellipse_minor (reject by minor <= ε_radius)**:
```rust
let major = LENGTH_TOLERANCE * 1.000_001;
let minor = LENGTH_TOLERANCE;
let result = tessellate_sketch_element(&ellipse, 32);
assert!(matches!(
    result,
    Err(KernelError::DegenerateSketchElement { reason, .. }) if reason == "minor <= ε_radius"
));
```
- major > TOL なので major check pass
- minor == TOL → `*minor <= LENGTH_TOLERANCE` が reject、reason "minor <= ε_radius"
- 実装を `<` strict に戻すと minor < TOL = false → ratio check (~1) pass → mesh ok = テスト fail → 偽陽性ガード成立

### Circle / Arc 境界テストへの reason 一致追加

Circle/Arc 側も同様に reject 理由を match に含める (現状は `..` のため):

**T_BOUNDARY_exact_tolerance_circle / arc**:
```rust
assert!(matches!(
    result,
    Err(KernelError::DegenerateSketchElement { reason, .. }) if reason == "radius <= ε_radius"
));
```

`t_deg_below_tolerance` (Circle) も同様に reason 一致を追加。

### acceptance test mirror

`crates/engawa-kernel/tests/length_tolerance_boundary_acceptance.rs` の Ellipse 4 件 + Circle/Arc reject 系も同じ入力・同じ reason 一致に揃える。
ただし KernelError は `engawa_kernel::error::KernelError` 直接参照。

## F02 (medium) — pub use の scope 逸脱削除

`crates/engawa-kernel/src/tessellation/mod.rs` の以下を **削除**:

```rust
pub use sketch::tessellate_sketch_element;
```

`pub mod sketch;` のため `engawa_kernel::tessellation::sketch::tessellate_sketch_element` で既にアクセス可能。

acceptance test の import を変更:

**before**:
```rust
use engawa_kernel::tessellation::tessellate_sketch_element;
```

**after**:
```rust
use engawa_kernel::tessellation::sketch::tessellate_sketch_element;
```

## 期待結果

- inline test 4 件 (Ellipse 境界) と acceptance test 4 件が新しい値で pass する
- reject 系 (exact_tolerance) は reason "major <= ε_radius" / "minor <= ε_radius" / "radius <= ε_radius" の一致で偽陽性ガード成立
- `pub use sketch::tessellate_sketch_element;` 削除後、acceptance test の import 変更で compile pass
- `cargo xtask ci` green

## 試した修正と結果

(STEP 6.7 round 2 で GLM 実装後に追記する)

## 次にやること

GLM に `--mode test` で再 dispatch (F01 の test 修正が主、F02 の `pub use` 削除も同 dispatch で一括対応)。
