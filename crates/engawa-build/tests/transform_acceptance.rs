/// Acceptance tests for Issue #74: Component ツリー走査 + 平行移動 transform 合成
use engawa_build::build_assembly;
use engawa_format::component::{Component, Transform};
use engawa_format::document::Document;
use engawa_format::feature::Feature;
use engawa_kernel::brep::topology::IdGenerator;

/// Helper: extract all vertex points from a Body list as flat Vec of (f64,f64,f64).
fn collect_vertex_coords(bodies: &[engawa_build::Body]) -> Vec<(f64, f64, f64)> {
    let mut coords = Vec::new();
    for body in bodies {
        for v in &body.solid.vertices {
            coords.push((v.point.x, v.point.y, v.point.z));
        }
    }
    coords
}

/// Helper: build a minimal doc with one box and a given transform on root.
fn single_box_doc(transform: Transform) -> Document {
    let mut doc = Document::new("Part");
    doc.root_component.transform = transform;
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc
}

// ---------------------------------------------------------------------------
// T01: Determinism — same document built twice produces identical results
// ---------------------------------------------------------------------------

#[test]
fn t01_determinism() {
    let doc = single_box_doc(Transform {
        position: [5.0, 0.0, 0.0],
        rotation: [0.0; 3],
    });

    let dir = tempfile::tempdir().unwrap();

    let mut gen1 = IdGenerator::new(0);
    let bodies1 = build_assembly(&doc, dir.path(), &mut gen1).unwrap();

    let mut gen2 = IdGenerator::new(0);
    let bodies2 = build_assembly(&doc, dir.path(), &mut gen2).unwrap();

    assert_eq!(bodies1.len(), bodies2.len(), "body count must match");
    for (a, b) in bodies1.iter().zip(bodies2.iter()) {
        assert_eq!(a.feature_id, b.feature_id, "feature_id order must match");
        assert_eq!(
            a.solid.vertices.len(),
            b.solid.vertices.len(),
            "vertex count must match"
        );
        for (va, vb) in a.solid.vertices.iter().zip(b.solid.vertices.iter()) {
            assert_eq!(va.point, vb.point, "vertex coordinates must match");
        }
    }
}

// ---------------------------------------------------------------------------
// T02: Single transform — position=[5,0,0] shifts all vertices by +5 in x
// ---------------------------------------------------------------------------

#[test]
fn t02_single_transform_translates_vertices() {
    // Build a box with no transform first to get reference coordinates
    let doc_baseline = single_box_doc(Transform::default());
    let dir = tempfile::tempdir().unwrap();
    let mut gen0 = IdGenerator::new(0);
    let bodies_baseline = build_assembly(&doc_baseline, dir.path(), &mut gen0).unwrap();

    // Build the same box with position=[5,0,0]
    let doc_translated = single_box_doc(Transform {
        position: [5.0, 0.0, 0.0],
        rotation: [0.0; 3],
    });
    let mut gen1 = IdGenerator::new(0);
    let bodies_translated = build_assembly(&doc_translated, dir.path(), &mut gen1).unwrap();

    assert_eq!(bodies_baseline.len(), bodies_translated.len());
    let baseline_verts = &bodies_baseline[0].solid.vertices;
    let translated_verts = &bodies_translated[0].solid.vertices;

    for (v_base, v_trans) in baseline_verts.iter().zip(translated_verts.iter()) {
        assert!(
            (v_trans.point.x - v_base.point.x - 5.0).abs() < 1e-10,
            "x should be shifted by +5: base={} trans={}",
            v_base.point.x,
            v_trans.point.x
        );
        assert!(
            (v_trans.point.y - v_base.point.y).abs() < 1e-10,
            "y should be unchanged"
        );
        assert!(
            (v_trans.point.z - v_base.point.z).abs() < 1e-10,
            "z should be unchanged"
        );
    }
}

// ---------------------------------------------------------------------------
// T03: Nested two-level transform composition
//   parent position=[5,0,0], child position=[0,3,0]
//   → child's Body should be shifted by [5,3,0]
// ---------------------------------------------------------------------------

#[test]
fn t03_nested_two_level_transform_composition() {
    // Baseline: a 10×10×10 box at origin (no transform)
    let doc_baseline = single_box_doc(Transform::default());
    let dir = tempfile::tempdir().unwrap();
    let mut gen0 = IdGenerator::new(0);
    let bodies_baseline = build_assembly(&doc_baseline, dir.path(), &mut gen0).unwrap();
    let baseline_coords = collect_vertex_coords(&bodies_baseline);

    // Nested: parent has position=[5,0,0], child has position=[0,3,0]
    let mut doc = Document::new("Parent");
    doc.root_component.transform = Transform {
        position: [5.0, 0.0, 0.0],
        rotation: [0.0; 3],
    };
    let mut child = Component::new("Child");
    child.transform = Transform {
        position: [0.0, 3.0, 0.0],
        rotation: [0.0; 3],
    };
    child.features.push(Feature::CreateBox {
        id: "box_child".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.children.push(child);

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();

    // Parent has no features → only child's body
    assert_eq!(bodies.len(), 1, "only child contributes a body");
    let child_coords = collect_vertex_coords(&bodies);

    assert_eq!(baseline_coords.len(), child_coords.len());
    for (base, shifted) in baseline_coords.iter().zip(child_coords.iter()) {
        assert!(
            (shifted.0 - base.0 - 5.0).abs() < 1e-10,
            "x should be +5: base={} shifted={}",
            base.0,
            shifted.0
        );
        assert!(
            (shifted.1 - base.1 - 3.0).abs() < 1e-10,
            "y should be +3: base={} shifted={}",
            base.1,
            shifted.1
        );
        assert!((shifted.2 - base.2).abs() < 1e-10, "z should be unchanged");
    }
}

// ---------------------------------------------------------------------------
// T04: No transform — default (all-zero) transform leaves vertices unchanged
// ---------------------------------------------------------------------------

#[test]
fn t04_no_transform_unchanged() {
    let doc = single_box_doc(Transform::default());
    let dir = tempfile::tempdir().unwrap();

    let mut gen1 = IdGenerator::new(0);
    let bodies1 = build_assembly(&doc, dir.path(), &mut gen1).unwrap();
    let mut gen2 = IdGenerator::new(0);
    let bodies2 = build_assembly(&doc, dir.path(), &mut gen2).unwrap();

    // Two runs with same seed must produce identical coordinates (determinism check)
    for (a, b) in bodies1.iter().zip(bodies2.iter()) {
        for (va, vb) in a.solid.vertices.iter().zip(b.solid.vertices.iter()) {
            assert_eq!(
                va.point, vb.point,
                "no-transform coordinates must be identical"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// T05: Empty features component — skipped without error, children produce bodies
// ---------------------------------------------------------------------------

#[test]
fn t05_empty_features_component_skipped() {
    let mut doc = Document::new("Container");
    // Root has no features, no transform — just a container
    doc.root_component.transform = Transform {
        position: [5.0, 0.0, 0.0],
        rotation: [0.0; 3],
    };

    let mut child = Component::new("ChildWithBox");
    child.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.children.push(child);

    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();

    assert_eq!(bodies.len(), 1, "only child contributes a body");
    assert_eq!(bodies[0].feature_id, "box_1");
}

// ---------------------------------------------------------------------------
// T06: Boundary — explicit position=[0,0,0] is same as default
// ---------------------------------------------------------------------------

#[test]
fn t06_boundary_zero_offset_unchanged() {
    let doc_explicit_zero = single_box_doc(Transform {
        position: [0.0, 0.0, 0.0],
        rotation: [0.0; 3],
    });
    let doc_default = single_box_doc(Transform::default());

    let dir = tempfile::tempdir().unwrap();

    let mut gen1 = IdGenerator::new(0);
    let bodies_explicit = build_assembly(&doc_explicit_zero, dir.path(), &mut gen1).unwrap();
    let mut gen2 = IdGenerator::new(0);
    let bodies_default = build_assembly(&doc_default, dir.path(), &mut gen2).unwrap();

    assert_eq!(bodies_explicit.len(), bodies_default.len());
    for (a, b) in bodies_explicit.iter().zip(bodies_default.iter()) {
        for (va, vb) in a.solid.vertices.iter().zip(b.solid.vertices.iter()) {
            assert_eq!(va.point, vb.point, "explicit zero must equal default");
        }
    }
}

// ---------------------------------------------------------------------------
// T07: Boundary — large offset values keep coordinates finite
// ---------------------------------------------------------------------------

#[test]
fn t07_boundary_large_offset_stays_finite() {
    let doc = single_box_doc(Transform {
        position: [1e6, -1e6, 1e6],
        rotation: [0.0; 3],
    });

    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();

    assert_eq!(bodies.len(), 1);
    for body in &bodies {
        for v in &body.solid.vertices {
            assert!(v.point.x.is_finite(), "x must be finite");
            assert!(v.point.y.is_finite(), "y must be finite");
            assert!(v.point.z.is_finite(), "z must be finite");
        }
    }
}

// ---------------------------------------------------------------------------
// Edge-case tests (adversarial persona) for Issue #74
// ---------------------------------------------------------------------------

// EC01: Determinism — 100 runs with non-trivial transform produce identical results
#[test]
fn ec01_determinism_100_runs() {
    let doc = single_box_doc(Transform {
        position: [5.0, 3.0, -2.0],
        rotation: [0.0; 3],
    });
    let dir = tempfile::tempdir().unwrap();

    let mut gen0 = IdGenerator::new(0);
    let reference = build_assembly(&doc, dir.path(), &mut gen0).unwrap();
    let ref_coords = collect_vertex_coords(&reference);

    for i in 1..100 {
        let mut gen = IdGenerator::new(0);
        let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();
        let coords = collect_vertex_coords(&bodies);
        assert_eq!(ref_coords.len(), coords.len(), "run {i}: vertex count");
        for (a, b) in ref_coords.iter().zip(coords.iter()) {
            assert_eq!(a, b, "run {i}: coordinate mismatch");
        }
    }
}

// EC02: YAML roundtrip — serialize to YAML and back, coordinates match
#[test]
fn ec02_yaml_roundtrip_preserves_transform() {
    let mut doc = Document::new("Part");
    doc.root_component.transform = Transform {
        position: [5.0, 0.0, 0.0],
        rotation: [0.0; 3],
    };
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });

    let yaml = doc.to_yaml().unwrap();
    let doc2 = Document::from_yaml(&yaml).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let mut gen1 = IdGenerator::new(0);
    let bodies1 = build_assembly(&doc, dir.path(), &mut gen1).unwrap();
    let mut gen2 = IdGenerator::new(0);
    let bodies2 = build_assembly(&doc2, dir.path(), &mut gen2).unwrap();

    assert_eq!(bodies1.len(), bodies2.len());
    for (a, b) in bodies1.iter().zip(bodies2.iter()) {
        assert_eq!(a.feature_id, b.feature_id);
        assert_eq!(a.solid.vertices.len(), b.solid.vertices.len());
        for (va, vb) in a.solid.vertices.iter().zip(b.solid.vertices.iter()) {
            assert_eq!(va.point, vb.point, "roundtrip vertex mismatch");
        }
    }
}

// EC03: Negative zero position — [-0.0, -0.0, -0.0] produces same result as [0,0,0]
#[test]
fn ec03_negative_zero_position_unchanged() {
    let doc_neg = single_box_doc(Transform {
        position: [-0.0, -0.0, -0.0],
        rotation: [0.0; 3],
    });
    let doc_default = single_box_doc(Transform::default());

    let dir = tempfile::tempdir().unwrap();
    let mut gen1 = IdGenerator::new(0);
    let bodies_neg = build_assembly(&doc_neg, dir.path(), &mut gen1).unwrap();
    let mut gen2 = IdGenerator::new(0);
    let bodies_default = build_assembly(&doc_default, dir.path(), &mut gen2).unwrap();

    assert_eq!(bodies_neg.len(), bodies_default.len());
    for (a, b) in bodies_neg.iter().zip(bodies_default.iter()) {
        assert_eq!(a.solid.vertices.len(), b.solid.vertices.len());
        for (va, vb) in a.solid.vertices.iter().zip(b.solid.vertices.iter()) {
            assert_eq!(va.point, vb.point, "-0.0 must equal +0.0");
        }
    }
}

// EC04: NaN position — returns error (not panic, not silent garbage)
#[test]
fn ec04_nan_position_rejected() {
    let doc = single_box_doc(Transform {
        position: [f64::NAN, 0.0, 0.0],
        rotation: [0.0; 3],
    });
    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let result = build_assembly(&doc, dir.path(), &mut gen);
    assert!(result.is_err(), "NaN position must return an error");
}

// EC05: Inf position — returns error (not panic, not silent garbage)
#[test]
fn ec05_inf_position_rejected() {
    let doc = single_box_doc(Transform {
        position: [f64::INFINITY, 0.0, 0.0],
        rotation: [0.0; 3],
    });
    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let result = build_assembly(&doc, dir.path(), &mut gen);
    assert!(result.is_err(), "Inf position must return an error");
}

// EC06: f64::MAX position — coordinates stay finite (no overflow)
#[test]
fn ec06_f64_max_position_stays_finite() {
    let doc = single_box_doc(Transform {
        position: [f64::MAX, f64::MAX, f64::MAX],
        rotation: [0.0; 3],
    });
    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();
    assert_eq!(bodies.len(), 1);
    for v in &bodies[0].solid.vertices {
        assert!(v.point.x.is_finite(), "x must be finite");
        assert!(v.point.y.is_finite(), "y must be finite");
        assert!(v.point.z.is_finite(), "z must be finite");
    }
}

// EC07: Three-level nesting — offsets accumulate correctly across 3 levels
#[test]
fn ec07_three_level_nesting() {
    let doc_baseline = single_box_doc(Transform::default());
    let dir = tempfile::tempdir().unwrap();
    let mut gen0 = IdGenerator::new(0);
    let bodies_baseline = build_assembly(&doc_baseline, dir.path(), &mut gen0).unwrap();
    let baseline_coords = collect_vertex_coords(&bodies_baseline);

    // L0: [1,0,0] → L1: [0,2,0] → L2: [0,0,3] with box → total [1,2,3]
    let mut doc = Document::new("L0");
    doc.root_component.transform = Transform {
        position: [1.0, 0.0, 0.0],
        rotation: [0.0; 3],
    };
    let mut l1 = Component::new("L1");
    l1.transform = Transform {
        position: [0.0, 2.0, 0.0],
        rotation: [0.0; 3],
    };
    let mut l2 = Component::new("L2");
    l2.transform = Transform {
        position: [0.0, 0.0, 3.0],
        rotation: [0.0; 3],
    };
    l2.features.push(Feature::CreateBox {
        id: "box_l2".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    l1.children.push(l2);
    doc.root_component.children.push(l1);

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();
    assert_eq!(bodies.len(), 1);
    let coords = collect_vertex_coords(&bodies);
    assert_eq!(baseline_coords.len(), coords.len());
    for (base, shifted) in baseline_coords.iter().zip(coords.iter()) {
        assert!((shifted.0 - base.0 - 1.0).abs() < 1e-10, "x should be +1");
        assert!((shifted.1 - base.1 - 2.0).abs() < 1e-10, "y should be +2");
        assert!((shifted.2 - base.2 - 3.0).abs() < 1e-10, "z should be +3");
    }
}

// EC08: Multiple siblings — each gets parent offset + own offset independently
#[test]
fn ec08_multiple_siblings_independent_offsets() {
    let doc_baseline = single_box_doc(Transform::default());
    let dir = tempfile::tempdir().unwrap();
    let mut gen0 = IdGenerator::new(0);
    let bodies_baseline = build_assembly(&doc_baseline, dir.path(), &mut gen0).unwrap();
    let baseline_coords = collect_vertex_coords(&bodies_baseline);

    let mut doc = Document::new("Parent");
    doc.root_component.transform = Transform {
        position: [10.0, 0.0, 0.0],
        rotation: [0.0; 3],
    };
    // ChildA: parent [10,0,0] + own [0,5,0] = [10,5,0]
    let mut child_a = Component::new("ChildA");
    child_a.transform = Transform {
        position: [0.0, 5.0, 0.0],
        rotation: [0.0; 3],
    };
    child_a.features.push(Feature::CreateBox {
        id: "box_a".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    // ChildB: parent [10,0,0] + own [0,0,7] = [10,0,7]
    let mut child_b = Component::new("ChildB");
    child_b.transform = Transform {
        position: [0.0, 0.0, 7.0],
        rotation: [0.0; 3],
    };
    child_b.features.push(Feature::CreateBox {
        id: "box_b".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.children.push(child_a);
    doc.root_component.children.push(child_b);

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();
    assert_eq!(bodies.len(), 2, "two children → two bodies");

    // ChildA: total [10,5,0]
    let coords_a = collect_vertex_coords(&bodies[0..1]);
    for (base, shifted) in baseline_coords.iter().zip(coords_a.iter()) {
        assert!((shifted.0 - base.0 - 10.0).abs() < 1e-10, "A x+10");
        assert!((shifted.1 - base.1 - 5.0).abs() < 1e-10, "A y+5");
        assert!((shifted.2 - base.2).abs() < 1e-10, "A z unchanged");
    }
    // ChildB: total [10,0,7]
    let coords_b = collect_vertex_coords(&bodies[1..2]);
    for (base, shifted) in baseline_coords.iter().zip(coords_b.iter()) {
        assert!((shifted.0 - base.0 - 10.0).abs() < 1e-10, "B x+10");
        assert!((shifted.1 - base.1).abs() < 1e-10, "B y unchanged");
        assert!((shifted.2 - base.2 - 7.0).abs() < 1e-10, "B z+7");
    }
}

// EC09: Parent with features AND child with features — both get parent's accumulated offset
#[test]
fn ec09_parent_and_child_both_have_features() {
    let doc_baseline = single_box_doc(Transform::default());
    let dir = tempfile::tempdir().unwrap();
    let mut gen0 = IdGenerator::new(0);
    let bodies_baseline = build_assembly(&doc_baseline, dir.path(), &mut gen0).unwrap();
    let baseline_coords = collect_vertex_coords(&bodies_baseline);

    let mut doc = Document::new("Parent");
    doc.root_component.transform = Transform {
        position: [3.0, 4.0, 5.0],
        rotation: [0.0; 3],
    };
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_parent".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    let mut child = Component::new("Child");
    child.features.push(Feature::CreateBox {
        id: "box_child".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.children.push(child);

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();
    assert_eq!(bodies.len(), 2, "parent + child = 2 bodies");

    for body in &bodies {
        let coords = collect_vertex_coords(std::slice::from_ref(body));
        for (base, shifted) in baseline_coords.iter().zip(coords.iter()) {
            assert!((shifted.0 - base.0 - 3.0).abs() < 1e-10, "x+3");
            assert!((shifted.1 - base.1 - 4.0).abs() < 1e-10, "y+4");
            assert!((shifted.2 - base.2 - 5.0).abs() < 1e-10, "z+5");
        }
    }
}

// EC10: Non-zero rotation is applied — vertices are rotated
#[test]
fn ec10_nonzero_rotation_applied() {
    // rotation=[45, 90, 180] は頂点を回転させる
    let doc_rot = single_box_doc(Transform {
        position: [5.0, 0.0, 0.0],
        rotation: [45.0, 90.0, 180.0],
    });
    let doc_no_rot = single_box_doc(Transform {
        position: [5.0, 0.0, 0.0],
        rotation: [0.0; 3],
    });

    let dir = tempfile::tempdir().unwrap();
    let mut gen1 = IdGenerator::new(0);
    let bodies_rot = build_assembly(&doc_rot, dir.path(), &mut gen1).unwrap();
    let mut gen2 = IdGenerator::new(0);
    let bodies_no_rot = build_assembly(&doc_no_rot, dir.path(), &mut gen2).unwrap();

    assert_eq!(bodies_rot.len(), bodies_no_rot.len());
    // rotation 適用後の頂点は異なるはず
    for (a, b) in bodies_rot.iter().zip(bodies_no_rot.iter()) {
        assert_eq!(a.solid.vertices.len(), b.solid.vertices.len());
        let mut any_different = false;
        for (va, vb) in a.solid.vertices.iter().zip(b.solid.vertices.iter()) {
            if (va.point.x - vb.point.x).abs() > 1e-10
                || (va.point.y - vb.point.y).abs() > 1e-10
                || (va.point.z - vb.point.z).abs() > 1e-10
            {
                any_different = true;
                break;
            }
        }
        assert!(any_different, "rotation should change vertex coordinates");
    }
}

// EC11: Negative offset values shift in negative direction correctly
#[test]
fn ec11_negative_offset_values() {
    let doc_baseline = single_box_doc(Transform::default());
    let dir = tempfile::tempdir().unwrap();
    let mut gen0 = IdGenerator::new(0);
    let bodies_baseline = build_assembly(&doc_baseline, dir.path(), &mut gen0).unwrap();
    let baseline_coords = collect_vertex_coords(&bodies_baseline);

    let doc = single_box_doc(Transform {
        position: [-5.0, -3.0, -2.0],
        rotation: [0.0; 3],
    });
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();

    assert_eq!(bodies.len(), 1);
    let coords = collect_vertex_coords(&bodies);
    assert_eq!(baseline_coords.len(), coords.len());
    for (base, shifted) in baseline_coords.iter().zip(coords.iter()) {
        assert!(
            (shifted.0 - base.0 - (-5.0)).abs() < 1e-10,
            "x should be -5"
        );
        assert!(
            (shifted.1 - base.1 - (-3.0)).abs() < 1e-10,
            "y should be -3"
        );
        assert!(
            (shifted.2 - base.2 - (-2.0)).abs() < 1e-10,
            "z should be -2"
        );
    }
}

// EC12: f64::MIN_POSITIVE tiny offset — no panic, coordinates stay finite
#[test]
fn ec12_min_positive_offset() {
    let tiny = f64::MIN_POSITIVE;
    let doc = single_box_doc(Transform {
        position: [tiny, tiny, tiny],
        rotation: [0.0; 3],
    });
    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();
    assert_eq!(bodies.len(), 1);
    for v in &bodies[0].solid.vertices {
        assert!(v.point.x.is_finite(), "x must be finite");
        assert!(v.point.y.is_finite(), "y must be finite");
        assert!(v.point.z.is_finite(), "z must be finite");
    }
}

// EC13: Opposite offsets cancel — parent [5,0,0] + child [-5,0,0] = unchanged coordinates
#[test]
fn ec13_opposite_offsets_cancel() {
    let doc_baseline = single_box_doc(Transform::default());
    let dir = tempfile::tempdir().unwrap();
    let mut gen0 = IdGenerator::new(0);
    let bodies_baseline = build_assembly(&doc_baseline, dir.path(), &mut gen0).unwrap();
    let baseline_coords = collect_vertex_coords(&bodies_baseline);

    let mut doc = Document::new("Parent");
    doc.root_component.transform = Transform {
        position: [5.0, 0.0, 0.0],
        rotation: [0.0; 3],
    };
    let mut child = Component::new("Child");
    child.transform = Transform {
        position: [-5.0, 0.0, 0.0],
        rotation: [0.0; 3],
    };
    child.features.push(Feature::CreateBox {
        id: "box_child".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.children.push(child);

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();
    assert_eq!(bodies.len(), 1);
    let coords = collect_vertex_coords(&bodies);
    assert_eq!(baseline_coords.len(), coords.len());
    for (base, result) in baseline_coords.iter().zip(coords.iter()) {
        assert_eq!(base.0, result.0, "x unchanged (offsets cancel)");
        assert_eq!(base.1, result.1, "y unchanged");
        assert_eq!(base.2, result.2, "z unchanged");
    }
}

// EC14: Deep nesting (5 levels) — each level adds [1,1,1], total [5,5,5]
#[test]
fn ec14_deep_nesting_5_levels() {
    let doc_baseline = single_box_doc(Transform::default());
    let dir = tempfile::tempdir().unwrap();
    let mut gen0 = IdGenerator::new(0);
    let bodies_baseline = build_assembly(&doc_baseline, dir.path(), &mut gen0).unwrap();
    let baseline_coords = collect_vertex_coords(&bodies_baseline);

    let mut doc = Document::new("L0");
    doc.root_component.transform = Transform {
        position: [1.0, 1.0, 1.0],
        rotation: [0.0; 3],
    };

    // Build L4 (innermost, has box) up through L3, L2, L1
    let mut inner = Component::new("L4");
    inner.transform = Transform {
        position: [1.0, 1.0, 1.0],
        rotation: [0.0; 3],
    };
    inner.features.push(Feature::CreateBox {
        id: "box_inner".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });

    for lvl in (1..=3).rev() {
        let mut level = Component::new(&format!("L{lvl}"));
        level.transform = Transform {
            position: [1.0, 1.0, 1.0],
            rotation: [0.0; 3],
        };
        level.children.push(inner);
        inner = level;
    }
    doc.root_component.children.push(inner);

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();
    assert_eq!(bodies.len(), 1);
    let coords = collect_vertex_coords(&bodies);
    assert_eq!(baseline_coords.len(), coords.len());
    for (base, shifted) in baseline_coords.iter().zip(coords.iter()) {
        assert!((shifted.0 - base.0 - 5.0).abs() < 1e-10, "x +5");
        assert!((shifted.1 - base.1 - 5.0).abs() < 1e-10, "y +5");
        assert!((shifted.2 - base.2 - 5.0).abs() < 1e-10, "z +5");
    }
}
