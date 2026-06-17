//! Acceptance tests for `Solid::find_face_by_entity_ref` (Issue #214 / Phase 8 sub-Issue 8a).

use engawa_format::{EntityKind, EntityRef};

fn make_test_cuboid() -> engawa_kernel::brep::topology::Solid {
    let mut gen = engawa_kernel::brep::topology::IdGenerator::new(1);
    engawa_kernel::primitives::make_cuboid(2.0, 3.0, 4.0, &mut gen).unwrap()
}

/// T01: Determinism — same input yields same index over 3 calls.
#[test]
fn t01_determinism_repeated_lookup() {
    let solid = make_test_cuboid();
    let ref_top = EntityRef::try_named("cuboid", EntityKind::Face, "f_y_pos").unwrap();
    let idx1 = solid.find_face_by_entity_ref(&ref_top);
    let idx2 = solid.find_face_by_entity_ref(&ref_top);
    let idx3 = solid.find_face_by_entity_ref(&ref_top);
    assert_eq!(idx1, idx2, "first and second calls must match");
    assert_eq!(idx2, idx3, "second and third calls must match");
}

/// T02: Resolves cuboid top face — returned index's face.name matches the ref.
#[test]
fn t02_resolves_cuboid_top_face() {
    let solid = make_test_cuboid();
    let ref_top = EntityRef::try_named("cuboid", EntityKind::Face, "f_y_pos").unwrap();
    let idx = solid.find_face_by_entity_ref(&ref_top);
    assert!(idx.is_some(), "top face must be found");
    let face = &solid.faces[idx.unwrap()];
    assert_eq!(face.name, Some(ref_top));
}

/// T03: Distinguishes role in same feature_id — top and bottom return different indices.
#[test]
fn t03_distinguishes_role_in_same_feature_id() {
    let solid = make_test_cuboid();
    let ref_top = EntityRef::try_named("cuboid", EntityKind::Face, "f_y_pos").unwrap();
    let ref_bot = EntityRef::try_named("cuboid", EntityKind::Face, "f_y_neg").unwrap();
    let idx_top = solid.find_face_by_entity_ref(&ref_top);
    let idx_bot = solid.find_face_by_entity_ref(&ref_bot);
    assert!(idx_top.is_some(), "top face must be found");
    assert!(idx_bot.is_some(), "bottom face must be found");
    assert_ne!(
        idx_top, idx_bot,
        "top and bottom must have different indices"
    );
}

/// T04_boundary_missing_ref — nonexistent feature_id returns None.
#[test]
fn t04_boundary_missing_ref_returns_none() {
    let solid = make_test_cuboid();
    let ref_missing = EntityRef::try_named("nonexistent", EntityKind::Face, "top").unwrap();
    let idx = solid.find_face_by_entity_ref(&ref_missing);
    assert_eq!(idx, None, "nonexistent ref must return None");
}

/// T05_degen_wrong_kind — EntityKind::Edge returns None.
#[test]
fn t05_degen_wrong_kind_returns_none() {
    let solid = make_test_cuboid();
    let ref_edge = EntityRef::try_named("cuboid", EntityKind::Edge, "e_0").unwrap();
    let idx = solid.find_face_by_entity_ref(&ref_edge);
    assert_eq!(idx, None, "edge ref must return None");
}

/// T06_degen_derived_variant — Derived returns None.
#[test]
fn t06_degen_derived_variant_returns_none() {
    use engawa_format::EntityRef;
    let solid = make_test_cuboid();
    let named = EntityRef::try_named("cuboid", EntityKind::Face, "f_y_pos").unwrap();
    let derived = EntityRef::Derived {
        kind: EntityKind::Face,
        op: "cut".to_string(),
        from: vec![named],
        selector: "s0".to_string(),
    };
    let idx = solid.find_face_by_entity_ref(&derived);
    assert_eq!(idx, None, "derived ref must return None");
}
