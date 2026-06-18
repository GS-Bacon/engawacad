use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::primitives::make_cuboid;
use engawa_kernel::tessellation::tessellate_solid;

fn bench_tessellate_cuboid(c: &mut Criterion) {
    c.bench_function("tessellate_cuboid_10x20x30", |b| {
        b.iter_batched(
            || {
                let mut gen = IdGenerator::new(0);
                make_cuboid(10.0, 20.0, 30.0, "cuboid", &mut gen).unwrap()
            },
            |solid| tessellate_solid(&solid).unwrap(),
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_tessellate_cuboid);
criterion_main!(benches);
