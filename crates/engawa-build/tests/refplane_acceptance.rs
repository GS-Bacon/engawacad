//! Issue #158: format-refplane acceptance tests (build layer)
//! STEP 6.6 実装完了

use engawa_build::build_bodies_from_features;
use engawa_format::{Document, PlaneRef, RefPlane, SketchPlane, SketchSegment};
use engawa_kernel::brep::topology::IdGenerator;

#[test]
fn t01_determinism() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.ancestors().nth(2).unwrap();
    let yaml_path = workspace_root.join("examples/sketch_via_refplane.engawa");
    let yaml = std::fs::read_to_string(yaml_path).unwrap();
    let doc1 = Document::from_yaml(&yaml).unwrap();
    let doc2 = Document::from_yaml(&yaml).unwrap();

    let features1 = &doc1.root_component.features;
    let ref_planes1 = &doc1.root_component.ref_planes;
    let features2 = &doc2.root_component.features;
    let ref_planes2 = &doc2.root_component.ref_planes;

    let mut gen1 = IdGenerator::new(0);
    let mut gen2 = IdGenerator::new(0);

    let built1 = build_bodies_from_features(features1, ref_planes1, &mut gen1).unwrap();
    let built2 = build_bodies_from_features(features2, ref_planes2, &mut gen2).unwrap();

    let body1 = &built1.all()[0].solid;
    let body2 = &built2.all()[0].solid;

    // Explicit Face/Edge/Vertex EntityId comparison
    let face_ids1: Vec<_> = body1.faces.iter().map(|f| f.id).collect();
    let face_ids2: Vec<_> = body2.faces.iter().map(|f| f.id).collect();
    assert_eq!(face_ids1, face_ids2);

    let edge_ids1: Vec<_> = body1.edges.iter().map(|e| e.id).collect();
    let edge_ids2: Vec<_> = body2.edges.iter().map(|e| e.id).collect();
    assert_eq!(edge_ids1, edge_ids2);

    let vertex_ids1: Vec<_> = body1.vertices.iter().map(|v| v.id).collect();
    let vertex_ids2: Vec<_> = body2.vertices.iter().map(|v| v.id).collect();
    assert_eq!(vertex_ids1, vertex_ids2);
}

#[test]
fn t05_new_format_e2e() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.ancestors().nth(2).unwrap();

    // New format with plane_ref: Front
    let yaml_path = workspace_root.join("examples/sketch_via_refplane.engawa");
    let yaml_new = std::fs::read_to_string(yaml_path).unwrap();
    let doc_new = Document::from_yaml(&yaml_new).unwrap();

    // Legacy format with plane: xy
    let yaml_path = workspace_root.join("examples/extruded_rect.engawa");
    let yaml_legacy = std::fs::read_to_string(yaml_path).unwrap();
    let doc_legacy = Document::from_yaml(&yaml_legacy).unwrap();

    let mut gen = IdGenerator::new(0);

    let built_new = build_bodies_from_features(
        &doc_new.root_component.features,
        &doc_new.root_component.ref_planes,
        &mut gen,
    )
    .unwrap();

    let mut gen = IdGenerator::new(0);

    let built_legacy = build_bodies_from_features(
        &doc_legacy.root_component.features,
        &doc_legacy.root_component.ref_planes,
        &mut gen,
    )
    .unwrap();

    let solid_new = &built_new.all()[0].solid;
    let solid_legacy = &built_legacy.all()[0].solid;

    // Same face/edge/vertex count
    assert_eq!(solid_new.faces.len(), solid_legacy.faces.len());
    assert_eq!(solid_new.edges.len(), solid_legacy.edges.len());
    assert_eq!(solid_new.vertices.len(), solid_legacy.vertices.len());
}

#[test]
fn t06_plane_ref_priority_over_plane() {
    use engawa_format::Feature;

    // plane_ref: Front (xy) should override plane: Yz
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Yz, // This should be ignored
            offset: 100.0,          // This offset should be ignored
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [10.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [10.0, 0.0],
                    to: [10.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [10.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: Some(PlaneRef::RefPlane("Front".to_string())), // plane_ref takes priority
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 8.0,
            fuse_target: None,
        },
    ];

    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap();
    let solid = &built.all()[0].solid;

    // If extruded on xy plane (Front), z range should be [0, 8]
    // If extruded on yz plane (ignored), x range would be [-100, -92]
    // Check that vertices are in xy plane extrusion range
    for vertex in &solid.vertices {
        // After xy extrusion, z should be in [0, 8]
        assert!(vertex.point.z >= 0.0 && vertex.point.z <= 8.0);
    }
}

#[test]
fn t07_legacy_plane_offset_still_works() {
    use engawa_format::Feature;

    // Legacy plane+offset path (no plane_ref)
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 5.0, // offset
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [10.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [10.0, 0.0],
                    to: [10.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [10.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: None, // no plane_ref
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 8.0,
            fuse_target: None,
        },
    ];

    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap();
    let solid = &built.all()[0].solid;

    // With offset 5.0 on xy plane, extrusion depth 8.0
    // z range should be [5.0, 13.0]
    for vertex in &solid.vertices {
        assert!(vertex.point.z >= 5.0 && vertex.point.z <= 13.0);
    }
}

#[test]
fn t10_degen_unknown_plane_ref() {
    use engawa_format::Feature;

    let features = vec![
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [10.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [10.0, 0.0],
                    to: [10.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [10.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: Some(PlaneRef::RefPlane("Nonexistent".to_string())),
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 8.0,
            fuse_target: None,
        },
    ];

    let ref_planes: Vec<RefPlane> = vec![];
    let mut gen = IdGenerator::new(0);

    let result = build_bodies_from_features(&features, &ref_planes, &mut gen);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = format!("{:?}", err);
    assert!(err_msg.contains("UnknownRefPlane") || err_msg.contains("unknown ref_plane"));
    assert!(err_msg.contains("Nonexistent"));
}

#[test]
fn t11_boundary_empty_plane_ref_string() {
    use engawa_format::Feature;

    let features = vec![
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [10.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [10.0, 0.0],
                    to: [10.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [10.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: Some(PlaneRef::RefPlane("".to_string())),
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 8.0,
            fuse_target: None,
        },
    ];

    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);

    let result = build_bodies_from_features(&features, &ref_planes, &mut gen);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = format!("{:?}", err);
    assert!(err_msg.contains("UnknownRefPlane"));
}

#[test]
fn t15_child_component_uses_own_ref_planes() {
    use engawa_build::build_assembly;
    use engawa_format::{Component, Document, Feature};

    let custom_plane = RefPlane {
        id: "Custom".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
    };

    let mut child = Component::new("child");
    child.features = vec![
        Feature::CreateSketch {
            id: "sk".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            plane_ref: Some(PlaneRef::RefPlane("Custom".into())),
            profile: vec![
                SketchSegment {
                    id: "s1".into(),
                    from: [0.0, 0.0],
                    to: [1.0, 0.0],
                },
                SketchSegment {
                    id: "s2".into(),
                    from: [1.0, 0.0],
                    to: [1.0, 1.0],
                },
                SketchSegment {
                    id: "s3".into(),
                    from: [1.0, 1.0],
                    to: [0.0, 1.0],
                },
                SketchSegment {
                    id: "s4".into(),
                    from: [0.0, 1.0],
                    to: [0.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ex".into(),
            sketch: "sk".into(),
            depth: 1.0,
            fuse_target: None,
        },
    ];
    child.ref_planes = vec![custom_plane];

    let mut parent = Component::new("parent");
    parent.children = vec![child];
    parent.ref_planes = RefPlane::default_canonical_three();

    let doc = Document {
        schema_version: 1,
        version: "0.1.0".into(),
        root_component: parent,
    };

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, std::path::Path::new("."), &mut gen)
        .expect("F01 fix: child must use its own ref_planes");

    assert!(!bodies.is_empty(), "child Extrude must produce a body");
}

#[test]
fn t16_empty_child_falls_back_to_canonical_not_parent() {
    use engawa_build::build_assembly;
    use engawa_format::component::Transform;
    use engawa_format::{Component, Document, Feature, SketchPlane, SketchSegment};

    // 親が custom ref_planes (Front 等は含まない) のみを持ち、child は空 ref_planes。
    // child の features 内で plane_ref: "Front" を使う。
    // F01 r2 修正後は canonical three が child のローカル fallback として使われ、
    // 親の custom ref_planes に影響されない。
    let custom_only = vec![RefPlane {
        id: "CustomParent".into(),
        plane: SketchPlane::Yz,
        offset: 100.0,
    }];

    let child = Component {
        name: "child".into(),
        transform: Transform::default(),
        reference: None,
        features: vec![
            Feature::CreateSketch {
                id: "sk".into(),
                plane: SketchPlane::Xy,
                offset: 0.0,
                plane_ref: Some(PlaneRef::RefPlane("Front".into())),
                profile: vec![
                    SketchSegment {
                        id: "s1".into(),
                        from: [0.0, 0.0],
                        to: [1.0, 0.0],
                    },
                    SketchSegment {
                        id: "s2".into(),
                        from: [1.0, 0.0],
                        to: [1.0, 1.0],
                    },
                    SketchSegment {
                        id: "s3".into(),
                        from: [1.0, 1.0],
                        to: [0.0, 1.0],
                    },
                    SketchSegment {
                        id: "s4".into(),
                        from: [0.0, 1.0],
                        to: [0.0, 0.0],
                    },
                ],
            },
            Feature::Extrude {
                id: "ex".into(),
                sketch: "sk".into(),
                depth: 1.0,
                fuse_target: None,
            },
        ],
        children: vec![],
        ref_planes: vec![], // 空。canonical three にフォールバックされるべき
    };

    let parent = Component {
        name: "parent".into(),
        transform: Transform::default(),
        reference: None,
        features: vec![],
        children: vec![child],
        ref_planes: custom_only, // 親は custom のみ、Front を含まない
    };

    let doc = Document {
        schema_version: 1,
        version: "0.1.0".into(),
        root_component: parent,
    };

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, std::path::Path::new("."), &mut gen)
        .expect("F01 r2 fix: empty child must use canonical three, not inherit custom-only parent");
    assert!(
        !bodies.is_empty(),
        "child Extrude with plane_ref=Front must succeed via canonical fallback"
    );
}

#[test]
fn t17_degen_grandchild_canonical_fallback() {
    use engawa_build::build_assembly;
    use engawa_format::component::Transform;
    use engawa_format::{Component, Document, Feature, SketchPlane, SketchSegment};

    // 親 custom-only + 中間 child custom-only + 孫 empty の深ネスト構成。
    // 孫の features で plane_ref: "Front" を使い、canonical fallback により解決されることを確認。
    let parent_custom_only = vec![RefPlane {
        id: "ParentCustom".into(),
        plane: SketchPlane::Yz,
        offset: 100.0,
    }];
    let child_custom_only = vec![RefPlane {
        id: "ChildCustom".into(),
        plane: SketchPlane::Xz,
        offset: 50.0,
    }];

    let grandchild = Component {
        name: "grandchild".into(),
        transform: Transform::default(),
        reference: None,
        features: vec![
            Feature::CreateSketch {
                id: "sk".into(),
                plane: SketchPlane::Xy,
                offset: 0.0,
                plane_ref: Some(PlaneRef::RefPlane("Front".into())),
                profile: vec![
                    SketchSegment {
                        id: "s1".into(),
                        from: [0.0, 0.0],
                        to: [1.0, 0.0],
                    },
                    SketchSegment {
                        id: "s2".into(),
                        from: [1.0, 0.0],
                        to: [1.0, 1.0],
                    },
                    SketchSegment {
                        id: "s3".into(),
                        from: [1.0, 1.0],
                        to: [0.0, 1.0],
                    },
                    SketchSegment {
                        id: "s4".into(),
                        from: [0.0, 1.0],
                        to: [0.0, 0.0],
                    },
                ],
            },
            Feature::Extrude {
                id: "ex".into(),
                sketch: "sk".into(),
                depth: 1.0,
                fuse_target: None,
            },
        ],
        children: vec![],
        ref_planes: vec![], // 空。canonical three にフォールバック
    };

    let child = Component {
        name: "child".into(),
        transform: Transform::default(),
        reference: None,
        features: vec![],
        children: vec![grandchild],
        ref_planes: child_custom_only, // 中間 child も custom-only
    };

    let parent = Component {
        name: "parent".into(),
        transform: Transform::default(),
        reference: None,
        features: vec![],
        children: vec![child],
        ref_planes: parent_custom_only, // 親も custom-only
    };

    let doc = Document {
        schema_version: 1,
        version: "0.1.0".into(),
        root_component: parent,
    };

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, std::path::Path::new("."), &mut gen).expect(
        "ADR-014: grandchild must use canonical fallback, not inherit parent/child custom-only",
    );
    assert!(
        !bodies.is_empty(),
        "grandchild Extrude with plane_ref=Front must succeed via canonical fallback"
    );

    // 同一 Document を 2 回 build し、Body 順序 + face/edge/vertex EntityId が完全一致することを確認。
    // (B-6 cross-cut critical: nested + IdGenerator 経路の決定性 regression)
    let mut gen2 = IdGenerator::new(0);
    let bodies2 = build_assembly(&doc, std::path::Path::new("."), &mut gen2)
        .expect("ADR-014: second build must also succeed");
    assert_eq!(
        bodies.len(),
        bodies2.len(),
        "ADR-014 determinism: Body count must match across runs"
    );
    for (b1, b2) in bodies.iter().zip(bodies2.iter()) {
        let face_ids1: Vec<_> = b1.solid.faces.iter().map(|f| f.id).collect();
        let face_ids2: Vec<_> = b2.solid.faces.iter().map(|f| f.id).collect();
        assert_eq!(
            face_ids1, face_ids2,
            "ADR-014 determinism: grandchild Face IDs must match"
        );
        let edge_ids1: Vec<_> = b1.solid.edges.iter().map(|e| e.id).collect();
        let edge_ids2: Vec<_> = b2.solid.edges.iter().map(|e| e.id).collect();
        assert_eq!(
            edge_ids1, edge_ids2,
            "ADR-014 determinism: grandchild Edge IDs must match"
        );
        let vertex_ids1: Vec<_> = b1.solid.vertices.iter().map(|v| v.id).collect();
        let vertex_ids2: Vec<_> = b2.solid.vertices.iter().map(|v| v.id).collect();
        assert_eq!(
            vertex_ids1, vertex_ids2,
            "ADR-014 determinism: grandchild Vertex IDs must match"
        );
    }
}

#[test]
fn t18_degen_build_layer_refplane_offset_not_finite() {
    use engawa_format::Feature;

    let bad_plane = vec![RefPlane {
        id: "BadOffset".into(),
        plane: SketchPlane::Xy,
        offset: f64::NAN,
    }];
    let features = vec![
        Feature::CreateSketch {
            id: "sk".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            plane_ref: Some(PlaneRef::RefPlane("BadOffset".into())),
            profile: vec![
                SketchSegment {
                    id: "s1".into(),
                    from: [0.0, 0.0],
                    to: [1.0, 0.0],
                },
                SketchSegment {
                    id: "s2".into(),
                    from: [1.0, 0.0],
                    to: [1.0, 1.0],
                },
                SketchSegment {
                    id: "s3".into(),
                    from: [1.0, 1.0],
                    to: [0.0, 1.0],
                },
                SketchSegment {
                    id: "s4".into(),
                    from: [0.0, 1.0],
                    to: [0.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ex".into(),
            sketch: "sk".into(),
            depth: 1.0,
            fuse_target: None,
        },
    ];
    let mut gen = IdGenerator::new(0);
    let err = build_bodies_from_features(&features, &bad_plane, &mut gen).unwrap_err();
    let s = format!("{err:?}");
    assert!(
        s.contains("BadOffset")
            && (s.contains("non-finite") || s.contains("InvalidRefPlaneOffset")),
        "expected InvalidRefPlaneOffset error: {s}"
    );
}
