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

pub struct SurfaceLoader<const BUFFER_SIZE: usize = 65536> {
    buffer: [u8; BUFFER_SIZE],
}

impl<const BUFFER_SIZE: usize> SurfaceLoader<BUFFER_SIZE> {
    pub fn from_file(
        self,
        file_name: impl AsRef<Path>,
    ) -> Result<(Vec<[f32; 3]>, SurfaceIndices), ()> {
        let file = File::open(file_name.as_ref()).map_err(|_| ())?;
        let mut reader = BufReader::new(file);
        let format_hint = file_name.as_ref().extension().map(|s| s.to_str()).flatten();

        self.from_buffer(&mut reader, format_hint)
    }

    pub fn from_buffer<B: BufRead>(
        mut self,
        reader: &mut B,
        format_hint: Option<&str>,
    ) -> Result<(Vec<[f32; 3]>, SurfaceIndices), ()> {
        match format_hint {
            Some("obj") => Ok(load_obj_buf(reader, &mut self.buffer, 0)),
            Some("off") => Ok(load_off_buf(reader, &mut self.buffer, 0)),
            Some("ply") => Ok(load_ply_buf(reader, &mut self.buffer, 0)),
            Some("stl") => Ok(load_stl_buf(reader, &mut self.buffer, 0)),
            _ => {
                let read = reader.read(&mut self.buffer).map_err(|_| ())?;
                match self.buffer.first_chunk::<3>().ok_or(())? {
                    b"OFF" => Ok(load_off_buf(reader, &mut self.buffer, read)),
                    b"ply" => Ok(load_ply_buf(reader, &mut self.buffer, read)),
                    _ => Err(()),
                }
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
        start += slice[start..].iter().position(|&c| c != b' ').unwrap();
        sep = find_blank_space(&slice[start + 1..]).unwrap() + 1;
        let f2 =
            FromStr::from_str(std::str::from_utf8_unchecked(&slice[start..(start + sep)])).unwrap();
        start += sep + 1;
        start += slice[start..].iter().position(|&c| c != b' ').unwrap();
        sep = find_blank_or_newline(&slice[start + 1..]).unwrap() + 1;
        let f3 =
            FromStr::from_str(std::str::from_utf8_unchecked(&slice[start..(start + sep)])).unwrap();
        start += sep;
        start += slice[start..]
            .iter()
            .position(|&c| c != b' ' && c != b'\r')
            .unwrap();
        let arr: [f32; 3] = [f1, f2, f3];

        (start, arr)
    }
}

fn find_blank_or_newline(slice: &[u8]) -> Option<usize> {
    slice
        .iter()
        .position(|&v| v == b' ' || v == b'\n' || v == b'\r')
}

fn find_blank_space(slice: &[u8]) -> Option<usize> {
    slice.iter().position(|&v| v == b' ')
}

// Taken from std https://github.com/rust-lang/rust/issues/142137
pub fn into_chunks<const N: usize>(mut this: Vec<u32>) -> Vec<[u32; N]> {
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

impl Into<SurfaceIndices> for Vec<[u32; 3]> {
    fn into(self) -> SurfaceIndices {
        SurfaceIndices::Triangles(self)
    }
}

impl Into<SurfaceIndices> for Vec<[u32; 4]> {
    fn into(self) -> SurfaceIndices {
        SurfaceIndices::Quads(self)
    }
}

impl Into<SurfaceIndices> for (Vec<u32>, Vec<u32>) {
    fn into(self) -> SurfaceIndices {
        let mut count = 0;
        let mut faces_indices = self
            .1
            .into_iter()
            .map(|s| {
                count += s;
                count - s
            })
            .collect::<Vec<_>>();
        faces_indices.push(count);
        SurfaceIndices::Polygons(self.0, faces_indices)
    }
}

impl Into<SurfaceIndices> for (Vec<u32>, Vec<u8>) {
    fn into(self) -> SurfaceIndices {
        let mut count = 0;
        let mut faces_indices = self
            .1
            .into_iter()
            .map(|s| {
                count += s as u32;
                count - s as u32
            })
            .collect::<Vec<_>>();
        faces_indices.push(count);
        SurfaceIndices::Polygons(self.0, faces_indices)
    }
}

pub enum SurfaceIndices {
    Triangles(Vec<[u32; 3]>),
    Quads(Vec<[u32; 4]>),
    Polygons(Vec<u32>, Vec<u32>),
}
