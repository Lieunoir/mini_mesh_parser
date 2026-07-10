use mesh_parsers::{SurfaceIndices, load_obj};
fn main() {
    let surf = load_obj("/home/lieunoir/meshes/lucy.obj");
    let f = match &surf.1 {
        SurfaceIndices::Triangles(t) => t,
        _ => panic!(),
    };
    assert!(surf.0.len() == 14027872);
    assert!(f.len() == 28055728);
}
