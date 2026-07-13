use criterion::{Criterion, criterion_group, criterion_main};
use mesh_parsers::parse_file;

pub fn parallel(c: &mut Criterion) {
    c.bench_function("Armadillo", |b| {
        b.iter(|| parse_file::<65536>("/home/lieunoir/meshes/armadillo.obj"));
    });
    c.bench_function("Bob", |b| {
        b.iter(|| parse_file::<65536>("/home/lieunoir/meshes/bob.obj"));
    });
    c.bench_function("Face", |b| {
        b.iter(|| parse_file::<65536>("/home/lieunoir/meshes/face.obj"));
    });
    c.bench_function("Spot", |b| {
        b.iter(|| parse_file::<65536>("/home/lieunoir/meshes/spot.obj"));
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets = parallel
);
criterion_main!(benches);
