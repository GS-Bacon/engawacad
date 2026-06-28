//! #273: Phase 10 Circle / Arc — golden YAML acceptance tests.
//!
//! T_GOLDEN_circle_yaml / T_GOLDEN_arc_yaml
//! F13: Legacy compat test removed — Document::from_yaml migration path is tested in document.rs

use engawa_format::SketchElement;

#[test]
fn t_golden_circle_yaml() {
    let original = SketchElement::Circle {
        id: "circle_test".to_string(),
        center: [5.0, 7.5],
        radius: 3.25,
    };
    let yaml = serde_yaml::to_string(&original).expect("serialize must succeed");
    // yaml が "kind: circle" を含むこと
    assert!(
        yaml.contains("kind: circle"),
        "yaml must be tagged: {}",
        yaml
    );
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

#[test]
fn t_golden_arc_yaml() {
    let original = SketchElement::Arc {
        id: "arc_test".to_string(),
        center: [0.0, 0.0],
        radius: 2.0,
        start_angle: 0.0,
        end_angle: std::f64::consts::FRAC_PI_2,
    };
    let yaml = serde_yaml::to_string(&original).expect("serialize must succeed");
    assert!(yaml.contains("kind: arc"), "yaml must be tagged: {}", yaml);
    let parsed: SketchElement = serde_yaml::from_str(&yaml).expect("round-trip parse must succeed");
    match parsed {
        SketchElement::Arc {
            id,
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            assert_eq!(id, "arc_test");
            assert_eq!(center, [0.0, 0.0]);
            assert_eq!(radius, 2.0);
            assert_eq!(start_angle, 0.0);
            assert_eq!(end_angle, std::f64::consts::FRAC_PI_2);
        }
        other => panic!("expected Arc variant, got {:?}", other),
    }
}

// === エッジケース・数値境界テスト（敵対ペルソナ追加） ===

#[test]
fn t_edge_nan_radius_deserialize() {
    // NaN radius を含む YAML は deserialize 自体は成功 (YAML は purely structural)
    let yaml = r#"
- kind: circle
  id: nan_circle
  center: [0.0, 0.0]
  radius: .nan
"#;
    let elements: Vec<SketchElement> = serde_yaml::from_str(yaml).expect("YAML parse must succeed");
    match &elements[0] {
        SketchElement::Circle { id, radius, .. } => {
            assert_eq!(id, "nan_circle");
            assert!(radius.is_nan(), "radius must be NaN");
        }
        other => panic!("expected Circle variant, got {:?}", other),
    }
}

#[test]
fn t_edge_inf_radius_deserialize() {
    // Inf radius を含む YAML は deserialize 自体は成功
    let yaml = r#"
- kind: circle
  id: inf_circle
  center: [0.0, 0.0]
  radius: .inf
"#;
    let elements: Vec<SketchElement> = serde_yaml::from_str(yaml).expect("YAML parse must succeed");
    match &elements[0] {
        SketchElement::Circle { id, radius, .. } => {
            assert_eq!(id, "inf_circle");
            assert!(radius.is_infinite(), "radius must be infinite");
        }
        other => panic!("expected Circle variant, got {:?}", other),
    }
}

#[test]
fn t_edge_negative_zero_radius() {
    // -0.0 radius は deserialize 成功（tessellate 時に退化判定されるはず）
    let yaml = r#"
- kind: circle
  id: neg_zero_circle
  center: [0.0, 0.0]
  radius: -0.0
"#;
    let elements: Vec<SketchElement> = serde_yaml::from_str(yaml).expect("YAML parse must succeed");
    match &elements[0] {
        SketchElement::Circle { id, radius, .. } => {
            assert_eq!(id, "neg_zero_circle");
            // -0.0 == 0.0 は true だが sign bit は異なる
            assert_eq!(*radius, 0.0);
            assert!(radius.is_sign_negative(), "should be negative zero");
        }
        other => panic!("expected Circle variant, got {:?}", other),
    }
}

#[test]
fn t_edge_max_value_coordinates() {
    // f64::MAX の座標値を含む YAML は deserialize 成功
    let yaml = format!(
        r#"
- kind: line
  id: max_line
  from: [{}, {}]
  to: [-{}, -{}]
"#,
        f64::MAX,
        f64::MAX,
        f64::MAX,
        f64::MAX
    );
    let elements: Vec<SketchElement> =
        serde_yaml::from_str(&yaml).expect("YAML parse must succeed");
    match &elements[0] {
        SketchElement::Line { id, from, to } => {
            assert_eq!(id, "max_line");
            assert_eq!(from[0], f64::MAX);
            assert_eq!(from[1], f64::MAX);
            assert_eq!(to[0], -f64::MAX);
            assert_eq!(to[1], -f64::MAX);
        }
        other => panic!("expected Line variant, got {:?}", other),
    }
}

#[test]
fn t_edge_min_positive_radius() {
    // f64::MIN_POSITIVE radius は deserialize 成功（tessellate 時に退化判定されるはず）
    let yaml = format!(
        r#"
- kind: circle
  id: tiny_circle
  center: [0.0, 0.0]
  radius: {}
"#,
        f64::MIN_POSITIVE
    );
    let elements: Vec<SketchElement> =
        serde_yaml::from_str(&yaml).expect("YAML parse must succeed");
    match &elements[0] {
        SketchElement::Circle { id, radius, .. } => {
            assert_eq!(id, "tiny_circle");
            assert_eq!(*radius, f64::MIN_POSITIVE);
        }
        _other => panic!("expected Circle variant, got {:?}", _other),
    }
}

#[test]
fn t_determinism_circle_100_calls() {
    // Circle を100回 serialize → deserialize して全結果が一致するか
    let original = SketchElement::Circle {
        id: "det_circle".to_string(),
        center: [3.5, 4.25],
        radius: 2.75,
    };
    let yaml = serde_yaml::to_string(&original).expect("serialize must succeed");
    for _ in 0..100 {
        let parsed: SketchElement = serde_yaml::from_str(&yaml).expect("parse must succeed");
        match parsed {
            SketchElement::Circle { id, center, radius } => {
                assert_eq!(id, "det_circle");
                assert_eq!(center, [3.5, 4.25]);
                assert_eq!(radius, 2.75);
            }
            _other => panic!("expected Circle variant on iteration"),
        }
    }
}

#[test]
fn t_determinism_arc_100_calls() {
    // Arc を100回 serialize → deserialize して全結果が一致するか
    let original = SketchElement::Arc {
        id: "det_arc".to_string(),
        center: [1.0, 2.0],
        radius: 1.5,
        start_angle: 0.25,
        end_angle: 2.5,
    };
    let yaml = serde_yaml::to_string(&original).expect("serialize must succeed");
    for _ in 0..100 {
        let parsed: SketchElement = serde_yaml::from_str(&yaml).expect("parse must succeed");
        match parsed {
            SketchElement::Arc {
                id,
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                assert_eq!(id, "det_arc");
                assert_eq!(center, [1.0, 2.0]);
                assert_eq!(radius, 1.5);
                assert_eq!(start_angle, 0.25);
                assert_eq!(end_angle, 2.5);
            }
            _other => panic!("expected Arc variant on iteration"),
        }
    }
}

#[test]
fn t_roundtrip_full_circle_arc() {
    // 全周回 Arc (0 → 2π) と Circle の round-trip 一致性
    let circle = SketchElement::Circle {
        id: "full_circle".to_string(),
        center: [0.0, 0.0],
        radius: 1.0,
    };
    let arc_full = SketchElement::Arc {
        id: "full_arc".to_string(),
        center: [0.0, 0.0],
        radius: 1.0,
        start_angle: 0.0,
        end_angle: std::f64::consts::TAU,
    };
    let circle_yaml = serde_yaml::to_string(&circle).expect("serialize circle");
    let arc_yaml = serde_yaml::to_string(&arc_full).expect("serialize arc");
    // 両方とも round-trip 成功
    let circle_rt: SketchElement = serde_yaml::from_str(&circle_yaml).expect("round-trip circle");
    let arc_rt: SketchElement = serde_yaml::from_str(&arc_yaml).expect("round-trip arc");
    match circle_rt {
        SketchElement::Circle { id, center, radius } => {
            assert_eq!(id, "full_circle");
            assert_eq!(center, [0.0, 0.0]);
            assert_eq!(radius, 1.0);
        }
        _other => panic!("expected Circle variant"),
    }
    match arc_rt {
        SketchElement::Arc {
            id,
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            assert_eq!(id, "full_arc");
            assert_eq!(center, [0.0, 0.0]);
            assert_eq!(radius, 1.0);
            assert_eq!(start_angle, 0.0);
            assert_eq!(end_angle, std::f64::consts::TAU);
        }
        _other => panic!("expected Arc variant"),
    }
}
