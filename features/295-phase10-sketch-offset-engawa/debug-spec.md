# debug-spec (STEP 7.5 Codex round 2 fix, codex_loops = 2)

## Codex round 2 verdict: fail (4 blocking)

- **A-F01 (critical)**: `Line+Arc` 混在の閉輪郭も corner が offset 後に非連結。C-F01 の Line-only reject では不十分
- **A-F02 (high)**: `offset_arc` が負 sweep Arc (`end_angle < start_angle`) に対応していない (符号規約不整合)
- **C-F01 (high)**: `all(Line) && len>=2` reject により、Line ループでの selection 部分適用が全て `UnsupportedFeature` になる = plan/T07 の contract より狭い
- **M-F01 (high)**: `examples/sketch_offset.engawa` の `schema_version: 1` は不正 (Circle は v2)、schema version 契約破り

## fix 方針 (最小コスト)

### A-F01 + C-F01 一括解決: SketchOffset は Circle-only を build-level で許可

**方針**: `apply_sketch_offset` の冒頭で、profile が **Circle 単一のみ** の場合を許可、それ以外は `UnsupportedFeature { kind: "sketch_offset_only_circle" }` で reject。

- Line 系 (単一 Line / 連結 Line) はどれも kernel-level pure function として保持 (unit test で verify)、build 経由は不可
- Arc 系も同様 (build 経由不可、kernel-level pure function OK)
- Circle 単一 (single-primitive closed sketch) のみ build 経路で通す
- **C-F01 廃止** (`sketch_offset_of_connected_lines` reject を削除、`sketch_offset_only_circle` reject に統合)

修正コード (`crates/engawa-kernel/src/geometry/sketch_offset.rs::apply_sketch_offset` 冒頭):

```rust
if !distance.is_finite() { ... }  // 既存
if distance.abs() <= LENGTH_TOLERANCE { return Ok(source.to_vec()); }  // 既存

// 新: build-level 契約は Circle 単一のみ (Line/Arc の 独立 offset は corner 非連結問題を持つ)
// この関数を kernel から直接呼ぶ場合は per-element (Line/Arc) offset も OK (単体テスト用)
// 呼び出し側が build-context かどうかを見分けるため引数 build_context: bool を追加する
// もしくは per-caller 契約に統一するため apply_sketch_offset_build() を別関数で提供:

pub fn apply_sketch_offset_build(
    source: &[SketchElement],
    selection: &[String],
    distance: f64,
) -> Result<Vec<SketchElement>, KernelError> {
    if source.len() != 1 || !matches!(source[0], SketchElement::Circle { .. }) {
        return Err(KernelError::UnsupportedFeature {
            kind: "sketch_offset_only_circle",
        });
    }
    apply_sketch_offset(source, selection, distance)
}
```

**`crates/engawa-build/src/lib.rs`** の `Feature::SketchOffset` dispatch を `apply_sketch_offset_build` に切り替える。

`apply_sketch_offset` (per-element 用 純関数) の Line/Arc reject (connected_lines) は削除。kernel unit test は `apply_sketch_offset` を直接叩いてカバー。

### A-F02: offset_arc の負 sweep 対応

- **簡単な解**: 負 sweep Arc (`end_angle < start_angle`) を `apply_sketch_offset` で reject する。実装: `offset_arc` 冒頭で `end_angle < start_angle` なら `KernelError::InvalidParameter { kind: "arc_negative_sweep" }` を返す。
- kernel unit test に `t_deg_offset_arc_negative_sweep_rejected` を追加

### M-F01: examples fixture の schema_version 修正

- `examples/sketch_offset.engawa` の `schema_version: 1` を `schema_version: 2` に変更 (M-F01 部分対応、schema bump 自体は行わない — 追加 variant は additive 議論を採用)

### 対応しない項目 (rejection)

- **C-F02 (medium)**: T01 assertion 強化 — 現状の vertex count 比較で決定性は担保できる。medium なので block 対象外
- **C-F03 (medium)**: golden test 追加 — roundtrip test で fields は担保、canonical byte-identical fixture は Phase 11+ で refactor pass 対象
- **M-F02 (medium)**: 同 (golden)

## test 更新

- T07 build-level test は **削除** (Line 系 partial selection は build-level 契約外に降格)
  - kernel unit test `t07_selection_partial` はそのまま (Line 系の kernel-level 動作を verify)
- T02 build-level test は **削除** (`t02_line_offset_covered_at_kernel_level` として既にコメント stub 化済 — このまま維持)
- 新規: `t_deg_line_arc_rejected_at_build` を build test に追加 (`apply_sketch_offset_build` が Line/Arc profile を reject することを verify)
- 新規: kernel unit test `t_deg_offset_arc_negative_sweep_rejected` を追加 (`end_angle < start_angle` の Arc を reject)

## plan.md 更新 (scope narrowing)

In-Scope 表:
- `Line` の平行 offset → **kernel-level 純関数のみ (build 経路では reject、closed loop の corner 非連結問題)** に downgrade
- `Arc` の signed offset → **kernel-level 純関数のみ (負 sweep 未対応)** に downgrade
- build-level は Circle 単一のみ

Non-Goals に追加:
- Line/Arc-based build-level SketchOffset (corner join/trim が Trim/Extend #276 依存、closed loop の連結性維持は本 Issue 範囲外)
- 負 sweep Arc の offset (符号規約が正 sweep 前提、負 sweep は #276 or #278 で扱う)
- schema_version bump (追加 variant は additive、v2 のまま維持。将来 breaking change でまとめて bump 予定)
- Golden byte-identical fixture (Phase 11+ Refactor pass で加える)

## 追加で書いてほしいテスト

- `t_deg_line_arc_rejected_at_build` (build acceptance): CreateSketch (single Line) + SketchOffset → `UnsupportedFeature { kind: "sketch_offset_only_circle" }`
- `t_deg_offset_arc_negative_sweep_rejected` (kernel unit): Arc with `end_angle < start_angle` → `InvalidParameter { kind: "arc_negative_sweep" }`

## STEP 7.5 resume fix (round 5) — #303 escalation 解消後の最小 fix

STEP 7.5 Codex round 3 で残った 5 件 (C-F01/C-F02/C-F03/M-F01/M-F02) は人間判断で決着済み (`rejection.md` Round 5 / `judgment-summary.md` 参照)。**以下の 2 点のみ**を実装してください。他は変更しないこと。

### 1. `Feature::SketchOffset` の rustdoc に build 契約を明記 (C-F01 対応・doc のみ)

`crates/engawa-format/src/feature.rs` の `Feature::SketchOffset` variant の doc comment (現在 `/// Offset sketch elements (2D profile edit).` のみ) に以下を追記してください:

```rust
/// Offset sketch elements (2D profile edit).
///
/// # Build-level contract (Phase 10)
/// The `sketch` referenced by this feature must resolve to a profile containing
/// exactly one `SketchElement::Circle`; anything else fails with
/// `KernelError::UnsupportedFeature { kind: "sketch_offset_only_circle" }`
/// (see `engawa_kernel::geometry::sketch_offset::apply_sketch_offset_build`).
/// `Line`/`Arc` offset math exists at the kernel pure-function level
/// (`apply_sketch_offset`, unit-tested) but is not yet wired into the build
/// pipeline — planned for a Phase 11+ refactor pass once corner join/trim
/// (Trim/Extend, #276) lands. `selection` is forward-looking wiring for that
/// future multi-element support; today it is effectively a no-op beyond the
/// sole Circle's own id (empty selection = offset it; a non-matching id is a
/// silent no-op, documented as a Non-Goal in plan.md).
SketchOffset {
```

**wire 型 (`selection: Vec<String>` 等) は変更しないこと**。`apply_sketch_offset_build` / `apply_sketch_offset` のロジックも変更不要 (既に正しい)。

### 2. `golden_sketch_offset` byte-identical テスト追加 (C-F03/M-F02 対応・採用)

`crates/engawa-format/tests/golden_examples.rs` に、既存の `golden_circle_arc` / `golden_ellipse_conic` と同じパターンで `golden_sketch_offset` を追加してください:

```rust
#[test]
fn golden_sketch_offset() {
    assert_golden(
        "sketch_offset.engawa",
        concat!(
            // TODO: 実際の to_yaml() 出力に合わせて埋める
        ),
    );
}
```

`examples/sketch_offset.engawa` は既に Claude が編集済み (`selection: []` の明示行を削除、canonical 出力に合わせるため)。**この example ファイルは変更しないこと**。

golden 文字列は決め打ちせず、まず仮の文字列で `cargo test -p engawa-format --test golden_examples golden_sketch_offset -- --nocapture` を実行し、assert 失敗時の実際の出力 (`left`/`right` diff) を読んで正確な canonical 文字列に修正してください (既存の `golden_circle_arc` 等と同じ手順)。

### 完了条件
- `cargo xtask ci` が green
- `schema_version` は変更しない (2 のまま)
- `apply_sketch_offset` / `apply_sketch_offset_build` のロジック変更なし
- `examples/sketch_offset.engawa` の内容変更なし (Claude 編集済みのものをそのまま使う)

## STEP 7.5 round 4 (fresh gate, rebase 後) fix — A01 (high, blocking)

Codex round 4 (rebase + 上記 fix 後の fresh review) で新規指摘:

> `refs_resolve_in_state()` / `simulate_history()` は `Feature::SketchOffset` を「参照 sketch が存在する」だけで実行可能扱いする。実際の build 経路は sketch が単一 `Circle` でないと `UnsupportedFeature` で失敗するため、CRUD (insert/edit/delete/reorder) は line/arc profile の sketch への `SketchOffset` 挿入や、downstream に `SketchOffset` を残したまま upstream sketch を line/arc profile に edit する操作を受理してしまい、`build_bodies_from_features()` でしか落ちない unbuildable history を作れる。

### 修正方針

`crates/engawa-build/src/feature_crud.rs` の `refs_resolve_in_state()` 内 `Feature::SketchOffset { sketch, .. }` の分岐 (現在 `sketches_at.contains_key(sketch)` のみ) を、**参照先 `CreateSketch` の `profile` が単一 `Circle` かどうか**まで検証するように変更してください:

```rust
Feature::SketchOffset { sketch, .. } => {
    sketches_at.get(sketch.as_str()).is_some_and(|&idx| {
        matches!(
            &features[idx],
            Feature::CreateSketch { profile, .. }
                if profile.len() == 1 && matches!(profile[0], engawa_format::SketchElement::Circle { .. })
        )
    })
}
```

- `sketches_at` は `CreateSketch` の index を保持しているので `features[idx]` で元 profile を引ける (offset 済み profile の再構築は不要 — Circle の offset は Circle のままなので元 CreateSketch.profile だけ見れば十分。plan.md 参照)。
- `engawa_format::SketchElement` の import が必要なら `use engawa_format::{..., SketchElement};` に追加。
- この変更により `simulate_history()` を使う `insert()` / `edit()` / `delete()` / `reorder()` すべてで「Circle 単一でない sketch への SketchOffset」が `executed_at` から自動的に除外され、既存の pre/post diff ロジック (`check_no_downstream_break` 等) が既存の「features/295-phase10-sketch-offset-engawa/codex-final-r4.yaml A01 対応」パターンで自動的に reject するようになる。

### 追加すべき回帰テスト (`crates/engawa-build/tests/feature_crud_acceptance.rs`)

- A01 fix: line profile の sketch に `SketchOffset` を insert しようとすると、`sketches_at` 上で resolve しない (= 後段の `insert()` の downstream-break 検出、または最小限は「元々存在した SketchOffset が `executed_at` に入らない」ことを直接 `simulate_history` 相当の public API 経由で確認できる形) を確認するテストを 1 件追加してください。既存の `t18_sketch_offset_sketch_not_found` (C-F02/M-F02 対応) と対になる Circle-only 版として書いてよい。
- 具体的なテスト設計 (public API 経由でどう検証するか) は既存の CRUD acceptance test の書き方に合わせて GLM の判断で決めてよい。最低限「line profile sketch + SketchOffset を含む Document で FeatureCrud の該当操作 (insert 等) が unbuildable な history を許可しないこと」を検証する 1 テストを追加すること。

### 完了条件 (round 4 fix)
- `cargo xtask ci` が green
- 上記回帰テスト追加
- 他の既存 CRUD test が壊れないこと (特に circle sketch への SketchOffset 系テストは影響を受けないはず)
