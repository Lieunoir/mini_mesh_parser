use mini_mesh_parser::load_mesh_file;

fn main() {
    let (v, f) = load_mesh_file::<65536>("./assets/bunny.off").unwrap();
    //let (v, f) = load_mesh_file::<65536>("/home/lieunoir/meshes/lucy.obj").unwrap();
}
