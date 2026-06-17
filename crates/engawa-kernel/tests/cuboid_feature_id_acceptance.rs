//! Issue #219 kernel-side acceptance tests for `make_cuboid` feature_id parameter.
//! STEP 6 (GLM core) で `make_cuboid(dx, dy, dz, feature_id: &str, id_gen)` シグネチャに変更後、
//! `#[ignore]` を外して実装する。

use engawa_format::{EntityKind, EntityRef};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::primitives::make_cuboid;

/// T01: make_cuboid を同一 fid + 同一 IdGenerator で 2 回呼び、全 vertex/edge/face id・position・name が一致することを assert
#[test]
fn t01_determinism() {
    let mut gen1 = IdGenerator::new(0);
    let mut gen2 = IdGenerator::new(0);

    let s1 = make_cuboid(10.0, 20.0, 30.0, "cuboid", &mut gen1).unwrap();
    let s2 = make_cuboid(10.0, 20.0, 30.0, "cuboid", &mut gen2).unwrap();

    assert_eq!(s1.vertices.len(), 8);
    assert_eq!(s2.vertices.len(), 8);
    assert_eq!(s1.edges.len(), 12);
    assert_eq!(s2.edges.len(), 12);
    assert_eq!(s1.faces.len(), 6);
    assert_eq!(s2.faces.len(), 6);

    for (v1, v2) in s1.vertices.iter().zip(s2.vertices.iter()) {
        assert_eq!(v1.id, v2.id);
        assert_eq!(v1.point, v2.point);
        assert_eq!(v1.name, v2.name);
    }

    for (e1, e2) in s1.edges.iter().zip(s2.edges.iter()) {
        assert_eq!(e1.id, e2.id);
        assert_eq!(e1.vertices, e2.vertices);
        assert_eq!(e1.name, e2.name);
    }

    for (f1, f2) in s1.faces.iter().zip(s2.faces.iter()) {
        assert_eq!(f1.id, f2.id);
        assert_eq!(f1.name, f2.name);
    }
}

/// T03: make_cuboid(1,1,1, "my_box") で生成 → 全 Face/Edge/Vertex の name.feature_id == "my_box" を assert
#[test]
fn t03_fid_propagation_to_named_entities() {
    let mut gen = IdGenerator::new(0);
    let solid = make_cuboid(1.0, 1.0, 1.0, "my_box", &mut gen).unwrap();

    for vertex in &solid.vertices {
        let name = vertex.name.as_ref().expect("vertex name must be Some");
        match name {
            EntityRef::Named {
                feature_id,
                kind: EntityKind::Vertex,
                ..
            } => {
                assert_eq!(feature_id, "my_box");
            }
            other => panic!("Expected Named Vertex, got {:?}", other),
        }
    }

    for edge in &solid.edges {
        let name = edge.name.as_ref().expect("edge name must be Some");
        match name {
            EntityRef::Named {
                feature_id,
                kind: EntityKind::Edge,
                ..
            } => {
                assert_eq!(feature_id, "my_box");
            }
            other => panic!("Expected Named Edge, got {:?}", other),
        }
    }

    for face in &solid.faces {
        let name = face.name.as_ref().expect("face name must be Some");
        match name {
            EntityRef::Named {
                feature_id,
                kind: EntityKind::Face,
                ..
            } => {
                assert_eq!(feature_id, "my_box");
            }
            other => panic!("Expected Named Face, got {:?}", other),
        }
    }
}

/// T04: make_cuboid(1,1,1, "", ...) を呼ぶ。EntityRef::try_named が空文字列を Err で reject するため、
/// name が None になることを確認する（現在の .ok() ガード semantics を保護）
#[test]
fn t04_degen_empty_fid() {
    let mut gen = IdGenerator::new(0);
    let solid = make_cuboid(1.0, 1.0, 1.0, "", &mut gen).unwrap();

    // 空文字列 fid の場合、EntityRef::try_named は Err を返すため .ok() で None になる
    for vertex in &solid.vertices {
        assert!(
            vertex.name.is_none(),
            "vertex name should be None for empty fid"
        );
    }
    for edge in &solid.edges {
        assert!(
            edge.name.is_none(),
            "edge name should be None for empty fid"
        );
    }
    for face in &solid.faces {
        assert!(
            face.name.is_none(),
            "face name should be None for empty fid"
        );
    }
}

/// T05: 長い fid ("a".repeat(256)) を渡しても truncate や panic が起きず、
/// Face/Edge/Vertex name の feature_id にそのまま入ることを assert
#[test]
fn t05_boundary_long_fid() {
    let long_fid = "a".repeat(256);
    let mut gen = IdGenerator::new(0);
    let solid = make_cuboid(1.0, 1.0, 1.0, &long_fid, &mut gen).unwrap();

    for vertex in &solid.vertices {
        let name = vertex.name.as_ref().expect("vertex name must be Some");
        match name {
            EntityRef::Named { feature_id, .. } => {
                assert_eq!(
                    feature_id.as_str(),
                    &long_fid,
                    "feature_id should be preserved without truncation"
                );
            }
            other => panic!("Expected Named, got {:?}", other),
        }
    }

    for edge in &solid.edges {
        let name = edge.name.as_ref().expect("edge name must be Some");
        match name {
            EntityRef::Named { feature_id, .. } => {
                assert_eq!(
                    feature_id.as_str(),
                    &long_fid,
                    "feature_id should be preserved without truncation"
                );
            }
            other => panic!("Expected Named, got {:?}", other),
        }
    }

    for face in &solid.faces {
        let name = face.name.as_ref().expect("face name must be Some");
        match name {
            EntityRef::Named { feature_id, .. } => {
                assert_eq!(
                    feature_id.as_str(),
                    &long_fid,
                    "feature_id should be preserved without truncation"
                );
            }
            other => panic!("Expected Named, got {:?}", other),
        }
    }
}
