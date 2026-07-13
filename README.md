# Mini mesh parsers

Minimalistic mesh library to parse an obj, off, ply or stl (binary only) file into a surface mesh (and only a surface mesh).

## Usage

```rust
use mini_mesh_parser::load_mesh_file;

let (v, f) = load_mesh_file("assets/bunny.obj").unwrap();
```
