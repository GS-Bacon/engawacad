# Test spec — #273 Phase 10 Circle / Arc

## 不足テスト (plan 計画分)

`crates/engawa-format/tests/sketch_element_golden_acceptance.rs` の以下 3 件が `todo!()` のまま残っている。STEP 6.6 で GLM が実装する。

### T_GOLDEN_legacy_compat

**目的**: legacy YAML (`{id, from, to}`、`kind` 欠落) を deserialize した結果が `SketchElement::Line` として読めることを保証。ADR-017 §4 後方互換の核。

**実装**:
```rust
#[test]
fn t_golden_legacy_compat() {
    // legacy YAML: kind フィールドなし、from/to のみ
    let yaml = r#"
- id: legacy_seg
  from: [0.0, 0.0]
  to: [10.0, 0.0]
"#;
    let elements: Vec<SketchElement> = serde_yaml::from_str(yaml).expect("legacy YAML must parse");
    assert_eq!(elements.len(), 1);
    match &elements[0] {
        SketchElement::Line { id, from, to } => {
            assert_eq!(id, "legacy_seg");
            assert_eq!(*from, [0.0, 0.0]);
            assert_eq!(*to, [10.0, 0.0]);
        }
        other => panic!("expected Line variant, got {:?}", other),
    }
}
```

### T_GOLDEN_circle_yaml

**目的**: tagged Circle YAML (`{kind: circle, ...}`) を serialize → deserialize round-trip した結果が元の値と一致することを保証。

**実装**:
```rust
#[test]
fn t_golden_circle_yaml() {
    let original = SketchElement::Circle {
        id: "circle_test".to_string(),
        center: [5.0, 7.5],
        radius: 3.25,
    };
    let yaml = serde_yaml::to_string(&original).expect("serialize must succeed");
    // yaml が "kind: circle" を含むこと
    assert!(yaml.contains("kind: circle"), "yaml must be tagged: {}", yaml);
    // round-trip
    let parsed: SketchElement = serde_yaml::from_str(&yaml).expect("round-trip parse must succeed");
    match parsed {
        SketchElement::Circle { id, center, radius } => {
            assert_eq!(id, "circle_test");
            assert_eq!(center, [5.0, 7.5]);
            assert_eq!(radius, 3.25);
        }
        other => panic!("expected Circle variant, got {:?}", other),
    }
}
```

### T_GOLDEN_arc_yaml

**目的**: tagged Arc YAML (`{kind: arc, ...}`) を serialize → deserialize round-trip した結果が元の値と一致することを保証。

**実装**:
```rust
#[test]
fn t_golden_arc_yaml() {
    let original = SketchElement::Arc {
        id: "arc_test".to_string(),
        center: [0.0, 0.0],
        radius: 2.0,
        start_angle: 0.0,
        end_angle: std::f64::consts::FRAC_PI_2,  // π/2
    };
    let yaml = serde_yaml::to_string(&original).expect("serialize must succeed");
    assert!(yaml.contains("kind: arc"), "yaml must be tagged: {}", yaml);
    let parsed: SketchElement = serde_yaml::from_str(&yaml).expect("round-trip parse must succeed");
    match parsed {
        SketchElement::Arc { id, center, radius, start_angle, end_angle } => {
            assert_eq!(id, "arc_test");
            assert_eq!(center, [0.0, 0.0]);
            assert_eq!(radius, 2.0);
            assert_eq!(start_angle, 0.0);
            assert_eq!(end_angle, std::f64::consts::FRAC_PI_2);
        }
        other => panic!("expected Arc variant, got {:?}", other),
    }
}
```

### 実装場所

`crates/engawa-format/tests/sketch_element_golden_acceptance.rs`

- 既存 `#[ignore]` + `todo!()` を 3 件とも置き換える
- `use engawa_format::SketchElement;` を file 先頭に追加
- `serde_yaml` は engawa-format の dev-dependencies に既に入っているはず (golden_examples.rs 等で使用中)

## 実装差分から追加すべきテスト

`git diff main..HEAD` で確認した実装差分から、plan.md 計画外で必要なテストは無い。GLM の実装内容:
- SketchElement enum (Line/Circle/Arc) — plan 通り
- `tessellate_sketch_element` — plan 通り (T01-T_BOUNDARY で網羅)
- `KernelError::DegenerateSketchElement` — plan 通り (T_DEG で網羅)
- custom Deserialize (legacy YAML 互換) — T_GOLDEN_legacy_compat で網羅
- dispatcher の Vec<SketchElement> 受領 — examples_smoke の circle_arc 関数で網羅

## エッジケース・退化入力

- T_DEG_zero_radius ✓ (実装済 — radius == 0.0)
- T_DEG_zero_angle ✓ (実装済 — start == end)

追加すべきエッジケースは無い (Phase 10 ε 値域は ADR-017 §2 でカバー済、本 Issue では Circle/Arc のみ)。

## 数値境界

- T_BOUNDARY_full_circle ✓ (Arc(0,2π) と Circle が同 polyline)

追加すべき数値境界は無い (Ellipse/Conic の axis_ratio / discriminant は #274 のスコープ)。

## 決定性

- T01 ✓ (実装済 — 2 回呼んで同一結果)

## 期待値乖離

GLM 実装と plan.md の期待値で乖離なし:
- `tessellate_sketch_element` API: `Result<Vec<[f64;2]>, KernelError>` — plan 通り
- 退化 reason 文字列: `"radius < ε_radius"` / `"|end_angle - start_angle| < ε_angle"` — plan 通り
- legacy YAML 受理 — plan 通り
- merged yaml に `kind: line` を emit — ADR-017 §1 通り (= 新 tagged format の canonical 出力)

STEP 6.6 は上記 3 件 (T_GOLDEN_legacy_compat / T_GOLDEN_circle_yaml / T_GOLDEN_arc_yaml) を埋めて `cargo xtask ci` green を維持するのが goal。
