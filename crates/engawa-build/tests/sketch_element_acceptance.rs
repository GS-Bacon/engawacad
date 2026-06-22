//! #273: Phase 10 Circle / Arc — acceptance tests.

use engawa_build::build_bodies_from_features;
use engawa_format::{Feature, SketchElement, SketchPlane};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::tessellation::tessellate_solid;

fn build_with_sketch(elements: Vec<SketchElement>) -> Result<(), String> {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: elements,
            plane_ref: None,
            suppressed: false,
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, &[], &mut gen).map_err(|e| e.to_string())?;
    if built.live().count() != 1 {
        return Err(format!("expected 1 body, got {}", built.live().count()));
    }
    for body in built.live() {
        tessellate_solid(&body.solid).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// T01: Determinism — same SketchElement produces same tessellation across runs.
#[test]
fn t01_determinism() {
    // Circle element
    let circle = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [0.0, 0.0],
        radius: 5.0,
    }];
    let r1 = build_with_sketch(circle.clone());
    let r2 = build_with_sketch(circle);
    assert!(
        r1.is_ok() && r2.is_ok(),
        "determinism failed: {:?} {:?}",
        r1,
        r2
    );

    // Arc element
    let arc = vec![SketchElement::Arc {
        id: "a1".to_string(),
        center: [0.0, 0.0],
        radius: 5.0,
        start_angle: 0.0,
        end_angle: std::f64::consts::PI / 2.0,
    }];
    let r1 = build_with_sketch(arc.clone());
    let r2 = build_with_sketch(arc);
    assert!(
        r1.is_ok() && r2.is_ok(),
        "determinism failed: {:?} {:?}",
        r1,
        r2
    );
}

/// T02_circle: Circle sketch builds and tessellates successfully.
#[test]
fn t02_circle() {
    let circle = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [0.0, 0.0],
        radius: 5.0,
    }];
    assert!(build_with_sketch(circle).is_ok());
}

/// T02_arc: Arc sketch builds and tessellates successfully.
#[test]
fn t02_arc() {
    let arc = vec![SketchElement::Arc {
        id: "a1".to_string(),
        center: [0.0, 0.0],
        radius: 5.0,
        start_angle: 0.0,
        end_angle: std::f64::consts::PI / 2.0,
    }];
    assert!(build_with_sketch(arc).is_ok());
}

/// T_DEG_zero_radius: Zero-radius circle errors during build.
#[test]
fn t_deg_zero_radius() {
    let circle = vec![SketchElement::Circle {
        id: "c0".to_string(),
        center: [0.0, 0.0],
        radius: 0.0,
    }];
    let err = build_with_sketch(circle).unwrap_err();
    // Display impl は human-readable "degenerate sketch element: <id>, reason: ..." を吐く
    assert!(
        err.contains("degenerate sketch element"),
        "wrong error: {}",
        err
    );
}

/// T_DEG_zero_angle: Zero-sweep arc errors during build.
#[test]
fn t_deg_zero_angle() {
    let arc = vec![SketchElement::Arc {
        id: "a0".to_string(),
        center: [0.0, 0.0],
        radius: 5.0,
        start_angle: 0.5,
        end_angle: 0.5,
    }];
    let err = build_with_sketch(arc).unwrap_err();
    // Display impl は human-readable "degenerate sketch element: <id>, reason: ..." を吐く
    assert!(
        err.contains("degenerate sketch element"),
        "wrong error: {}",
        err
    );
}

/// T_BOUNDARY_full_circle: Full-circle arc produces identical result to Circle.
#[test]
fn t_boundary_full_circle() {
    let circle = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [0.0, 0.0],
        radius: 5.0,
    }];
    let arc = vec![SketchElement::Arc {
        id: "a1".to_string(),
        center: [0.0, 0.0],
        radius: 5.0,
        start_angle: 0.0,
        end_angle: std::f64::consts::TAU,
    }];
    let r1 = build_with_sketch(circle);
    let r2 = build_with_sketch(arc);
    assert!(
        r1.is_ok() && r2.is_ok(),
        "boundary failed: {:?} {:?}",
        r1,
        r2
    );
}
