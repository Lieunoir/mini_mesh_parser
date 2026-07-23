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
    let (chunks, rem) = slice.as_chunks::<8>();
    if let Some((i, word)) = chunks.iter().enumerate().find_map(|(i, &c)| {
        let word = u64::from_le_bytes(c);
        let word = word ^ 0x0A0A0A0A0A0A0A0A;
        let word = word.wrapping_sub(0x0101010101010101) & !word & 0x8080808080808080;
        if word != 0 { Some((i, word)) } else { None }
    }) {
        let off = word.trailing_zeros() / 8;
        return Some(i * 8 + off as usize);
    }
    rem.iter()
        .position(|&v| v == b'\n')
        .map(|off| off + chunks.len() * 8)
}

fn find_blank_or_newline(slice: &[u8]) -> Option<usize> {
    let (chunks, rem) = slice.as_chunks::<8>();
    if let Some((i, word)) = chunks.iter().enumerate().find_map(|(i, &c)| {
        let word = u64::from_le_bytes(c);
        // From : https://graphics.stanford.edu/~seander/bithacks.html#HasLessInWord
        let word = (word.wrapping_sub(!0u64 / 255 * b'!' as u64)) & !word & 0x8080808080808080;
        if word != 0 { Some((i, word)) } else { None }
    }) {
        let off = word.trailing_zeros() / 8;
        return Some(i * 8 + off as usize);
    }
    rem.iter()
        .position(|&v| v == b' ' || v == b'\n' || v == b'\r')
        .map(|off| off + chunks.len() * 8)
}

fn find_blank_space(slice: &[u8]) -> Option<usize> {
    let (chunks, rem) = slice.as_chunks::<8>();
    if let Some((i, word)) = chunks.iter().enumerate().find_map(|(i, &c)| {
        let word = u64::from_le_bytes(c);
        let word = word ^ 0x2020202020202020;
        let word = word.wrapping_sub(0x0101010101010101) & !word & 0x8080808080808080;
        if word != 0 { Some((i, word)) } else { None }
    }) {
        let off = word.trailing_zeros() / 8;
        return Some(i * 8 + off as usize);
    }
    rem.iter()
        .position(|&v| v == b' ')
        .map(|off| off + chunks.len() * 8)
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
    let (chunks, rem) = data.as_chunks::<8>();
    for &c in chunks {
        let word = u64::from_le_bytes(c);
        let mask = ((word) + !0u64 / 255 * (127 - (b'/' as u64)) | word) & !0u64 / 255 * 128;
        let sep_mask = mask ^ 0x8080808080808080;
        let sep_mask = mask & (sep_mask >> 8);
        if sep_mask != 0 {
            let to_shift = sep_mask.trailing_zeros() + 1;
            let num_word = word & 0x0f0f0f0f0f0f0f0f;
            let num_word = num_word << 64 - to_shift;

            let mask: u64 = 0x000000FF000000FF;
            let mul1: u64 = 0x000F424000000064; // 100 + (1000000ULL << 32)
            let mul2: u64 = 0x0000271000000001; // 1 + (10000ULL << 32)
            let num = (num_word * 10) + (num_word >> 8); // num_word = (num_word * 2561) >> 8;
            let num = (num & mask).wrapping_mul(mul1) + ((num >> 16) & mask).wrapping_mul(mul2);
            let num_bytes = num.to_le_bytes();
            let num = u32::from_le_bytes(*num_bytes.last_chunk().unwrap());
            return Some((num, (to_shift / 8) as usize));
            //println!("{} {num}", to_shift / 8 - 1);
        }
    }
    rem.first().map(|&first_b| {
        let first_b = if first_b == b'+' { 0 } else { first_b & 0x0f };
        let (i, acc) = rem[1..]
            .iter()
            .take_while(|&val| val.is_ascii_digit())
            .fold((0, first_b as u32), |(i, acc), &val| {
                (i + 1, acc * 10 + (val - b'0') as u32)
            });
        (acc, chunks.len() * 8 + i + 1)
    })
}

const TABLE_LEN: usize = 8;

// https://jk-jeon.github.io/posts/2023/08/optimal-bounds-integer-division/
// https://en.algorithmica.org/hpc/arithmetic/division/#lemire-reduction
const MUL_FOR_DIV_10_POW_N: [(u64, u64); TABLE_LEN] = {
    let mut res = [(1, 1); TABLE_LEN];
    let mut i = 1;
    while i < TABLE_LEN {
        let divisor = 10u64.pow(i as u32);
        // rhs_num = (v + 1) / divisor
        let div_helper = (!0u64) / divisor + 1;
        res[i] = (divisor, div_helper);
        i += 1;
    }
    res
};

const TEN_POW_N: [u32; TABLE_LEN] = {
    let mut res = [1; TABLE_LEN];
    let mut i = 0;
    let mut acc = 1;
    while i < TABLE_LEN {
        res[i] = acc;
        acc *= 10;
        i += 1;
    }
    res
};

fn parse_uints(data: &[u8], mut n: u32, indices: &mut Vec<u32>) -> Option<usize> {
    let (chunks, rem) = data.as_chunks::<8>();
    let mut num_rem = None;
    for (i, &c) in chunks.iter().enumerate() {
        let word = u64::from_le_bytes(c);
        let digit_mask = ((word) + !0u64 / 255 * (127 - (b'/' as u64)) | word) & !0u64 / 255 * 128;
        //let full_mask = (digit_mask << 1).wrapping_sub(digit_mask >> 7);
        //Make mask go to the 4 lsb instead of full bits, avoid ascii number bit masking after
        let mut num_ends = digit_mask & !digit_mask >> 8;
        let digit_mask = (digit_mask >> 3) - (digit_mask >> 7);
        let has_rem = (digit_mask >> 56) != 0;
        //store remainder now
        let num_word = word & digit_mask;
        let mask = 0x000000ff000000ff;
        let mul1 = 0x000f424000000064; // 100 + (1000000ull << 32)
        let mul2 = 0x0000271000000001; // 1 + (10000ull << 32)
        let num = (num_word * 10) + (num_word >> 8); // num_word = (num_word * 2561) >> 8;
        let mut num = ((((num & mask).wrapping_mul(mul1))
            + (((num >> 16) & mask).wrapping_mul(mul2)))
            >> 32) as u32;

        //handle carry and no new number edge_case
        if (digit_mask & 1 == 0)
            && let Some(val) = num_rem.take()
        {
            indices.push(val);
            n -= 1;

            if n == 0 {
                return Some(i * 8);
            }
        }

        for _ in 0..4 {
            if num_ends != 0 {
                let num_digit = num_ends.trailing_zeros() / 8;
                num_ends &= num_ends - 1;
                let off = 7 - num_digit;
                let num_rem = num_rem.take().unwrap_or(0) * TEN_POW_N[num_digit as usize + 1];

                let (divisor, to_mul) = MUL_FOR_DIV_10_POW_N[off as usize];
                let (low, high) = (num as u64).carrying_mul(to_mul, 0);
                let quo = high as u32;
                let rem = low.carrying_mul(divisor as u64, 0).1 as u32;
                indices.push(quo + num_rem);
                num = rem;
                n -= 1;

                if n == 0 {
                    return Some(i * 8 + 1 + num_digit as usize);
                }
            } else {
                break;
            }
        }

        num_rem = if has_rem {
            Some(num + num_rem.unwrap_or(0) * 10u32.pow(8))
        } else {
            None
        };
    }

    std::hint::cold_path();
    let mut i = 0;
    while n > 0 {
        let mut acc = num_rem.unwrap_or(0);
        while i < rem.len() && rem[i].is_ascii_digit() {
            acc = acc * 10 + (rem[i] - b'0') as u32;
            i += 1;
        }
        indices.push(acc);
        n -= 1;
        if n > 0 {
            i += rem.get(i + 1..)?.iter().position(|&c| c.is_ascii_digit())? + 1;
        }
    }
    if n == 0 {
        Some(chunks.len() * 8 + i)
    } else {
        None
    }
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

    let endword = parse_uints(data, face_len as u32, indices)?;
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
