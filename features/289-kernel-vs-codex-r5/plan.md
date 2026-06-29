# Plan: #289 退化判定境界を `<` vs `<=` で統一する

## 自律判断ログ (自律モード B-3 で確定)

- **採用方針**: kernel 全体で `<= LENGTH_TOLERANCE` (inclusive) 退化判定に統一する
- **Issue body M-F01 の `<` strict 提案からの逸脱理由**:
  1. `geometry::math::length_near(a, b)` は `(a - b).abs() <= LENGTH_TOLERANCE` 規約。これは kernel の根本判定関数で「TOL 以下の差は等しい」が標準
  2. `booleans/mod.rs` の TX6 既存テストは `radius == LENGTH_TOLERANCE` を **reject** と検証 (`<=` 規約)
  3. `primitives/extrusion.rs` の depth/du²+dv² 退化判定は既に `<= length_eps` (`<=` 規約)
  4. r5 migration M-F01 (= `<` 統一) は `length_near` の `<=` 規約を見落としている可能性が高い。逆に r3 architect の `<=` 統一推奨が `length_near` と整合
- **trade-off**: 既存 `T_EDGE_length_tolerance_boundary: radius = LENGTH_TOLERANCE passes` を意味反転 (pass → reject) する。これは破壊的変更だが、Rectangle/Polygon/Slot (#275) が既に `<=` で導入され golden に取り込まれているため、Circle/Arc/Ellipse を `<=` に揃える方が修正範囲が小さい
- **ADR 化**: 新規 ADR-018「LENGTH_TOLERANCE 比較規約: `<=` で退化判定 standard」を draft する (ADR-013 auto-accept チェーンへ載せる)

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `tessellation/sketch.rs` の Circle/Arc/Ellipse 退化判定を `< LENGTH_TOLERANCE` → `<= LENGTH_TOLERANCE` に変更 | Rectangle/Polygon/Slot (既に `<=`、変更不要) |
| 既存 `T_EDGE_length_tolerance_boundary` 等の境界テストを assertion 反転 (pass → reject) | `EPS_DISCRIMINANT` / `EPS_AXIS_RATIO` 等の別系統 ε (Conic 判別式/axis ratio は別軸) |
| 境界の挙動を網羅する新規テスト追加: `T_BOUNDARY_above_tolerance` (radius = TOL * 1.000_001 pass) / `T_BOUNDARY_exact_tolerance` (radius = TOL reject) | `Tolerance` newtype 化 (#31, Phase 5 で対応) |
| `extrusion.rs` の境界判定が `<=` であることを確認 (変更なし、コメント補足のみ可) | `booleans/mod.rs` の `<=` 規約は既に整合しているため変更なし |
| 新規 ADR-018 draft で LENGTH_TOLERANCE 比較規約を明文化 | `ANGLE_TOLERANCE` の比較規約統一 (本 Issue では扱わない) |

## Non-Goals

- Tolerance newtype 化 / per-entity tolerance: ADR-004 で Phase 5 #31 と明記済み
- `ANGLE_TOLERANCE` 比較規約の統一: スコープ膨張防止のため別 Issue
- `EPS_DISCRIMINANT` / `EPS_AXIS_RATIO` 等の比較規約: 別系統の ε のため別 Issue
- 数値モデル本体の見直し (ADR-004 改訂)

## 実装対象

- 影響クレート/ファイル:
  - `crates/engawa-kernel/src/tessellation/sketch.rs` (Circle / Arc / Ellipse 退化判定 3 箇所 + 既存境界テスト)
  - `docs/decisions/018-length-tolerance-boundary-convention.md` (新規 ADR draft)
  - (確認のみ・変更なし) `crates/engawa-kernel/src/primitives/extrusion.rs` / `crates/engawa-kernel/src/booleans/mod.rs`

### tessellation/sketch.rs 修正パターン

**before** (line 41 周辺、Circle):
```rust
if *radius < LENGTH_TOLERANCE {
    return Err(TessellationError::DegenerateSketchElement { ... });
}
```

**after**:
```rust
if *radius <= LENGTH_TOLERANCE {
    return Err(TessellationError::DegenerateSketchElement { ... });
}
```

同パターンで line 76 (Arc `radius`), line 119 (Ellipse `major`), line 125 (Ellipse `minor`) を `<` → `<=` に変更する。
docstring (line 19) の `radius < LENGTH_TOLERANCE` 記述も `radius <= LENGTH_TOLERANCE` に更新する。

### 既存テストの assertion 反転

**before** (line 578 周辺):
```rust
/// T_EDGE_length_tolerance_boundary: radius = LENGTH_TOLERANCE passes.
#[test]
fn t_edge_length_tolerance_boundary() {
    let circle = Circle { center: ..., radius: LENGTH_TOLERANCE, ... };
    let mesh = tessellate_sketch(&sketch);
    assert!(mesh.is_ok());  // pass 検証
}
```

**after**:
```rust
/// T_EDGE_length_tolerance_boundary: radius = LENGTH_TOLERANCE rejected (`<=` convention, ADR-018).
#[test]
fn t_edge_length_tolerance_boundary() {
    let circle = Circle { center: ..., radius: LENGTH_TOLERANCE, ... };
    let mesh = tessellate_sketch(&sketch);
    assert!(matches!(mesh, Err(TessellationError::DegenerateSketchElement { .. })));
}
```

### 新規テスト追加

```rust
/// T_BOUNDARY_above_tolerance: radius slightly above LENGTH_TOLERANCE passes (Circle/Arc/Ellipse).
/// T_BOUNDARY_exact_tolerance: radius == LENGTH_TOLERANCE rejected (`<=` convention).
```

これらを Circle / Arc / Ellipse major / Ellipse minor それぞれに対し 1 件ずつ追加し、4 element × 2 境界 = 8 個の境界テストを追加する。

## 設計方針

- **決定性要件**: 比較演算子の変更のみ。決定性に影響なし
- **B-rep トポロジー**: 退化 reject の挙動が境界で 1 段厳しくなる。トポロジー妥当性 (Euler-Poincaré) は不変
- **退化幾何の扱い**: 「TOL 以下は退化」を kernel 統一規約とする。これは `length_near` の `<= TOL` 規約 (= 「TOL 以下の差は等しい」) と整合
- **derive 規約**: 変更なし
- **エラーハンドリング**: 既存 `TessellationError::DegenerateSketchElement` をそのまま使用
- **workspace.dependencies**: 変更なし

### 数値モデル (LENGTH_TOLERANCE 比較規約 — 本 Issue のテーマ)

- **退化判定境界**: `value <= LENGTH_TOLERANCE` で退化と判定する (inclusive)
- **根拠**: `length_near(a, b) := (a - b).abs() <= LENGTH_TOLERANCE` が「TOL 以下の差は等しい」規約。これは「`value <= TOL` ならば `value == 0` と等しい (= 退化)」と等価
- **trade-off (`<` strict 案との対比)**: `<` strict は「TOL は最小許容値」と解釈、`<=` inclusive は「TOL 以下は無視」と解釈。後者を採用するのは `length_near` 規約への適合と多数派 (extrusion/booleans) との整合のため
- **ADR-004 準拠方針**: トレラント方式 (Phase 4 で確定済)。本 Issue は global LENGTH_TOLERANCE の比較規約のみを統一する。per-entity Tolerance newtype 化は #31 (Phase 5) で対応

## テスト計画 (ID 付き)

| ID | 種別 | 対象 | 内容 | 期待結果 |
|----|------|------|------|----------|
| T01 | 決定性 | tessellate_sketch | 同一入力 2 回実行で mesh 完全一致 | assert_eq! |
| T_BOUNDARY_above_tolerance_circle | 境界 | Circle | `radius = LENGTH_TOLERANCE * 1.000_001` で pass | Ok(mesh) |
| T_BOUNDARY_above_tolerance_arc | 境界 | Arc | 同上 | Ok(mesh) |
| T_BOUNDARY_above_tolerance_ellipse_major | 境界 | Ellipse major | 同上 | Ok(mesh) |
| T_BOUNDARY_above_tolerance_ellipse_minor | 境界 | Ellipse minor | 同上 | Ok(mesh) |
| T_BOUNDARY_exact_tolerance_circle | 境界 (退化) | Circle | `radius = LENGTH_TOLERANCE` で reject | DegenerateSketchElement |
| T_BOUNDARY_exact_tolerance_arc | 境界 (退化) | Arc | 同上 | DegenerateSketchElement |
| T_BOUNDARY_exact_tolerance_ellipse_major | 境界 (退化) | Ellipse major | 同上 | DegenerateSketchElement |
| T_BOUNDARY_exact_tolerance_ellipse_minor | 境界 (退化) | Ellipse minor | 同上 | DegenerateSketchElement |
| T_DEG_below_tolerance | 退化 | 4 形状全て | `radius = TOL / 2.0` で reject (既存挙動回帰) | DegenerateSketchElement |
| T_EDGE_length_tolerance_boundary | 境界 (既存反転) | Circle | `radius == TOL` 既存テストを反転 | reject へ反転 |

退化/境界 ID 命名は `T_DEG_*` / `T_BOUNDARY_*` (3ai 規約)。

## 幾何的不変条件チェックリスト

- [N/A] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか
- [N/A] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか
- [N/A] flip_normals / same_sense の意味論が明確か
- [N/A] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか

(本 Issue は退化判定境界の比較演算子変更のみで Boolean/Partition/Assemble 系は触らないため全項目 N/A)

## ADR-018 draft の要点 (Issue 内で別途生成)

- **Title**: LENGTH_TOLERANCE 比較規約: `<=` で退化判定 standard
- **Status**: Proposed
- **Context**: kernel 内に `<` strict と `<=` inclusive の 2 流派が共存していた (#275 codex-7.5 で発覚)
- **Decision**: 全 LENGTH_TOLERANCE 退化判定を `<=` inclusive に統一する
- **Rationale**: `length_near()` の `<= TOL` 規約 / 多数派整合 (extrusion/booleans) / `value <= TOL ↔ value == 0` の意味的同値性
- **Related**: ADR-004 (tolerance 規約), ADR-017 (Phase 10 sketch)
- **Migration**: 既存 `T_EDGE_length_tolerance_boundary` テストの assertion 反転を伴う (一過性、Phase 10 内で吸収)
