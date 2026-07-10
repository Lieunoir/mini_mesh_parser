use std::{
    fs::File,
    io::{BufReader, prelude::*},
    path::Path,
    str::FromStr,
};

unsafe fn parse_float3(slice: &[u8]) -> (usize, [f32; 3]) {
    unsafe {
        let mut start = 0;
        while slice[start] == b' ' {
            start += 1;
        }
        let mut sep = find_blank_space(&slice[start + 2..]).unwrap() + 2;
        let f1 =
            FromStr::from_str(std::str::from_utf8_unchecked(&slice[start..(start + sep)])).unwrap();
        start += sep + 1;
        start += slice[start..].iter().position(|&c| c != b' ').unwrap();
        sep = find_blank_space(&slice[start + 2..]).unwrap() + 2;
        let f2 =
            FromStr::from_str(std::str::from_utf8_unchecked(&slice[start..(start + sep)])).unwrap();
        start += sep + 1;
        start += slice[start..].iter().position(|&c| c != b' ').unwrap();
        sep = find_blank_or_newline(&slice[start + 2..]).unwrap() + 2;
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

fn parse_int(data: &[u8], pos_sz: u32) -> Option<(u32, usize)> {
    data.first().map(|&first_b| {
        let neg = first_b == b'-';
        let start = (first_b == b'+' || neg) as usize;
        let (i, acc) = data[start..]
            .iter()
            .take_while(|&val| val.is_ascii_digit())
            .fold((0, 0), |(i, acc), &val| {
                (i + 1, acc * 10 + (val - b'0') as u32)
            });
        let res = if !neg { acc - 1 } else { pos_sz - acc };
        (res, i + start)
    })
}

fn parse_face_indices(
    //face_str: SplitAsciiblankspace,
    face_str: &[u8],
    mode: &mut FaceMode,
    indices: &mut Vec<u32>,
    strides: &mut Vec<u8>,
    pos_sz: u32,
) -> usize {
    let mut data = face_str;
    let mut off = 0;

    off += data.iter().position(|&c| c != b' ').unwrap();
    data = &face_str[off..];
    let (f0, end) = parse_int(data, pos_sz).unwrap();
    off += end;
    data = &face_str[off..];
    off += data.iter().position(|&c| c == b' ').unwrap() + 1;
    data = &face_str[off..];
    off += data.iter().position(|&c| c != b' ').unwrap();
    data = &face_str[off..];
    let (f1, end) = parse_int(data, pos_sz).unwrap();
    off += end;
    data = &face_str[off..];
    off += data.iter().position(|&c| c == b' ').unwrap() + 1;
    data = &face_str[off..];
    off += data.iter().position(|&c| c != b' ').unwrap();
    data = &face_str[off..];
    // let (f2, end) = parse_int(data, pos_sz).unwrap();
    // off += end;
    // let mut i = 3;
    let mut i = 2;
    indices.push(f0);
    indices.push(f1);
    // indices.push(f3);

    while let Some((v_i, mut endword)) = parse_int(data, pos_sz) {
        indices.push(v_i);
        i += 1;
        if endword == data.len() {
            break;
        }
        if data[endword] == b'/' {
            match find_blank_or_newline(&data[(endword + 1)..]) {
                Some(value) => endword += 1 + value,
                None => break,
            }
        }

        endword += data[endword..].iter().position(|&c| c != b' ').unwrap();

        off += endword;
        if data[endword] == b'\r' || data[endword] == b'\n' {
            off += data[endword..]
                .iter()
                .position(|&c| c != b' ' && c != b'\r')
                .unwrap();
            break;
        }
        data = &data[endword..];
    }
    //if data.len() > 0 {
    //    let v_i = VertexIndices::parse_pos(data, pos_sz).unwrap();
    //    indices.push(v_i);
    //    i += 1;
    //}
    if i >= 3 && *mode != FaceMode::Polygon {
        if *mode == FaceMode::Undetermined {
            if i == 3 {
                *mode = FaceMode::Triangle;
            } else if i == 4 {
                *mode = FaceMode::Quad;
            } else {
                *mode = FaceMode::Polygon;
            }
        } else if *mode == FaceMode::Triangle && i != 3 {
            //add missing strides
            *strides = vec![3; (indices.len() - i) / 3];
            strides.reserve(3 * 2 * pos_sz as usize - strides.len());
            *mode = FaceMode::Polygon;
        } else if *mode == FaceMode::Quad && i != 4 {
            //add missing strides
            *strides = vec![4; (indices.len() - i) / 4];
            *mode = FaceMode::Polygon;
            strides.reserve(4 * 2 * pos_sz as usize - strides.len());
        }
    }
    if i >= 3 && *mode == FaceMode::Polygon {
        strides.push(i as u8);
    }
    off
}

pub fn load_obj(file_name: impl AsRef<Path>) -> (Vec<[f32; 3]>, SurfaceIndices) {
    let file = match File::open(file_name.as_ref()) {
        Ok(f) => f,
        Err(_e) => {
            panic!()
            //return Err(LoadError::OpenFileFailed);
        }
    };
    let mut reader = BufReader::new(file);
    load_obj_buf(&mut reader)
}

pub fn load_obj_buf<B>(reader: &mut B) -> (Vec<[f32; 3]>, SurfaceIndices)
where
    B: BufRead,
{
    let mut vertices = Vec::new();
    let mut mode = FaceMode::Undetermined;
    let mut indices: Vec<u32> = Vec::new();
    let mut strides: Vec<u8> = Vec::new();
    const BUFFER_SIZE: usize = 65536;
    let mut buf = [0; BUFFER_SIZE];
    let mut encountered_f = false;
    let mut start = 0;
    while let Ok(size) = reader.read(&mut buf[start..]) {
        if size == 0 && start == 0 {
            break;
        }
        let end = start + size;
        let mut last = end - 1;
        while buf[last] != b'\n' && last > 0 {
            last -= 1;
        }
        if buf[last] != b'\n' {
            break;
        }
        last += 1;

        let mut i = 0;
        while i < last {
            match buf[i] {
                b'v' => match buf[i + 1] {
                    b' ' => {
                        let (off, pos) = unsafe { parse_float3(&buf[i + 2..]) };
                        vertices.push(pos);
                        i += off + 2;
                    }
                    _ => i += find_newline(&buf[i + 1..]).unwrap() + 2,
                },
                b'f' => {
                    if !encountered_f {
                        encountered_f = true;
                        // first estimate that `nf = 2 * nv`
                        indices.reserve(vertices.len() * 2 * 3);
                    }
                    let off = parse_face_indices(
                        &buf[i + 2..],
                        &mut mode,
                        &mut indices,
                        &mut strides,
                        vertices.len() as u32,
                    );
                    i += 2 + off;
                }
                _ => i += find_newline(&buf[i..]).unwrap() + 1,
            }
        }

        start = end - last;
        buf.copy_within(last..end, 0);
    }
    let indices = if mode == FaceMode::Polygon {
        (indices, strides).into()
    } else if mode == FaceMode::Quad {
        into_chunks::<4>(indices).into()
    } else {
        into_chunks::<3>(indices).into()
    };
    (vertices, indices)
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

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_lens() {
        let surf = load_obj("/home/lieunoir/meshes/armadillo.obj");
        let f = match &surf.1 {
            SurfaceIndices::Triangles(t) => t,
            _ => panic!(),
        };
        assert!(surf.0.len() == 49990);
        assert!(f.len() == 99976);

        let surf = load_obj("/home/lieunoir/meshes/bob.obj");
        let f = match &surf.1 {
            SurfaceIndices::Triangles(t) => t,
            _ => panic!(),
        };
        assert!(surf.0.len() == 5344);
        assert!(f.len() == 10688);

        let surf = load_obj("/home/lieunoir/meshes/lucy.obj");
        let f = match &surf.1 {
            SurfaceIndices::Triangles(t) => t,
            _ => panic!(),
        };
        assert!(surf.0.len() == 14027872);
        assert!(f.len() == 28055728);
    }
}
