use mesh_parsers::{SurfaceIndices, obj::load_obj, ply::load_ply};
fn main() {
    //let surf = load_obj("/home/lieunoir/meshes/lucy.obj");
    //let f = match &surf.1 {
    //    SurfaceIndices::Triangles(t) => t,
    //    _ => panic!(),
    //};
    //assert!(surf.0.len() == 14027872);
    //assert!(f.len() == 28055728);
    //load_ply("/home/lieunoir/rust/mesh_parsers/assets/cube1.ply");
    //let f = load_ply("/home/lieunoir/rust/mesh_parsers/assets/cube2.ply");
    let f = load_ply("/home/lieunoir/meshes/bunny_ply/bun_zipper.ply");
    println!("{:?}", f.0);
}
