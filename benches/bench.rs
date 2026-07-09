use criterion::{Criterion, criterion_group, criterion_main};
use mesh_parsers::load_obj;

pub fn parallel(c: &mut Criterion) {
    c.bench_function("Armadillo", |b| {
        b.iter(|| load_obj("/home/lieunoir/meshes/armadillo.obj"));
    });
    c.bench_function("Bob", |b| {
        b.iter(|| load_obj("/home/lieunoir/meshes/bob.obj"));
    });
    c.bench_function("Face", |b| {
        b.iter(|| load_obj("/home/lieunoir/meshes/face.obj"));
    });
    c.bench_function("Spot", |b| {
        b.iter(|| load_obj("/home/lieunoir/meshes/spot.obj"));
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default();//.measurement_time(std::time::Duration::from_secs(11));
    targets = parallel
);
criterion_main!(benches);
