use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::primitives::make_cuboid;
use mycad_kernel::tessellation::tessellate_solid;

/// Integration test: create a cuboid, tessellate it, and verify the pipeline.
#[test]
fn cuboid_to_mesh_pipeline() {
    let mut id_gen = IdGenerator::new(0);
    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);

    // Verify topology
    assert_eq!(solid.vertices.len(), 8);
    assert_eq!(solid.edges.len(), 12);
    assert_eq!(solid.faces.len(), 6);

    // Tessellate
    let mesh = tessellate_solid(&solid);

    // Verify mesh
    assert_eq!(mesh.triangle_count(), 12); // 6 faces * 2 triangles
    assert_eq!(mesh.positions.len(), 24); // 6 faces * 4 vertices
    assert_eq!(mesh.normals.len(), 24);
    assert_eq!(mesh.indices.len(), 36); // 12 triangles * 3 indices
}

/// Integration test: deterministic pipeline produces identical results.
#[test]
fn cuboid_pipeline_is_deterministic() {
    let run = || {
        let mut id_gen = IdGenerator::new(42);
        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
        tessellate_solid(&solid)
    };

    let mesh1 = run();
    let mesh2 = run();

    assert_eq!(mesh1.positions, mesh2.positions);
    assert_eq!(mesh1.normals, mesh2.normals);
    assert_eq!(mesh1.indices, mesh2.indices);
}
