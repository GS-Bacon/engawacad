//! Issue #215: CreateSketch.plane_ref EntityRef (Face) 経路 acceptance tests

use engawa_build::build_bodies_from_features;
use engawa_format::{EntityKind, EntityRef, Feature, PlaneRef, SketchPlane, SketchSegment};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::geometry::surface::Surface;

/// T01: 決定性 — 同一 features 列を 2 回 build し全 FaceIndex/Plane が一致
#[test]
fn t01_determinism() {
    let features = vec![
        // cuboid を生成
        Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        // cuboid の上面 Face を参照する sketch を作成
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [5.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [5.0, 0.0],
                    to: [5.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [5.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
                feature_id: "cuboid".to_string(),
                kind: EntityKind::Face,
                role: "f_z_pos".to_string(),
            })),
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 5.0,
            fuse_target: None,
        },
    ];

    let ref_planes = &[];
    let mut gen1 = IdGenerator::new(0);
    let mut gen2 = IdGenerator::new(0);

    let built1 = build_bodies_from_features(&features, ref_planes, &mut gen1).unwrap();
    let built2 = build_bodies_from_features(&features, ref_planes, &mut gen2).unwrap();

    // 全 live body を比較 (cuboid + extrusion の 2 body) — Codex F02 対応
    let bodies1: Vec<_> = built1.live().collect();
    let bodies2: Vec<_> = built2.live().collect();
    assert_eq!(
        bodies1.len(),
        bodies2.len(),
        "live body 数が 2 回の build で一致"
    );
    assert!(bodies1.len() >= 2, "cuboid + extrusion で >= 2 body");

    for (b1, b2) in bodies1.iter().zip(bodies2.iter()) {
        assert_eq!(b1.feature_id, b2.feature_id, "feature_id 順序が決定的");
        let s1 = &b1.solid;
        let s2 = &b2.solid;

        // FaceIndex 一致
        assert_eq!(
            s1.faces.len(),
            s2.faces.len(),
            "{} faces.len",
            b1.feature_id
        );
        assert_eq!(
            s1.edges.len(),
            s2.edges.len(),
            "{} edges.len",
            b1.feature_id
        );
        assert_eq!(
            s1.vertices.len(),
            s2.vertices.len(),
            "{} vertices.len",
            b1.feature_id
        );

        // EntityId 一致
        let face_ids1: Vec<_> = s1.faces.iter().map(|f| f.id).collect();
        let face_ids2: Vec<_> = s2.faces.iter().map(|f| f.id).collect();
        assert_eq!(face_ids1, face_ids2, "{} face ids", b1.feature_id);

        let edge_ids1: Vec<_> = s1.edges.iter().map(|e| e.id).collect();
        let edge_ids2: Vec<_> = s2.edges.iter().map(|e| e.id).collect();
        assert_eq!(edge_ids1, edge_ids2, "{} edge ids", b1.feature_id);

        let vertex_ids1: Vec<_> = s1.vertices.iter().map(|v| v.id).collect();
        let vertex_ids2: Vec<_> = s2.vertices.iter().map(|v| v.id).collect();
        assert_eq!(vertex_ids1, vertex_ids2, "{} vertex ids", b1.feature_id);

        // Surface も完全一致 (Plane 4 ベクトル含む)
        let face_surfaces1: Vec<_> = s1.faces.iter().map(|f| &f.surface).collect();
        let face_surfaces2: Vec<_> = s2.faces.iter().map(|f| &f.surface).collect();
        assert_eq!(face_surfaces1.len(), face_surfaces2.len());
        for (fs1, fs2) in face_surfaces1.iter().zip(face_surfaces2.iter()) {
            assert_eq!(fs1, fs2, "{} Surface 完全一致", b1.feature_id);
        }
    }
}

/// T02: Legacy string PlaneRef — PlaneRef::RefPlane("Front") で従来通り Extrude 成功
#[test]
fn t02_legacy_string_planeref_backward_compat() {
    use engawa_format::RefPlane;
    let ref_planes = vec![RefPlane {
        id: "Front".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
    }];
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
            plane_ref: Some(PlaneRef::RefPlane("Front".to_string())),
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 8.0,
            fuse_target: None,
        },
    ];
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap();
    assert_eq!(built.live().count(), 1);
    let solid = &built.live().collect::<Vec<_>>()[0].solid;
    assert!(
        solid.faces.len() >= 6,
        "Extrusion must have at least 6 faces"
    );
}

/// T03: 正常系 (Entity) — cuboid 生成 → 上面 Face の EntityRef を取り出し → CreateSketch → Extrude 成功
#[test]
fn t03_plane_ref_entity_extrude() {
    let features = vec![
        Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [5.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [5.0, 0.0],
                    to: [5.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [5.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
                feature_id: "cuboid".to_string(),
                kind: EntityKind::Face,
                role: "f_z_pos".to_string(),
            })),
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 5.0,
            fuse_target: None,
        },
    ];

    let ref_planes = &[];
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, ref_planes, &mut gen).unwrap();

    // (1) Extrude 後の BuiltBodies::live() に 2 つの Solid (元 cuboid + 新 extrusion) が含まれる
    let bodies: Vec<_> = built.live().collect();
    assert_eq!(bodies.len(), 2, "cuboid + extrusion = 2 bodies");

    // cuboid (元 box) は extrude の fuse_target ではないので消費されていない
    // extrude は新しい body を生成
    let _body_cuboid = &bodies[0];
    let body_extrusion = &bodies[1];

    // (3) 新 extrusion Solid のトポロジー妥当性 (face 数 ≥ 6 をスポット確認)
    assert!(
        body_extrusion.solid.faces.len() >= 6,
        "extrusion must have at least 6 faces"
    );

    // (2) Plane 4 ベクトル一致: 元 cuboid 上面 Face の Surface::Plane と extrusion 底面が一致
    let body_cuboid = &bodies[0].solid;
    let face_idx = body_cuboid
        .find_face_by_entity_ref(&EntityRef::Named {
            feature_id: "cuboid".to_string(),
            kind: EntityKind::Face,
            role: "f_z_pos".to_string(),
        })
        .expect("f_z_pos face exists");
    let face_surface = &body_cuboid.faces[face_idx].surface;
    let (face_origin, face_normal, face_u, face_v) = match face_surface {
        Surface::Plane {
            origin,
            normal,
            u_axis,
            v_axis,
        } => (*origin, *normal, *u_axis, *v_axis),
        _ => panic!("Surface::Plane expected"),
    };

    // extrusion の底面 (faces[0]) は入力 sketch plane と同一 origin/axes を持つ
    // ただし make_extrusion 仕様により normal は depth.signum() で反転 (depth > 0 で逆符号)
    let bottom_surface = &body_extrusion.solid.faces[0].surface;
    match bottom_surface {
        Surface::Plane {
            origin,
            normal,
            u_axis,
            v_axis,
        } => {
            assert_eq!(
                *origin, face_origin,
                "底面 origin が Face EntityRef 由来の Plane と一致"
            );
            assert_eq!(
                *normal, -face_normal,
                "底面 normal は入力 plane の逆符号 (make_extrusion 仕様)"
            );
            assert_eq!(*u_axis, face_u, "底面 u_axis が一致");
            assert_eq!(*v_axis, face_v, "底面 v_axis が一致");
        }
        _ => panic!("Surface::Plane expected"),
    }
}

/// T04: 正常系 (Entity + ExtrudeCut) — Face EntityRef を ExtrudeCut で使う
#[test]
fn t04_plane_ref_entity_extrude_cut() {
    let features = vec![
        Feature::CreateBox {
            id: "box_target".to_string(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSketch {
            id: "sketch_cut".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [2.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [2.0, 0.0],
                    to: [2.0, 2.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [2.0, 2.0],
                    to: [0.0, 2.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 2.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
                feature_id: "cuboid".to_string(),
                kind: EntityKind::Face,
                role: "f_z_pos".to_string(),
            })),
        },
        Feature::ExtrudeCut {
            id: "cut_1".to_string(),
            sketch: "sketch_cut".to_string(),
            depth: 5.0,
            target: "box_target".to_string(),
        },
    ];

    let ref_planes = &[];
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, ref_planes, &mut gen).unwrap();

    // cut 成功、boolean 後の Solid 構造が manifold
    let bodies: Vec<_> = built.live().collect();
    assert_eq!(bodies.len(), 1, "cut result = 1 body");
    let result_solid = &bodies[0].solid;
    assert!(
        result_solid.faces.len() >= 6,
        "cut result must have at least 6 faces"
    );
    // Codex F03 対応: face 数だけでなく B-rep 構造の manifold 妥当性を検証
    result_solid
        .validate_manifold()
        .expect("ExtrudeCut 後の Solid は manifold");
}

/// T07_degen_unknown_entity_ref: 存在しない EntityRef を指定
#[test]
fn t07_degen_unknown_entity_ref() {
    let features = vec![
        Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [5.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [5.0, 0.0],
                    to: [5.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [5.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
                feature_id: "nonexistent_box".to_string(),
                kind: EntityKind::Face,
                role: "top".to_string(),
            })),
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 5.0,
            fuse_target: None,
        },
    ];

    let ref_planes = &[];
    let mut gen = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, ref_planes, &mut gen);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = format!("{:?}", err);
    assert!(
        err_msg.contains("FaceEntityRefNotFound") || err_msg.contains("not found"),
        "expected FaceEntityRefNotFound: {}",
        err_msg
    );
}

/// T08_degen_non_planar_face: 平面でない Face → エラー
/// Cylinder の lat_face (Surface::Cylinder) を参照すると FaceNotPlanar エラー
#[test]
fn t08_degen_non_planar_face() {
    // Cylinder の lat_face は Surface::Cylinder (非平面)
    // これを plane_ref に使うと FaceNotPlanar エラーになる
    let features = vec![
        Feature::CreateCylinder {
            id: "cyl_1".to_string(),
            radius: 5.0,
            height: 10.0,
            origin: [0.0, 0.0, 0.0],
        },
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [5.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [5.0, 0.0],
                    to: [5.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [5.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
                feature_id: "cylinder".to_string(),
                kind: EntityKind::Face,
                role: "lat_face".to_string(),
            })),
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 5.0,
            fuse_target: None,
        },
    ];

    let ref_planes = &[];
    let mut gen = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, ref_planes, &mut gen);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = format!("{:?}", err);
    assert!(
        err_msg.contains("FaceNotPlanar") || err_msg.contains("not planar"),
        "expected FaceNotPlanar: {}",
        err_msg
    );
}

/// T09_boundary_derived_ref: EntityRef::Derived を指定 → find_face_by_entity_ref が None → エラー
#[test]
fn t09_boundary_derived_ref() {
    let features = vec![
        Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [5.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [5.0, 0.0],
                    to: [5.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [5.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: Some(PlaneRef::Entity(EntityRef::Derived {
                kind: EntityKind::Face,
                op: "cut".to_string(),
                from: vec![],
                selector: "s0".to_string(),
            })),
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 5.0,
            fuse_target: None,
        },
    ];

    let ref_planes = &[];
    let mut gen = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, ref_planes, &mut gen);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = format!("{:?}", err);
    assert!(
        err_msg.contains("FaceEntityRefNotFound") || err_msg.contains("not found"),
        "expected FaceEntityRefNotFound for Derived ref: {}",
        err_msg
    );
}
