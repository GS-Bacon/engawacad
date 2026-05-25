use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::primitives::make_cuboid;
use mycad_kernel::tessellation::tessellate_solid;

fn main() {
    // Simple manual benchmark until criterion is added.
    // Replace with criterion when ready:
    //   use criterion::{criterion_group, criterion_main, Criterion};

    let iterations = 10_000;
    let start = std::time::Instant::now();

    for i in 0..iterations {
        let mut id_gen = IdGenerator::new(i as u64);
        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
        let _mesh = tessellate_solid(&solid).unwrap();
    }

    let elapsed = start.elapsed();
    println!(
        "Tessellated {} cuboids in {:?} ({:.2} us/iter)",
        iterations,
        elapsed,
        elapsed.as_micros() as f64 / iterations as f64
    );
}
