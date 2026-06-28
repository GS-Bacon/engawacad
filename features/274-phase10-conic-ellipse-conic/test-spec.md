# Test Spec for #274 — Ellipse / Conic

## STEP 6 で実装されたテスト (再確認)

STEP 6 GLM core dispatch で以下のテストが既に実装され CI green。すべて plan.md のテスト計画 ID 表と対応する。

### inline tests (crates/engawa-kernel/src/tessellation/sketch.rs)

| plan ID | impl fn | 場所 |
|---------|---------|------|
| T01_determinism | `t01_ellipse_determinism` | sketch.rs:596 |
| T01b_conic_determinism | `t01b_conic_determinism` | sketch.rs:612 |
| T02_normal_ellipse_axis_aligned | `t02_normal_ellipse_axis_aligned` | sketch.rs:625 |
| T02b_normal_ellipse_rotated | `t02b_normal_ellipse_rotated` | sketch.rs:656 |
| T03_normal_conic_ellipse | `t03_normal_conic_ellipse` | sketch.rs:676 |
| T03b_normal_conic_hyperbola | `t03b_normal_conic_hyperbola` | sketch.rs:708 |
| T_DEG_zero_axis_major | `t_degenerate_zero_axis_major` | sketch.rs:744 |
| T_DEG_zero_axis_minor | `t_degenerate_zero_axis_minor` | sketch.rs:764 |
| T_DEG_circle_degeneracy | `t_boundary_circle_degeneracy` | sketch.rs:784 |
| T_DEG_axis_ratio | `t_degenerate_axis_ratio` | sketch.rs:807 |
| T_DEG_conic_degenerate | `t_degenerate_conic_degenerate` | sketch.rs:827 |
| T_DEG_conic_nan | `t_degenerate_conic_nan` | sketch.rs:845 |
| T_DEG_ellipse_nan_rotation | `t_degenerate_ellipse_nan_rotation` | sketch.rs:862 |

### inline tests (crates/engawa-format/src/feature.rs)

| plan ID | impl fn | 場所 |
|---------|---------|------|
| T_GOLDEN_yaml (ellipse) | `t_golden_ellipse_roundtrip` | feature.rs:1522 |
| T_GOLDEN_yaml (conic) | `t_golden_conic_roundtrip` | feature.rs:1561 |

### integration tests (crates/engawa-build/tests/examples_smoke.rs)

| plan ID | impl fn | 場所 |
|---------|---------|------|
| T_SMOKE_examples | `ellipse_conic` | examples_smoke.rs:186 |

## 不足テスト (plan 計画分)

全 plan T ID が既存実装でカバー済。追加実装不要。

## 実装差分から追加すべきテスト

GLM が実装した `tessellate_sketch_element` の Conic 主軸変換ロジック (固有値分解 + 平行移動) について、現状の T03 / T03b は楕円型・双曲型の各 1 ケースしか触らない。以下を追加検討:

- **対角項 B ≠ 0 の場合の楕円 conic 主軸推定** — 例 `coeffs=[1.0, 1.0, 1.0, 0.0, 0.0]` (= `x² + xy + y² = 1`、main axes at 45°)
- **D, E ≠ 0 の場合の平行移動 conic** — 例 `coeffs=[1.0, 0.0, 1.0, -2.0, -2.0]` (= `(x-1)² + (y-1)² = 3`)

これらは「期待値乖離」の埋蔵対象ではないが、主軸変換の数値正しさを増強する目的で**追加可能**。優先度は medium (Phase 11+ の adaptive sampling 評価時にあわせて拡張するのでも可)。

本 Issue では plan T ID 全カバー済を最優先とし、上記 2 件は **scope-defer** で記録 (post-merge follow-up Issue 起票候補)。

## エッジケース・退化入力

すべての退化ケース (T_DEG_*) は STEP 6 で実装済。追加不要。

## 数値境界

- `EPS_AXIS_RATIO = 1e-6` 境界: `T_DEG_axis_ratio` が `minor=1e-7` を退化扱いするテストでカバー
- `EPS_DISCRIMINANT = 1e-9` 境界: `T_DEG_conic_degenerate` が `discriminant = 0` のケースをカバー
- `LENGTH_TOLERANCE = 1e-9` 境界: `T_DEG_zero_axis_major` / `T_DEG_zero_axis_minor` が `0.0` ケースをカバー
- 追加不要 (border-just-above / just-below は本 Issue では Phase 11+ の adaptive sampling 評価時にあわせて拡張)

## 決定性

`T01_determinism` (Ellipse) と `T01b_conic_determinism` (Conic) の同一入力 2 回 tessellate → `assert_eq!` で bit 一致を確認済。100 calls 規模の long-run 決定性チェックは Phase 11+ の代表 Conic ケース選定後に追加 (本 Issue では同一 trace 2 回で十分)。

## 期待値乖離

`check-spec-divergence.ts` は `git diff main..HEAD` 比較に依存し、本リポジトリのデフォルトブランチ名 (`claude/add-claude-guidelines-BKKtD`) が異なるため自動 diff 取得不可。Claude 手動レビューで以下を確認済:

- plan.md T02 期待値: `LENGTH_TOLERANCE` 内で `(2,0)`, `(0,1)`, `(-2,0)`, `(0,-1)` を含む → 実装 `t02_normal_ellipse_axis_aligned` で対応の `assert!((pts[i][0] - 2.0).abs() < ...)` パターンが存在することを確認 (sketch.rs:625-654)
- plan.md T03 期待値: 全点で `|A x² + B xy + C y² + D x + E y - 1| < LENGTH_TOLERANCE * 100` → 実装 `t03_normal_conic_ellipse` で同等の conic 評価誤差許容 assertion を確認 (sketch.rs:676-707)
- 乖離なし。

## STEP 6.6 GLM dispatch 要否

**不要**。全 plan T ID 既存カバー、追加実装の必要性なし。scope-defer 項目 (対角項 B ≠ 0 / 平行移動 D,E ≠ 0) は post-merge follow-up とし、本 Issue では glm_impl を直接 passed にマークする。
