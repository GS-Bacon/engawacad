# Test Spec — Issue #53 (テッセレーション穴面頂点過密バグ)

## 不足テスト（plan 計画分）

T01〜T04 は `crates/mycad-build/tests/hole_tessellation_acceptance.rs` に GLM+linter が実装済み。
T05 は `crates/mycad-kernel/src/geometry/math.rs` に GLM が inline で実装済み（5 ケース: 全円=32、四分円=8、1/64弧=1、ゼロスパン=1、決定性×100回）。

計画全 ID の実装状況:
| ID | 実装状況 |
|----|---------|
| T01 | ✅ 実装済み（hole_tessellation_acceptance.rs） |
| T02 | ✅ 実装済み（< 400 閾値） |
| T03 | ✅ 実装済み（閾値は plan の 80 → 130 に変更、乖離あり: 下記参照） |
| T04 | ✅ 実装済み（ε_area = LENGTH_TOLERANCE²） |
| T05 | ✅ 実装済み（math.rs inline、5 関数） |

## 期待値乖離

**T03 閾値**: plan は `<= 80`、実装は `<= 130`。

理由: `z ≈ 5.0` の頂点には、annular top face の分（外周4 + 内周64 = 68）に加え、
円柱側壁面の上辺リム頂点（各面独立の頂点セット、≤64）が含まれるため。
合計上限は ~132。130 は正当な閾値で、バグ時 2093 と比較して充分に絞れている。

乖離判定: **許容**（より精密な上界。CI green 確認済み）。plan 修正は不要。

## 実装差分から追加すべきテスト

### 追加済み（GLM が実装）
- `t05_arc_segment_count_determinism`: 100回ループで決定性を確認

### 追加推奨（今後のリグレッション防止）
- **既存テスト t14b の更新確認**: `t14b_circle_pcurve_n_points` が旧動作（32 pts）から新動作（21 pts）に更新されているか → GLM run2 で更新済み。
- **cylinder wall top-rim count**: cylindrical side face の上辺に ≤64 頂点が存在することの確認テストは今回 Out-of-Scope（issue #53 が annular face のバグ修正に限定）。

## エッジケース・退化入力

plan の `arc_segment_count(span=0, 32) = 1` (T05) がゼロスパンをカバーしており実装済み。
追加で注意が必要なケース:
- `base_segments = 0`: `arc_segment_count` は `0.max(1) = 1` を返す（`n.max(1)` で保護済み）。

## 数値境界

- 全周 (2π): base_segments 点（現状維持確認済み）
- 1/64 弧 × 64 エッジ: 各 1 点 → フチ合計 64 点（T03 で間接的に保証）

## 決定性

T01（2回実行で positions/indices 完全一致）と T05 の 100 回ループが担保。
`arc_segment_count` は純粋関数（同一入力→同一出力）で IdGenerator 非依存。
