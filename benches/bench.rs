use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use mesh_parsers::parse_file;

fn obj(c: &mut Criterion) {
    c.bench_function("Armadillo", |b| {
        b.iter(|| {
            parse_file::<65536>(
                &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/armadillo.obj"),
            )
        });
    });
    c.bench_function("Bob", |b| {
        b.iter(|| {
            parse_file::<65536>(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/bob.obj"))
        });
    });
    c.bench_function("Face", |b| {
        b.iter(|| {
            parse_file::<65536>(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/face.obj"))
        });
    });
    c.bench_function("Spot", |b| {
        b.iter(|| {
            parse_file::<65536>(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/spot.obj"))
        });
    });
}

fn off(c: &mut Criterion) {
    c.bench_function("Beetle", |b| {
        b.iter(|| {
            parse_file::<65536>(
                &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/beetle.off"),
            )
        });
    });
    c.bench_function("Rocker arm", |b| {
        b.iter(|| {
            parse_file::<65536>(
                &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/rocker-arm.off"),
            )
        });
    });
}

fn ply(c: &mut Criterion) {
    c.bench_function("Bunny", |b| {
        b.iter(|| {
            parse_file::<65536>(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/bunny.ply"))
        });
    });
}

fn compare(c: &mut Criterion) {
    let mut group = c.benchmark_group("Compare bunny");
    group.bench_function("OBJ", |b| {
        b.iter(|| {
            parse_file::<65536>(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/bunny.obj"))
        });
    });
    group.bench_function("OFF", |b| {
        b.iter(|| {
            parse_file::<65536>(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/bunny.off"))
        });
    });
    group.bench_function("PLY", |b| {
        b.iter(|| {
            parse_file::<65536>(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/bunny.ply"))
        });
    });
    group.finish();
}

criterion_group!(benches, obj, off, ply, compare);
criterion_main!(benches);
