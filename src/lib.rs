#![doc = include_str!("../README.md")]

use crate::{obj::load_obj_buf, off::load_off_buf, ply::load_ply_buf, stl::load_stl_buf};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    str::FromStr,
};

pub mod obj;
pub mod off;
pub mod ply;
pub mod stl;

/// Parse a surface from a file
pub fn load_mesh_file<const BUFFER_SIZE: usize>(
    file_name: impl AsRef<Path>,
) -> Result<(Vec<[f32; 3]>, SurfaceIndices), ()> {
    let file = File::open(file_name.as_ref()).map_err(|_| ())?;
    let mut reader = BufReader::new(file);
    let format_hint = file_name.as_ref().extension().and_then(|s| s.to_str());
    load_mesh_reader::<_, BUFFER_SIZE>(&mut reader, format_hint)
}

/// Parse a surface from a reader
pub fn load_mesh_reader<B: BufRead, const BUFFER_SIZE: usize>(
    reader: &mut B,
    format_hint: Option<&str>,
) -> Result<(Vec<[f32; 3]>, SurfaceIndices), ()> {
    let mut buffer = [0; BUFFER_SIZE];
    match format_hint {
        Some("obj") => load_obj_buf(reader, &mut buffer, 0),
        Some("off") => load_off_buf(reader, &mut buffer, 0),
        Some("ply") => load_ply_buf(reader, &mut buffer, 0),
        Some("stl") => load_stl_buf(reader, &mut buffer, 0),
        _ => {
            let read = reader.read(&mut buffer).map_err(|_| ())?;
            match buffer.first_chunk::<3>().ok_or(())? {
                b"OFF" => load_off_buf(reader, &mut buffer, read),
                b"ply" => load_ply_buf(reader, &mut buffer, read),
                _ => Err(()),
            }
        }
    }
}

unsafe fn parse_float3(slice: &[u8]) -> (usize, [f32; 3]) {
    unsafe {
        let mut start = 0;
        while slice[start] == b' ' {
            start += 1;
        }
        let mut sep = find_blank_space(&slice[start + 1..]).unwrap() + 1;
        let f1 =
            FromStr::from_str(std::str::from_utf8_unchecked(&slice[start..(start + sep)])).unwrap();
        start += sep + 1;
        if slice[start] == b' ' {
            std::hint::cold_path();
            start += slice[start..].iter().position(|&c| c != b' ').unwrap();
        }
        sep = find_blank_space(&slice[start + 1..]).unwrap() + 1;
        let f2 =
            FromStr::from_str(std::str::from_utf8_unchecked(&slice[start..(start + sep)])).unwrap();
        start += sep + 1;
        if slice[start] == b' ' {
            std::hint::cold_path();
            start += slice[start..].iter().position(|&c| c != b' ').unwrap();
        }
        sep = find_blank_or_newline(&slice[start + 1..]).unwrap() + 1;
        let f3 =
            FromStr::from_str(std::str::from_utf8_unchecked(&slice[start..(start + sep)])).unwrap();
        start += sep;
        start += match slice[start] {
            b'\r' => 1,
            b'\n' => 0,
            _ => {
                std::hint::cold_path();
                slice[start + 1..].iter().position(|&c| c == b'\n').unwrap() + 1
            }
        };
        let arr: [f32; 3] = [f1, f2, f3];

        (start, arr)
    }
}

fn find_newline(slice: &[u8]) -> Option<usize> {
    slice.iter().position(|&v| v == b'\n')
}

fn find_blank_or_newline(slice: &[u8]) -> Option<usize> {
    slice
        .iter()
        .position(|&v| v == b' ' || v == b'\n' || v == b'\r')
}

fn find_blank_space(slice: &[u8]) -> Option<usize> {
    slice.iter().position(|&v| v == b' ')
}

fn parse_u8(data: &mut &[u8]) -> Option<u8> {
    let first_b = *data.get(0)?;
    let mut res = if first_b == b'+' { 0 } else { first_b & 0x0f };
    *data = &data[1..];
    while !data.is_empty() && data[0].is_ascii_digit() {
        res = res * 10 + (data[0] - b'0');
        *data = &data[1..];
    }
    Some(res)
}

fn parse_uint(data: &[u8]) -> Option<(u32, usize)> {
    data.first().map(|&first_b| {
        let first_b = if first_b == b'+' { 0 } else { first_b & 0x0f };
        let (i, acc) = data[1..]
            .iter()
            .take_while(|&val| val.is_ascii_digit())
            .fold((0, first_b as u32), |(i, acc), &val| {
                (i + 1, acc * 10 + (val - b'0') as u32)
            });
        (acc, i + 1)
    })
}

fn parse_face_indices_list(
    data: &mut &[u8],
    mode: &mut FaceMode,
    indices: &mut Vec<u32>,
    strides: &mut Vec<u8>,
    nf: usize,
) -> Option<()> {
    // get_line already stripped blanks
    let face_len = parse_u8(data)?;

    if data.is_empty() || face_len < 3 {
        std::hint::cold_path();
        return None;
    }
    *data = data.get(1..)?;

    if *mode != FaceMode::Polygon {
        if *mode == FaceMode::Undetermined {
            std::hint::cold_path();
            if face_len == 3 {
                *mode = FaceMode::Triangle;
            } else if face_len == 4 {
                *mode = FaceMode::Quad;
            } else {
                *mode = FaceMode::Polygon;
            }
        } else if *mode == FaceMode::Triangle && face_len != 3 {
            std::hint::cold_path();
            //add missing strides
            *strides = vec![3; indices.len() / 3];
            strides.reserve(nf - strides.len());
            *mode = FaceMode::Polygon;
        } else if *mode == FaceMode::Quad && face_len != 4 {
            std::hint::cold_path();
            //add missing strides
            *strides = vec![4; indices.len() / 4];
            *mode = FaceMode::Polygon;
            strides.reserve(nf - strides.len());
        }
    }
    if *mode == FaceMode::Polygon {
        strides.push(face_len as u8);
    }

    if *data.get(0)? == b' ' {
        std::hint::cold_path();
        *data = &data[1..];
        while !data.is_empty() && data[0] == b' ' {
            *data = &data[1..];
        }
    }

    let (v, endword) = match parse_uint(data) {
        Some(v) => v,
        None => {
            std::hint::cold_path();
            return None;
        }
    };
    *data = data.get(endword + 1..)?;
    indices.push(v);

    if *data.get(0)? == b' ' {
        std::hint::cold_path();
        *data = &data[1..];
        while !data.is_empty() && data[0] == b' ' {
            *data = &data[1..];
        }
    }

    let (v, endword) = match parse_uint(data) {
        Some(v) => v,
        None => {
            std::hint::cold_path();
            return None;
        }
    };
    *data = data.get(endword + 1..)?;
    indices.push(v);

    if *data.get(0)? == b' ' {
        std::hint::cold_path();
        *data = &data[1..];
        while !data.is_empty() && data[0] == b' ' {
            *data = &data[1..];
        }
    }

    for _ in 0..face_len - 3 {
        let (v, endword) = parse_uint(data)?;
        *data = &data.get(endword + 1..)?;
        indices.push(v);

        if *data.get(0)? == b' ' {
            std::hint::cold_path();
            *data = &data[1..];
            while !data.is_empty() && data[0] == b' ' {
                *data = &data[1..];
            }
        }
    }

    let (v, endword) = match parse_uint(data) {
        Some(v) => v,
        None => {
            std::hint::cold_path();
            return None;
        }
    };
    indices.push(v);
    *data = data.get(endword..)?;
    let off = match data.get(0)? {
        b'\r' => 2,
        b'\n' => 1,
        _ => data[1..].iter().position(|&c| c == b'\n')? + 2,
    };
    *data = data.get(off..)?;
    Some(())
}

// Taken from std https://github.com/rust-lang/rust/issues/142137
fn into_chunks<const N: usize>(mut this: Vec<u32>) -> Vec<[u32; N]> {
    const {
        assert!(N != 0, "chunk size must be greater than zero");
    }

    let (len, cap) = (this.len(), this.capacity());

    let len_remainder = len % N;
    if len_remainder != 0 {
        this.truncate(len - len_remainder);
    }

    let cap_remainder = cap % N;
    if cap_remainder != 0 {
        this.shrink_to_fit();
    }
    let (ptr, _, _) = this.into_raw_parts();

    unsafe { Vec::from_raw_parts(ptr.cast(), len / N, cap / N) }
}

#[derive(PartialEq)]
enum FaceMode {
    Triangle,
    Quad,
    Polygon,
    Undetermined,
}

impl From<Vec<[u32; 3]>> for SurfaceIndices {
    fn from(value: Vec<[u32; 3]>) -> Self {
        SurfaceIndices::Triangles(value)
    }
}

impl From<Vec<[u32; 4]>> for SurfaceIndices {
    fn from(value: Vec<[u32; 4]>) -> Self {
        SurfaceIndices::Quads(value)
    }
}

impl From<(Vec<u32>, Vec<u32>)> for SurfaceIndices {
    fn from(value: (Vec<u32>, Vec<u32>)) -> SurfaceIndices {
        let mut count = 0;
        let mut faces_indices = value
            .1
            .into_iter()
            .map(|s| {
                count += s;
                count - s
            })
            .collect::<Vec<_>>();
        faces_indices.push(count);
        SurfaceIndices::Polygons(value.0, faces_indices)
    }
}

impl From<(Vec<u32>, Vec<u8>)> for SurfaceIndices {
    fn from(value: (Vec<u32>, Vec<u8>)) -> SurfaceIndices {
        let mut count = 0;
        let mut faces_indices = value
            .1
            .into_iter()
            .map(|s| {
                count += s as u32;
                count - s as u32
            })
            .collect::<Vec<_>>();
        faces_indices.push(count);
        SurfaceIndices::Polygons(value.0, faces_indices)
    }
}

/// Various indices representations
pub enum SurfaceIndices {
    Triangles(Vec<[u32; 3]>),
    Quads(Vec<[u32; 4]>),
    Polygons(Vec<u32>, Vec<u32>),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::load_mesh_file;

    #[test]
    fn test_obj() {
        for path in [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/armadillo.obj"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/bob.obj"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/face.obj"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/spot.obj"),
        ] {
            assert!(matches!(load_mesh_file::<65536>(&path), Ok(_)));
        }
    }

    #[test]
    fn test_obj_quad() {
        for path in [PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/dragon-surfel.obj")] {
            assert!(matches!(load_mesh_file::<65536>(&path), Ok(_)));
        }
    }

    #[test]
    fn test_obj_poly() {
        for path in [PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/bimbaPoly.obj")] {
            assert!(matches!(load_mesh_file::<65536>(&path), Ok(_)));
        }
    }

    #[test]
    fn test_off() {
        for path in [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/beetle.off"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/rocker-arm.off"),
        ] {
            assert!(matches!(load_mesh_file::<65536>(&path), Ok(_)));
        }
    }

    #[test]
    fn test_stl() {
        for path in [PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/bunny.stl")] {
            assert!(matches!(load_mesh_file::<65536>(&path), Ok(_)));
        }
    }

    #[test]
    fn test_ply() {
        for path in [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/cube1.ply"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/cube2.ply"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/bunny.ply"),
        ] {
            assert!(matches!(load_mesh_file::<65536>(&path), Ok(_)));
        }
    }

    #[test]
    fn test_unknown() {
        for path in [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/bunny.txt"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/beetle.txt"),
        ] {
            assert!(matches!(load_mesh_file::<65536>(&path), Ok(_)));
        }
    }
}
