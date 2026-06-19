/// Acceptance tests for Issue #73: ComponentRef 参照解決 (stdlib + file)
use engawa_build::build_assembly;
use engawa_format::component::{Component, ComponentRef};
use engawa_format::document::Document;
use engawa_format::feature::Feature;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::error::KernelError;
use serial_test::file_serial;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a minimal .engawa file with a single create_box feature.
fn write_box_engawa(dir: &Path, filename: &str, box_id: &str, size: f64) -> PathBuf {
    let mut doc = Document::new(filename.trim_end_matches(".engawa"));
    doc.root_component.features.push(Feature::CreateBox {
        suppressed: false,
        id: box_id.to_string(),
        width: size,
        height: size,
        depth: size,
    });
    let path = dir.join(filename);
    fs::write(&path, doc.to_yaml().unwrap()).unwrap();
    path
}

/// Build a Document whose root references an external file.
fn file_ref_doc(name: &str, rel_path: &str) -> Document {
    let mut doc = Document::new(name);
    doc.root_component.reference = Some(ComponentRef::File(rel_path.to_string()));
    doc
}

/// Build a Document whose root references a stdlib part.
fn stdlib_ref_doc(name: &str, stdlib_path: &str) -> Document {
    let mut doc = Document::new(name);
    doc.root_component.reference = Some(ComponentRef::StdLib(stdlib_path.to_string()));
    doc
}

/// Set ENGAWA_STDLIB_PATH env var for the process. Returns the old value (if any).
fn set_stdlib_env(path: &Path) -> Option<String> {
    let old = std::env::var("ENGAWA_STDLIB_PATH").ok();
    std::env::set_var("ENGAWA_STDLIB_PATH", path);
    old
}

/// Restore ENGAWA_STDLIB_PATH to its previous state.
fn restore_stdlib_env(old: Option<String>) {
    match old {
        Some(v) => std::env::set_var("ENGAWA_STDLIB_PATH", v),
        None => std::env::remove_var("ENGAWA_STDLIB_PATH"),
    }
}

// ---------------------------------------------------------------------------
// T01: Determinism — same assembly → identical Body list
// ---------------------------------------------------------------------------

#[test]
fn t01_determinism() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    write_box_engawa(base, "child.engawa", "box_child", 5.0);

    let mut doc = Document::new("Parent");
    doc.root_component.features.push(Feature::CreateBox {
        suppressed: false,
        id: "box_root".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    let mut child_comp = Component::new("RefChild");
    child_comp.reference = Some(ComponentRef::File("child.engawa".to_string()));
    doc.root_component.children.push(child_comp);

    let mut gen1 = IdGenerator::new(0);
    let bodies1 = build_assembly(&doc, base, &mut gen1).unwrap();

    let mut gen2 = IdGenerator::new(0);
    let bodies2 = build_assembly(&doc, base, &mut gen2).unwrap();

    assert_eq!(bodies1.len(), bodies2.len(), "body count must match");
    for (a, b) in bodies1.iter().zip(bodies2.iter()) {
        assert_eq!(a.feature_id, b.feature_id, "feature_id order must match");
        // Compare topology deterministically: vertex count, edge count, face count
        assert_eq!(
            a.solid.vertices.len(),
            b.solid.vertices.len(),
            "vertex count must match"
        );
        assert_eq!(
            a.solid.edges.len(),
            b.solid.edges.len(),
            "edge count must match"
        );
        assert_eq!(
            a.solid.faces.len(),
            b.solid.faces.len(),
            "face count must match"
        );
        // Compare vertex coordinates for full geometric determinism
        for (va, vb) in a.solid.vertices.iter().zip(b.solid.vertices.iter()) {
            assert_eq!(va.point, vb.point, "vertex coordinates must match");
        }
    }
}

// ---------------------------------------------------------------------------
// T02: StdLib reference resolution
// ---------------------------------------------------------------------------

#[test]
#[file_serial(engawa_stdlib_path)]
fn t02_stdlib_reference_resolved() {
    let dir = tempfile::tempdir().unwrap();
    let stdlib_dir = dir.path().join("stdlib");
    fs::create_dir_all(&stdlib_dir).unwrap();

    write_box_engawa(&stdlib_dir, "part.engawa", "box_stdlib", 3.0);

    let doc = stdlib_ref_doc("Parent", "part");

    let _old = set_stdlib_env(&stdlib_dir);
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen);
    restore_stdlib_env(_old);

    let bodies = bodies.unwrap();
    assert_eq!(bodies.len(), 1, "should have one body from stdlib ref");
    assert_eq!(bodies[0].feature_id, "box_stdlib");
}

// ---------------------------------------------------------------------------
// T03: File reference resolution (relative path)
// ---------------------------------------------------------------------------

#[test]
fn t03_file_reference_resolved() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    write_box_engawa(base, "child.engawa", "box_child", 5.0);

    let doc = file_ref_doc("Parent", "child.engawa");

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, base, &mut gen).unwrap();

    assert_eq!(bodies.len(), 1, "should have one body from file ref");
    assert_eq!(bodies[0].feature_id, "box_child");
}

// ---------------------------------------------------------------------------
// T04: Features + children all aggregated in traversal order
// ---------------------------------------------------------------------------

#[test]
fn t04_features_and_children_aggregated() {
    let mut doc = Document::new("Root");
    doc.root_component.features.push(Feature::CreateBox {
        id: "root_box".to_string(),
        width: 1.0,
        height: 2.0,
        depth: 3.0,
        suppressed: false,
    });

    let mut child_a = Component::new("ChildA");
    child_a.features.push(Feature::CreateBox {
        id: "child_a_box".to_string(),
        width: 4.0,
        height: 5.0,
        depth: 6.0,
        suppressed: false,
    });
    let mut child_b = Component::new("ChildB");
    child_b.features.push(Feature::CreateBox {
        id: "child_b_box".to_string(),
        width: 7.0,
        height: 8.0,
        depth: 9.0,
        suppressed: false,
    });

    doc.root_component.children.push(child_a);
    doc.root_component.children.push(child_b);

    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();

    assert_eq!(bodies.len(), 3, "root + 2 children = 3 bodies");
    assert_eq!(bodies[0].feature_id, "root_box");
    assert_eq!(bodies[1].feature_id, "child_a_box");
    assert_eq!(bodies[2].feature_id, "child_b_box");
}

// ---------------------------------------------------------------------------
// T05: Circular reference detection (A → B → A)
// ---------------------------------------------------------------------------

#[test]
fn t05_circular_reference_detected() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // A references B, B references A
    let yaml_a = "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: A\n  ref: b.engawa\n  features: []\n";
    let yaml_b = "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: B\n  ref: a.engawa\n  features: []\n";
    fs::write(base.join("a.engawa"), yaml_a).unwrap();
    fs::write(base.join("b.engawa"), yaml_b).unwrap();

    let doc = Document::from_yaml(yaml_a).unwrap();
    let mut gen = IdGenerator::new(0);
    let result = build_assembly(&doc, base, &mut gen);

    match result {
        Err(KernelError::CircularReference { .. }) => {}
        other => panic!("expected CircularReference, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T06: Self-reference (A → A)
// ---------------------------------------------------------------------------

#[test]
fn t06_boundary_self_reference() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    let yaml = "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: A\n  ref: self.engawa\n  features: []\n";
    fs::write(base.join("self.engawa"), yaml).unwrap();

    let doc = Document::from_yaml(yaml).unwrap();
    let mut gen = IdGenerator::new(0);
    let result = build_assembly(&doc, base, &mut gen);

    match result {
        Err(KernelError::CircularReference { .. }) => {}
        other => panic!("expected CircularReference, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T07: Depth boundary — 16 levels ok, 17 levels fail
// ---------------------------------------------------------------------------

#[test]
fn t07_boundary_depth_16_vs_17() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // Create chain: level_0 → level_1 → ... → level_N
    // level_N has no reference (leaf with a box).
    let leaf_yaml = "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: leaf\n  features:\n    - type: create_box\n      id: leaf_box\n      width: 1.0\n      height: 1.0\n      depth: 1.0\n";
    fs::write(base.join("level_16.engawa"), leaf_yaml).unwrap();

    for i in (0..16).rev() {
        let yaml = format!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: level_{i}\n  ref: level_{}.engawa\n  features: []\n",
            i + 1
        );
        fs::write(base.join(format!("level_{i}.engawa")), &yaml).unwrap();
    }

    // --- 16 levels should succeed (0..15 reference level 1..16, level_16 is leaf) ---
    let doc_16 = Document::from_path(&base.join("level_0.engawa")).unwrap();
    let mut gen = IdGenerator::new(0);
    let result = build_assembly(&doc_16, base, &mut gen);
    assert!(result.is_ok(), "16 reference levels should succeed");
    let bodies = result.unwrap();
    assert_eq!(bodies.len(), 1, "one leaf body");
    assert_eq!(bodies[0].feature_id, "leaf_box");

    // --- 17 levels: create level_17 as leaf, level_16 references it ---
    fs::write(base.join("level_17.engawa"), leaf_yaml).unwrap();
    // Overwrite level_16 to point to level_17 instead of being a leaf
    let yaml_16 = "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: level_16\n  ref: level_17.engawa\n  features: []\n";
    fs::write(base.join("level_16.engawa"), yaml_16).unwrap();

    let doc_17 = Document::from_path(&base.join("level_0.engawa")).unwrap();
    let mut gen = IdGenerator::new(0);
    let result = build_assembly(&doc_17, base, &mut gen);
    match result {
        Err(KernelError::MaxDepthExceeded { max, .. }) => {
            assert_eq!(max, 16);
        }
        other => panic!("expected MaxDepthExceeded, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T08: StdLib root not set → ReferenceResolution error
// ---------------------------------------------------------------------------

#[test]
#[file_serial(engawa_stdlib_path)]
fn t08_stdlib_root_not_set_error() {
    let old = std::env::var("ENGAWA_STDLIB_PATH").ok();
    std::env::remove_var("ENGAWA_STDLIB_PATH");

    let doc = stdlib_ref_doc("Parent", "nonexistent/part");

    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let result = build_assembly(&doc, dir.path(), &mut gen);

    // Restore env
    match old {
        Some(v) => std::env::set_var("ENGAWA_STDLIB_PATH", v),
        None => {}
    }

    match result {
        Err(KernelError::ReferenceResolution { reason, .. }) => {
            assert!(
                reason.contains("stdlib")
                    || reason.contains("No such file")
                    || reason.contains("not found"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("expected ReferenceResolution, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T09: Determinism — 100 runs of same assembly produce identical results
// ---------------------------------------------------------------------------

#[test]
fn t09_determinism_100_runs() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    write_box_engawa(base, "child.engawa", "box_child", 5.0);

    let mut doc = Document::new("Parent");
    doc.root_component.features.push(Feature::CreateBox {
        suppressed: false,
        id: "box_root".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    let mut child_comp = Component::new("RefChild");
    child_comp.reference = Some(ComponentRef::File("child.engawa".to_string()));
    doc.root_component.children.push(child_comp);

    // Build a reference result
    let mut gen0 = IdGenerator::new(0);
    let reference = build_assembly(&doc, base, &mut gen0).unwrap();

    for i in 1..=100 {
        let mut gen = IdGenerator::new(0);
        let bodies = build_assembly(&doc, base, &mut gen).unwrap();
        assert_eq!(
            bodies.len(),
            reference.len(),
            "run {i}: body count mismatch"
        );
        for (a, b) in bodies.iter().zip(reference.iter()) {
            assert_eq!(a.feature_id, b.feature_id, "run {i}: feature_id mismatch");
            assert_eq!(
                a.solid.vertices.len(),
                b.solid.vertices.len(),
                "run {i}: vertex count mismatch"
            );
            assert_eq!(
                a.solid.edges.len(),
                b.solid.edges.len(),
                "run {i}: edge count mismatch"
            );
            assert_eq!(
                a.solid.faces.len(),
                b.solid.faces.len(),
                "run {i}: face count mismatch"
            );
            for (va, vb) in a.solid.vertices.iter().zip(b.solid.vertices.iter()) {
                assert_eq!(va.point, vb.point, "run {i}: vertex coordinate mismatch");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// T10: Roundtrip — build → YAML serialize → deserialize → rebuild
// ---------------------------------------------------------------------------

#[test]
fn t10_roundtrip_yaml_build() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // Create a child .engawa file on disk
    write_box_engawa(base, "child.engawa", "box_child", 5.0);

    // Build a parent doc with features + a child referencing the file
    let mut doc = Document::new("Parent");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_root".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
        suppressed: false,
    });
    let mut ref_child = Component::new("RefChild");
    ref_child.reference = Some(ComponentRef::File("child.engawa".to_string()));
    doc.root_component.children.push(ref_child);

    // Original build
    let mut gen1 = IdGenerator::new(0);
    let bodies_original = build_assembly(&doc, base, &mut gen1).unwrap();

    // Roundtrip: serialize → deserialize → rebuild
    let yaml = doc.to_yaml().unwrap();
    let doc2 = Document::from_yaml(&yaml).unwrap();
    let mut gen2 = IdGenerator::new(0);
    let bodies_roundtrip = build_assembly(&doc2, base, &mut gen2).unwrap();

    // Compare
    assert_eq!(
        bodies_original.len(),
        bodies_roundtrip.len(),
        "body count after roundtrip"
    );
    for (a, b) in bodies_original.iter().zip(bodies_roundtrip.iter()) {
        assert_eq!(a.feature_id, b.feature_id, "feature_id after roundtrip");
        assert_eq!(
            a.solid.vertices.len(),
            b.solid.vertices.len(),
            "vertex count after roundtrip"
        );
        assert_eq!(
            a.solid.edges.len(),
            b.solid.edges.len(),
            "edge count after roundtrip"
        );
        assert_eq!(
            a.solid.faces.len(),
            b.solid.faces.len(),
            "face count after roundtrip"
        );
        for (va, vb) in a.solid.vertices.iter().zip(b.solid.vertices.iter()) {
            assert_eq!(va.point, vb.point, "vertex coordinates after roundtrip");
        }
    }
}

// ---------------------------------------------------------------------------
// T11: Empty root component — no features, no children, no reference
// ---------------------------------------------------------------------------

#[test]
fn t11_empty_root_component() {
    let doc = Document::new("EmptyRoot");
    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();
    assert!(
        bodies.is_empty(),
        "empty component should produce zero bodies"
    );
}

// ---------------------------------------------------------------------------
// T12: Features AND reference on same component — both contribute bodies
// ---------------------------------------------------------------------------

#[test]
fn t12_features_and_reference_combined() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // Child with a box
    write_box_engawa(base, "child.engawa", "box_child", 3.0);

    // Root has its own features AND a reference
    let mut doc = Document::new("Parent");
    doc.root_component.features.push(Feature::CreateBox {
        suppressed: false,
        id: "box_root".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.reference = Some(ComponentRef::File("child.engawa".to_string()));

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, base, &mut gen).unwrap();

    // Root's own features come first, then the reference's features
    assert_eq!(bodies.len(), 2, "root features + ref features = 2 bodies");
    assert_eq!(bodies[0].feature_id, "box_root");
    assert_eq!(bodies[1].feature_id, "box_child");
}

// ---------------------------------------------------------------------------
// T13: Non-existent file reference → ReferenceResolution error
// ---------------------------------------------------------------------------

#[test]
fn t13_nonexistent_file_reference() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // Reference to a file that does not exist on disk
    let doc = file_ref_doc("Parent", "nonexistent_child.engawa");

    let mut gen = IdGenerator::new(0);
    let result = build_assembly(&doc, base, &mut gen);

    match result {
        Err(KernelError::ReferenceResolution { path, reason }) => {
            assert!(
                path.contains("nonexistent_child"),
                "path should reference the missing file: {path}"
            );
            assert!(
                reason.contains("No such file")
                    || reason.contains("not found")
                    || reason.contains("not a file"),
                "reason should indicate file not found: {reason}"
            );
        }
        other => panic!("expected ReferenceResolution, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T14: Deep children nesting (no references) — all bodies aggregated
// ---------------------------------------------------------------------------

#[test]
fn t14_deep_children_nesting() {
    // Create a tree: Root → child_0 → child_1 → ... → child_9
    // Each has a unique box feature. No references, so depth stays 0.
    let mut doc = Document::new("Root");
    doc.root_component.features.push(Feature::CreateBox {
        suppressed: false,
        id: "box_root".to_string(),
        width: 1.0,
        height: 1.0,
        depth: 1.0,
    });

    let mut current = &mut doc.root_component;
    for i in 0..10 {
        let mut child = Component::new(&format!("child_{i}"));
        child.features.push(Feature::CreateBox {
            suppressed: false,
            id: format!("box_child_{i}"),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        });
        current.children.push(child);
        // Descend into the last child for next iteration
        current = &mut current.children[0];
    }

    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();

    // Root (1) + 10 nested children = 11 bodies
    assert_eq!(bodies.len(), 11, "root + 10 nested children = 11 bodies");
    assert_eq!(bodies[0].feature_id, "box_root");
    for i in 0..10 {
        assert_eq!(
            bodies[i + 1].feature_id,
            format!("box_child_{i}"),
            "body order should follow traversal"
        );
    }
}

// ---------------------------------------------------------------------------
// T15: Mixed tree — root features + file ref + children with features
// ---------------------------------------------------------------------------

#[test]
fn t15_mixed_tree_features_ref_children() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // External child file
    write_box_engawa(base, "ext.engawa", "box_ext", 7.0);

    let mut doc = Document::new("Root");
    doc.root_component.features.push(Feature::CreateBox {
        suppressed: false,
        id: "box_root".to_string(),
        width: 1.0,
        height: 2.0,
        depth: 3.0,
    });

    // Child A: has features + a file reference
    let mut child_a = Component::new("ChildA");
    child_a.features.push(Feature::CreateBox {
        suppressed: false,
        id: "box_a".to_string(),
        width: 4.0,
        height: 5.0,
        depth: 6.0,
    });
    child_a.reference = Some(ComponentRef::File("ext.engawa".to_string()));

    // Child B: features only
    let mut child_b = Component::new("ChildB");
    child_b.features.push(Feature::CreateBox {
        suppressed: false,
        id: "box_b".to_string(),
        width: 8.0,
        height: 9.0,
        depth: 10.0,
    });

    doc.root_component.children.push(child_a);
    doc.root_component.children.push(child_b);

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, base, &mut gen).unwrap();

    // Traversal: root features → root ref (none) → child_a features → child_a ref → child_b features
    assert_eq!(bodies.len(), 4, "root + child_a + ext + child_b = 4 bodies");
    assert_eq!(bodies[0].feature_id, "box_root");
    assert_eq!(bodies[1].feature_id, "box_a");
    assert_eq!(bodies[2].feature_id, "box_ext");
    assert_eq!(bodies[3].feature_id, "box_b");
}

// ---------------------------------------------------------------------------
// T16: Container component (no features, no ref) with children
// ---------------------------------------------------------------------------

#[test]
fn t16_container_component_with_children() {
    let mut doc = Document::new("Container");
    // Root has no features, no reference — purely a container

    let mut child_a = Component::new("ChildA");
    child_a.features.push(Feature::CreateBox {
        suppressed: false,
        id: "box_a".to_string(),
        width: 1.0,
        height: 1.0,
        depth: 1.0,
    });
    let mut child_b = Component::new("ChildB");
    child_b.features.push(Feature::CreateBox {
        suppressed: false,
        id: "box_b".to_string(),
        width: 2.0,
        height: 2.0,
        depth: 2.0,
    });

    doc.root_component.children.push(child_a);
    doc.root_component.children.push(child_b);

    let dir = tempfile::tempdir().unwrap();
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, dir.path(), &mut gen).unwrap();

    assert_eq!(bodies.len(), 2, "container with 2 children = 2 bodies");
    assert_eq!(bodies[0].feature_id, "box_a");
    assert_eq!(bodies[1].feature_id, "box_b");
}
