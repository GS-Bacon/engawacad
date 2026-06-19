//! Issue #219 acceptance tests: CreateBox dispatcher must propagate Feature.id
//! to kernel Face/Edge/Vertex EntityRef::Named.feature_id.

use engawa_build::build_bodies_from_features;
use engawa_format::{EntityRef, Feature};
use engawa_kernel::brep::topology::IdGenerator;

/// T02: CreateBox(id="box_1") は kernel Solid の全 Face/Edge/Vertex の
/// `EntityRef::Named { feature_id, .. }` を "box_1" にしなければならない。
/// 現状 (#215 close 時点) では make_cuboid 内で fid="cuboid" がハードコードされており、
/// `feature_id == "box_1"` の assert が FAIL する (= バグ再現を確認済み)。
#[test]
fn t02_create_box_propagates_feature_id_to_face_names() {
    let features = vec![Feature::CreateBox {
        id: "box_1".to_string(),
        width: 2.0,
        height: 3.0,
        depth: 4.0,
        suppressed: false,
    }];
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, &[], &mut gen).expect("build");
    let body = built.get("box_1").expect("box_1 body");
    assert_eq!(body.solid.faces.len(), 6, "cuboid has 6 faces");
    for face in &body.solid.faces {
        match face.name.as_ref().expect("face name must be Some") {
            EntityRef::Named { feature_id, .. } => assert_eq!(
                feature_id, "box_1",
                "Face.name.feature_id must be \"box_1\" (Feature.id), not the kernel-default \"cuboid\""
            ),
            other => panic!("Face name must be Named variant: {other:?}"),
        }
    }
    // Edge / Vertex も同じ feature_id でなければならない
    for edge in &body.solid.edges {
        match edge.name.as_ref().expect("edge name must be Some") {
            EntityRef::Named { feature_id, .. } => assert_eq!(feature_id, "box_1"),
            other => panic!("Edge name must be Named variant: {other:?}"),
        }
    }
    for vertex in &body.solid.vertices {
        match vertex.name.as_ref().expect("vertex name must be Some") {
            EntityRef::Named { feature_id, .. } => assert_eq!(feature_id, "box_1"),
            other => panic!("Vertex name must be Named variant: {other:?}"),
        }
    }
}
