# #288 debug-spec — Codex 7.5 round 1 指摘の修正

## Codex 指摘 (round 1) — critical/high

### A-F02 / M-F01 (high): Arc 全周が closed 判定から漏れている

`crates/engawa-build/src/lib.rs:597-612` の `is_closed_primitive` は `Arc` を
常に open 扱いするため、sweep が `2π` 近傍 (整数倍) の Arc を含む multi-element
profile が guard を素通りする:

- `[Arc(start=0, end=2π), Line]` → guard 通過 → flat_map で 1 本の壊れた polyline
- `[Arc(start=0, end=2π), Arc(start=0, end=2π)]` → guard 通過 → 同上

これは Issue #288 で防ぎたい「閉曲線を flat_map で潰す」不整合のサブセット。

### A-F01 / C-F01 (critical, 受容)

「git diff にテストが含まれていない」は Codex review が `git diff base...HEAD`
を見るため、未 commit / 未追跡の test file が映らない構造的アーティファクト。
本サイクル後の中間 commit で diff に乗せて round 2 で解消する想定。

### C-F02 (medium, 受容)

T05 が pre-fix でも downstream luck で同 error variant を返すため call site
固定として弱い指摘。T06_mid_closed が中央 closed のリグレッション guard と
して機能しているため受容。GLM 再実装は行わない。

## 修正方針

### 1. `is_closed_primitive` の Arc 判定追加

`Arc { start_angle, end_angle, .. }` を以下のロジックで closed 扱いにする:

```rust
SketchElement::Arc { start_angle, end_angle, .. } => {
    use std::f64::consts::TAU;
    use engawa_kernel::ANGLE_TOLERANCE;
    let sweep = (end_angle - start_angle).abs();
    // sweep が 2π の正整数倍に近い場合は full-circle 相当として closed 扱い
    let n = (sweep / TAU).round();
    n >= 1.0 && (sweep - n * TAU).abs() <= ANGLE_TOLERANCE
}
```

注意点:
- `ANGLE_TOLERANCE = 1e-9` (`engawa_kernel::geometry::math` で定義済、`engawa_kernel`
  ルートから re-export 済 — `crates/engawa-kernel/src/lib.rs:14` 参照)
- `engawa_kernel` は既に `engawa-build/Cargo.toml` の依存に含まれているはず
  (現行コードが `engawa_kernel::tessellation::sketch::tessellate_sketch_element`
   を呼び出しているため依存済み)
- `n >= 1.0` の整数判定は `(sweep / TAU).round()` の値が正である必要がある。
  `sweep = 0` (角度差ゼロ) は既存 `tessellate_sketch_element` が
  `DegenerateSketchElement` で reject するため guard では false で OK。
- `match` を網羅的にすることで Phase 11+ で新 variant が追加されたとき
  compile-time に検出できるようにする (Claude self-review M-01 の対処も同時に
  行う、追加修正コスト 0):

```rust
fn is_closed_primitive(elem: &engawa_format::SketchElement) -> bool {
    use engawa_format::SketchElement;
    use std::f64::consts::TAU;
    use engawa_kernel::ANGLE_TOLERANCE;
    match elem {
        SketchElement::Circle { .. }
        | SketchElement::Ellipse { .. }
        | SketchElement::Conic { .. } => true,
        SketchElement::Arc {
            start_angle,
            end_angle,
            ..
        } => {
            let sweep = (end_angle - start_angle).abs();
            let n = (sweep / TAU).round();
            n >= 1.0 && (sweep - n * TAU).abs() <= ANGLE_TOLERANCE
        }
        SketchElement::Line { .. } => false,
    }
}
```

### 2. 回帰テスト追加 (`tests/create_sketch_closed_acceptance.rs`)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T10_full_arc_rejected | 境界 | `[Arc(0, 2π), Line]` → reject | `Err(InvalidParameter { kind: "profile" })` |
| T11_two_full_arcs_rejected | 境界 | `[Arc(0, 2π), Arc(0, 2π)]` → reject | `Err(InvalidParameter { kind: "profile" })` |
| T12_partial_arc_ok | 正常系 | `[Arc(0, π/2), Line]` (sweep < 2π) → guard 通過 | guard では `InvalidParameter { kind: "profile" }` 以外 |
| T13_negative_full_arc_rejected | 境界 | `[Arc(2π, 0), Line]` (逆方向 sweep) → reject (`abs(sweep) == 2π`) | `Err(InvalidParameter { kind: "profile" })` |

T13 は `sweep = (end_angle - start_angle).abs()` の abs 処理を担保する regression。

### 3. CI green

`cargo xtask ci` が green であることを確認する (既存テスト含む)。

## 完了条件 (core モード)

1. `is_closed_primitive` を Arc 対応かつ exhaustive match に書き換える
2. T10-T13 の 4 テストを `tests/create_sketch_closed_acceptance.rs` に追加する
3. `cargo xtask ci` が green
4. `glm-result.json` を success で書き出す
